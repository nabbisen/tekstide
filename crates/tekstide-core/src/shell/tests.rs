use super::ApplicationShell;
use crate::command::AppCommand;
use crate::project::{ProjectMode, ProjectOpenSurface, ProjectResourceLimits};
use crate::route::AppRoute;
use std::fs;

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

#[test]
fn text_document_workflow_is_visible_in_active_content_workspace() {
    let sandbox = TestSandbox::new("shell-content-workflow");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/src/lib.rs", b"original\n");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    shell
        .open_active_project_text_document("src/lib.rs")
        .expect("text document should open");
    let rendered = shell.render_text();

    assert_eq!(shell.route(), AppRoute::ActiveProjectWorkspace);
    assert!(rendered.contains("surface: Text Editor"));
    assert!(rendered.contains("content status: open"));
    assert!(rendered.contains("active file: src/lib.rs"));
    assert!(rendered.contains("document: clean"));
    assert!(rendered.contains("content panes: 1"));

    shell
        .replace_active_project_text("changed\n")
        .expect("active document should edit");
    let rendered = shell.render_text();
    assert!(rendered.contains("document: dirty"));
    assert!(rendered.contains("dirty files: 1"));

    shell
        .save_active_project_text_document()
        .expect("active document should save");
    let rendered = shell.render_text();
    assert!(rendered.contains("content status: saved"));
    assert!(rendered.contains("document: clean"));
    assert!(rendered.contains("dirty files: 0"));
    assert_eq!(
        fs::read_to_string(project_dir.join("src/lib.rs")).unwrap(),
        "changed\n"
    );
}

#[test]
fn content_workspace_renders_bounded_explorer_scan_without_file_contents() {
    let sandbox = TestSandbox::new("shell-content-explorer");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/src/lib.rs", b"pub fn secret() {}\n");
    sandbox.create_dir("project/target");
    sandbox.create_file_with_contents("project/README.md", b"# private readme\n");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    shell
        .scan_active_project_explorer_directory("")
        .expect("project root should scan");
    let rendered = shell.render_text();

    assert!(rendered.contains("explorer: ready"));
    assert!(rendered.contains("explorer directory: (project root)"));
    assert!(rendered.contains("README.md [file | available"));
    assert!(rendered.contains("src [directory | available"));
    assert!(rendered.contains("target [directory | collapsed"));
    assert!(rendered.contains("symlink: none"));
    assert!(rendered.contains("content panes: 1"));
    assert!(!rendered.contains("pub fn secret"));
    assert!(!rendered.contains("private readme"));
}

#[test]
fn content_explorer_scan_error_is_visible_without_replacing_active_document() {
    let sandbox = TestSandbox::new("shell-content-explorer-error");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");
    shell
        .open_active_project_text_document("file.txt")
        .expect("text document should open");

    shell
        .scan_active_project_explorer_directory("file.txt")
        .expect_err("file target is not a directory");
    let rendered = shell.render_text();

    assert!(rendered.contains("explorer: error"));
    assert!(rendered.contains("active file: file.txt"));
    assert!(rendered.contains("document: clean"));
    assert!(rendered.contains("explorer message:"));
}

#[test]
fn failed_explorer_scan_clears_previous_scan_result() {
    let sandbox = TestSandbox::new("shell-content-explorer-stale");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    sandbox.create_file_with_contents("project/README.md", b"# readme\n");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    shell
        .scan_active_project_explorer_directory("")
        .expect("project root should scan");
    assert!(shell.render_text().contains("README.md [file | available"));

    shell
        .scan_active_project_explorer_directory("file.txt")
        .expect_err("file target is not a directory");
    let rendered = shell.render_text();

    assert!(rendered.contains("explorer: error"));
    assert!(rendered.contains("explorer message:"));
    assert!(!rendered.contains("README.md [file | available"));
    assert!(!rendered.contains("explorer directory: (project root)"));
}

#[test]
fn explorer_scan_from_terminal_mode_forces_content_mode() {
    let sandbox = TestSandbox::new("shell-content-explorer-forces-mode");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let mut shell = ApplicationShell::new();
    let project_id = shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added")
        .project_id()
        .clone();

    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);
    shell.dispatch(AppCommand::ToggleActiveProjectMode);
    assert_eq!(
        shell.state().project(&project_id).unwrap().mode(),
        ProjectMode::TerminalImmersion
    );

    shell
        .scan_active_project_explorer_directory("")
        .expect("project root should scan");

    let project = shell.state().project(&project_id).unwrap();
    assert_eq!(shell.route(), AppRoute::ActiveProjectWorkspace);
    assert_eq!(project.mode(), ProjectMode::Content);
    assert_eq!(project.open_surface(), ProjectOpenSurface::TextEditor);
    assert!(shell.render_text().contains("explorer: ready"));
}

