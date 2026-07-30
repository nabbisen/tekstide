//! RFC-021 PR-021-E1/E2: the trusted-context coordinator.
//!
//! This is where a `CommandProposal`'s untrusted claims meet the actual,
//! already-authenticated `AgentRun`/`ProjectSession` state that
//! `approval::channel` and `approval::risk` deliberately do not depend on.
//! Properties this module exists specifically to enforce:
//!
//! 1. **`CommandProposal::cwd()` is never trusted for classification.**
//!    Response 111 (PR-021-C re-review) and response 113 (PR-021-D
//!    re-review) both flagged this as a gap between two individually-
//!    correct scope decisions: B correctly declined path containment as
//!    out of scope for a protocol decoder; C correctly takes paths as
//!    given, since a pure classifier has no independent way to know the
//!    "true" cwd. Neither was wrong to stop there -- but if this module
//!    read `proposal.cwd()` and passed it (or anything derived from it)
//!    into `risk::classify` as the project root or the resolution base, an
//!    adapter could choose the frame its own command is judged in --
//!    concretely, a proposal claiming `cwd = "/"` could make an external
//!    absolute path look project-internal if `cwd()`'s value were ever
//!    used as (or to derive) the containment boundary. This module's
//!    public API makes that structurally unreachable: every function here
//!    that needs a cwd or a project root takes it as a separate,
//!    caller-supplied parameter sourced from real, already-verified
//!    context. **There is no code path in this module that reads
//!    `CommandProposal::cwd()` at all.** `receive_proposal` takes
//!    `&agent::VerifiedCwd`, not `&Path`, for its `verified_cwd` parameter
//!    (response 114 Q3 / response 115): `VerifiedCwd` has no public
//!    constructor accepting an arbitrary path, so a caller cannot even
//!    typecheck passing `proposal.cwd()` here by mistake -- the only way
//!    to obtain one is to have already gone through
//!    `agent::AgentRunLaunchValidator::validate`.
//! 2. **A repeated proposal id is rejected outright, not resolved
//!    inertly.** Response 114 Required 1: an earlier version of this
//!    module returned the *original*, possibly-already-decided request on
//!    a repeat -- which meant a caller holding the repeat's *new* argv
//!    could read an `ApprovedOnce` decision that was never granted for
//!    that argv at all, laundering approval for one command onto another.
//!    A repeat now gets `ReceiveOutcome::DuplicateRejected`, which carries
//!    no `ApprovalRequest` whatsoever -- there is no value to
//!    (mis)interpret as authorization, regardless of whether the repeat's
//!    argv happens to match the original.
//! 3. **A decision, once made, is final.** A repeated `decide` call
//!    (whether from a genuine retry or a replayed decision message)
//!    returns the existing terminal state unchanged rather than
//!    re-running anything or overwriting the first decision.
//!
//! **Explicitly out of scope for this slice (deferred to PR-021-E2):**
//! writing any of this to durable audit via `AuditCoordinator`, sending
//! `CommandDecision` back over the wire, re-classifying edited argv for
//! `EditedAndApproved` (`implementation-handoff.md` §7), and auditing a
//! `cwd` mismatch between what a proposal claims and `verified_cwd` as an
//! anomaly signal (response 114 Recommended 3 -- not comparing at all
//! here is deliberate; see the module-doc point above). This module only
//! produces in-memory `domain::ApprovalRequest` values; E2 wires those to
//! the audit family and the channel.

use std::collections::HashMap;
use std::path::Path;

use crate::agent::VerifiedCwd;
use crate::domain::{AgentRunId, ApprovalDecision, ApprovalRequest};
use crate::project::ProjectId;

use super::channel::{AcceptedProposal, ApprovalChannelError};
use super::protocol::{CommandDecision, DecisionOutcome, PROTOCOL_VERSION, ProposalId};
use super::risk;

/// The `requested_action_kind` recorded on every `ApprovalRequest` this
/// coordinator creates. RFC-021 defines exactly one proposal kind (a
/// command execution request); kept as a named constant rather than an
/// inline literal so a future second kind has one place to branch on.
const COMMAND_EXECUTION_ACTION_KIND: &str = "command_execution";

