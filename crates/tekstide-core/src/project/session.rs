use std::path::PathBuf;

use crate::close::{CloseResourceProviderState, CloseResourceSummary};
use crate::domain::{
    AgentRun, AgentRunId, AgentRunStatus, ApprovalDecision, ApprovalId, ApprovalRequest,
    AuditEvent, ChangeSet, DomainTimestamp, OwnershipError, ReviewState, TerminalId,
    TerminalSession, TerminalStatus, Transcript,
};

use super::{
    ProjectFileState, ProjectGitSummary, ProjectId, ProjectMode, ProjectOpenSurface,
    ProjectProviderState, ProjectResourceLimits, ProjectRuntimeSummary, ProjectWarningState,
    WorkspaceTrust,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSession {
    id: ProjectId,
    display_name: String,
    root_path: PathBuf,
    canonical_root_path: PathBuf,
    trust_state: WorkspaceTrust,
    created_at: DomainTimestamp,
    last_opened_at: DomainTimestamp,
    last_activity_at: DomainTimestamp,
    open_surface: ProjectOpenSurface,
    mode: ProjectMode,
    resource_limits: ProjectResourceLimits,
    file_state: ProjectFileState,
    git_summary: ProjectGitSummary,
    warning_state: ProjectWarningState,
    runtime_summary: ProjectRuntimeSummary,
    terminal_sessions: Vec<TerminalSession>,
    agent_runs: Vec<AgentRun>,
    approval_requests: Vec<ApprovalRequest>,
    transcripts: Vec<Transcript>,
    change_sets: Vec<ChangeSet>,
    audit_events: Vec<AuditEvent>,
}

impl ProjectSession {
    pub fn new(
        id: ProjectId,
        display_name: impl Into<String>,
        root_path: impl Into<PathBuf>,
        canonical_root_path: impl Into<PathBuf>,
    ) -> Self {
        let opened_at = DomainTimestamp::now_utc();
        Self {
            id,
            display_name: display_name.into(),
            root_path: root_path.into(),
            canonical_root_path: canonical_root_path.into(),
            trust_state: WorkspaceTrust::Restricted,
            created_at: opened_at.clone(),
            last_opened_at: opened_at.clone(),
            last_activity_at: opened_at,
            open_surface: ProjectOpenSurface::ProjectDashboard,
            mode: ProjectMode::Content,
            resource_limits: ProjectResourceLimits::default(),
            file_state: ProjectFileState::default(),
            git_summary: ProjectGitSummary::default(),
            warning_state: ProjectWarningState::default(),
            runtime_summary: ProjectRuntimeSummary::default(),
            terminal_sessions: Vec::new(),
            agent_runs: Vec::new(),
            approval_requests: Vec::new(),
            transcripts: Vec::new(),
            change_sets: Vec::new(),
            audit_events: Vec::new(),
        }
    }

    pub fn id(&self) -> &ProjectId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }

    pub fn canonical_root_path(&self) -> &PathBuf {
        &self.canonical_root_path
    }

    pub fn trust_state(&self) -> WorkspaceTrust {
        self.trust_state
    }

    pub fn created_at(&self) -> &DomainTimestamp {
        &self.created_at
    }

    pub fn last_opened_at(&self) -> &DomainTimestamp {
        &self.last_opened_at
    }

    pub fn last_activity_at(&self) -> &DomainTimestamp {
        &self.last_activity_at
    }

    pub fn open_surface(&self) -> ProjectOpenSurface {
        self.open_surface
    }

    pub fn mode(&self) -> ProjectMode {
        self.mode
    }

    pub fn resource_limits(&self) -> ProjectResourceLimits {
        self.resource_limits
    }

    pub fn file_state(&self) -> &ProjectFileState {
        &self.file_state
    }

    pub fn git_summary(&self) -> &ProjectGitSummary {
        &self.git_summary
    }

    pub fn warning_state(&self) -> &ProjectWarningState {
        &self.warning_state
    }

    pub fn runtime_summary(&self) -> &ProjectRuntimeSummary {
        &self.runtime_summary
    }

    pub fn close_resource_summary(&self) -> &CloseResourceSummary {
        &self.runtime_summary.close_resources
    }

    pub fn terminal_sessions(&self) -> &[TerminalSession] {
        &self.terminal_sessions
    }

    pub fn agent_runs(&self) -> &[AgentRun] {
        &self.agent_runs
    }

    pub fn approval_requests(&self) -> &[ApprovalRequest] {
        &self.approval_requests
    }

    pub fn transcripts(&self) -> &[Transcript] {
        &self.transcripts
    }

    pub fn change_sets(&self) -> &[ChangeSet] {
        &self.change_sets
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        &self.audit_events
    }

    pub fn grant_trust(&mut self, summary: impl Into<String>) -> &AuditEvent {
        self.trust_state = WorkspaceTrust::Trusted;
        self.audit_events
            .push(AuditEvent::trust_granted(self.id.clone(), summary));
        self.record_activity();
        self.audit_events
            .last()
            .expect("trust audit event should be present after push")
    }

    pub fn revoke_trust(&mut self, summary: impl Into<String>) -> &AuditEvent {
        self.trust_state = WorkspaceTrust::Revoked;
        self.audit_events
            .push(AuditEvent::trust_revoked(self.id.clone(), summary));
        self.record_activity();
        self.audit_events
            .last()
            .expect("trust audit event should be present after push")
    }

    pub fn add_terminal_session(
        &mut self,
        terminal: TerminalSession,
    ) -> Result<(), OwnershipError> {
        self.ensure_project_member(&terminal.project_id)?;
        if self
            .terminal_sessions
            .iter()
            .any(|existing| existing.id == terminal.id)
        {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.terminal_sessions.push(terminal);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn add_agent_run(&mut self, run: AgentRun) -> Result<(), OwnershipError> {
        self.ensure_project_member(&run.project_id)?;
        if self.agent_runs.iter().any(|existing| existing.id == run.id) {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.agent_runs.push(run);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn add_approval_request(
        &mut self,
        approval: ApprovalRequest,
    ) -> Result<(), OwnershipError> {
        self.ensure_project_member(&approval.project_id)?;
        if let Some(agent_run_id) = &approval.agent_run_id {
            self.ensure_agent_run_exists(agent_run_id)?;
        }
        if self
            .approval_requests
            .iter()
            .any(|existing| existing.id == approval.id)
        {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.approval_requests.push(approval);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn add_transcript(&mut self, transcript: Transcript) -> Result<(), OwnershipError> {
        self.ensure_project_member(&transcript.project_id)?;
        self.ensure_terminal_exists(&transcript.terminal_id)?;
        if let Some(agent_run_id) = &transcript.agent_run_id {
            self.ensure_agent_run_exists(agent_run_id)?;
        }
        if self
            .transcripts
            .iter()
            .any(|existing| existing.id == transcript.id)
        {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.transcripts.push(transcript);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn add_change_set(&mut self, change_set: ChangeSet) -> Result<(), OwnershipError> {
        self.ensure_project_member(&change_set.project_id)?;
        if let Some(agent_run_id) = &change_set.agent_run_id {
            self.ensure_agent_run_exists(agent_run_id)?;
        }
        if self
            .change_sets
            .iter()
            .any(|existing| existing.id == change_set.id)
        {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.change_sets.push(change_set);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn add_audit_event(&mut self, event: AuditEvent) -> Result<(), OwnershipError> {
        let project_id = event
            .project_id
            .as_ref()
            .ok_or(OwnershipError::MissingProject)?;
        self.ensure_project_member(project_id)?;
        if let Some(terminal_id) = &event.terminal_id {
            self.ensure_terminal_exists(terminal_id)?;
        }
        if let Some(agent_run_id) = &event.agent_run_id {
            self.ensure_agent_run_exists(agent_run_id)?;
        }
        if let Some(approval_id) = &event.approval_id {
            self.ensure_approval_exists(approval_id)?;
        }
        if self
            .audit_events
            .iter()
            .any(|existing| existing.id == event.id)
        {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.audit_events.push(event);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn mark_opened(&mut self) {
        self.mark_opened_at(DomainTimestamp::now_utc());
    }

    pub fn mark_opened_at(&mut self, opened_at: DomainTimestamp) {
        self.last_opened_at = opened_at.clone();
        self.last_activity_at = opened_at;
    }

    pub fn record_activity(&mut self) {
        self.record_activity_at(DomainTimestamp::now_utc());
    }

    pub fn record_activity_at(&mut self, activity_at: DomainTimestamp) {
        self.last_activity_at = activity_at;
    }

    pub fn set_open_surface(&mut self, open_surface: ProjectOpenSurface) {
        self.open_surface = open_surface;
        self.record_activity();
    }

    pub fn set_mode(&mut self, mode: ProjectMode) {
        self.mode = mode;
        self.record_activity();
    }

    pub fn set_resource_limits(&mut self, resource_limits: ProjectResourceLimits) {
        self.resource_limits = resource_limits;
        self.record_activity();
    }

    pub fn set_file_state(&mut self, file_state: ProjectFileState) {
        self.runtime_summary.dirty_files = file_state.dirty_file_count;
        self.runtime_summary.close_resources.dirty_files =
            if file_state.provider_state == ProjectProviderState::Complete {
                file_state.dirty_file_count
            } else {
                if self.runtime_summary.close_resources.provider_state
                    == CloseResourceProviderState::Complete
                {
                    self.runtime_summary.close_resources.provider_state =
                        CloseResourceProviderState::Unavailable;
                }
                0
            };
        self.file_state = file_state;
        self.record_activity();
    }

    pub fn set_git_summary(&mut self, git_summary: ProjectGitSummary) {
        self.git_summary = git_summary;
        self.record_activity();
    }

    pub fn set_warning_state(&mut self, warning_state: ProjectWarningState) {
        self.runtime_summary.risk_warning = warning_state.has_risk_warning();
        self.warning_state = warning_state;
        self.record_activity();
    }

    #[cfg(test)]
    pub fn set_runtime_summary(&mut self, runtime_summary: ProjectRuntimeSummary) {
        self.runtime_summary = runtime_summary;
    }

    fn ensure_project_member(&self, project_id: &ProjectId) -> Result<(), OwnershipError> {
        if &self.id == project_id {
            Ok(())
        } else {
            Err(OwnershipError::CrossProject)
        }
    }

    // Live collection inserts are intentionally order-dependent. Runtime callers must add a
    // referenced entity before adding records that link to it; a future restore builder can validate
    // out-of-order persisted graphs separately.
    fn ensure_terminal_exists(&self, terminal_id: &TerminalId) -> Result<(), OwnershipError> {
        self.terminal_sessions
            .iter()
            .any(|terminal| terminal.id == *terminal_id)
            .then_some(())
            .ok_or(OwnershipError::MissingReference)
    }

    fn ensure_agent_run_exists(&self, agent_run_id: &AgentRunId) -> Result<(), OwnershipError> {
        self.agent_runs
            .iter()
            .any(|run| run.id == *agent_run_id)
            .then_some(())
            .ok_or(OwnershipError::MissingReference)
    }

    fn ensure_approval_exists(&self, approval_id: &ApprovalId) -> Result<(), OwnershipError> {
        self.approval_requests
            .iter()
            .any(|approval| approval.id == *approval_id)
            .then_some(())
            .ok_or(OwnershipError::MissingReference)
    }

    fn refresh_runtime_summary_from_collections(&mut self) {
        let terminal_count = len_as_u32(self.terminal_sessions.len());
        let agent_run_count = len_as_u32(self.agent_runs.len());
        let pending_approvals = len_as_u32(
            self.approval_requests
                .iter()
                .filter(|approval| approval.decision == ApprovalDecision::Pending)
                .count(),
        );
        let review_ready_changes = len_as_u32(
            self.change_sets
                .iter()
                .filter(|change_set| change_set.review_state == ReviewState::Unreviewed)
                .count(),
        );
        let running_processes = len_as_u32(
            self.terminal_sessions
                .iter()
                .filter(|terminal| terminal_status_is_active(terminal.status))
                .count()
                + self
                    .agent_runs
                    .iter()
                    .filter(|run| agent_run_status_is_active(run.status))
                    .count(),
        );
        let failed_processes = len_as_u32(
            self.terminal_sessions
                .iter()
                .filter(|terminal| terminal.status == TerminalStatus::Failed)
                .count()
                + self
                    .agent_runs
                    .iter()
                    .filter(|run| run.status == AgentRunStatus::Failed)
                    .count(),
        );
        let dirty_files = self.runtime_summary.dirty_files;

        self.runtime_summary.terminal_count = Some(terminal_count);
        self.runtime_summary.agent_run_count = Some(agent_run_count);
        self.runtime_summary.pending_approvals = pending_approvals;
        self.runtime_summary.review_ready_changes = review_ready_changes;
        self.runtime_summary.running_processes = running_processes;
        self.runtime_summary.failed_processes = failed_processes;
        self.runtime_summary.close_resources.running_processes = running_processes;
        self.runtime_summary.close_resources.dirty_files = dirty_files;
        self.runtime_summary.close_resources.pending_approvals = pending_approvals;
        self.runtime_summary.close_resources.review_ready_changes = review_ready_changes;
    }
}

fn terminal_status_is_active(status: TerminalStatus) -> bool {
    matches!(
        status,
        TerminalStatus::Starting
            | TerminalStatus::Running
            | TerminalStatus::Terminating
            | TerminalStatus::OrphanedUnknown
    )
}

fn agent_run_status_is_active(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Preparing | AgentRunStatus::Running | AgentRunStatus::AwaitingApproval
    )
}

fn len_as_u32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
