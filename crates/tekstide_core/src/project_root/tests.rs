use super::{ProjectRootValidationError, ProjectRootValidator, SymlinkPolicy, ValidProjectRoot};
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

fn validate(path: &Path, symlink_policy: SymlinkPolicy) -> ValidProjectRoot {
    ProjectRootValidator
        .validate(path, symlink_policy)
        .expect("project root should validate")
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
        let path = self.path(name);
        fs::write(&path, b"not a project directory").unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