/// Result of [`ApprovalCoordinator::receive_proposal`].
#[derive(Clone, Debug)]
pub enum ReceiveOutcome {
    /// First time this `(AgentRunId, ProposalId)` pair has been seen: a
    /// fresh `Pending` request was created and classified. The
    /// classifier's reasons live on `request.risk_reasons` itself
    /// (response 115 Q2 -- relocated from a separate field here, since a
    /// caller that stores `request` and lets this `ReceiveOutcome` go out
    /// of scope still needs to answer "why" later).
    Created {
        // Boxed per clippy::large_enum_variant: `ApprovalRequest` is large
        // relative to `DuplicateRejected`'s single `ProposalId`, and this
        // enum is returned by value from `receive_proposal` on every call,
        // not just the rare duplicate case.
        request: Box<ApprovalRequest>,
    },
    /// This proposal id was already seen for this run. **No
    /// `ApprovalRequest` is returned, regardless of whether the repeat's
    /// argv matches the original or not** (response 114 Required 1) -- a
    /// repeated proposal id is either an adapter bug or a replay, and
    /// neither should be serviceable by handing back a value a caller
    /// could read as authorization for whatever argv arrived this time.
    /// The duplicate's connection is dropped along with it (its
    /// `AcceptedProposal` is not retained), so no decision can ever be
    /// sent back over it either.
    DuplicateRejected { proposal_id: ProposalId },
}

impl ReceiveOutcome {
    /// The created request, if this is a fresh receipt. Deliberately
    /// `None` for `DuplicateRejected` -- there is no request to return,
    /// not merely one this accessor withholds.
    pub fn request(&self) -> Option<&ApprovalRequest> {
        match self {
            ReceiveOutcome::Created { request, .. } => Some(request.as_ref()),
            ReceiveOutcome::DuplicateRejected { .. } => None,
        }
    }

    pub fn is_duplicate_rejected(&self) -> bool {
        matches!(self, ReceiveOutcome::DuplicateRejected { .. })
    }
}

/// Result of [`ApprovalCoordinator::decide`] and
/// [`ApprovalCoordinator::decide_with_edited_argv`].
#[derive(Debug)]
pub enum DecideOutcome {
    /// The first decision for this proposal id. `request` now reflects
    /// it, and the coordinator has attempted to send the corresponding
    /// `CommandDecision` back over the connection the proposal arrived
    /// on. `sent` is `Err` if that send failed (e.g. the adapter already
    /// disconnected) -- the decision was still made and is still final
    /// (`request.decision` is authoritative regardless), but the adapter
    /// may never learn about it. Not treated as a reason to undo the
    /// decision: PR-021-E2 has no execution wiring yet for this to race
    /// against, and a decision the user actually made must not be
    /// silently reverted because a notification failed.
    Decided {
        request: ApprovalRequest,
        sent: Result<(), ApprovalChannelError>,
    },
    /// This proposal id was already decided; the decision requested this
    /// time (whatever it was) had no effect -- the existing terminal state
    /// is returned unchanged, and nothing is sent again (a replay must be
    /// fully inert, not just non-overwriting).
    AlreadyDecided(ApprovalRequest),
    /// No request exists for this `(AgentRunId, ProposalId)` pair -- a
    /// decision cannot be made for a proposal that was never received.
    NotFound,
}

impl DecideOutcome {
    pub fn request(&self) -> Option<&ApprovalRequest> {
        match self {
            DecideOutcome::Decided { request, .. } => Some(request),
            DecideOutcome::AlreadyDecided(request) => Some(request),
            DecideOutcome::NotFound => None,
        }
    }
}

/// The decisions [`ApprovalCoordinator::decide`] accepts directly.
/// `EditedAndApproved` is deliberately not a variant here: it always
/// requires edited argv, both to re-classify against and to include on
/// the wire (`CommandDecision::decode` itself rejects `EditedAndApproved`
/// with no edited argv), and `decide` has no parameter for that.
/// [`ApprovalCoordinator::decide_with_edited_argv`] is the only way to
/// reach it -- reaching it that way makes the required argv structurally
/// unavoidable rather than a runtime check this type could otherwise skip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimpleDecision {
    ApprovedOnce,
    Rejected,
}

impl From<SimpleDecision> for ApprovalDecision {
    fn from(simple: SimpleDecision) -> Self {
        match simple {
            SimpleDecision::ApprovedOnce => ApprovalDecision::ApprovedOnce,
            SimpleDecision::Rejected => ApprovalDecision::Rejected,
        }
    }
}

impl From<SimpleDecision> for DecisionOutcome {
    fn from(simple: SimpleDecision) -> Self {
        match simple {
            SimpleDecision::ApprovedOnce => DecisionOutcome::ApprovedOnce,
            SimpleDecision::Rejected => DecisionOutcome::Rejected,
        }
    }
}

