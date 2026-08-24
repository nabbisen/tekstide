use crate::agent::AgentRunLaunchPlan;
use crate::close::{CloseAssessment, assess_close};
use crate::content::{ExternalChangeDecision, SaveDecision, TextCursor};
use crate::domain::{
    AgentRunId, ChangeSetId, OwnershipError, TerminalId, TerminalSession, VisibleSlot,
};
use crate::project::recent::{
    RECENT_PROJECT_STATE_VERSION, RecentProject, RecentProjectAvailability, RecentProjectState,
    RestoredRecentProject, Timestamp, assess_recent_project_availability,
};
use crate::project::root::{
    ProjectRootValidationError, ProjectRootValidator, SymlinkPolicy, ValidProjectRoot,
};
use crate::project::{
    DetectedChanges, ProjectAgentLaunchError, ProjectAgentRuntimeLaunchError,
    ProjectChangeSetError, ProjectContentError, ProjectId, ProjectMode, ProjectOpenSurface,
    ProjectSession, ProjectTerminalError, ReviewBaseline,
};
use crate::runtime::terminal::{LinuxTerminalRuntime, TerminalRuntimeEvent, TerminationOutcome};

#[derive(Debug, Default)]
pub struct AppState {
    projects: Vec<ProjectSession>,
    recent_projects: Vec<RestoredRecentProject>,
    active_project_id: Option<ProjectId>,
}

impl AppState {
    pub fn projects(&self) -> &[ProjectSession] {
        &self.projects
    }

    /// RFC-033 PR-033-C: the `app_retained_bytes` input
    /// `TranscriptLocalDataSummary` needs. Sums each open
    /// `ProjectSession`'s own [`ProjectSession::real_retained_transcript_bytes`]
    /// -- real bytes on disk, not the `byte_count` field that method's
    /// own doc comment explains is stale for every real run today. Sums
    /// only across currently open `ProjectSession`s -- there is no
    /// aggregate byte count kept independent of an open session, so a
    /// project closed since its last transcript write is not counted
    /// here. This is the best real data available, not a full-disk
    /// scan; stated here rather than silently claiming completeness,
    /// the same honesty `what-purge-must-remove.md` requires of the
    /// purge confirmation itself.
    pub fn app_wide_retained_transcript_bytes(&self) -> u64 {
        self.projects
            .iter()
            .map(ProjectSession::real_retained_transcript_bytes)
            .sum()
    }

    pub fn active_project_id(&self) -> Option<&ProjectId> {
        self.active_project_id.as_ref()
    }

    pub fn recent_projects(&self) -> &[RestoredRecentProject] {
        &self.recent_projects
    }

    pub fn restore_recent_projects(&mut self, state: RecentProjectState) {
        self.recent_projects = state
            .projects
            .into_iter()
            .map(|recent_project| {
                let availability = assess_recent_project_availability(&recent_project);
                RestoredRecentProject {
                    recent_project,
                    availability,
                }
            })
            .collect();
    }

    pub fn recent_project_state(&self) -> RecentProjectState {
        let mut projects = self
            .recent_projects
            .iter()
            .map(|restored| restored.recent_project.clone())
            .collect::<Vec<_>>();

        for project in &self.projects {
            upsert_recent_project(
                &mut projects,
                RecentProject::with_timestamps(
                    project.id().clone(),
                    project.display_name(),
                    project.root_path().clone(),
                    project.canonical_root_path().clone(),
                    Timestamp::from_domain(project.last_opened_at()),
                    Timestamp::from_domain(project.last_activity_at()),
                    project.trust_state(),
                )
                .with_transcript_capture_declined(project.transcript_capture_declined()),
            );
        }

        RecentProjectState {
            state_version: RECENT_PROJECT_STATE_VERSION,
            projects,
        }
    }

    pub fn active_project(&self) -> Option<&ProjectSession> {
        let active_id = self.active_project_id.as_ref()?;
        self.project(active_id)
    }

