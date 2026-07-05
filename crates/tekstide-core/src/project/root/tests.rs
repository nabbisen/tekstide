use super::{
    FileAccessBlockedReason, FileAccessContainmentStatus, FileAccessSymlinkStatus,
    ProjectFileAccessPolicy, ProjectRootHandle, ProjectRootValidationError, ProjectRootValidator,
    SymlinkPolicy, ValidProjectRoot,
};
use crate::project::{ProjectId, ProjectSession};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn validates_existing_directory() {
    let sandbox = TestSandbox::new("valid-directory");
    let project_dir = sandbox.create_dir("project");

    let root = validate(&project_dir, SymlinkPolicy::FailClosed);

    assert_eq!(root.display_name, "project");
    assert_eq!(root.selected_path, project_dir);
    assert_eq!(
        root.canonical_path,
        fs::canonicalize(sandbox.path("project")).unwrap()
    );
}

#[test]
fn rejects_missing_path() {
    let sandbox = TestSandbox::new("missing-path");
    let missing_path = sandbox.path("missing");

    let error = ProjectRootValidator
        .validate(&missing_path, SymlinkPolicy::FailClosed)
        .expect_err("missing path should be rejected");

    assert_eq!(
        error,
        ProjectRootValidationError::DoesNotExist { path: missing_path }
    );
}

#[test]
fn rejects_non_directory_path() {
    let sandbox = TestSandbox::new("non-directory");
    let file_path = sandbox.create_file("file.txt");

    let error = ProjectRootValidator
        .validate(&file_path, SymlinkPolicy::FailClosed)
        .expect_err("non-directory path should be rejected");

    assert_eq!(
        error,
        ProjectRootValidationError::NotDirectory { path: file_path }
    );
}

#[cfg(unix)]
#[test]
fn symlinked_root_fails_closed_by_default() {
    let sandbox = TestSandbox::new("symlink-fail-closed");
    let target_dir = sandbox.create_dir("target");
    let link_path = sandbox.path("link");
    std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();

    let error = ProjectRootValidator
        .validate(&link_path, SymlinkPolicy::FailClosed)
        .expect_err("symlink should require confirmation");

    assert_eq!(
        error,
        ProjectRootValidationError::SymlinkAmbiguous {
            selected_path: link_path,
            canonical_path: fs::canonicalize(target_dir).unwrap(),
        }
    );
}

#[cfg(unix)]
#[test]
fn symlinked_root_can_be_confirmed_to_canonical_target() {
    let sandbox = TestSandbox::new("symlink-confirmed");
    let target_dir = sandbox.create_dir("target");
    let link_path = sandbox.path("link");
    std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();

    let root = validate(&link_path, SymlinkPolicy::AllowCanonicalTarget);

    assert_eq!(root.selected_path, link_path);
    assert_eq!(root.canonical_path, fs::canonicalize(target_dir).unwrap());
}

#[test]
fn project_root_handle_binds_project_identity_to_root_context() {
    let sandbox = TestSandbox::new("root-handle-ownership");
    let project_dir = sandbox.create_dir("project");
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let expected_canonical_path = root.canonical_path.clone();
    let project_id = ProjectId::for_test(7);

    let root = root_handle(project_id.clone(), root);

    assert_eq!(root.project_id(), &project_id);
    assert_eq!(root.valid_root().canonical_path, expected_canonical_path);
}

#[test]
fn file_access_target_records_selected_and_canonical_paths_for_in_root_file() {
    let sandbox = TestSandbox::new("file-access-target");
    let project_dir = sandbox.create_dir("project");
    let file_path = sandbox.create_file_with_contents("project/src/lib.rs", b"pub fn lib() {}\n");
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let expected_root_canonical_path = root.canonical_path.clone();
    let project_id = ProjectId::for_test(1);
    let root = root_handle(project_id.clone(), root);

    let target = ProjectFileAccessPolicy
        .resolve_existing(&root, "src/lib.rs")
        .expect("in-root file should resolve");

    assert_eq!(target.project_id, project_id);
    assert_eq!(target.selected_relative_path, PathBuf::from("src/lib.rs"));
    assert_eq!(
        target.selected_absolute_path,
        project_dir.join("src/lib.rs")
    );
    assert_eq!(target.canonical_path, fs::canonicalize(file_path).unwrap());
    assert_eq!(target.root_canonical_path, expected_root_canonical_path);
    assert_eq!(target.symlink_status, FileAccessSymlinkStatus::NoSymlink);
    assert_eq!(
        target.containment_status,
        FileAccessContainmentStatus::InsideRoot
    );
}

