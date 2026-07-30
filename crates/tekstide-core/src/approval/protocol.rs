//! `CommandProposal` / `CommandDecision`: the two sideband messages RFC-021
//! defines. Both are constructed only through a fallible, validating
//! constructor -- there is no way to hold an instance of either type that
//! has not already passed every bound and shape check below. This is
//! deliberately stricter than `audit::record::DurableAuditRecordV1`, whose
//! `validate()` runs separately from construction: that type is built from
//! already-trusted, internally-generated data, while these two cross an
//! untrusted boundary from an adapter process, so parsing and validating
//! happen as one inseparable step.
//!
//! What this module does *not* do: decide whether a token is the *correct*
//! token for a run, decide whether a proposal id has been seen before, or
//! decide whether a path stays inside a project root. Those all require
//! context (the real per-run token, prior proposal ids, the project root)
//! that a pure protocol decoder does not have. They belong to
//! `approval::channel` and `approval::coordinator`.

use std::path::{Path, PathBuf};

/// The only protocol version this build understands. Per RFC-021: unknown
/// version fails closed -- there is no negotiation, no "best effort" parse
/// of a newer or older shape.
pub const PROTOCOL_VERSION: u32 = 1;

pub const MAX_PROPOSAL_ID_LEN: usize = 128;
pub const MAX_TOKEN_LEN: usize = 256;
/// Raised from an initial 64 per response 109 Q3: realistic commands
/// exceed that trivially (`git add` over a large changeset, glob
/// expansion, build invocations with many flags), and the cost of a
/// count this low was not buying safety proportionate to what it
/// rejected -- `ARG_MAX` on Linux is ~2 MB, far above what this and
/// `MAX_ARGV_TOTAL_LEN` together allow.
pub const MAX_ARGV_ENTRIES: usize = 512;
/// Raised from an initial 4096 alongside the entry-count increase: a
/// single legitimate argument can exceed 4 KiB (e.g. `git commit -m
/// "<long message>"`). `MAX_ARGV_TOTAL_LEN` is the real resource bound;
/// this exists to reject a single grotesquely oversized entry rather
/// than to be the primary size control.
pub const MAX_ARGV_ENTRY_LEN: usize = 65536;
/// Total-argv-size bound (sum of entry lengths), per response 109 Q3:
/// count and per-entry bounds are proxies for the actual resource
/// question, which this answers directly. 1 MiB is comfortably above
/// any realistic proposal and comfortably below any resource concern.
pub const MAX_ARGV_TOTAL_LEN: usize = 1_048_576;
pub const MAX_CWD_LEN: usize = 4096;
pub const MAX_INTENT_LEN: usize = 512;
pub const MAX_EFFECTS_HINT_LEN: usize = 512;

/// An adapter-generated, opaque proposal identifier. Bounded and
/// charset-restricted so it can be logged, compared, and used as a
/// correlation key without ever being interpreted as anything else.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ProposalId(String);

impl ProposalId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A per-run capability credential, opaque to everything except the
/// channel that issued it. This type only validates *shape* (bounded,
/// printable). Whether a given token is the correct one for a given run is
/// a `approval::channel` concern -- this layer cannot know that.
#[derive(Clone)]
pub struct RunCapabilityToken(String);

impl RunCapabilityToken {
    /// `pub(crate)`, not `pub` (response 114 Recommended 1): a `pub`
    /// accessor gives any downstream crate (`tekstide-gui`, `tekstide`) a
    /// way to extract the raw secret and log it, which would make "token
    /// never persisted to disk unencrypted" and "token never appears in a
    /// durable audit record" unenforceable by the type system rather than
    /// merely unviolated today. `inject_token_into_environment` -- the one
    /// sanctioned choke point for handing this value to a child process --
    /// lives inside this crate, so `pub(crate)` is sufficient for it.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Constant-time comparison against a raw, not-yet-validated wire
    /// string, for `approval::channel` to check an incoming token before
    /// it has gone through full proposal decoding (response 112
    /// Recommended 7). Uses the same `constant_time_eq` the `PartialEq`
    /// impl uses, so this and `==` never disagree.
    pub(crate) fn matches_raw(&self, raw: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), raw.as_bytes())
    }
}