    fn active_project_mut(&mut self) -> Option<&mut ProjectSession> {
        let active_id = self.active_project_id.clone()?;
        self.projects
            .iter_mut()
            .find(|project| project.id() == &active_id)
    }

    pub fn project(&self, project_id: &ProjectId) -> Option<&ProjectSession> {
        self.projects
            .iter()
            .find(|project| project.id() == project_id)
    }

    /// RFC-022 PR-022-E ("the arrival model"): de-gated from
    /// `#[cfg(test)]` -- a real, non-test caller now exists
    /// (`decide_approval`, `crates/tekstide/src/shell.rs`), which needs
    /// to update a specific project's own `ProjectSession` by id rather
    /// than only the currently active one (`active_project_mut`,
    /// private): an approval decision names the project the proposal
    /// belongs to, not necessarily whichever project happens to be on
    /// screen when the decision is made.
    pub fn project_mut(&mut self, project_id: &ProjectId) -> Option<&mut ProjectSession> {
        self.projects
            .iter_mut()
            .find(|project| project.id() == project_id)
    }

    pub(crate) fn add_project_session(
        &mut self,
        display_name: impl Into<String>,
        root_path: impl Into<std::path::PathBuf>,
        canonical_root_path: impl Into<std::path::PathBuf>,
    ) -> ProjectId {
        let root_path = root_path.into();
        let canonical_root_path = canonical_root_path.into();

        if let Some(existing_project_id) = self.project_id_by_canonical_root(&canonical_root_path) {
            self.active_project_id = Some(existing_project_id.clone());
            return existing_project_id;
        }

        let project_id = self
            .recent_project_id_by_canonical_root(&canonical_root_path)
            .unwrap_or_else(ProjectId::new_uuid);
        // RFC-032: restores a *previous* session's trust decision, bound
        // to the exact same canonical-path key `recent_project_id_by_canonical_root`
        // just used to reuse `project_id` -- if a symlink was redirected
        // since this entry was saved, the freshly-computed
        // `canonical_root_path` here no longer matches the stored one,
        // this lookup finds nothing, and the project below keeps
        // `ProjectSession::new`'s own `Restricted` default. Trust never
        // silently follows a path that no longer resolves to what was
        // trusted.
        let restored_trust = self.recent_trust_by_canonical_root(&canonical_root_path);
        let restored_capture_declined =
            self.recent_transcript_capture_declined_by_canonical_root(&canonical_root_path);
        let mut project = ProjectSession::new(
            project_id.clone(),
            display_name,
            root_path,
            canonical_root_path,
        );
        if let Some(trust_state) = restored_trust {
            project.restore_trust_state(trust_state);
        }
        if let Some(declined) = restored_capture_declined {
            project.restore_transcript_capture_declined(declined);
        }

        if self.active_project_id.is_none() {
            self.active_project_id = Some(project_id.clone());
        }

        self.projects.push(project);
        self.upsert_open_project_recent(project_id.clone());
        project_id
    }

    pub fn add_project_from_path(
        &mut self,
        selected_path: impl AsRef<std::path::Path>,
    ) -> Result<AddProjectOutcome, ProjectRootValidationError> {
        self.add_project_from_path_with_symlink_policy(selected_path, SymlinkPolicy::FailClosed)
    }

    pub fn add_project_from_path_with_symlink_policy(
        &mut self,
        selected_path: impl AsRef<std::path::Path>,
        symlink_policy: SymlinkPolicy,
    ) -> Result<AddProjectOutcome, ProjectRootValidationError> {
        let root = ProjectRootValidator.validate(selected_path, symlink_policy)?;
        Ok(self.add_validated_project_root(root))
    }

    pub fn add_validated_project_root(&mut self, root: ValidProjectRoot) -> AddProjectOutcome {
        if let Some(existing_project_id) = self.project_id_by_canonical_root(&root.canonical_path) {
            self.active_project_id = Some(existing_project_id.clone());
            return AddProjectOutcome::FocusedExisting(existing_project_id);
        }

        let project_id =
            self.add_project_session(root.display_name, root.selected_path, root.canonical_path);
        AddProjectOutcome::Added(project_id)
    }

