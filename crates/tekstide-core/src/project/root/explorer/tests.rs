use super::{
    BrowseNodeState, DirectoryBrowseError, ExplorerNodeKind, ExplorerNodeState, ExplorerScanError,
    FileExplorerScanPolicy, FileExplorerScanner, browse_directory,
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

// RFC-038 PR-038-G: `browse_directory`, the folder browser's own,
// project-independent scanner.

#[test]
fn browse_directory_lists_only_subdirectories_sorted() {
    let sandbox = TestSandbox::new("browse-dirs-only");
    let dir = sandbox.create_dir("start");
    sandbox.create_file("start/readme.md");
    sandbox.create_dir("start/zeta");
    sandbox.create_dir("start/alpha");

    let scan =
        browse_directory(&dir, &FileExplorerScanPolicy::linux_mvp()).expect("dir should scan");

    let names: Vec<_> = scan.nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(
        names,
        ["alpha", "zeta"],
        "files must not appear at all -- a folder browser chooses a project root, not a file"
    );
    assert!(!scan.truncated);
}

#[test]
fn browse_directory_bounds_child_count() {
    let sandbox = TestSandbox::new("browse-bound");
    let dir = sandbox.create_dir("start");
    for index in 0..5 {
        sandbox.create_dir(&format!("start/dir-{index}"));
    }
    let policy = FileExplorerScanPolicy {
        max_children_per_directory: 3,
        collapsed_directory_names: Vec::new(),
    };

    let scan = browse_directory(&dir, &policy).expect("dir should scan");

    assert_eq!(scan.nodes.len(), 3);
    assert!(scan.truncated);
}

#[test]
fn browse_directory_collapses_ignored_directory_names() {
    let sandbox = TestSandbox::new("browse-collapse");
    let dir = sandbox.create_dir("start");
    sandbox.create_dir("start/.git");
    sandbox.create_dir("start/real-project");

    let scan =
        browse_directory(&dir, &FileExplorerScanPolicy::linux_mvp()).expect("dir should scan");

    let git_node = scan
        .nodes
        .iter()
        .find(|node| node.name == ".git")
        .expect("the same shared FileExplorerScanPolicy must still list it, just collapsed");
    assert_eq!(git_node.state, BrowseNodeState::Collapsed);
    let real_node = scan
        .nodes
        .iter()
        .find(|node| node.name == "real-project")
        .expect("an ordinary directory must be listed and available");
    assert_eq!(real_node.state, BrowseNodeState::Available);
}

#[test]
fn browse_directory_parent_dir_is_some_except_at_filesystem_root() {
    let sandbox = TestSandbox::new("browse-parent");
    let dir = sandbox.create_dir("start/nested");

    let scan =
        browse_directory(&dir, &FileExplorerScanPolicy::linux_mvp()).expect("dir should scan");
    assert!(scan.parent_dir.is_some());

    let root_scan = browse_directory("/", &FileExplorerScanPolicy::linux_mvp())
        .expect("filesystem root should scan");
    assert_eq!(
        root_scan.parent_dir, None,
        "there is nothing above the filesystem root to navigate up into"
    );
}

#[test]
fn browse_directory_rejects_a_file_path() {
    let sandbox = TestSandbox::new("browse-file");
    let file = sandbox.create_file("not-a-directory.txt");

    let error = browse_directory(&file, &FileExplorerScanPolicy::linux_mvp())
        .expect_err("a file path must be refused");

    assert!(matches!(error, DirectoryBrowseError::NotDirectory { .. }));
}

#[test]
fn browse_directory_rejects_a_path_that_does_not_exist() {
    let sandbox = TestSandbox::new("browse-missing");
    let missing = sandbox.path("does-not-exist-at-all");

    let error = browse_directory(&missing, &FileExplorerScanPolicy::linux_mvp())
        .expect_err("a nonexistent path must be refused");

    assert!(matches!(
        error,
        DirectoryBrowseError::CannotReadDirectory { .. }
    ));
}

#[test]
fn browse_directory_resolves_a_real_symlinked_directory_rather_than_refusing_it() {
    // The whole point of BrowseNode's own doc comment: browsing follows
    // symlinks freely, since there is no root to escape and whatever is
    // chosen is re-validated at commit time by add_project_from_path.
    let sandbox = TestSandbox::new("browse-symlink");
    let real_dir = sandbox.create_dir("real");
    sandbox.create_dir("real/child");
    let link_path = sandbox.path("link-to-real");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_dir, &link_path).expect("symlink must be creatable");

    let scan = browse_directory(&link_path, &FileExplorerScanPolicy::linux_mvp())
        .expect("a symlinked directory must scan, not be refused");

    assert_eq!(
        scan.current_dir, real_dir,
        "current_dir must be the canonical (resolved) path, not the symlink's own path"
    );
    assert_eq!(scan.nodes.len(), 1);
    assert_eq!(scan.nodes[0].name, "child");
}
