use super::ApplicationShell;
use crate::command::AppCommand;
use crate::project::{ProjectMode, ProjectOpenSurface, ProjectResourceLimits};
use crate::route::AppRoute;

#[test]
fn shell_starts_on_project_board_route() {
    let shell = ApplicationShell::new();

    assert_eq!(shell.route(), AppRoute::ProjectBoard);
}

#[test]
fn command_router_changes_top_level_route() {
    let mut shell = ApplicationShell::new();
    shell
        .state_mut()
        .add_project_session("Tekstide", "/workspace/tekstide", "/workspace/tekstide");

    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);
    assert_eq!(shell.route(), AppRoute::ActiveProjectWorkspace);

    shell.dispatch(AppCommand::OpenProjectBoard);
    assert_eq!(shell.route(), AppRoute::ProjectBoard);
}

#[test]
fn direct_workspace_open_without_active_project_stays_on_project_board() {
    let mut shell = ApplicationShell::new();

    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);

    assert_eq!(shell.route(), AppRoute::ProjectBoard);
    assert!(shell.render_text().contains("Project Board"));
}

#[test]
fn active_project_workspace_toggles_between_content_and_terminal_modes() {
    let mut shell = ApplicationShell::new();
    let project_id = shell.state_mut().add_project_session(
        "Tekstide",
        "/workspace/tekstide",
        "/workspace/tekstide",
    );

    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);
    assert_eq!(shell.route(), AppRoute::ActiveProjectWorkspace);
    assert_eq!(
        shell.state().project(&project_id).unwrap().mode(),
        ProjectMode::Content
    );

    shell.dispatch(AppCommand::ToggleActiveProjectMode);
    assert_eq!(
        shell.state().project(&project_id).unwrap().mode(),
        ProjectMode::TerminalImmersion
    );
    assert!(
        shell
            .render_text()
            .contains("Terminal / Agent Immersion Mode")
    );
    assert!(shell.render_text().contains("visible terminal panes: 2"));

    shell.dispatch(AppCommand::ToggleActiveProjectMode);
    assert_eq!(
        shell.state().project(&project_id).unwrap().mode(),
        ProjectMode::Content
    );
}

#[test]
fn active_workspace_visible_panes_are_capped_by_navigation_policy() {
    let mut shell = ApplicationShell::new();
    let project_id = shell.state_mut().add_project_session(
        "Tekstide",
        "/workspace/tekstide",
        "/workspace/tekstide",
    );
    shell
        .state_mut()
        .project_mut(&project_id)
        .expect("project should exist")
        .set_resource_limits(ProjectResourceLimits {
            visible_terminal_limit: Some(8),
            terminal_session_limit: None,
            agent_run_limit: None,
            approval_request_limit: None,
        });

    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);

    assert!(shell.render_text().contains("visible terminal panes: 2"));
}

#[test]
fn opening_content_surface_returns_to_content_mode_without_losing_active_project() {
    let mut shell = ApplicationShell::new();
    let project_id = shell.state_mut().add_project_session(
        "Tekstide",
        "/workspace/tekstide",
        "/workspace/tekstide",
    );

    shell.dispatch(AppCommand::ToggleActiveProjectMode);
    shell.dispatch(AppCommand::OpenActiveProjectSurface(
        ProjectOpenSurface::DiffReview,
    ));

    let project = shell.state().project(&project_id).unwrap();
    assert_eq!(shell.route(), AppRoute::ActiveProjectWorkspace);
    assert_eq!(project.mode(), ProjectMode::Content);
    assert_eq!(project.open_surface(), ProjectOpenSurface::DiffReview);
    assert!(shell.render_text().contains("surface: Diff Review"));
}

#[test]
fn workspace_commands_without_active_project_do_not_leave_project_board() {
    let mut shell = ApplicationShell::new();

    shell.dispatch(AppCommand::ToggleActiveProjectMode);
    assert_eq!(shell.route(), AppRoute::ProjectBoard);

    shell.dispatch(AppCommand::OpenActiveProjectSurface(
        ProjectOpenSurface::AgentRunDetail,
    ));
    assert_eq!(shell.route(), AppRoute::ProjectBoard);
}

#[test]
fn first_run_project_board_renders_empty_state() {
    let shell = ApplicationShell::new();

    let rendered = shell.render_text();

    assert!(rendered.contains("Tekstide"));
    assert!(rendered.contains("Project Board"));
    assert!(rendered.contains("No projects yet."));
    assert!(rendered.contains("[Add Project] [Open from path]"));
}

#[test]
fn populated_project_board_renders_placeholder_branch_status_without_process_probe() {
    let mut shell = ApplicationShell::new();
    shell
        .state_mut()
        .add_project_session("Tekstide", "/workspace/tekstide", "/workspace/tekstide");

    let rendered = shell.render_text();

    assert!(rendered.contains("branch/status: not available"));
    assert!(rendered.contains("trust: Restricted"));
    assert!(rendered.contains("security: Restricted Mode"));
    assert!(rendered.contains("blocked automation: 9"));
}

#[test]
fn shell_add_project_uses_shared_validation_and_returns_to_project_board() {
    let sandbox = TestSandbox::new("shell-add");
    let project_dir = sandbox.create_dir("project");
    let mut shell = ApplicationShell::new();
    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);

    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    assert_eq!(shell.route(), AppRoute::ProjectBoard);
    assert!(shell.render_text().contains("project"));
    assert!(shell.render_text().contains("trust: Restricted"));
}

struct TestSandbox {
    root: std::path::PathBuf,
}

impl TestSandbox {
    fn new(name: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tekstide-shell-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn create_dir(&self, name: &str) -> std::path::PathBuf {
        let path = self.root.join(name);
        std::fs::create_dir(&path).unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
