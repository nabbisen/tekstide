use super::{
    ExplorerNodeKind, ExplorerNodeState, ExplorerScanError, FileExplorerScanPolicy,
    FileExplorerScanner,
};
use crate::project::root::{
    FileAccessBlockedReason, FileAccessSymlinkStatus, ProjectRootHandle, ProjectRootValidator,
    SymlinkPolicy, ValidProjectRoot,
};
use crate::project::{ProjectId, ProjectSession};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::{ffi::OsString, os::unix::ffi::OsStringExt};

#[test]
fn scanner_reads_one_directory_as_sorted_read_model() {
    let sandbox = TestSandbox::new("explorer-basic");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file("project/src/lib.rs");
    sandbox.create_file("project/Cargo.toml");
    sandbox.create_dir("project/tests");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let scan = FileExplorerScanner
        .scan_directory(&root, "", &FileExplorerScanPolicy::linux_mvp())
        .expect("project root should scan");

    let names: Vec<_> = scan.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(names, ["Cargo.toml", "src", "tests"]);
    assert!(!scan.truncated);
    assert_eq!(scan.nodes[0].kind, ExplorerNodeKind::File);
    assert_eq!(scan.nodes[1].kind, ExplorerNodeKind::Directory);
}

#[test]
fn scanner_bounds_child_count_without_recursive_indexing() {
    let sandbox = TestSandbox::new("explorer-bound");
    let project_dir = sandbox.create_dir("project");
    for index in 0..5 {
        sandbox.create_file(&format!("project/file-{index}.txt"));
    }
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let policy = FileExplorerScanPolicy {
        max_children_per_directory: 3,
        collapsed_directory_names: Vec::new(),
    };

    let scan = FileExplorerScanner
        .scan_directory(&root, "", &policy)
        .expect("project root should scan");

    assert_eq!(scan.nodes.len(), 3);
    assert!(scan.truncated);
}

#[test]
fn scanner_zero_child_cap_returns_empty_truncated_nonempty_directory() {
    let sandbox = TestSandbox::new("explorer-zero-bound");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file("project/file.txt");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let policy = FileExplorerScanPolicy {
        max_children_per_directory: 0,
        collapsed_directory_names: Vec::new(),
    };

    let scan = FileExplorerScanner
        .scan_directory(&root, "", &policy)
        .expect("project root should scan");

    assert!(scan.nodes.is_empty());
    assert!(scan.truncated);
}

#[test]
fn scanner_uses_normalized_directory_target_for_child_paths() {
    let sandbox = TestSandbox::new("explorer-normalized-base");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file("project/src/lib.rs");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let scan = FileExplorerScanner
        .scan_directory(&root, "./src//", &FileExplorerScanPolicy::linux_mvp())
        .expect("normalized directory should scan");

    assert_eq!(
        node(&scan, "lib.rs").relative_path,
        PathBuf::from("src/lib.rs")
    );
}

#[test]
fn scanner_collapses_heavy_directories_by_name() {
    let sandbox = TestSandbox::new("explorer-collapsed");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file("project/target/debug/output");
    sandbox.create_file("project/node_modules/package/index.js");
    sandbox.create_file("project/.git/objects/pack");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let scan = FileExplorerScanner
        .scan_directory(&root, "", &FileExplorerScanPolicy::linux_mvp())
        .expect("project root should scan");

    assert_eq!(node_state(&scan, ".git"), ExplorerNodeState::Collapsed);
    assert_eq!(
        node_state(&scan, "node_modules"),
        ExplorerNodeState::Collapsed
    );
    assert_eq!(node_state(&scan, "target"), ExplorerNodeState::Collapsed);
}

#[test]
fn scanner_rejects_root_escape_for_requested_directory() {
    let sandbox = TestSandbox::new("explorer-root-escape");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_dir("outside");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let error = FileExplorerScanner
        .scan_directory(&root, "../outside", &FileExplorerScanPolicy::linux_mvp())
        .expect_err("root escape should not scan");

    assert!(matches!(
        error,
        ExplorerScanError::Access(error)
            if error.reason == FileAccessBlockedReason::InvalidRelativePath
    ));
}

