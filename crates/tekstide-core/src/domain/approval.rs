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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