    pub fn switch_active_project(&mut self, project_id: &ProjectId) -> bool {
        let Some(project) = self
            .projects
            .iter_mut()
            .find(|project| project.id() == project_id)
        else {
            return false;
        };

        project.mark_opened();
        self.active_project_id = Some(project_id.clone());
        true
    }

    pub fn assess_project_close(&self, project_id: &ProjectId) -> CloseAssessment {
        let Some(project) = self.project(project_id) else {
            return CloseAssessment::UnsupportedOrUnknown {
                reason: "project is not active".to_owned(),
            };
        };

        assess_close(project.close_resource_summary())
    }

    pub fn toggle_active_project_mode(&mut self) -> bool {
        let Some(project) = self.active_project_mut() else {
            return false;
        };

        project.toggle_mode();
        true
    }

    pub fn open_active_project_surface(&mut self, surface: ProjectOpenSurface) -> bool {
        let Some(project) = self.active_project_mut() else {
            return false;
        };

        project.set_open_surface(surface);
        project.set_mode(ProjectMode::Content);
        true
    }

    /// Terminal launch UX handoff: sets the mode directly to
    /// `TerminalImmersion`, unlike `toggle_active_project_mode` --
    /// launching a terminal must land the user in the terminal
    /// workspace regardless of which mode they were already in, not
    /// flip them out of it if they happened to already be there.
    pub fn open_active_project_terminal_workspace(&mut self) -> bool {
        let Some(project) = self.active_project_mut() else {
            return false;
        };

        project.set_mode(ProjectMode::TerminalImmersion);
        true
    }

    /// RFC-017 PR-017-E: the missing lifecycle glue identified while
    /// implementing PR-017-D -- a caller outside `tekstide-core` had no
    /// way to attach a real, already-launched `TerminalSession` to the
    /// active project (`ProjectSession::add_terminal_session` existed
    /// with no external caller). This is that caller's entry point,
    /// delegating straight to it; the ownership/duplicate-id checks stay
    /// exactly where they already were.
    pub fn attach_terminal_session(
        &mut self,
        terminal: TerminalSession,
    ) -> Result<(), ProjectTerminalError> {
        let project = self
            .active_project_mut()
            .ok_or(ProjectTerminalError::Ownership(
                OwnershipError::MissingProject,
            ))?;
        project.add_terminal_session(terminal)
    }

    /// RFC-022 PR-022-D: the same "missing lifecycle glue" gap
    /// `attach_terminal_session` closed for a plain terminal --
    /// `ProjectSession::launch_agent_run_with_runtime` existed with no
    /// caller outside `tekstide-core` itself until this slice. No active
    /// project maps onto the same `OwnershipError::MissingProject` shape
    /// `attach_terminal_session` uses, wrapped in `ProjectAgentLaunchError::Ownership`
    /// so it fits `ProjectAgentRuntimeLaunchError`'s existing `Launch`
    /// variant rather than adding a second "missing project" case.
    pub fn launch_agent_run_with_runtime(
        &mut self,
        plan: AgentRunLaunchPlan,
        runtime: &mut LinuxTerminalRuntime,
    ) -> Result<
        (
            AgentRunId,
            Vec<TerminalRuntimeEvent>,
            Option<crate::approval::ApprovalChannelEndpoint>,
        ),
        ProjectAgentRuntimeLaunchError,
    > {
        let project = self
            .active_project_mut()
            .ok_or(ProjectAgentRuntimeLaunchError::Launch(
                ProjectAgentLaunchError::Ownership(OwnershipError::MissingProject),
            ))?;
        project.launch_agent_run_with_runtime(plan, runtime)
    }

    /// The other half of the same gap: assigning which slot (`Primary`,
    /// `Secondary`, or `Hidden`) a registered terminal occupies, against
    /// the active project.
    pub fn assign_terminal_visible_slot(
        &mut self,
        terminal_id: &TerminalId,
        visible_slot: VisibleSlot,
    ) -> Result<(), ProjectTerminalError> {
        let project = self
            .active_project_mut()
            .ok_or(ProjectTerminalError::Ownership(
                OwnershipError::MissingProject,
            ))?;
        project.assign_terminal_visible_slot(terminal_id, visible_slot)
    }

