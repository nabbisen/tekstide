use crate::agent::AgentRunLaunchPlan;
use crate::agent::AgentRunTranscriptCaptureError;
use crate::runtime::terminal::{
    LinuxTerminalRuntime, TerminalEnvironmentPolicy, TerminalLaunchError, TerminalRuntimeEvent,
    TerminationOutcome,
};
use crate::transcript::{
    TranscriptLocalDataSummary, TranscriptRetentionLimits, TranscriptRetentionState,
    TranscriptWriteSummary,
};
use std::fs;
use std::path::{Path, PathBuf};

use crate::close::{CloseResourceProviderState, CloseResourceSummary};
use crate::content::{ExternalChangeDecision, SaveDecision, TextDocumentOpenPolicy};
use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunId, AgentRunStatus, AgentRunTransitionError,
    ApprovalDecision, ApprovalId, ApprovalRequest, AuditEvent, ChangeAssociationConfidence,
    ChangeDetectionStatus, ChangeSet, ChangeSetId, DomainTimestamp, OwnershipError, ReviewState,
    TerminalId, TerminalKind, TerminalSession, TerminalStatus, TerminalTransitionError, Transcript,
    TranscriptId, TranscriptLifecycleState, VisibleSlot,
};

use super::change_detection::{
    ChangedPathValidationError, DetectedChanges, GeneratedChangeDetector, ReviewBaseline,
};
use super::root::{FileExplorerScanPolicy, ProjectRootHandle};
use super::{
    ProjectActiveFileLaunchAssessment, ProjectContentError, ProjectContentWorkspace,
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
    content_workspace: ProjectContentWorkspace,
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
            content_workspace: ProjectContentWorkspace::default(),
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

    pub fn content_workspace(&self) -> &ProjectContentWorkspace {
        &self.content_workspace
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

    pub fn terminal_session(&self, terminal_id: &TerminalId) -> Option<&TerminalSession> {
        self.terminal_sessions
            .iter()
            .find(|terminal| terminal.id == *terminal_id)
    }

    pub fn visible_terminal_sessions(&self) -> impl Iterator<Item = &TerminalSession> {
        self.terminal_sessions
            .iter()
            .filter(|terminal| terminal.visible_slot() != VisibleSlot::Hidden)
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

    pub fn transcript_local_data_summary(
        &self,
        app_retained_bytes: u64,
        limits: TranscriptRetentionLimits,
    ) -> TranscriptLocalDataSummary {
        let project_retained_bytes = self
            .transcripts
            .iter()
            .filter(|transcript| transcript.has_retained_bytes())
            .map(|transcript| transcript.byte_count)
            .sum();

        TranscriptLocalDataSummary::new(
            project_retained_bytes,
            app_retained_bytes,
            self.transcripts.len() as u64,
            limits,
        )
    }

    pub fn change_sets(&self) -> &[ChangeSet] {
        &self.change_sets
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        &self.audit_events
    }

    pub(crate) fn grant_trust(&mut self, summary: impl Into<String>) -> &AuditEvent {
        self.trust_state = WorkspaceTrust::Trusted;
        self.audit_events
            .push(AuditEvent::trust_granted(self.id.clone(), summary));
        self.record_activity();
        self.audit_events
            .last()
            .expect("trust audit event should be present after push")
    }

    pub(crate) fn revoke_trust(&mut self, summary: impl Into<String>) -> &AuditEvent {
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

    pub fn transition_terminal_status(
        &mut self,
        terminal_id: &TerminalId,
        status: TerminalStatus,
    ) -> Result<(), ProjectTerminalError> {
        let terminal = self.terminal_session_mut(terminal_id)?;
        terminal.transition_to(status)?;
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn mark_terminal_exited(
        &mut self,
        terminal_id: &TerminalId,
        exit_status: Option<i32>,
    ) -> Result<(), ProjectTerminalError> {
        let terminal = self.terminal_session_mut(terminal_id)?;
        terminal.transition_to(TerminalStatus::Exited)?;
        terminal.exit_status = exit_status;
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn assign_terminal_visible_slot(
        &mut self,
        terminal_id: &TerminalId,
        visible_slot: VisibleSlot,
    ) -> Result<(), ProjectTerminalError> {
        self.ensure_terminal_exists(terminal_id)?;

        if visible_slot != VisibleSlot::Hidden {
            for terminal in &mut self.terminal_sessions {
                if terminal.id != *terminal_id && terminal.visible_slot() == visible_slot {
                    terminal.assign_visible_slot(VisibleSlot::Hidden);
                }
            }
        }

        let terminal = self.terminal_session_mut(terminal_id)?;
        terminal.assign_visible_slot(visible_slot);
        self.record_activity();
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

    pub fn attach_agent_launch_plan(
        &mut self,
        plan: AgentRunLaunchPlan,
        mut terminal: TerminalSession,
    ) -> Result<AgentRunId, ProjectAgentLaunchError> {
        self.ensure_project_member(plan.spec().project_id())?;
        self.ensure_project_member(&plan.terminal_launch_spec().project_id)?;
        self.ensure_project_member(&plan.agent_run().project_id)?;
        self.ensure_project_member(&terminal.project_id)?;

        if self
            .terminal_sessions
            .iter()
            .any(|existing| existing.id == terminal.id)
        {
            return Err(ProjectAgentLaunchError::Ownership(
                OwnershipError::DuplicateAttachment,
            ));
        }
        if self
            .agent_runs
            .iter()
            .any(|existing| existing.id == plan.agent_run().id)
        {
            return Err(ProjectAgentLaunchError::Ownership(
                OwnershipError::DuplicateAttachment,
            ));
        }
        if !terminal_matches_launch_spec(&terminal, &plan) {
            return Err(ProjectAgentLaunchError::TerminalDoesNotMatchLaunchSpec);
        }

        let (_, mut agent_run, terminal_launch_spec) = plan.into_parts();
        agent_run.attach_terminal(&terminal)?;
        terminal.environment_policy_ref =
            terminal_environment_policy_ref(&terminal_launch_spec.environment_policy);
        let agent_run_id = agent_run.id.clone();

        self.terminal_sessions.push(terminal);
        self.agent_runs.push(agent_run);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();

        Ok(agent_run_id)
    }

    #[cfg(test)]
    pub(crate) fn launch_agent_run_with_runtime(
        &mut self,
        mut plan: AgentRunLaunchPlan,
        runtime: &mut LinuxTerminalRuntime,
    ) -> Result<(AgentRunId, Vec<TerminalRuntimeEvent>), ProjectAgentRuntimeLaunchError> {
        self.prepare_agent_run_launch(&mut plan)?;
        self.launch_prepared_agent_run_with_runtime(plan, runtime)
    }

    pub(crate) fn prepare_agent_run_launch(
        &mut self,
        plan: &mut AgentRunLaunchPlan,
    ) -> Result<(), ProjectAgentRuntimeLaunchError> {
        self.validate_agent_launch_plan_before_runtime(plan)?;
        self.ensure_agent_launch_active_file_safety()?;
        plan.prepare_transcript_capture()?;
        Ok(())
    }

    pub(crate) fn launch_prepared_agent_run_with_runtime(
        &mut self,
        mut plan: AgentRunLaunchPlan,
        runtime: &mut LinuxTerminalRuntime,
    ) -> Result<(AgentRunId, Vec<TerminalRuntimeEvent>), ProjectAgentRuntimeLaunchError> {
        let transcript_storage_path = plan.transcript_storage_path().cloned();

        plan.transition_agent_run_to(AgentRunStatus::Preparing)?;
        let (terminal, events) =
            runtime.launch_project_shell(self, plan.terminal_launch_spec_for_runtime())?;
        let terminal_id = terminal.id.clone();
        plan.transition_agent_run_to(AgentRunStatus::Running)?;

        let agent_run_id = self.attach_agent_launch_plan(plan, terminal)?;
        if let Some(storage_path) = transcript_storage_path {
            self.attach_agent_run_transcript(
                agent_run_id.clone(),
                terminal_id,
                storage_path.transcript_file().to_path_buf(),
                "local-bounded-agent-run",
            )?;
        }
        Ok((agent_run_id, events))
    }

    pub(crate) fn apply_agent_terminal_outcome(
        &mut self,
        agent_run_id: &AgentRunId,
        terminal_id: &TerminalId,
        outcome: &TerminationOutcome,
    ) -> Result<(), ProjectAgentRuntimeLaunchError> {
        self.ensure_terminal_exists(terminal_id)?;
        self.ensure_agent_run_exists(agent_run_id)?;
        self.ensure_agent_run_attached_to_terminal(agent_run_id, terminal_id)?;

        match outcome {
            TerminationOutcome::Exited { exit_status } => {
                self.mark_terminal_exited(terminal_id, Some(*exit_status))?;
                if *exit_status == 0 {
                    self.transition_agent_run_status(agent_run_id, AgentRunStatus::Completed)?;
                } else {
                    self.transition_agent_run_status(agent_run_id, AgentRunStatus::Failed)?;
                }
            }
            TerminationOutcome::TerminatedBySignal { .. }
            | TerminationOutcome::KilledAfterTimeout { .. } => {
                self.mark_terminal_exited(terminal_id, None)?;
                self.transition_agent_run_status(agent_run_id, AgentRunStatus::Cancelled)?;
            }
            TerminationOutcome::OrphanedUnknown { .. } => {
                self.transition_terminal_status(terminal_id, TerminalStatus::OrphanedUnknown)?;
                self.transition_agent_run_status(agent_run_id, AgentRunStatus::Detached)?;
            }
            TerminationOutcome::Failed { .. } => {
                self.transition_terminal_status(terminal_id, TerminalStatus::Failed)?;
                self.transition_agent_run_status(agent_run_id, AgentRunStatus::Failed)?;
            }
        }

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

    pub fn record_terminal_transcript_write_summary(
        &mut self,
        terminal_id: &TerminalId,
        summary: TranscriptWriteSummary,
    ) -> Result<(), ProjectTranscriptError> {
        let transcript_id = self
            .terminal_sessions
            .iter()
            .find(|terminal| terminal.id == *terminal_id)
            .ok_or(ProjectTranscriptError::Ownership(
                OwnershipError::MissingReference,
            ))?
            .transcript_ref
            .clone()
            .ok_or(ProjectTranscriptError::MissingTerminalTranscript)?;

        self.record_transcript_write_summary(&transcript_id, summary)
    }

    pub fn record_transcript_write_summary(
        &mut self,
        transcript_id: &TranscriptId,
        summary: TranscriptWriteSummary,
    ) -> Result<(), ProjectTranscriptError> {
        let transcript = self.transcript_mut(transcript_id)?;
        match summary.retention_state {
            TranscriptRetentionState::Active => transcript.record_active_write(summary.byte_count),
            TranscriptRetentionState::Truncated { .. } => {
                transcript.record_truncated_write(summary.byte_count)
            }
            TranscriptRetentionState::Expired => {
                transcript.record_lifecycle_state(TranscriptLifecycleState::Expired)
            }
            TranscriptRetentionState::DisabledByOptOut => {
                transcript.record_lifecycle_state(TranscriptLifecycleState::DisabledByOptOut)
            }
            TranscriptRetentionState::CaptureFailed => {
                transcript.record_lifecycle_state(TranscriptLifecycleState::CaptureFailed)
            }
            TranscriptRetentionState::Purged => transcript.mark_purged(),
        }
        self.record_activity();
        Ok(())
    }

    pub fn purge_transcript(
        &mut self,
        transcript_id: &TranscriptId,
    ) -> Result<ProjectTranscriptPurgeSummary, ProjectTranscriptError> {
        let index = self
            .transcripts
            .iter()
            .position(|transcript| transcript.id == *transcript_id)
            .ok_or(ProjectTranscriptError::MissingTranscript)?;
        self.purge_transcript_at(index)
    }

    pub fn purge_agent_run_transcripts(
        &mut self,
        agent_run_id: &AgentRunId,
    ) -> Result<ProjectTranscriptPurgeSummary, ProjectTranscriptError> {
        self.ensure_agent_run_exists(agent_run_id)
            .map_err(ProjectTranscriptError::Ownership)?;
        let transcript_ids = self
            .transcripts
            .iter()
            .filter(|transcript| transcript.agent_run_id.as_ref() == Some(agent_run_id))
            .map(|transcript| transcript.id.clone())
            .collect::<Vec<_>>();

        self.purge_transcripts_by_id(transcript_ids)
    }

    pub fn purge_project_transcripts(
        &mut self,
    ) -> Result<ProjectTranscriptPurgeSummary, ProjectTranscriptError> {
        let transcript_ids = self
            .transcripts
            .iter()
            .map(|transcript| transcript.id.clone())
            .collect::<Vec<_>>();

        self.purge_transcripts_by_id(transcript_ids)
    }

    fn attach_agent_run_transcript(
        &mut self,
        agent_run_id: AgentRunId,
        terminal_id: TerminalId,
        storage_path: PathBuf,
        retention_policy: impl Into<String>,
    ) -> Result<(), OwnershipError> {
        self.ensure_terminal_exists(&terminal_id)?;
        self.ensure_agent_run_exists(&agent_run_id)?;
        self.ensure_agent_run_attached_to_terminal_for_ownership(&agent_run_id, &terminal_id)?;

        let transcript = Transcript::metadata(
            self.id.clone(),
            terminal_id.clone(),
            Some(agent_run_id.clone()),
            storage_path,
            retention_policy,
        );
        let transcript_id = transcript.id.clone();
        self.transcripts.push(transcript);

        let terminal = self.terminal_session_mut(&terminal_id)?;
        terminal.transcript_ref = Some(transcript_id.clone());
        let run = self.agent_run_mut(&agent_run_id)?;
        run.transcript_ref = Some(transcript_id);

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
        if let Some(agent_run_id) = &change_set.agent_run_id {
            let run = self.agent_run_mut(agent_run_id)?;
            run.add_change_set(&change_set)?;
        }
        self.change_sets.push(change_set);
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn transition_change_set_review_state(
        &mut self,
        change_set_id: &ChangeSetId,
        review_state: ReviewState,
    ) -> Result<(), ProjectChangeSetError> {
        let change_set = self
            .change_sets
            .iter_mut()
            .find(|change_set| change_set.id == *change_set_id)
            .ok_or(ProjectChangeSetError::Ownership(
                OwnershipError::MissingReference,
            ))?;
        change_set.transition_review_to(review_state)?;
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    pub fn add_detected_generated_change_set(
        &mut self,
        baseline: &ReviewBaseline,
        detected: &DetectedChanges,
        candidate_agent_run_id: Option<&AgentRunId>,
        summary: impl Into<String>,
    ) -> Result<Option<ChangeSetId>, ProjectChangeSetError> {
        self.ensure_project_member(&baseline.project_id)?;
        self.ensure_project_member(&detected.project_id)?;
        if detected.baseline_snapshot_ref.as_ref() != Some(&baseline.baseline_snapshot_ref) {
            return Err(ProjectChangeSetError::BaselineMismatch);
        }
        if baseline.status != ChangeDetectionStatus::Complete {
            return Err(ProjectChangeSetError::DetectionNotComplete(baseline.status));
        }
        if detected.status != ChangeDetectionStatus::Complete {
            return Err(ProjectChangeSetError::DetectionNotComplete(detected.status));
        }

        let detector = GeneratedChangeDetector::default();
        let changed_files = detected
            .changed_paths
            .iter()
            .map(|changed_path| detector.validate_changed_path(self, &changed_path.relative_path))
            .collect::<Result<Vec<_>, _>>()?;
        if changed_files.is_empty() {
            return Ok(None);
        }

        let association = candidate_agent_run_id
            .map(|agent_run_id| self.detected_change_association(baseline, agent_run_id))
            .transpose()?;

        let change_set = match association {
            Some(DetectedChangeAssociation::Strong(agent_run_id)) => ChangeSet::agent_run_detected(
                self.id.clone(),
                agent_run_id,
                baseline.baseline_snapshot_ref.clone(),
                changed_files,
                summary,
            )
            .with_detection(detected.source, detected.status),
            Some(DetectedChangeAssociation::Ambiguous) => {
                ChangeSet::unreviewed(self.id.clone(), None, changed_files, summary)
                    .with_detection(detected.source, detected.status)
                    .with_association_confidence(ChangeAssociationConfidence::Ambiguous)
                    .with_baseline_snapshot_ref(baseline.baseline_snapshot_ref.clone())
            }
            None => ChangeSet::unreviewed(self.id.clone(), None, changed_files, summary)
                .with_detection(detected.source, detected.status)
                .with_baseline_snapshot_ref(baseline.baseline_snapshot_ref.clone()),
        };
        let change_set_id = change_set.id.clone();

        self.add_change_set(change_set)?;
        Ok(Some(change_set_id))
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

    pub fn toggle_mode(&mut self) {
        self.set_mode(self.mode.toggled());
    }

    pub fn set_resource_limits(&mut self, resource_limits: ProjectResourceLimits) {
        self.resource_limits = resource_limits;
        self.record_activity();
    }

    pub fn open_text_document(
        &mut self,
        selected_relative_path: impl AsRef<std::path::Path>,
    ) -> Result<(), ProjectContentError> {
        let root = ProjectRootHandle::from_project_session(self);
        let result = self.content_workspace.open_text_document(
            &root,
            selected_relative_path,
            TextDocumentOpenPolicy::linux_mvp(),
        );
        self.sync_file_state_from_content_workspace();
        self.set_open_surface(ProjectOpenSurface::TextEditor);
        self.set_mode(ProjectMode::Content);
        result
    }

    pub fn scan_content_explorer_directory(
        &mut self,
        selected_relative_path: impl Into<PathBuf>,
    ) -> Result<(), ProjectContentError> {
        let root = ProjectRootHandle::from_project_session(self);
        let result = self.content_workspace.scan_explorer_directory(
            &root,
            selected_relative_path,
            &FileExplorerScanPolicy::linux_mvp(),
        );
        self.set_open_surface(ProjectOpenSurface::TextEditor);
        self.set_mode(ProjectMode::Content);
        result
    }

    pub fn replace_active_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<(), ProjectContentError> {
        let result = self.content_workspace.replace_active_text(text);
        self.sync_file_state_from_content_workspace();
        self.record_activity();
        result
    }

    pub fn save_active_text_document(&mut self) -> Result<SaveDecision, ProjectContentError> {
        let root = ProjectRootHandle::from_project_session(self);
        let result = self
            .content_workspace
            .save_active_document(&root, TextDocumentOpenPolicy::linux_mvp());
        self.sync_file_state_from_content_workspace();
        self.record_activity();
        result
    }

    pub fn refresh_active_text_document(
        &mut self,
    ) -> Result<ExternalChangeDecision, ProjectContentError> {
        let root = ProjectRootHandle::from_project_session(self);
        let result = self
            .content_workspace
            .refresh_active_document(&root, TextDocumentOpenPolicy::linux_mvp());
        self.sync_file_state_from_content_workspace();
        self.record_activity();
        result
    }

    pub fn assess_agent_launch_active_file_safety(
        &mut self,
    ) -> Result<ProjectActiveFileLaunchAssessment, ProjectContentError> {
        if self.content_workspace.active_document().is_some() {
            let root = ProjectRootHandle::from_project_session(self);
            let result = self
                .content_workspace
                .refresh_active_document(&root, TextDocumentOpenPolicy::linux_mvp());
            self.sync_file_state_from_content_workspace();
            self.record_activity();
            result?;
        }

        Ok(self.content_workspace.active_file_launch_assessment())
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

    fn sync_file_state_from_content_workspace(&mut self) {
        self.set_file_state(ProjectFileState {
            provider_state: ProjectProviderState::Complete,
            open_buffer_count: self.content_workspace.open_buffer_count(),
            dirty_file_count: self.content_workspace.dirty_file_count(),
            active_path_hint: self.content_workspace.active_path_hint(),
        });
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

    fn terminal_session_mut(
        &mut self,
        terminal_id: &TerminalId,
    ) -> Result<&mut TerminalSession, OwnershipError> {
        self.terminal_sessions
            .iter_mut()
            .find(|terminal| terminal.id == *terminal_id)
            .ok_or(OwnershipError::MissingReference)
    }

    fn transcript_mut(
        &mut self,
        transcript_id: &TranscriptId,
    ) -> Result<&mut Transcript, ProjectTranscriptError> {
        self.transcripts
            .iter_mut()
            .find(|transcript| transcript.id == *transcript_id)
            .ok_or(ProjectTranscriptError::MissingTranscript)
    }

    fn purge_transcripts_by_id(
        &mut self,
        transcript_ids: Vec<TranscriptId>,
    ) -> Result<ProjectTranscriptPurgeSummary, ProjectTranscriptError> {
        let mut summary = ProjectTranscriptPurgeSummary::default();
        for transcript_id in transcript_ids {
            summary.merge(self.purge_transcript(&transcript_id)?);
        }
        Ok(summary)
    }

    fn purge_transcript_at(
        &mut self,
        index: usize,
    ) -> Result<ProjectTranscriptPurgeSummary, ProjectTranscriptError> {
        let transcript_id = self.transcripts[index].id.clone();
        if self.transcripts[index].is_tombstone() {
            return Ok(ProjectTranscriptPurgeSummary {
                requested_transcripts: 1,
                purged_transcripts: 0,
                bytes_removed: 0,
                tombstones_preserved: 1,
            });
        }

        let storage_path = self.transcripts[index].storage_path.clone();
        if transcript_path_is_project_local(&storage_path, &self.canonical_root_path) {
            return Err(ProjectTranscriptError::UnsafeProjectPath {
                transcript_id,
                path: storage_path,
            });
        }

        let bytes_removed = remove_transcript_file(&transcript_id, &storage_path)?;
        self.transcripts[index].mark_purged();
        self.record_activity();

        Ok(ProjectTranscriptPurgeSummary {
            requested_transcripts: 1,
            purged_transcripts: 1,
            bytes_removed,
            tombstones_preserved: 1,
        })
    }

    fn ensure_agent_run_exists(&self, agent_run_id: &AgentRunId) -> Result<(), OwnershipError> {
        self.agent_runs
            .iter()
            .any(|run| run.id == *agent_run_id)
            .then_some(())
            .ok_or(OwnershipError::MissingReference)
    }

    fn agent_run(&self, agent_run_id: &AgentRunId) -> Result<&AgentRun, OwnershipError> {
        self.agent_runs
            .iter()
            .find(|run| run.id == *agent_run_id)
            .ok_or(OwnershipError::MissingReference)
    }

    fn agent_run_mut(
        &mut self,
        agent_run_id: &AgentRunId,
    ) -> Result<&mut AgentRun, OwnershipError> {
        self.agent_runs
            .iter_mut()
            .find(|run| run.id == *agent_run_id)
            .ok_or(OwnershipError::MissingReference)
    }

    fn ensure_agent_run_attached_to_terminal(
        &self,
        agent_run_id: &AgentRunId,
        terminal_id: &TerminalId,
    ) -> Result<(), ProjectAgentRuntimeLaunchError> {
        let run = self
            .agent_runs
            .iter()
            .find(|run| run.id == *agent_run_id)
            .ok_or(OwnershipError::MissingReference)?;

        if run.terminal_id.as_ref() == Some(terminal_id) {
            Ok(())
        } else {
            Err(ProjectAgentRuntimeLaunchError::AgentTerminalMismatch)
        }
    }

    fn ensure_agent_run_attached_to_terminal_for_ownership(
        &self,
        agent_run_id: &AgentRunId,
        terminal_id: &TerminalId,
    ) -> Result<(), OwnershipError> {
        let run = self
            .agent_runs
            .iter()
            .find(|run| run.id == *agent_run_id)
            .ok_or(OwnershipError::MissingReference)?;

        if run.terminal_id.as_ref() == Some(terminal_id) {
            Ok(())
        } else {
            Err(OwnershipError::WrongAgentRun)
        }
    }

    fn transition_agent_run_status(
        &mut self,
        agent_run_id: &AgentRunId,
        status: AgentRunStatus,
    ) -> Result<(), ProjectAgentRuntimeLaunchError> {
        let run = self.agent_run_mut(agent_run_id)?;
        run.transition_to(status)?;
        self.record_activity();
        self.refresh_runtime_summary_from_collections();
        Ok(())
    }

    fn validate_agent_launch_plan_before_runtime(
        &self,
        plan: &AgentRunLaunchPlan,
    ) -> Result<(), ProjectAgentLaunchError> {
        self.ensure_project_member(plan.spec().project_id())?;
        self.ensure_project_member(&plan.terminal_launch_spec().project_id)?;
        self.ensure_project_member(&plan.agent_run().project_id)?;
        if self
            .agent_runs
            .iter()
            .any(|existing| existing.id == plan.agent_run().id)
        {
            return Err(ProjectAgentLaunchError::Ownership(
                OwnershipError::DuplicateAttachment,
            ));
        }
        if plan.terminal_launch_spec().kind
            != terminal_kind_from_compatibility(plan.agent_run().compatibility_level)
        {
            return Err(ProjectAgentLaunchError::TerminalDoesNotMatchLaunchSpec);
        }
        Ok(())
    }

    fn ensure_agent_launch_active_file_safety(
        &mut self,
    ) -> Result<ProjectActiveFileLaunchAssessment, ProjectAgentActiveFileLaunchError> {
        let assessment = self
            .assess_agent_launch_active_file_safety()
            .map_err(ProjectAgentActiveFileLaunchError::Refresh)?;
        if assessment.allows_launch() {
            Ok(assessment)
        } else {
            Err(ProjectAgentActiveFileLaunchError::Blocked(assessment))
        }
    }

    fn detected_change_association(
        &self,
        baseline: &ReviewBaseline,
        agent_run_id: &AgentRunId,
    ) -> Result<DetectedChangeAssociation, ProjectChangeSetError> {
        let run = self.agent_run(agent_run_id)?;
        if baseline.agent_run_id.as_ref() != Some(agent_run_id) {
            return Ok(DetectedChangeAssociation::Ambiguous);
        }
        if !agent_run_status_can_own_strong_changes(run.status) {
            return Ok(DetectedChangeAssociation::Ambiguous);
        }
        if self.agent_runs.iter().any(|other_run| {
            other_run.id != *agent_run_id
                && (agent_run_status_blocks_strong_association(other_run.status)
                    || other_run_temporally_overlaps_baseline(other_run, baseline))
        }) {
            return Ok(DetectedChangeAssociation::Ambiguous);
        }

        Ok(DetectedChangeAssociation::Strong(agent_run_id.clone()))
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
                .filter(|terminal| terminal_status_is_active(terminal.status()))
                .count()
                + self
                    .agent_runs
                    .iter()
                    .filter(|run| agent_run_status_is_active(run.status))
                    .filter(|run| {
                        !self.agent_run_has_terminal_with_status(run, terminal_status_is_active)
                    })
                    .count(),
        );
        let failed_processes = len_as_u32(
            self.terminal_sessions
                .iter()
                .filter(|terminal| terminal.status() == TerminalStatus::Failed)
                .count()
                + self
                    .agent_runs
                    .iter()
                    .filter(|run| run.status == AgentRunStatus::Failed)
                    .filter(|run| {
                        !self.agent_run_has_terminal_with_status(run, |status| {
                            status == TerminalStatus::Failed
                        })
                    })
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

    fn agent_run_has_terminal_with_status(
        &self,
        run: &AgentRun,
        matches_status: impl Fn(TerminalStatus) -> bool,
    ) -> bool {
        run.terminal_id.as_ref().is_some_and(|terminal_id| {
            self.terminal_sessions
                .iter()
                .any(|terminal| terminal.id == *terminal_id && matches_status(terminal.status()))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectTerminalError {
    Ownership(OwnershipError),
    InvalidTransition(TerminalTransitionError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectChangeSetError {
    Ownership(OwnershipError),
    InvalidReviewTransition(crate::domain::ReviewStateTransitionError),
    BaselineMismatch,
    DetectionNotComplete(ChangeDetectionStatus),
    InvalidChangedPath(ChangedPathValidationError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectAgentLaunchError {
    Ownership(OwnershipError),
    InvalidAgentRunTransition(AgentRunTransitionError),
    TerminalDoesNotMatchLaunchSpec,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectAgentRuntimeLaunchError {
    Launch(ProjectAgentLaunchError),
    TerminalLaunch(TerminalLaunchError),
    Terminal(ProjectTerminalError),
    ActiveFile(ProjectAgentActiveFileLaunchError),
    TranscriptCapture(AgentRunTranscriptCaptureError),
    InvalidAgentRunTransition(AgentRunTransitionError),
    AgentTerminalMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectAgentActiveFileLaunchError {
    Refresh(ProjectContentError),
    Blocked(ProjectActiveFileLaunchAssessment),
}

impl From<ProjectAgentLaunchError> for ProjectAgentRuntimeLaunchError {
    fn from(error: ProjectAgentLaunchError) -> Self {
        Self::Launch(error)
    }
}

impl From<TerminalLaunchError> for ProjectAgentRuntimeLaunchError {
    fn from(error: TerminalLaunchError) -> Self {
        Self::TerminalLaunch(error)
    }
}

impl From<ProjectTerminalError> for ProjectAgentRuntimeLaunchError {
    fn from(error: ProjectTerminalError) -> Self {
        Self::Terminal(error)
    }
}

impl From<ProjectAgentActiveFileLaunchError> for ProjectAgentRuntimeLaunchError {
    fn from(error: ProjectAgentActiveFileLaunchError) -> Self {
        Self::ActiveFile(error)
    }
}

impl From<AgentRunTranscriptCaptureError> for ProjectAgentRuntimeLaunchError {
    fn from(error: AgentRunTranscriptCaptureError) -> Self {
        Self::TranscriptCapture(error)
    }
}

impl From<OwnershipError> for ProjectAgentRuntimeLaunchError {
    fn from(error: OwnershipError) -> Self {
        Self::Launch(ProjectAgentLaunchError::Ownership(error))
    }
}

impl From<AgentRunTransitionError> for ProjectAgentRuntimeLaunchError {
    fn from(error: AgentRunTransitionError) -> Self {
        Self::InvalidAgentRunTransition(error)
    }
}

impl From<OwnershipError> for ProjectAgentLaunchError {
    fn from(error: OwnershipError) -> Self {
        Self::Ownership(error)
    }
}

impl From<AgentRunTransitionError> for ProjectAgentLaunchError {
    fn from(error: AgentRunTransitionError) -> Self {
        Self::InvalidAgentRunTransition(error)
    }
}

impl From<OwnershipError> for ProjectTerminalError {
    fn from(error: OwnershipError) -> Self {
        Self::Ownership(error)
    }
}

impl From<TerminalTransitionError> for ProjectTerminalError {
    fn from(error: TerminalTransitionError) -> Self {
        Self::InvalidTransition(error)
    }
}

impl From<OwnershipError> for ProjectChangeSetError {
    fn from(error: OwnershipError) -> Self {
        Self::Ownership(error)
    }
}

impl From<crate::domain::ReviewStateTransitionError> for ProjectChangeSetError {
    fn from(error: crate::domain::ReviewStateTransitionError) -> Self {
        Self::InvalidReviewTransition(error)
    }
}

impl From<ChangedPathValidationError> for ProjectChangeSetError {
    fn from(error: ChangedPathValidationError) -> Self {
        Self::InvalidChangedPath(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DetectedChangeAssociation {
    Strong(AgentRunId),
    Ambiguous,
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

fn agent_run_status_can_own_strong_changes(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Completed
            | AgentRunStatus::Failed
            | AgentRunStatus::Cancelled
            | AgentRunStatus::ReviewReady
    )
}

fn agent_run_status_blocks_strong_association(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Preparing
            | AgentRunStatus::Running
            | AgentRunStatus::AwaitingApproval
            | AgentRunStatus::ReviewReady
            | AgentRunStatus::Detached
    )
}

fn other_run_temporally_overlaps_baseline(run: &AgentRun, baseline: &ReviewBaseline) -> bool {
    if !matches!(
        run.status,
        AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Cancelled
    ) {
        return false;
    }

    run.ended_at
        .as_ref()
        .is_none_or(|ended_at| ended_at.as_str() >= baseline.captured_at.as_str())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProjectTranscriptPurgeSummary {
    pub requested_transcripts: u64,
    pub purged_transcripts: u64,
    pub bytes_removed: u64,
    pub tombstones_preserved: u64,
}

impl ProjectTranscriptPurgeSummary {
    fn merge(&mut self, other: Self) {
        self.requested_transcripts += other.requested_transcripts;
        self.purged_transcripts += other.purged_transcripts;
        self.bytes_removed += other.bytes_removed;
        self.tombstones_preserved += other.tombstones_preserved;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectTranscriptError {
    Ownership(OwnershipError),
    MissingTranscript,
    MissingTerminalTranscript,
    UnsafeProjectPath {
        transcript_id: TranscriptId,
        path: PathBuf,
    },
    StoragePathIsDirectory {
        transcript_id: TranscriptId,
        path: PathBuf,
    },
    DeleteFailed {
        transcript_id: TranscriptId,
        path: PathBuf,
    },
}

impl From<OwnershipError> for ProjectTranscriptError {
    fn from(error: OwnershipError) -> Self {
        Self::Ownership(error)
    }
}

fn remove_transcript_file(
    transcript_id: &TranscriptId,
    storage_path: &Path,
) -> Result<u64, ProjectTranscriptError> {
    match fs::symlink_metadata(storage_path) {
        Ok(metadata) if metadata.is_dir() => Err(ProjectTranscriptError::StoragePathIsDirectory {
            transcript_id: transcript_id.clone(),
            path: storage_path.to_path_buf(),
        }),
        Ok(metadata) => {
            let byte_count = metadata.len();
            fs::remove_file(storage_path).map_err(|_| ProjectTranscriptError::DeleteFailed {
                transcript_id: transcript_id.clone(),
                path: storage_path.to_path_buf(),
            })?;
            Ok(byte_count)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(_) => Err(ProjectTranscriptError::DeleteFailed {
            transcript_id: transcript_id.clone(),
            path: storage_path.to_path_buf(),
        }),
    }
}

fn transcript_path_is_project_local(storage_path: &Path, canonical_root_path: &Path) -> bool {
    if storage_path.as_os_str().is_empty() || storage_path.is_relative() {
        return true;
    }
    if storage_path.starts_with(canonical_root_path) {
        return true;
    }
    let Ok(canonical_storage_path) = fs::canonicalize(storage_path) else {
        return false;
    };
    let Ok(canonical_project_root) = fs::canonicalize(canonical_root_path) else {
        return canonical_storage_path.starts_with(canonical_root_path);
    };
    canonical_storage_path.starts_with(canonical_project_root)
}

fn agent_run_status_is_active(status: AgentRunStatus) -> bool {
    matches!(
        status,
        AgentRunStatus::Preparing | AgentRunStatus::Running | AgentRunStatus::AwaitingApproval
    )
}

fn terminal_matches_launch_spec(terminal: &TerminalSession, plan: &AgentRunLaunchPlan) -> bool {
    let spec = plan.terminal_launch_spec();

    terminal.project_id == spec.project_id
        && terminal.kind == spec.kind
        && terminal.title == spec.title
        && terminal.cwd == spec.cwd
        && terminal.command_line_summary == spec.command_line_summary
}

fn terminal_environment_policy_ref(policy: &TerminalEnvironmentPolicy) -> Option<String> {
    match policy {
        TerminalEnvironmentPolicy::Minimal => None,
        TerminalEnvironmentPolicy::Named(name) => Some(name.clone()),
        TerminalEnvironmentPolicy::ExplicitAllowlist(names) => {
            Some(format!("explicit allowlist: {}", names.join(", ")))
        }
    }
}

fn terminal_kind_from_compatibility(level: AgentCompatibilityLevel) -> TerminalKind {
    match level {
        AgentCompatibilityLevel::Plain => TerminalKind::Plain,
        AgentCompatibilityLevel::Supervised => TerminalKind::Supervised,
        AgentCompatibilityLevel::Managed => TerminalKind::Managed,
    }
}

fn len_as_u32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
