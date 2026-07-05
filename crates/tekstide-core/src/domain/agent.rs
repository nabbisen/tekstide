use super::{
    AgentRunId, ApprovalId, ApprovalRequest, AuditEventId, ChangeSetId, DomainTimestamp,
    OwnershipError, TerminalId, TerminalSession, TranscriptId,
};
use crate::domain::ownership::ensure_same_project;
use crate::project::ProjectId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentCompatibilityLevel {
    Plain,
    Supervised,
    Managed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunStatus {
    Draft,
    Ready,
    Preparing,
    Running,
    AwaitingApproval,
    ReviewReady,
    Completed,
    Failed,
    Cancelled,
    Detached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRun {
    // Persistent/reference metadata.
    pub id: AgentRunId,
    pub project_id: ProjectId,
    pub profile_id: String,
    pub terminal_id: Option<TerminalId>,
    pub prompt_summary: String,
    pub full_prompt_ref: Option<String>,
    pub compatibility_level: AgentCompatibilityLevel,
    pub started_at: Option<DomainTimestamp>,
    pub ended_at: Option<DomainTimestamp>,
    pub transcript_ref: Option<TranscriptId>,
    pub approval_ids: Vec<ApprovalId>,
    pub change_set_ids: Vec<ChangeSetId>,
    pub artifact_refs: Vec<String>,
    pub audit_event_ids: Vec<AuditEventId>,
    // Runtime lifecycle summary. It records Tekstide's known lifecycle state, not a process
    // handle or proof of supervision after `Detached`.
    pub status: AgentRunStatus,
}

impl AgentRun {
    pub fn draft(
        project_id: ProjectId,
        profile_id: impl Into<String>,
        prompt_summary: impl Into<String>,
        compatibility_level: AgentCompatibilityLevel,
    ) -> Self {
        Self {
            id: AgentRunId::new_uuid(),
            project_id,
            profile_id: profile_id.into(),
            terminal_id: None,
            prompt_summary: prompt_summary.into(),
            full_prompt_ref: None,
            compatibility_level,
            started_at: None,
            ended_at: None,
            transcript_ref: None,
            approval_ids: Vec::new(),
            change_set_ids: Vec::new(),
            artifact_refs: Vec::new(),
            audit_event_ids: Vec::new(),
            status: AgentRunStatus::Draft,
        }
    }

    pub fn transition_to(&mut self, next: AgentRunStatus) -> Result<(), AgentRunTransitionError> {
        if can_transition_agent_run(self.status, next) {
            self.status = next;
            Ok(())
        } else {
            Err(AgentRunTransitionError {
                from: self.status,
                to: next,
            })
        }
    }

    pub fn attach_terminal(&mut self, terminal: &TerminalSession) -> Result<(), OwnershipError> {
        ensure_same_project(&self.project_id, &terminal.project_id)?;
        if self.terminal_id.as_ref() == Some(&terminal.id) {
            return Ok(());
        }
        if self.terminal_id.is_some() {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.terminal_id = Some(terminal.id.clone());
        Ok(())
    }

    pub fn add_approval(&mut self, approval: &ApprovalRequest) -> Result<(), OwnershipError> {
        ensure_same_project(&self.project_id, &approval.project_id)?;
        if approval.agent_run_id.as_ref() != Some(&self.id) {
            return Err(OwnershipError::WrongAgentRun);
        }
        if self.approval_ids.contains(&approval.id) {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.approval_ids.push(approval.id.clone());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentRunTransitionError {
    pub from: AgentRunStatus,
    pub to: AgentRunStatus,
}

fn can_transition_agent_run(from: AgentRunStatus, to: AgentRunStatus) -> bool {
    use AgentRunStatus::{
        AwaitingApproval, Cancelled, Completed, Detached, Draft, Failed, Preparing, Ready,
        ReviewReady, Running,
    };

    matches!(
        (from, to),
        (Draft, Ready)
            | (Draft, Cancelled)
            | (Ready, Preparing)
            | (Ready, Cancelled)
            | (Preparing, Running)
            | (Preparing, Failed)
            | (Preparing, Cancelled)
            | (Running, AwaitingApproval)
            | (Running, ReviewReady)
            | (Running, Completed)
            | (Running, Failed)
            | (Running, Cancelled)
            | (Running, Detached)
            | (AwaitingApproval, Running)
            | (AwaitingApproval, Failed)
            | (AwaitingApproval, Cancelled)
            | (ReviewReady, Completed)
            | (ReviewReady, Failed)
            | (ReviewReady, Cancelled)
    )
}