    /// Terminal launch UX handoff: the same "missing lifecycle glue"
    /// gap `attach_terminal_session`/`assign_terminal_visible_slot`
    /// closed for registration -- `ProjectSession::transition_terminal_status`
    /// existed with no caller outside `tekstide-core` itself.
    pub fn transition_terminal_status(
        &mut self,
        terminal_id: &TerminalId,
        status: crate::domain::TerminalStatus,
    ) -> Result<(), ProjectTerminalError> {
        let project = self
            .active_project_mut()
            .ok_or(ProjectTerminalError::Ownership(
                OwnershipError::MissingProject,
            ))?;
        project.transition_terminal_status(terminal_id, status)
    }

    /// The other half of exit detection: `mark_terminal_exited` also
    /// records the real exit code, which a generic `transition_terminal_status`
    /// call cannot (`TerminalSession::exit_status` has no setter of its
    /// own).
    pub fn mark_terminal_exited(
        &mut self,
        terminal_id: &TerminalId,
        exit_status: Option<i32>,
    ) -> Result<(), ProjectTerminalError> {
        let project = self
            .active_project_mut()
            .ok_or(ProjectTerminalError::Ownership(
                OwnershipError::MissingProject,
            ))?;
        project.mark_terminal_exited(terminal_id, exit_status)
    }

    /// change-detection-wiring handoff, Slice C (D3): the same
    /// "missing lifecycle glue" gap `mark_terminal_exited` closed for
    /// plain terminals -- `ProjectSession::apply_agent_terminal_outcome`
    /// existed, `pub(crate)`, with no caller outside `tekstide-core`
    /// itself. Marks the terminal exited and transitions the owning
    /// `AgentRun`'s status together, so the two facts cannot land out of
    /// step with each other.
    pub fn apply_agent_terminal_outcome(
        &mut self,
        agent_run_id: &AgentRunId,
        terminal_id: &TerminalId,
        outcome: &TerminationOutcome,
    ) -> Result<(), ProjectAgentRuntimeLaunchError> {
        let project = self
            .active_project_mut()
            .ok_or(ProjectAgentRuntimeLaunchError::Launch(
                ProjectAgentLaunchError::Ownership(OwnershipError::MissingProject),
            ))?;
        project.apply_agent_terminal_outcome(agent_run_id, terminal_id, outcome)
    }

    /// RFC-039 PR-039-C: the same call as [`Self::apply_agent_terminal_outcome`]
    /// above, `project_id`-addressed instead of implicit. Closing a
    /// project must work from any tab, not only the active one
    /// (`what-closing-a-project-must-not-lose.md` §6's confirmed
    /// sequence has no "switch to it first" step) -- but retiring a live
    /// agent run's own status is the one piece of that sequence with no
    /// existing project-scoped path: `mark_terminal_exited`/
    /// `transition_terminal_status` are reachable for any project via
    /// `project_mut` already, since `ProjectSession`'s own versions of
    /// those two are `pub`, but `ProjectSession::apply_agent_terminal_outcome`
    /// is `pub(crate)`, so only this crate can call it, and until now
    /// only through the active-project-only wrapper above.
    pub fn apply_agent_terminal_outcome_for_project(
        &mut self,
        project_id: &ProjectId,
        agent_run_id: &AgentRunId,
        terminal_id: &TerminalId,
        outcome: &TerminationOutcome,
    ) -> Result<(), ProjectAgentRuntimeLaunchError> {
        let project =
            self.project_mut(project_id)
                .ok_or(ProjectAgentRuntimeLaunchError::Launch(
                    ProjectAgentLaunchError::Ownership(OwnershipError::MissingProject),
                ))?;
        project.apply_agent_terminal_outcome(agent_run_id, terminal_id, outcome)
    }

