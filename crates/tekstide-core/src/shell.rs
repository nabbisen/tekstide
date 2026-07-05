use crate::app::{AddProjectOutcome, AppState, RemoveProjectError};
use crate::close::CloseAssessment;
use crate::command::AppCommand;
use crate::content::{ExternalChangeDecision, SaveDecision};
use crate::navigation::{TerminalLayoutClass, TerminalPanePolicy};
use crate::project::recent::RecentProjectState;
use crate::project::root::ProjectRootValidationError;
use crate::project::{ProjectContentError, ProjectId, text_document_state_label};
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
                if self.state.active_project().is_some() {
                    self.route = AppRoute::ActiveProjectWorkspace;
                }
            }
            AppCommand::ToggleActiveProjectMode => {
                if self.state.toggle_active_project_mode() {
                    self.route = AppRoute::ActiveProjectWorkspace;
                }
            }
            AppCommand::OpenActiveProjectSurface(surface) => {
                if self.state.open_active_project_surface(surface) {
                    self.route = AppRoute::ActiveProjectWorkspace;
                }
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

    pub fn open_active_project_text_document(
        &mut self,
        selected_relative_path: impl AsRef<std::path::Path>,
    ) -> Result<(), ProjectContentError> {
        let result = self
            .state
            .open_active_project_text_document(selected_relative_path);
        if self.state.active_project().is_some() {
            self.route = AppRoute::ActiveProjectWorkspace;
        }
        result
    }

    pub fn replace_active_project_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<(), ProjectContentError> {
        let result = self.state.replace_active_project_text(text);
        if self.state.active_project().is_some() {
            self.route = AppRoute::ActiveProjectWorkspace;
        }
        result
    }

    pub fn save_active_project_text_document(
        &mut self,
    ) -> Result<SaveDecision, ProjectContentError> {
        let result = self.state.save_active_project_text_document();
        if self.state.active_project().is_some() {
            self.route = AppRoute::ActiveProjectWorkspace;
        }
        result
    }

    pub fn refresh_active_project_text_document(
        &mut self,
    ) -> Result<ExternalChangeDecision, ProjectContentError> {
        let result = self.state.refresh_active_project_text_document();
        if self.state.active_project().is_some() {
            self.route = AppRoute::ActiveProjectWorkspace;
        }
        result
    }

    pub fn project_board(&self) -> ProjectBoardViewModel {
        ProjectBoardViewModel::from_app_state(&self.state)
    }

    pub fn render_text(&self) -> String {
        match self.route {
            AppRoute::ProjectBoard => render_project_board(&self.project_board()),
            AppRoute::ActiveProjectWorkspace => render_active_project_workspace(&self.state),
        }
    }
}

fn render_active_project_workspace(state: &AppState) -> String {
    let Some(project) = state.active_project() else {
        return "Active Project Workspace\n\nNo project surface is open yet.\n".to_owned();
    };

    let mut output = String::from("Active Project Workspace\n\n");
    output.push_str(project.display_name());
    output.push_str(" | ");
    output.push_str(project.mode().label());
    output.push_str(" | surface: ");
    output.push_str(project.open_surface().label());
    output.push_str(" | visible terminal panes: ");
    let pane_policy = TerminalPanePolicy::for_layout(TerminalLayoutClass::Wide);
    let requested_panes = requested_visible_panes(project.resource_limits().visible_terminal_limit);
    output.push_str(&pane_policy.visible_pane_count(requested_panes).to_string());
    output.push('\n');

    let content_workspace = project.content_workspace();
    output.push_str("content status: ");
    output.push_str(content_workspace.status().label());
    output.push_str(" | selected: ");
    let selected = content_workspace.selected_explorer_path();
    if selected.as_os_str().is_empty() {
        output.push_str("(project root)");
    } else {
        output.push_str(&selected.display().to_string());
    }
    if let Some(document) = content_workspace.active_document() {
        output.push_str(" | active file: ");
        output.push_str(
            &document
                .target()
                .selected_relative_path
                .display()
                .to_string(),
        );
        output.push_str(" | document: ");
        output.push_str(text_document_state_label(document.state()));
        output.push_str(" | dirty files: ");
        output.push_str(&project.runtime_summary().dirty_files.to_string());
    } else {
        output.push_str(" | active file: none | dirty files: ");
        output.push_str(&project.runtime_summary().dirty_files.to_string());
    }
    if let Some(message) = content_workspace.status().message() {
        output.push_str(" | message: ");
        output.push_str(message);
    }
    output.push('\n');
    output.push_str("content panes: 1\n");
    output
}

fn requested_visible_panes(limit: Option<u32>) -> u8 {
    limit
        .unwrap_or(u32::from(
            TerminalPanePolicy::for_layout(TerminalLayoutClass::Wide).max_visible_panes,
        ))
        .min(u32::from(u8::MAX)) as u8
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
        output.push_str(" | security: ");
        output.push_str(&row.security_mode_label);
        output.push_str(" | blocked automation: ");
        output.push_str(&row.blocked_automation_count.to_string());
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
