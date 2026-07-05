use super::ApplicationShell;
use crate::command::AppCommand;
use crate::route::AppRoute;

#[test]
fn shell_starts_on_project_board_route() {
    let shell = ApplicationShell::new();

    assert_eq!(shell.route(), AppRoute::ProjectBoard);
}

#[test]
fn command_router_changes_top_level_route() {
    let mut shell = ApplicationShell::new();

    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);
    assert_eq!(shell.route(), AppRoute::ActiveProjectWorkspace);

    shell.dispatch(AppCommand::OpenProjectBoard);
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
