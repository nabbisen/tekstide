//! RFC-021 PR-021-E1: the trusted-context coordinator.
//!
//! This is where a `CommandProposal`'s untrusted claims meet the actual,
//! already-authenticated `AgentRun`/`ProjectSession` state that
//! `approval::channel` and `approval::risk` deliberately do not depend on.
//! Two properties this module exists specifically to enforce:
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
//!    `CommandProposal::cwd()` at all.**
//! 2. **A proposal id is single-use within an `AgentRun`.** A second
//!    proposal carrying an id already seen for this run returns the
//!    existing request inertly -- no reclassification, no new
//!    `ApprovalRequest`. A decision, once made, is likewise final: a
//!    repeated `decide` call (whether from a genuine retry or a replayed
//!    decision message) returns the existing terminal state unchanged
//!    rather than re-running anything or overwriting the first decision.
//!
//! **Explicitly out of scope for this slice (deferred to PR-021-E2):**
//! writing any of this to durable audit via `AuditCoordinator`, sending
//! `CommandDecision` back over the wire, and re-classifying edited argv
//! for `EditedAndApproved` (`implementation-handoff.md` §7). This module
//! only produces in-memory `domain::ApprovalRequest` values; E2 wires
//! those to the audit family and the channel.

use std::collections::HashMap;
use std::path::Path;

use crate::domain::{AgentRunId, ApprovalDecision, ApprovalRequest};
use crate::project::ProjectId;

use super::protocol::{CommandProposal, ProposalId};
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
    /// fresh `Pending` request was created and classified.
    Created(ApprovalRequest),
    /// This exact proposal id was already seen for this run. The existing
    /// request (whatever its current state -- still `Pending`, or already
    /// decided) is returned unchanged; nothing is reclassified or mutated.
    Duplicate(ApprovalRequest),
}

impl ReceiveOutcome {
    pub fn request(&self) -> &ApprovalRequest {
        match self {
            ReceiveOutcome::Created(request) | ReceiveOutcome::Duplicate(request) => request,
        }
    }

    pub fn is_duplicate(&self) -> bool {
        matches!(self, ReceiveOutcome::Duplicate(_))
    }
}

/// Result of [`ApprovalCoordinator::decide`].
#[derive(Clone, Debug)]
pub enum DecideOutcome {
    /// The first decision for this proposal id. `request` now reflects it.
    Decided(ApprovalRequest),
    /// This proposal id was already decided; the decision requested this
    /// time (whatever it was) had no effect -- the existing terminal state
    /// is returned unchanged.
    AlreadyDecided(ApprovalRequest),
    /// No request exists for this `(AgentRunId, ProposalId)` pair -- a
    /// decision cannot be made for a proposal that was never received.
    NotFound,
}

impl DecideOutcome {
    pub fn request(&self) -> Option<&ApprovalRequest> {
        match self {
            DecideOutcome::Decided(request) | DecideOutcome::AlreadyDecided(request) => {
                Some(request)
            }
            DecideOutcome::NotFound => None,
        }
    }
}

/// Per-run, in-memory bookkeeping for proposal-id uniqueness and single-use
/// decisions. Nothing here is durable -- see the module doc for why audit
/// wiring is explicitly out of scope for this slice.
#[derive(Default)]
pub struct ApprovalCoordinator {
    requests: HashMap<(AgentRunId, ProposalId), ApprovalRequest>,
}

impl ApprovalCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Classifies and registers a freshly-authenticated `CommandProposal`.
    ///
    /// `verified_cwd` and `project_root` must come from the caller's own,
    /// already-authenticated `AgentRun`/`ProjectSession` state -- **never**
    /// from `proposal.cwd()`. There is no parameter here that accepts
    /// `proposal.cwd()`'s value at all: the type signature is the
    /// enforcement mechanism, not a comment asking callers to be careful.
    #[allow(clippy::too_many_arguments)]
    pub fn receive_proposal(
        &mut self,
        project_id: ProjectId,
        agent_run_id: AgentRunId,
        verified_cwd: &Path,
        project_root: &Path,
        state_root: &Path,
        proposal: &CommandProposal,
    ) -> ReceiveOutcome {
        let key = (agent_run_id.clone(), proposal.proposal_id().clone());
        if let Some(existing) = self.requests.get(&key) {
            return ReceiveOutcome::Duplicate(existing.clone());
        }

        let assessment = risk::classify(proposal.argv(), verified_cwd, project_root, state_root);
        let display_command = proposal.argv().join(" ");
        let request = ApprovalRequest::pending(
            project_id,
            Some(agent_run_id),
            COMMAND_EXECUTION_ACTION_KIND,
            display_command,
            assessment.level,
            verified_cwd,
        );
        self.requests.insert(key, request.clone());
        ReceiveOutcome::Created(request)
    }

    /// Applies a decision to a previously-received request. See
    /// [`DecideOutcome`] for what happens on a replay.
    pub fn decide(
        &mut self,
        agent_run_id: &AgentRunId,
        proposal_id: &ProposalId,
        decision: ApprovalDecision,
    ) -> DecideOutcome {
        let key = (agent_run_id.clone(), proposal_id.clone());
        let Some(request) = self.requests.get_mut(&key) else {
            return DecideOutcome::NotFound;
        };
        if request.decision != ApprovalDecision::Pending {
            // Single-use, inert replay: the decision requested this call
            // (whatever it is) is discarded, and the existing terminal
            // state is returned unchanged -- not an error the caller must
            // specially handle, since a replay is an expected event on
            // this path (a resent decision message, a retried UI action),
            // not an exceptional one.
            return DecideOutcome::AlreadyDecided(request.clone());
        }
        request
            .decide(decision)
            .expect("guarded immediately above: request.decision was just checked to be Pending");
        DecideOutcome::Decided(request.clone())
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
    }
}