#[test]
fn opening_text_document_from_terminal_mode_forces_content_mode() {
    let sandbox = TestSandbox::new("shell-content-forces-mode");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let mut shell = ApplicationShell::new();
    let project_id = shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added")
        .project_id()
        .clone();

    shell.dispatch(AppCommand::OpenActiveProjectWorkspace);
    shell.dispatch(AppCommand::ToggleActiveProjectMode);
    assert_eq!(
        shell.state().project(&project_id).unwrap().mode(),
        ProjectMode::TerminalImmersion
    );

    shell
        .open_active_project_text_document("file.txt")
        .expect("text document should open");

    let project = shell.state().project(&project_id).unwrap();
    assert_eq!(shell.route(), AppRoute::ActiveProjectWorkspace);
    assert_eq!(project.mode(), ProjectMode::Content);
    assert_eq!(project.open_surface(), ProjectOpenSurface::TextEditor);
    assert!(shell.render_text().contains("Content Mode"));
    assert!(
        !shell
            .render_text()
            .contains("Terminal / Agent Immersion Mode | surface: Text Editor")
    );
}

#[test]
fn unsupported_text_open_error_is_rendered_without_losing_workspace() {
    let sandbox = TestSandbox::new("shell-content-open-error");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/binary.dat", b"text\0more");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    let error = shell
        .open_active_project_text_document("binary.dat")
        .expect_err("binary-looking file should be rejected");
    let rendered = shell.render_text();

    assert!(error.to_string().contains("binary.dat"));
    assert!(rendered.contains("content status: open error"));
    assert!(rendered.contains("active file: none"));
    assert!(rendered.contains("message: file appears to be binary: binary.dat"));
}

#[test]
fn failed_open_preserves_existing_dirty_active_document_without_ambiguous_selection() {
    let sandbox = TestSandbox::new("shell-content-failed-open-preserves");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    sandbox.create_file_with_contents("project/binary.dat", b"text\0more");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");
    shell
        .open_active_project_text_document("file.txt")
        .expect("text document should open");
    shell
        .replace_active_project_text("changed\n")
        .expect("text edit should succeed");

    shell
        .open_active_project_text_document("binary.dat")
        .expect_err("binary-looking file should be rejected");
    let rendered = shell.render_text();

    assert!(rendered.contains("content status: open error"));
    assert!(rendered.contains("selected: file.txt"));
    assert!(rendered.contains("active file: file.txt"));
    assert!(rendered.contains("document: dirty"));
    assert!(rendered.contains("dirty files: 1"));
    assert!(rendered.contains("message: file appears to be binary: binary.dat"));
}

#[test]
fn identical_replacement_does_not_report_edited_clean_document() {
    let sandbox = TestSandbox::new("shell-content-identical-edit");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"same\n");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");
    shell
        .open_active_project_text_document("file.txt")
        .expect("text document should open");

    shell
        .replace_active_project_text("same\n")
        .expect("identical replacement should succeed");
    let rendered = shell.render_text();

    assert!(rendered.contains("content status: open"));
    assert!(rendered.contains("document: clean"));
    assert!(!rendered.contains("content status: edited | document: clean"));
}

#[test]
fn external_dirty_conflict_is_visible_without_overwriting_disk() {
    let sandbox = TestSandbox::new("shell-content-conflict");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let mut shell = ApplicationShell::new();
    shell
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");
    shell
        .open_active_project_text_document("file.txt")
        .expect("text document should open");
    shell
        .replace_active_project_text("tekstide edit\n")
        .expect("active document should edit");
    fs::write(project_dir.join("file.txt"), b"external edit\n").unwrap();

    shell
        .save_active_project_text_document()
        .expect_err("external change should block save");
    let rendered = shell.render_text();

    assert!(rendered.contains("content status: conflict"));
    assert!(rendered.contains("document: conflict"));
    assert!(rendered.contains("dirty files: 1"));
    assert_eq!(
        fs::read_to_string(project_dir.join("file.txt")).unwrap(),
        "external edit\n"
    );
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
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn create_file_with_contents(&self, name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
