use super::{AgentRunId, ApprovalId, AuditEventId, DomainTimestamp};
use crate::project::ProjectId;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRequest {
    // Persistent/reference metadata. Decisions are append-only after leaving `Pending`.
    pub id: ApprovalId,
    pub project_id: ProjectId,
    pub agent_run_id: Option<AgentRunId>,
    pub requested_action_kind: String,
    pub display_command: String,
    pub risk_level: RiskLevel,
    /// Why `risk_level` is what it is (RFC-021 PR-021-E1 response 114
    /// Recommended 2, relocated here per response 115 Q2). Kept alongside
    /// `risk_level` rather than only on the transient value that carried
    /// it out of the classifier (`approval::ReceiveOutcome`), since the
    /// two fields are meaningless apart: everything downstream that reads
    /// a stored `ApprovalRequest` -- a dialog, an audit record, a
    /// re-render after the user scrolls away and back -- needs "why", not
    /// just "how severe", and none of those call sites still have the
    /// original `ReceiveOutcome` in hand.
    pub risk_reasons: Vec<RiskReason>,
    pub cwd: PathBuf,
    pub environment_summary: Option<String>,
    pub created_at: DomainTimestamp,
    pub decided_at: Option<DomainTimestamp>,
    pub decision_audit_event_id: Option<AuditEventId>,
    pub decision: ApprovalDecision,
}

impl ApprovalRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn pending(
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        requested_action_kind: impl Into<String>,
        display_command: impl Into<String>,
        risk_level: RiskLevel,
        risk_reasons: Vec<RiskReason>,
        cwd: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id: ApprovalId::new_uuid(),
            project_id,
            agent_run_id,
            requested_action_kind: requested_action_kind.into(),
            display_command: display_command.into(),
            risk_level,
            risk_reasons,
            cwd: cwd.into(),
            environment_summary: None,
            created_at: DomainTimestamp::now_utc(),
            decided_at: None,
            decision_audit_event_id: None,
            decision: ApprovalDecision::Pending,
        }
    }

    pub fn decide(&mut self, decision: ApprovalDecision) -> Result<(), ApprovalDecisionError> {
        if self.decision != ApprovalDecision::Pending {
            return Err(ApprovalDecisionError::AlreadyDecided);
        }
        if decision == ApprovalDecision::Pending {
            return Err(ApprovalDecisionError::StillPending);
        }
        self.decision = decision;
        self.decided_at = Some(DomainTimestamp::now_utc());
        Ok(())
    }
}

/// Variants are declared in ascending severity order deliberately: derived
/// `Ord` makes "at least `High`"-style comparisons (`level >= RiskLevel::High`)
/// a type-checked fact rather than a hand-rolled match every caller has to
/// get right independently (per RFC-021 PR-021-C response 110 recommended-8).
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Destructive,
}

/// A structural reason a proposal was classified the way it was.
/// Deliberately content-free: no captured path, no captured argv entry.
/// Defined here, next to `RiskLevel`, rather than in `approval::risk`
/// (where `RiskLevel` is *consumed* but not defined) -- response 115 Q2:
/// the two fields are meaningless apart, and `approval::ApprovalRequest`
/// (this module) is where both ultimately need to live so a stored
/// request can still answer "why", not just "how severe", after the
/// `ReceiveOutcome` that first carried them out of the classifier is long
/// gone. `approval::risk` re-exports this rather than defining its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiskReason {
    PathOutsideProjectRoot,
    PrivilegeElevation,
    OpaqueShellInvocation,
    OpaqueWrapper,
    GitRemoteMutating,
    SecretLikePath,
    TekstideStateRoot,
    RecursiveDeletion,
    DiskLevelOperation,
    HistoryRewrite,
    /// `git checkout -- <path>` / `git checkout .` / `git checkout -f`
    /// (or `--force`): discards uncommitted working-tree changes,
    /// unrecoverably (nothing was ever committed, so there is no reflog
    /// rescue) -- the same category of data loss as `git reset --hard`,
    /// which is `Destructive`. Also used for `git stash clear`/`drop`
    /// (response 111 Required 3): a purge of saved work is the same shape
    /// of loss, one step removed. Kept distinct from `HistoryRewrite`
    /// because no history is being rewritten; nothing was ever committed
    /// in the first place.
    WorkingTreeDiscard,
    /// `git push --force`/`--force-with-lease`: rewrites history on a
    /// **remote other people pull from**, where there is no reflog to
    /// rescue and the loss is not the operator's alone -- response 111
    /// Required 2 judged this strictly worse than local history rewriting
    /// (`HistoryRewrite`, `Destructive` via `git rebase`/`reset --hard`),
    /// so it gets its own reason at the same `Destructive` level rather
    /// than being folded into either `HistoryRewrite` or the `High`-level
    /// `GitRemoteMutating`.
    RemoteHistoryRewrite,
    Unrecognized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalDecision {
    Pending,
    ApprovedOnce,
    Rejected,
    EditedAndApproved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalDecisionError {
    AlreadyDecided,
    StillPending,
}