/// A request still awaiting (or already past) a decision, alongside the
/// still-open connection its proposal arrived on. Response 114 Required 1
/// keeps `ApprovalRequest` itself Clone-friendly and freely returnable;
/// `AcceptedProposal` (holding a live `UnixStream`) is not clonable and is
/// not returned to callers at all -- it is used exactly once, internally,
/// when `decide`/`decide_with_edited_argv` sends the resulting
/// `CommandDecision` back.
struct PendingRequest {
    request: ApprovalRequest,
    accepted: AcceptedProposal,
}

/// Per-run, in-memory bookkeeping for proposal-id uniqueness, single-use
/// decisions, and the connection each proposal arrived on. Nothing here is
/// durable -- see the module doc for why audit wiring is explicitly out of
/// scope for this slice, and `qa-evidence.md`'s Known Limitations for why
/// that non-durability is safe rather than merely unaddressed (response
/// 114 Q2: a restart loses the map, but `bind()` also generates a fresh
/// token per endpoint, so a pre-restart token cannot authenticate
/// afterward either -- the failure mode is "forgotten and nothing
/// executes," not "replayed").
#[derive(Default)]
pub struct ApprovalCoordinator {
    requests: HashMap<(AgentRunId, ProposalId), PendingRequest>,
}

impl ApprovalCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Classifies and registers a freshly-authenticated proposal, taking
    /// ownership of the `AcceptedProposal` (and the live connection it
    /// holds) so that a later `decide`/`decide_with_edited_argv` call can
    /// send the resulting `CommandDecision` back over the same
    /// connection -- the protocol has no separate mechanism to address a
    /// decision to a specific adapter connection, so the connection the
    /// proposal arrived on is the only route back to it.
    ///
    /// `verified_cwd` and `project_root` must come from the caller's own,
    /// already-authenticated `AgentRun`/`ProjectSession` state -- **never**
    /// from `accepted.proposal.cwd()`. There is no parameter here that
    /// accepts `proposal.cwd()`'s value at all: this module never reads it.
    ///
    /// `verified_cwd` takes `&VerifiedCwd`, not `&Path` (response 114 Q3 /
    /// response 115: a plain `&Path` parameter did not stop a caller from
    /// *passing* `proposal.cwd()` here by mistake, since both are `&Path`
    /// and the compiler cannot tell an adapter's untrusted claim apart
    /// from a validated one). `VerifiedCwd` has no public constructor
    /// that accepts an arbitrary path -- the only way a caller can obtain
    /// one is by already having gone through
    /// `agent::AgentRunLaunchValidator::validate`, so passing
    /// `proposal.cwd()` here no longer typechecks at all.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_proposal(
        &mut self,
        project_id: ProjectId,
        agent_run_id: AgentRunId,
        verified_cwd: &VerifiedCwd,
        project_root: &Path,
        state_root: &Path,
        accepted: AcceptedProposal,
    ) -> ReceiveOutcome {
        let key = (
            agent_run_id.clone(),
            accepted.proposal.proposal_id().clone(),
        );
        if self.requests.contains_key(&key) {
            // The duplicate's own connection (`accepted`) is dropped here,
            // without ever being stored -- it is never retained, so
            // nothing can later send a decision back over it either.
            return ReceiveOutcome::DuplicateRejected {
                proposal_id: accepted.proposal.proposal_id().clone(),
            };
        }

        let verified_cwd_path = verified_cwd.as_path();
        let assessment = risk::classify(
            accepted.proposal.argv(),
            verified_cwd_path,
            project_root,
            state_root,
        );
        let display_command = display_argv(accepted.proposal.argv());
        let request = ApprovalRequest::pending(
            project_id,
            Some(agent_run_id),
            COMMAND_EXECUTION_ACTION_KIND,
            display_command,
            assessment.level,
            assessment.reasons,
            verified_cwd_path,
        );
        let returned = request.clone();
        self.requests
            .insert(key, PendingRequest { request, accepted });
        ReceiveOutcome::Created {
            request: Box::new(returned),
        }
    }

    /// Applies `ApprovedOnce` or `Rejected` to a previously-received
    /// request and sends the corresponding `CommandDecision` back over
    /// the connection its proposal arrived on. See [`DecideOutcome`] for
    /// what happens on a replay, and [`Self::decide_with_edited_argv`]
    /// for `EditedAndApproved`.
    pub fn decide(
        &mut self,
        agent_run_id: &AgentRunId,
        proposal_id: &ProposalId,
        decision: SimpleDecision,
    ) -> DecideOutcome {
        let key = (agent_run_id.clone(), proposal_id.clone());
        let Some(pending) = self.requests.get_mut(&key) else {
            return DecideOutcome::NotFound;
        };
        if pending.request.decision != ApprovalDecision::Pending {
            // Single-use, inert replay: the decision requested this call
            // (whatever it is) is discarded, and the existing terminal
            // state is returned unchanged -- not an error the caller must
            // specially handle, since a replay is an expected event on
            // this path (a resent decision message, a retried UI action),
            // not an exceptional one. Nothing is sent again either: a
            // replay must be fully inert, not merely non-overwriting.
            return DecideOutcome::AlreadyDecided(pending.request.clone());
        }
        pending
            .request
            .decide(decision.into())
            .expect("guarded immediately above: request.decision was just checked to be Pending");

        let wire_decision = build_wire_decision(proposal_id, decision.into(), None);
        let sent = pending.accepted.send_decision(&wire_decision);
        DecideOutcome::Decided {
            request: pending.request.clone(),
            sent,
        }
    }

    /// `EditedAndApproved`: re-runs the risk classifier on `edited_argv`
    /// and updates the stored request's `display_command`/`risk_level`/
    /// `risk_reasons` to reflect the *edited* argv before recording the
    /// decision -- `implementation-handoff.md` §7's rule that "the audit
    /// record must describe what was approved, not what was proposed."
    /// `verified_cwd`/`project_root`/`state_root` are required again for
    /// the same reason `receive_proposal` needs them: re-classification
    /// must use the same trusted context, never anything derived from the
    /// proposal's own claims.
    #[allow(clippy::too_many_arguments)]
    pub fn decide_with_edited_argv(
        &mut self,
        agent_run_id: &AgentRunId,
        proposal_id: &ProposalId,
        edited_argv: Vec<String>,
        verified_cwd: &VerifiedCwd,
        project_root: &Path,
        state_root: &Path,
    ) -> DecideOutcome {
        let key = (agent_run_id.clone(), proposal_id.clone());
        let Some(pending) = self.requests.get_mut(&key) else {
            return DecideOutcome::NotFound;
        };
        if pending.request.decision != ApprovalDecision::Pending {
            return DecideOutcome::AlreadyDecided(pending.request.clone());
        }

        let assessment = risk::classify(
            &edited_argv,
            verified_cwd.as_path(),
            project_root,
            state_root,
        );
        pending.request.display_command = display_argv(&edited_argv);
        pending.request.risk_level = assessment.level;
        pending.request.risk_reasons = assessment.reasons;
        pending
            .request
            .decide(ApprovalDecision::EditedAndApproved)
            .expect("guarded immediately above: request.decision was just checked to be Pending");

        let wire_decision = build_wire_decision(
            proposal_id,
            DecisionOutcome::EditedAndApproved,
            Some(edited_argv),
        );
        let sent = pending.accepted.send_decision(&wire_decision);
        DecideOutcome::Decided {
            request: pending.request.clone(),
            sent,
        }
    }

    /// Looks up a request without mutating anything, for callers that need
    /// to inspect current state (e.g. before deciding).
    pub fn find(
        &self,
        agent_run_id: &AgentRunId,
        proposal_id: &ProposalId,
    ) -> Option<&ApprovalRequest> {
        self.requests
            .get(&(agent_run_id.clone(), proposal_id.clone()))
            .map(|pending| &pending.request)
    }
}

