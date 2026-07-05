use crate::app::{AddProjectOutcome, AppState, RemoveProjectError};
use crate::close::CloseAssessment;
use crate::command::AppCommand;
use crate::project::ProjectId;
use crate::project::recent::RecentProjectState;
use crate::project::root::ProjectRootValidationError;
use crate::project_board::ProjectBoardViewModel;
use crate::route::AppRoute;

#[derive(Debug, Default)]
pub struct ApplicationShell {
    state: AppState,
    route: AppRoute,
}

impl ApplicationShell {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route(&self) -> AppRoute {
        self.route
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    pub fn restore_recent_projects(&mut self, recent_project_state: RecentProjectState) {
        self.state.restore_recent_projects(recent_project_state);
    }

    pub fn recent_project_state(&self) -> RecentProjectState {
        self.state.recent_project_state()
    }

    pub fn dispatch(&mut self, command: AppCommand) {
        match command {
            AppCommand::OpenProjectBoard => {
                self.route = AppRoute::ProjectBoard;
            }
            AppCommand::OpenActiveProjectWorkspace => {
                self.route = AppRoute::ActiveProjectWorkspace;
            }
        }
    }

    pub fn add_project_from_path(
        &mut self,
        selected_path: impl AsRef<std::path::Path>,
    ) -> Result<AddProjectOutcome, ProjectRootValidationError> {
        let outcome = self.state.add_project_from_path(selected_path)?;
        self.route = AppRoute::ProjectBoard;
        Ok(outcome)
    }

    pub fn close_project(
        &mut self,
        project_id: &ProjectId,
    ) -> Result<CloseAssessment, RemoveProjectError> {
        let assessment = self.state.close_project(project_id)?;
        self.route = AppRoute::ProjectBoard;
        Ok(assessment)
    }

    pub fn remove_recent_project(
        &mut self,
        project_id: &ProjectId,
    ) -> Result<(), RemoveProjectError> {
        self.state.remove_recent_project(project_id)
    }

    pub fn project_board(&self) -> ProjectBoardViewModel {
        ProjectBoardViewModel::from_app_state(&self.state)
    }

    pub fn render_text(&self) -> String {
        match self.route {
            AppRoute::ProjectBoard => render_project_board(&self.project_board()),
            AppRoute::ActiveProjectWorkspace => {
                "Active Project Workspace\n\nNo project surface is open yet.\n".to_owned()
            }
        }
    }
}

fn render_project_board(view_model: &ProjectBoardViewModel) -> String {
    let mut output = String::from("Tekstide\nProject Board\n\n");

    if let Some(empty_state) = &view_model.empty_state {
        output.push_str(&empty_state.heading);
        output.push('\n');
        output.push('[');
        output.push_str(&empty_state.primary_action);
        output.push_str("] [");
        output.push_str(&empty_state.secondary_action);
        output.push_str("]\n");
        return output;
    }

    for row in &view_model.rows {
        output.push_str(&row.display_name);
        output.push_str(" | ");
        output.push_str(&row.root_path_hint);
        if let Some(secondary_path_hint) = &row.secondary_path_hint {
            output.push_str(" -> ");
            output.push_str(secondary_path_hint);
        }
        if let Some(availability_label) = &row.availability_label {
            output.push_str(" | ");
            output.push_str(availability_label);
        }
        output.push_str(" | trust: ");
        output.push_str(&row.trust_label);
        output.push_str(" | branch/status: ");
        output.push_str(&row.branch_status.label());
        output.push_str(" | terminals: ");
        output.push_str(&row.terminal_count.label());
        output.push_str(" | agents: ");
        output.push_str(&row.agent_run_count.label());
        output.push_str(" | approvals: ");
        output.push_str(&row.approval_count.label());
        output.push_str(" | reviews: ");
        output.push_str(&row.review_count.label());
        output.push_str(" | dirty: ");
        output.push_str(&row.dirty_file_count.label());
        output.push_str(" | attention: ");
        output.push_str(&row.attention_label);
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests;