// Deliberately no `Debug` derive carrying the token value: it is a secret
// credential, and printing it anywhere it might reach a log is exactly the
// leakage RFC-021 §"Token leakage" warns against.
impl std::fmt::Debug for RunCapabilityToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RunCapabilityToken(<redacted>)")
    }
}

impl Eq for RunCapabilityToken {}

/// Constant-time comparison (response 112 Recommended 8): this is a
/// capability comparison in a security boundary, not an ordinary value
/// equality check. A derived `PartialEq` on the underlying `String` would
/// short-circuit at the first differing byte -- realistically
/// unexploitable once `accept()` and JSON parsing sit between an attacker
/// and this comparison, but "we measured the noise floor and judged it
/// unexploitable" is a weaker claim than "it does not vary," and the fix
/// is three lines.
impl PartialEq for RunCapabilityToken {
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq(self.0.as_bytes(), other.0.as_bytes())
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (byte_a, byte_b) in a.iter().zip(b.iter()) {
        diff |= byte_a ^ byte_b;
    }
    diff == 0
}

/// Adapter-declared effects text, e.g. "reads only". Wrapped so the type
/// system itself makes the RFC's rule visible at every use site: this is a
/// display hint, never a basis for lowering risk or skipping approval.
/// There is intentionally no accessor that returns anything richer than
/// the raw string -- nothing here should ever be parsed as structured
/// data or treated as a classification input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntrustedEffectsHint(String);

impl UntrustedEffectsHint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Adapter -> Tekstide. Every field is private with a read-only accessor:
/// there is no way to hold an instance of this type that has not already
/// passed every bound and shape check below, and no way -- via a `&mut
/// CommandProposal` obtained after decoding -- to push a field back out of
/// bounds or swap a token between two decoded proposals. (An earlier
/// version left these fields `pub`; response 109 correctly identified
/// that as violating the module's own stated invariant, since `pub` fields
/// are mutable through any `&mut` reference without going through
/// `decode` at all.) See [`CommandProposal::decode`].
#[derive(Clone, Debug, PartialEq)]
pub struct CommandProposal {
    run_token: RunCapabilityToken,
    proposal_id: ProposalId,
    argv: Vec<String>,
    cwd: PathBuf,
    declared_intent: Option<String>,
    declared_effects: Option<UntrustedEffectsHint>,
}

impl CommandProposal {
    pub fn run_token(&self) -> &RunCapabilityToken {
        &self.run_token
    }

    pub fn proposal_id(&self) -> &ProposalId {
        &self.proposal_id
    }

    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn declared_intent(&self) -> Option<&str> {
        self.declared_intent.as_deref()
    }

    pub fn declared_effects(&self) -> Option<&UntrustedEffectsHint> {
        self.declared_effects.as_ref()
    }

