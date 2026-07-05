use super::{
    AgentRun, AgentRunId, ApprovalId, ApprovalRequest, AuditEventId, DomainTimestamp,
    OwnershipError, TerminalId, TerminalSession,
};
use crate::domain::ownership::ensure_same_project;
use crate::project::ProjectId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEvent {
    // Append-only audit metadata. Storage remains a later concern.
    pub id: AuditEventId,
    pub project_id: Option<ProjectId>,
    pub class: AuditEventClass,
    pub terminal_id: Option<TerminalId>,
    pub agent_run_id: Option<AgentRunId>,
    pub approval_id: Option<ApprovalId>,
    pub summary: String,
    pub created_at: DomainTimestamp,
}

impl AuditEvent {
    pub fn new(
        project_id: Option<ProjectId>,
        class: AuditEventClass,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            id: AuditEventId::new_uuid(),
            project_id,
            class,
            terminal_id: None,
            agent_run_id: None,
            approval_id: None,
            summary: summary.into(),
            created_at: DomainTimestamp::now_utc(),
        }
    }

    pub fn for_terminal(mut self, terminal: &TerminalSession) -> Result<Self, OwnershipError> {
        self.ensure_or_adopt_project_id(&terminal.project_id)?;
        self.terminal_id = Some(terminal.id.clone());
        Ok(self)
    }

    pub fn for_agent_run(mut self, run: &AgentRun) -> Result<Self, OwnershipError> {
        self.ensure_or_adopt_project_id(&run.project_id)?;
        self.agent_run_id = Some(run.id.clone());
        Ok(self)
    }

    pub fn for_approval(mut self, approval: &ApprovalRequest) -> Result<Self, OwnershipError> {
        self.ensure_or_adopt_project_id(&approval.project_id)?;
        self.approval_id = Some(approval.id.clone());
        Ok(self)
    }

    fn ensure_or_adopt_project_id(
        &mut self,
        linked_project_id: &ProjectId,
    ) -> Result<(), OwnershipError> {
        if let Some(project_id) = &self.project_id {
            ensure_same_project(project_id, linked_project_id)
        } else {
            self.project_id = Some(linked_project_id.clone());
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditEventClass {
    ProjectAdded,
    TrustGranted,
    TrustRevoked,
    TerminalStarted,
    AgentRunStarted,
    CommandApprovalRequested,
    CommandApproved,
    CommandRejected,
    PasteBlocked,
    ProcessTerminated,
    SafeCloseDecision,
    ConfigChanged,
    TranscriptPurged,
}