/// Builds the wire `CommandDecision` for an already-validated proposal id
/// and an outcome this coordinator itself determined -- both are trusted,
/// internally-generated values by this point (the proposal id was already
/// validated when the original proposal was decoded; `outcome`/
/// `edited_argv` come from this module's own logic, not from re-parsing
/// anything adapter-supplied), so a decode failure here would indicate a
/// bug in this function, not adversarial input.
fn build_wire_decision(
    proposal_id: &ProposalId,
    outcome: DecisionOutcome,
    edited_argv: Option<Vec<String>>,
) -> CommandDecision {
    CommandDecision::decode(
        PROTOCOL_VERSION,
        proposal_id.as_str().to_string(),
        outcome,
        edited_argv,
    )
    .expect("a decision built from an already-validated proposal id must satisfy decode's bounds")
}

// --- Display-only argv rendering (response 114 Required 2) -------------

/// Every Unicode **Format (`Cf`)** category codepoint -- response 115
/// Required A: escaping only the two bidi-override ranges (RFC-016
/// §Security point 1) implements point 1 but not point 3, which requires
/// "other invisible or format characters" to be made visible "on the same
/// principle" -- zero-width joiners/non-joiners, zero-width space, soft
/// hyphen, and other bidi *marks* (not just overrides) all sit outside
/// those two ranges. The reviewer's probe showed three genuine bidi
/// controls (LRM `U+200E`, RLM `U+200F`, ALM `U+061C`) and several
/// zero-width/invisible characters (ZWSP, ZWNJ, ZWJ, soft hyphen, BOM)
/// passing through unescaped under the narrower rule -- `U+200B` in a
/// filename argument (`impor<ZWSP>tant.txt` rendering as `important.txt`)
/// is Required 2's display-ambiguity defect surviving in a form the
/// original fix did not cover, since it is neither whitespace nor a `Cc`
/// control by Rust's own classification.
///
/// Hand-rolled rather than a dependency, per the reviewer: `Cf` is a
/// small, stable set of ranges.
fn is_format_char(c: char) -> bool {
    matches!(
        c as u32,
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x13438
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// Any character this module escapes to a visible `<U+XXXX>` marker
/// rather than passing through raw: every Unicode **Control (`Cc`)**
/// character (`char::is_control`) plus every **Format (`Cf`)** character
/// (`is_format_char`, above) -- the general-category rule the reviewer
/// prescribed in place of an enumerated list, so it covers whatever
/// invisible-character shape nobody has thought of yet, not just the ones
/// already found.
///
/// **Deliberately not extended to homoglyphs/confusables** (response 115
/// Q1): Cyrillic `о` versus Latin `o` is a different problem class
/// (script-mixing detection, a skeleton algorithm), which RFC-016 does
/// not require, and a policy loose enough to catch it would over-escape
/// legitimate Cyrillic, Greek, and CJK text -- breaking the i18n
/// requirement RFC-016 exists to satisfy. Invisible characters are
/// unambiguously wrong in a security display; visible non-Latin
/// characters are the point of having i18n. This is the line, and it
/// stops here.
fn is_escaped_control(c: char) -> bool {
    c.is_control() || is_format_char(c)
}

/// Shell metacharacters that make an argument's boundary ambiguous when
/// simply concatenated with neighbours -- not an attempt at a complete or
/// execution-safe shell-quoting implementation (nothing here is ever
/// re-parsed or executed), just enough to keep a human reading the
/// rendered string from mistaking one argument for several.
const SHELL_METACHARACTERS: &[char] = &[
    ';', '|', '&', '`', '$', '\\', '"', '\'', '<', '>', '(', ')', '*', '?', '[', ']', '{', '}',
    '~', '#', '!',
];

fn needs_quoting(entry: &str) -> bool {
    entry.is_empty()
        || entry.chars().any(|c| {
            c.is_whitespace() || is_escaped_control(c) || SHELL_METACHARACTERS.contains(&c)
        })
}

/// Renders one argv entry for display: control characters (including
/// RFC-016's bidi range) become visible `<U+XXXX>` markers, and the whole
/// entry is single-quoted if it is empty or contains anything that would
/// otherwise make its boundary ambiguous next to its neighbours.
fn display_entry(entry: &str) -> String {
    let mut escaped = String::with_capacity(entry.len());
    for c in entry.chars() {
        if is_escaped_control(c) {
            escaped.push_str(&format!("<U+{:04X}>", c as u32));
        } else {
            escaped.push(c);
        }
    }
    if !needs_quoting(entry) {
        return escaped;
    }
    format!("'{}'", escaped.replace('\'', r"'\''"))
}

/// Renders `argv` for human display only -- never re-parsed, never
/// executed. Response 114 Required 1: the previous version,
/// `argv.join(" ")`, put back exactly the ambiguity "argv is a vector,
/// never a shell string" exists to prevent, at the one layer where a
/// human makes the approval decision -- an entry containing a semicolon
/// could read as two shell commands, an entry containing a space could
/// read as two arguments, an embedded newline could hide text, and an
/// empty entry vanished with no trace. Quoting and escaping per entry
/// closes all four.
fn display_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|entry| display_entry(entry))
        .collect::<Vec<_>>()
        .join(" ")
}
