use crate::close::{CloseAssessment, assess_close};
use crate::project::recent::{
    RECENT_PROJECT_STATE_VERSION, RecentProject, RecentProjectAvailability, RecentProjectState,
    RestoredRecentProject, Timestamp, assess_recent_project_availability,
};
use crate::project::root::{
    ProjectRootValidationError, ProjectRootValidator, SymlinkPolicy, ValidProjectRoot,
};
use crate::project::{ProjectId, ProjectMode, ProjectOpenSurface, ProjectSession};

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
                    project.trust_state().label(),
                ),
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

    #[cfg(test)]
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
        let project = ProjectSession::new(
            project_id.clone(),
            display_name,
            root_path,
            canonical_root_path,
        );

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
            project.trust_state().label(),
        );

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