#[test]
fn file_access_rejects_missing_relative_path_with_project_context() {
    let sandbox = TestSandbox::new("missing-file-access");
    let project_dir = sandbox.create_dir("project");
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let expected_root_canonical_path = root.canonical_path.clone();
    let project_id = ProjectId::for_test(1);
    let root = root_handle(project_id.clone(), root);

    let error = ProjectFileAccessPolicy
        .resolve_existing(&root, "src/missing.rs")
        .expect_err("missing file should be rejected");

    assert_eq!(error.project_id, project_id);
    assert_eq!(
        error.selected_relative_path,
        PathBuf::from("src/missing.rs")
    );
    assert_eq!(
        error.selected_absolute_path,
        project_dir.join("src/missing.rs")
    );
    assert_eq!(error.root_canonical_path, expected_root_canonical_path);
    assert_eq!(error.reason, FileAccessBlockedReason::MissingPath);
}

#[test]
fn file_access_rejects_absolute_selected_paths_before_resolution() {
    let sandbox = TestSandbox::new("absolute-file-access");
    let project_dir = sandbox.create_dir("project");
    let outside_file = sandbox.create_file_with_contents("outside.txt", b"outside\n");
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let project_id = ProjectId::for_test(1);
    let root = root_handle(project_id.clone(), root);

    let error = ProjectFileAccessPolicy
        .resolve_existing(&root, &outside_file)
        .expect_err("absolute paths should not be selected directly");

    assert_eq!(error.project_id, project_id);
    assert_eq!(error.selected_relative_path, outside_file);
    assert_eq!(
        error.reason,
        FileAccessBlockedReason::AbsolutePathNotAllowed
    );
}

#[test]
fn file_access_rejects_parent_traversal_before_resolution() {
    let sandbox = TestSandbox::new("parent-traversal");
    let project_dir = sandbox.create_dir("project");
    let _outside_file = sandbox.create_file_with_contents("outside.txt", b"outside\n");
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let project_id = ProjectId::for_test(1);
    let root = root_handle(project_id.clone(), root);

    let error = ProjectFileAccessPolicy
        .resolve_existing(&root, "../outside.txt")
        .expect_err("parent traversal should be rejected lexically");

    assert_eq!(error.project_id, project_id);
    assert_eq!(
        error.selected_relative_path,
        PathBuf::from("../outside.txt")
    );
    assert_eq!(
        error.selected_absolute_path,
        project_dir.join("../outside.txt")
    );
    assert_eq!(error.reason, FileAccessBlockedReason::InvalidRelativePath);
}

#[test]
fn file_access_rejects_in_root_parent_traversal_instead_of_normalizing_it() {
    let sandbox = TestSandbox::new("in-root-parent-traversal");
    let project_dir = sandbox.create_dir("project");
    let _file_path = sandbox.create_file_with_contents("project/lib.rs", b"root lib\n");
    let root = root_handle(
        ProjectId::for_test(1),
        validate(&project_dir, SymlinkPolicy::FailClosed),
    );

    let error = ProjectFileAccessPolicy
        .resolve_existing(&root, "src/../lib.rs")
        .expect_err("parent traversal is rejected even when canonical target is in root");

    assert_eq!(error.selected_relative_path, PathBuf::from("src/../lib.rs"));
    assert_eq!(error.reason, FileAccessBlockedReason::InvalidRelativePath);
}

#[test]
fn file_access_normalizes_current_dir_and_repeated_separators() {
    let sandbox = TestSandbox::new("relative-normalization");
    let project_dir = sandbox.create_dir("project");
    let file_path = sandbox.create_file_with_contents("project/src/lib.rs", b"root lib\n");
    let root = root_handle(
        ProjectId::for_test(1),
        validate(&project_dir, SymlinkPolicy::FailClosed),
    );

    let target = ProjectFileAccessPolicy
        .resolve_existing(&root, "./src//lib.rs")
        .expect("current-dir and repeated separators should normalize");

    assert_eq!(target.selected_relative_path, PathBuf::from("src/lib.rs"));
    assert_eq!(
        target.selected_absolute_path,
        project_dir.join("src/lib.rs")
    );
    assert_eq!(target.canonical_path, fs::canonicalize(file_path).unwrap());
}