    /// Validates and constructs a `CommandProposal` from raw, untrusted
    /// wire fields. This is the only way to obtain one.
    ///
    /// `protocol_version` and `run_token` are checked first and
    /// independently of everything else, since an unknown version or an
    /// absent/malformed token means the rest of the message cannot be
    /// trusted to mean what its shape suggests -- reject before looking at
    /// anything else.
    #[allow(clippy::too_many_arguments)]
    pub fn decode(
        protocol_version: u32,
        run_token: String,
        proposal_id: String,
        argv: Vec<String>,
        cwd: PathBuf,
        declared_intent: Option<String>,
        declared_effects: Option<String>,
    ) -> Result<Self, ProposalValidationError> {
        if protocol_version != PROTOCOL_VERSION {
            return Err(ProposalValidationError::UnsupportedProtocolVersion);
        }
        let run_token = validate_token(run_token)?;
        let proposal_id = validate_proposal_id(proposal_id)?;
        let argv = validate_argv(argv)?;
        let cwd = validate_cwd(cwd)?;
        let declared_intent = validate_display_text(declared_intent, MAX_INTENT_LEN)
            .map_err(|_| ProposalValidationError::IntentInvalid)?;
        let declared_effects = validate_display_text(declared_effects, MAX_EFFECTS_HINT_LEN)
            .map_err(|_| ProposalValidationError::EffectsHintInvalid)?
            .map(UntrustedEffectsHint);

        Ok(Self {
            run_token,
            proposal_id,
            argv,
            cwd,
            declared_intent,
            declared_effects,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalValidationError {
    UnsupportedProtocolVersion,
    TokenInvalid,
    ProposalIdInvalid,
    ArgvEmpty,
    ArgvTooManyEntries,
    ArgvEntryInvalid,
    ArgvTotalTooLarge,
    CwdInvalid,
    IntentInvalid,
    EffectsHintInvalid,
}

/// Kept as a distinct type from [`ProposalValidationError`] even though the
/// variant sets overlap: a decision message and a proposal message fail
/// for different reasons (a decision has no argv-empty case, but has an
/// edit-presence rule a proposal does not), and collapsing them would
/// force irrelevant variants onto call sites that can never produce them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionValidationErrorReason {
    UnsupportedProtocolVersion,
    ProposalIdInvalid,
    EditedArgvMissingForEditedAndApproved,
    EditedArgvPresentForOtherDecision,
    EditedArgvInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecisionValidationError {
    pub reason: DecisionValidationErrorReason,
}

/// Tekstide -> adapter. Deliberately has no `Pending` variant: a decision
/// message is only ever sent once a decision has actually been made, so
/// "pending" is not a representable wire state -- unlike
/// `domain::ApprovalDecision`, which models the request's lifecycle and
/// does need a pending state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionOutcome {
    ApprovedOnce,
    Rejected,
    EditedAndApproved,
}

/// Fields are private for the same reason as [`CommandProposal`]'s: no
/// path to a valid-but-mutated-past-validation instance.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandDecision {
    proposal_id: ProposalId,
    outcome: DecisionOutcome,
    edited_argv: Option<Vec<String>>,
}

impl CommandDecision {
    pub fn proposal_id(&self) -> &ProposalId {
        &self.proposal_id
    }

    pub fn outcome(&self) -> DecisionOutcome {
        self.outcome
    }

    pub fn edited_argv(&self) -> Option<&[String]> {
        self.edited_argv.as_deref()
    }

    /// Validates and constructs a `CommandDecision`. `edited_argv` must be
    /// `Some` if and only if `outcome` is `EditedAndApproved` -- the RFC's
    /// "edited argv, present only for `EditedAndApproved`" rule is enforced
    /// here as a shape check, not left to whoever reads the field later.
    pub fn decode(
        protocol_version: u32,
        proposal_id: String,
        outcome: DecisionOutcome,
        edited_argv: Option<Vec<String>>,
    ) -> Result<Self, DecisionValidationError> {
        if protocol_version != PROTOCOL_VERSION {
            return Err(DecisionValidationError {
                reason: DecisionValidationErrorReason::UnsupportedProtocolVersion,
            });
        }
        let proposal_id =
            validate_proposal_id(proposal_id).map_err(|_| DecisionValidationError {
                reason: DecisionValidationErrorReason::ProposalIdInvalid,
            })?;

        let edited_argv = match (outcome, edited_argv) {
            (DecisionOutcome::EditedAndApproved, Some(argv)) => {
                Some(validate_argv(argv).map_err(|_| DecisionValidationError {
                    reason: DecisionValidationErrorReason::EditedArgvInvalid,
                })?)
            }
            (DecisionOutcome::EditedAndApproved, None) => {
                return Err(DecisionValidationError {
                    reason: DecisionValidationErrorReason::EditedArgvMissingForEditedAndApproved,
                });
            }
            (_, None) => None,
            (_, Some(_)) => {
                return Err(DecisionValidationError {
                    reason: DecisionValidationErrorReason::EditedArgvPresentForOtherDecision,
                });
            }
        };

        Ok(Self {
            proposal_id,
            outcome,
            edited_argv,
        })
    }
}

/// `pub(crate)` (response 112 Defect 2): `approval::channel` needs to
/// validate a freshly-generated token against the exact same rules an
/// incoming proposal's token is checked against, without constructing an
/// entire `CommandProposal` (with a placeholder `cwd`/argv) just to reach
/// this check indirectly -- that indirection was itself the bug (a token
/// self-check that could panic on a hostile `$TMPDIR`, despite having
/// nothing to do with paths or argv at all). One shared implementation,
/// callable directly by both.
pub(crate) fn validate_token(raw: String) -> Result<RunCapabilityToken, ProposalValidationError> {
    if raw.is_empty()
        || raw.len() > MAX_TOKEN_LEN
        || !raw.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(ProposalValidationError::TokenInvalid);
    }
    Ok(RunCapabilityToken(raw))
}

fn validate_proposal_id(raw: String) -> Result<ProposalId, ProposalValidationError> {
    if raw.is_empty()
        || raw.len() > MAX_PROPOSAL_ID_LEN
        || !raw.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(ProposalValidationError::ProposalIdInvalid);
    }
    Ok(ProposalId(raw))
}