    /// change-detection-wiring handoff, Slice C: the same gap as
    /// `apply_agent_terminal_outcome` immediately above --
    /// `ProjectSession::add_detected_generated_change_set` existed,
    /// fully tested, with no production caller anywhere in this crate.
    pub fn add_detected_generated_change_set(
        &mut self,
        baseline: &ReviewBaseline,
        detected: &DetectedChanges,
        candidate_agent_run_id: Option<&AgentRunId>,
        summary: impl Into<String>,
    ) -> Result<Option<ChangeSetId>, ProjectChangeSetError> {
        let project = self
            .active_project_mut()
            .ok_or(ProjectChangeSetError::Ownership(
                OwnershipError::MissingProject,
            ))?;
        project.add_detected_generated_change_set(
            baseline,
            detected,
            candidate_agent_run_id,
            summary,
        )
    }

    pub fn open_active_project_text_document(
        &mut self,
        selected_relative_path: impl AsRef<std::path::Path>,
    ) -> Result<(), ProjectContentError> {
        let Some(project) = self.active_project_mut() else {
            return Err(ProjectContentError::NoActiveProject);
        };

        project.open_text_document(selected_relative_path)
    }

    pub fn scan_active_project_explorer_directory(
        &mut self,
        selected_relative_path: impl Into<std::path::PathBuf>,
    ) -> Result<(), ProjectContentError> {
        let Some(project) = self.active_project_mut() else {
            return Err(ProjectContentError::NoActiveProject);
        };

        project.scan_content_explorer_directory(selected_relative_path)
    }

    /// RFC-038 PR-038-F: see [`ProjectSession::scan_content_explorer_directory_without_navigating`]'s
    /// own doc -- the scan-only counterpart, threaded through unchanged.
    pub fn scan_active_project_explorer_directory_without_navigating(
        &mut self,
        selected_relative_path: impl Into<std::path::PathBuf>,
    ) -> Result<(), ProjectContentError> {
        let Some(project) = self.active_project_mut() else {
            return Err(ProjectContentError::NoActiveProject);
        };

        project.scan_content_explorer_directory_without_navigating(selected_relative_path)
    }

    pub fn replace_active_project_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<(), ProjectContentError> {
        let Some(project) = self.active_project_mut() else {
            return Err(ProjectContentError::NoActiveProject);
        };

        project.replace_active_text(text)
    }

    /// RFC-006 Amendment 1.
    pub fn set_active_project_cursor(
        &mut self,
        cursor: TextCursor,
    ) -> Result<(), ProjectContentError> {
        let Some(project) = self.active_project_mut() else {
            return Err(ProjectContentError::NoActiveProject);
        };

        project.set_active_cursor(cursor)
    }

    pub fn save_active_project_text_document(
        &mut self,
    ) -> Result<SaveDecision, ProjectContentError> {
        let Some(project) = self.active_project_mut() else {
            return Err(ProjectContentError::NoActiveProject);
        };

        project.save_active_text_document()
    }

    pub fn refresh_active_project_text_document(
        &mut self,
    ) -> Result<ExternalChangeDecision, ProjectContentError> {
        let Some(project) = self.active_project_mut() else {
            return Err(ProjectContentError::NoActiveProject);
        };

        project.refresh_active_text_document()
    }

    pub fn close_project(
        &mut self,
        project_id: &ProjectId,
    ) -> Result<CloseAssessment, RemoveProjectError> {
        let assessment = self.assess_project_close(project_id);
        if !assessment.is_safe_to_close() {
            return Ok(assessment);
        }

        self.remove_active_project_session(project_id)
            .then_some(CloseAssessment::SafeToClose)
            .ok_or(RemoveProjectError::ProjectNotFound)
    }

    pub fn remove_recent_project(
        &mut self,
        project_id: &ProjectId,
    ) -> Result<(), RemoveProjectError> {
        if self.project(project_id).is_some() {
            return Err(RemoveProjectError::ProjectIsActive);
        }

        let before_len = self.recent_projects.len();
        self.recent_projects
            .retain(|restored| &restored.recent_project.project_id != project_id);

        if self.recent_projects.len() == before_len {
            Err(RemoveProjectError::ProjectNotFound)
        } else {
            Ok(())
        }
    }

