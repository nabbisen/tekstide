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
    pub cwd: PathBuf,
    pub environment_summary: Option<String>,
    pub created_at: DomainTimestamp,
    pub decided_at: Option<DomainTimestamp>,
    pub decision_audit_event_id: Option<AuditEventId>,
    pub decision: ApprovalDecision,
}

impl ApprovalRequest {
    pub fn pending(
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        requested_action_kind: impl Into<String>,
        display_command: impl Into<String>,
        risk_level: RiskLevel,
        cwd: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id: ApprovalId::new_uuid(),
            project_id,
            agent_run_id,
            requested_action_kind: requested_action_kind.into(),
            display_command: display_command.into(),
            risk_level,
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