/// argv is validated as a vector of independent strings, never as a shell
/// string to be split. A NUL byte is rejected outright: it cannot appear
/// in a real argv entry passed to `exec`, so its presence indicates a
/// malformed or adversarial proposal, not a legitimate argument. Beyond
/// that, argument content is intentionally permissive -- real command
/// arguments legitimately contain spaces, quotes, and arbitrary UTF-8, and
/// this layer does not interpret shell grammar (an explicit RFC-021
/// non-goal).
///
/// **Empty entries are allowed** (response 109 Q2): `printf '%s' ""`,
/// `grep "" file`, and scripts passing an empty string as an unset
/// placeholder are all ordinary, legitimate argv. The security property
/// that matters is that argv is a vector at all -- no shell tokenization,
/// no quoting ambiguity between what is displayed and what executes.
/// Entry non-emptiness contributes nothing to that, and a rejected
/// proposal is not a soft failure: the adapter's command simply does not
/// run, which pushes users toward routing around approval entirely if it
/// happens often enough for no good reason.
fn validate_argv(argv: Vec<String>) -> Result<Vec<String>, ProposalValidationError> {
    if argv.is_empty() {
        return Err(ProposalValidationError::ArgvEmpty);
    }
    if argv.len() > MAX_ARGV_ENTRIES {
        return Err(ProposalValidationError::ArgvTooManyEntries);
    }
    let mut total_len: usize = 0;
    for entry in &argv {
        if entry.len() > MAX_ARGV_ENTRY_LEN || entry.contains('\0') {
            return Err(ProposalValidationError::ArgvEntryInvalid);
        }
        total_len += entry.len();
    }
    if total_len > MAX_ARGV_TOTAL_LEN {
        return Err(ProposalValidationError::ArgvTotalTooLarge);
    }
    Ok(argv)
}

/// Only shape (absolute, bounded, no embedded NUL) is checked here.
/// Whether the path stays inside the canonical project root is a risk-
/// classification and coordinator concern, not a decoding concern: this
/// layer has no project context to check it against.
fn validate_cwd(cwd: PathBuf) -> Result<PathBuf, ProposalValidationError> {
    let as_str = cwd.to_str().ok_or(ProposalValidationError::CwdInvalid)?;
    if as_str.is_empty() || as_str.len() > MAX_CWD_LEN || as_str.contains('\0') {
        return Err(ProposalValidationError::CwdInvalid);
    }
    if !cwd.is_absolute() {
        return Err(ProposalValidationError::CwdInvalid);
    }
    Ok(cwd)
}

/// Shared bound/shape check for the two free-text display fields (declared
/// intent, declared effects). Both are display-only text, never a
/// classification input, so the only requirements are a length bound and
/// no control characters -- control characters in text a GUI dialog will
/// render verbatim (RFC-022) are a display-spoofing vector in their own
/// right (e.g. embedding line breaks to pad or hide content), independent
/// of the terminal-escape concern RFC-009 already covers for the PTY path.
fn validate_display_text(value: Option<String>, max_len: usize) -> Result<Option<String>, ()> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() || value.len() > max_len || value.chars().any(|c| c.is_control()) {
        return Err(());
    }
    Ok(Some(value))
}