    fn remove_active_project_session(&mut self, project_id: &ProjectId) -> bool {
        let Some(position) = self
            .projects
            .iter()
            .position(|project| project.id() == project_id)
        else {
            return false;
        };

        self.projects.remove(position);
        if self.active_project_id.as_ref() == Some(project_id) {
            self.active_project_id = self.projects.first().map(|project| project.id().clone());
        }

        true
    }

    fn project_id_by_canonical_root(
        &self,
        canonical_root_path: &std::path::Path,
    ) -> Option<ProjectId> {
        self.projects
            .iter()
            .find(|project| project.canonical_root_path() == canonical_root_path)
            .map(ProjectSession::id)
            .cloned()
    }

    fn recent_project_id_by_canonical_root(
        &self,
        canonical_root_path: &std::path::Path,
    ) -> Option<ProjectId> {
        self.recent_projects
            .iter()
            .find(|restored| restored.recent_project.canonical_root_path == canonical_root_path)
            .map(|restored| restored.recent_project.project_id.clone())
    }

    /// RFC-032: the trust-restoration analogue of
    /// `recent_project_id_by_canonical_root` -- same key, same match,
    /// deliberately not folded into one lookup returning both, so each
    /// caller's intent (reuse an id vs. restore a trust decision) stays
    /// separately named and separately testable.
    fn recent_trust_by_canonical_root(
        &self,
        canonical_root_path: &std::path::Path,
    ) -> Option<crate::project::WorkspaceTrust> {
        self.recent_projects
            .iter()
            .find(|restored| restored.recent_project.canonical_root_path == canonical_root_path)
            .map(|restored| restored.recent_project.trust_state)
    }

    /// RFC-033 PR-033-B: the transcript-capture-opt-out analogue of
    /// `recent_trust_by_canonical_root` -- same key, same match,
    /// deliberately separate rather than returning both from one lookup,
    /// for the same reason that function's own doc comment states.
    fn recent_transcript_capture_declined_by_canonical_root(
        &self,
        canonical_root_path: &std::path::Path,
    ) -> Option<bool> {
        self.recent_projects
            .iter()
            .find(|restored| restored.recent_project.canonical_root_path == canonical_root_path)
            .map(|restored| restored.recent_project.transcript_capture_declined)
    }

    fn upsert_open_project_recent(&mut self, project_id: ProjectId) {
        let Some(project) = self.project(&project_id) else {
            return;
        };
        let recent_project = RecentProject::with_timestamps(
            project.id().clone(),
            project.display_name(),
            project.root_path().clone(),
            project.canonical_root_path().clone(),
            Timestamp::from_domain(project.last_opened_at()),
            Timestamp::from_domain(project.last_activity_at()),
            project.trust_state(),
        )
        .with_transcript_capture_declined(project.transcript_capture_declined());

        if let Some(restored) = self.recent_projects.iter_mut().find(|restored| {
            restored.recent_project.canonical_root_path == recent_project.canonical_root_path
        }) {
            restored.recent_project = recent_project;
            restored.availability = RecentProjectAvailability::Available;
            return;
        }

        self.recent_projects.push(RestoredRecentProject {
            recent_project,
            availability: RecentProjectAvailability::Available,
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoveProjectError {
    ProjectNotFound,
    ProjectIsActive,
}

fn upsert_recent_project(projects: &mut Vec<RecentProject>, recent_project: RecentProject) {
    if let Some(existing_project) = projects
        .iter_mut()
        .find(|project| project.canonical_root_path == recent_project.canonical_root_path)
    {
        *existing_project = recent_project;
        return;
    }

    projects.push(recent_project);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddProjectOutcome {
    Added(ProjectId),
    FocusedExisting(ProjectId),
}

impl AddProjectOutcome {
    pub fn project_id(&self) -> &ProjectId {
        match self {
            Self::Added(project_id) | Self::FocusedExisting(project_id) => project_id,
        }
    }
}

#[cfg(test)]
mod tests;