#[test]
fn scanner_reports_file_target_as_not_directory() {
    let sandbox = TestSandbox::new("explorer-file-target");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file("project/Cargo.toml");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let error = FileExplorerScanner
        .scan_directory(&root, "Cargo.toml", &FileExplorerScanPolicy::linux_mvp())
        .expect_err("file target should not scan as directory");

    assert!(matches!(error, ExplorerScanError::NotDirectory { .. }));
}

#[cfg(unix)]
#[test]
fn scanner_preserves_non_utf8_filename_identity_in_relative_path() {
    let sandbox = TestSandbox::new("explorer-non-utf8");
    let project_dir = sandbox.create_dir("project");
    let raw_name = OsString::from_vec(b"bad-\xFF-name.txt".to_vec());
    fs::write(project_dir.join(&raw_name), b"file").unwrap();
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let scan = FileExplorerScanner
        .scan_directory(&root, "", &FileExplorerScanPolicy::linux_mvp())
        .expect("project root should scan");

    let node = scan
        .nodes
        .iter()
        .find(|node| node.relative_path.as_os_str() == raw_name.as_os_str())
        .expect("node should preserve raw filename identity");

    assert!(node.name.contains("bad-"));
}

#[cfg(unix)]
#[test]
fn scanner_labels_in_root_symlink_and_blocks_escaping_symlink_node() {
    let sandbox = TestSandbox::new("explorer-symlink-labels");
    let project_dir = sandbox.create_dir("project");
    let target_file = sandbox.create_file("project/src/lib.rs");
    let outside_file = sandbox.create_file("outside.txt");
    std::os::unix::fs::symlink(&target_file, project_dir.join("lib-link.rs")).unwrap();
    std::os::unix::fs::symlink(&outside_file, project_dir.join("outside-link.txt")).unwrap();
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let scan = FileExplorerScanner
        .scan_directory(&root, "", &FileExplorerScanPolicy::linux_mvp())
        .expect("project root should scan");

    let in_root = node(&scan, "lib-link.rs");
    assert_eq!(
        in_root.symlink_status,
        FileAccessSymlinkStatus::InRootSymlink
    );
    assert_eq!(in_root.state, ExplorerNodeState::Available);

    let escaping = node(&scan, "outside-link.txt");
    assert_eq!(
        escaping.symlink_status,
        FileAccessSymlinkStatus::EscapesRoot
    );
    assert_eq!(
        escaping.state,
        ExplorerNodeState::Blocked(FileAccessBlockedReason::SymlinkEscape)
    );
}

#[cfg(unix)]
#[test]
fn scanner_reports_broken_symlink_as_blocked_stale_node() {
    let sandbox = TestSandbox::new("explorer-broken-symlink");
    let project_dir = sandbox.create_dir("project");
    std::os::unix::fs::symlink(
        sandbox.path("missing.txt"),
        project_dir.join("missing-link.txt"),
    )
    .unwrap();
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let scan = FileExplorerScanner
        .scan_directory(&root, "", &FileExplorerScanPolicy::linux_mvp())
        .expect("project root should scan");

    assert_eq!(
        node_state(&scan, "missing-link.txt"),
        ExplorerNodeState::Blocked(FileAccessBlockedReason::MissingPath)
    );
    assert_eq!(
        node(&scan, "missing-link.txt").symlink_status,
        FileAccessSymlinkStatus::UnresolvedSymlink
    );
}

fn node<'a>(scan: &'a super::ExplorerDirectoryScan, name: &str) -> &'a super::ExplorerNode {
    scan.nodes
        .iter()
        .find(|node| node.name == name)
        .expect("node should exist")
}

fn node_state(scan: &super::ExplorerDirectoryScan, name: &str) -> ExplorerNodeState {
    node(scan, name).state.clone()
}

fn validate(path: &Path) -> ValidProjectRoot {
    ProjectRootValidator
        .validate(path, SymlinkPolicy::FailClosed)
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
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn create_file(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, b"file").unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