#[test]
fn file_access_can_resolve_project_root_directory_for_explorer() {
    let sandbox = TestSandbox::new("root-directory-target");
    let project_dir = sandbox.create_dir("project");
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let expected_root_canonical_path = root.canonical_path.clone();
    let root = root_handle(ProjectId::for_test(1), root);

    let target = ProjectFileAccessPolicy
        .resolve_existing(&root, "")
        .expect("empty relative path should resolve the project root for explorer");

    assert_eq!(target.selected_relative_path, PathBuf::new());
    assert_eq!(target.selected_absolute_path, project_dir);
    assert_eq!(target.canonical_path, expected_root_canonical_path);
    assert_eq!(
        target.containment_status,
        FileAccessContainmentStatus::InsideRoot
    );
}

#[test]
fn file_access_blocks_cross_project_relative_traversal() {
    let sandbox = TestSandbox::new("cross-project-traversal");
    let project_one_dir = sandbox.create_dir("project-one");
    let project_two_dir = sandbox.create_dir("project-two");
    let _project_two_file = sandbox.create_file_with_contents("project-two/src/lib.rs", b"other\n");
    let project_one_root = validate(&project_one_dir, SymlinkPolicy::FailClosed);
    let _project_two_root = validate(&project_two_dir, SymlinkPolicy::FailClosed);
    let project_one_id = ProjectId::for_test(1);
    let project_one_root = root_handle(project_one_id.clone(), project_one_root);

    let error = ProjectFileAccessPolicy
        .resolve_existing(&project_one_root, "../project-two/src/lib.rs")
        .expect_err("one project root should not authorize another project file");

    assert_eq!(error.project_id, project_one_id);
    assert_eq!(error.reason, FileAccessBlockedReason::InvalidRelativePath);
}

#[cfg(unix)]
#[test]
fn file_access_allows_in_root_symlink_to_in_root_target() {
    let sandbox = TestSandbox::new("in-root-symlink");
    let project_dir = sandbox.create_dir("project");
    let target_file = sandbox.create_file_with_contents("project/src/lib.rs", b"target\n");
    let link_dir = sandbox.create_dir("project/links");
    std::os::unix::fs::symlink(&target_file, link_dir.join("lib-link.rs")).unwrap();
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let root = root_handle(ProjectId::for_test(1), root);

    let target = ProjectFileAccessPolicy
        .resolve_existing(&root, "links/lib-link.rs")
        .expect("in-root symlink should resolve");

    assert_eq!(
        target.symlink_status,
        FileAccessSymlinkStatus::InRootSymlink
    );
    assert_eq!(
        target.containment_status,
        FileAccessContainmentStatus::InsideRoot
    );
    assert_eq!(
        target.canonical_path,
        fs::canonicalize(target_file).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn file_access_blocks_symlink_escape() {
    let sandbox = TestSandbox::new("escaping-symlink");
    let project_dir = sandbox.create_dir("project");
    let outside_file = sandbox.create_file_with_contents("outside.txt", b"outside\n");
    std::os::unix::fs::symlink(&outside_file, project_dir.join("outside-link.txt")).unwrap();
    let root = validate(&project_dir, SymlinkPolicy::FailClosed);
    let project_id = ProjectId::for_test(1);
    let root = root_handle(project_id.clone(), root);

    let error = ProjectFileAccessPolicy
        .resolve_existing(&root, "outside-link.txt")
        .expect_err("escaping symlink should be blocked");

    assert_eq!(error.project_id, project_id);
    assert_eq!(error.reason, FileAccessBlockedReason::SymlinkEscape);
    assert_eq!(
        error.selected_absolute_path,
        project_dir.join("outside-link.txt")
    );
}

fn validate(path: &Path, symlink_policy: SymlinkPolicy) -> ValidProjectRoot {
    ProjectRootValidator
        .validate(path, symlink_policy)
        .expect("project root should validate")
}

fn root_handle(project_id: ProjectId, root: ValidProjectRoot) -> ProjectRootHandle {
    let project = ProjectSession::new(
        project_id,
        root.display_name,
        root.selected_path,
        root.canonical_path,
    );
    ProjectRootHandle::from_project_session(&project)
}

struct TestSandbox {
    root: PathBuf,
}

impl TestSandbox {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn create_dir(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir(&path).unwrap();
        path
    }

    fn create_file(&self, name: &str) -> PathBuf {
        self.create_file_with_contents(name, b"not a project directory")
    }

    fn create_file_with_contents(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
