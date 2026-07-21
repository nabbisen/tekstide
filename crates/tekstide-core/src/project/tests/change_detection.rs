use super::project_session;
use crate::domain::{ChangeDetectionFailureReason, ChangeDetectionSource, ChangeDetectionStatus};
use crate::project::{
    ChangePathKind, ChangedPathValidationErrorReason, GeneratedChangeDetectionPolicy,
    GeneratedChangeDetector, ProjectId, ProjectSession,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn filesystem_detector_reports_created_modified_deleted_and_renamed_metadata_paths() {
    let sandbox = TestSandbox::new("change-detection-basic");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"fn main() {}\n");
    sandbox.create_file_with_contents("project/src/delete.rs", b"delete me\n");
    sandbox.create_file_with_contents("project/src/old.rs", b"rename me\n");

    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);

    fs::write(
        sandbox.path("project/src/lib.rs"),
        b"fn main() { println!(\"changed\"); }\n",
    )
    .unwrap();
    fs::remove_file(sandbox.path("project/src/delete.rs")).unwrap();
    fs::rename(
        sandbox.path("project/src/old.rs"),
        sandbox.path("project/src/new.rs"),
    )
    .unwrap();
    sandbox.create_file_with_contents("project/src/created.rs", b"new file\n");

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(baseline.source, ChangeDetectionSource::FilesystemSnapshot);
    assert_eq!(baseline.status, ChangeDetectionStatus::Complete);
    assert_eq!(detected.status, ChangeDetectionStatus::Complete);
    assert_eq!(
        detected.changed_files(),
        vec![
            PathBuf::from("src/created.rs"),
            PathBuf::from("src/delete.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/new.rs"),
            PathBuf::from("src/old.rs"),
        ]
    );
    assert_eq!(
        detected
            .changed_paths
            .iter()
            .find(|path| path.relative_path == Path::new("src/delete.rs"))
            .unwrap()
            .kind,
        ChangePathKind::Deleted
    );
}

#[test]
fn changed_path_validation_accepts_absolute_paths_only_after_root_containment() {
    let sandbox = TestSandbox::new("change-detection-absolute");
    let project = sandbox.project_session(1);
    let in_root_file = sandbox.create_file_with_contents("project/src/lib.rs", b"metadata\n");
    let outside_file = sandbox.create_file_with_contents("outside.rs", b"outside secret\n");

    let detector = GeneratedChangeDetector::default();
    let normalized = detector
        .validate_changed_path(&project, &in_root_file)
        .expect("absolute path under root should normalize");
    let error = detector
        .validate_changed_path(&project, &outside_file)
        .expect_err("absolute path outside root should be rejected");

    assert_eq!(normalized, PathBuf::from("src/lib.rs"));
    assert_eq!(error.project_id, *project.id());
    assert_eq!(error.reason, ChangedPathValidationErrorReason::RootEscape);
    assert!(
        !format!("{error:?}").contains("outside secret"),
        "diagnostics must not include file contents"
    );
}

#[test]
fn changed_path_validation_rejects_parent_traversal_before_resolution() {
    let sandbox = TestSandbox::new("change-detection-traversal");
    let project = sandbox.project_session(1);
    let _outside_file = sandbox.create_file_with_contents("outside.rs", b"outside secret\n");

    let error = GeneratedChangeDetector::default()
        .validate_changed_path(&project, "../outside.rs")
        .expect_err("relative traversal should not be normalized into a project path");

    assert_eq!(
        error.reason,
        ChangedPathValidationErrorReason::InvalidRelativePath
    );
}

#[cfg(unix)]
#[test]
fn changed_path_validation_allows_valid_paths_when_root_has_symlinked_ancestor() {
    let sandbox = TestSandbox::new("change-detection-root-symlink-ancestor");
    let real_root = sandbox.create_dir("real");
    let real_project = sandbox.create_dir("real/project");
    let link_root = sandbox.path("link");
    std::os::unix::fs::symlink(&real_root, &link_root).unwrap();
    sandbox.create_file_with_contents("real/project/src/lib.rs", b"metadata\n");
    let project = ProjectSession::new(
        ProjectId::for_test(1),
        "Project 1",
        link_root.join("project"),
        fs::canonicalize(real_project).unwrap(),
    );

    let normalized = GeneratedChangeDetector::default()
        .validate_changed_path(&project, "src/lib.rs")
        .expect("symlinked ancestors above the project root are not project escapes");

    assert_eq!(normalized, PathBuf::from("src/lib.rs"));
}

#[cfg(unix)]
#[test]
fn filesystem_detector_labels_symlinks_and_does_not_follow_escape_targets() {
    let sandbox = TestSandbox::new("change-detection-symlink");
    let project = sandbox.project_session(1);
    let outside_dir = sandbox.create_dir("outside");
    sandbox.create_file_with_contents("outside/secret.txt", b"outside secret\n");
    std::os::unix::fs::symlink(&outside_dir, sandbox.path("project/outside-link")).unwrap();

    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);
    let error = detector
        .validate_changed_path(&project, "outside-link/secret.txt")
        .expect_err("paths through escaping symlinks should be rejected");

    assert_eq!(baseline.status, ChangeDetectionStatus::Complete);
    assert_eq!(
        baseline
            .entries
            .iter()
            .find(|entry| entry.relative_path == Path::new("outside-link"))
            .unwrap()
            .kind,
        ChangePathKind::Symlink
    );
    assert!(
        baseline
            .entries
            .iter()
            .all(|entry| entry.relative_path != Path::new("outside-link/secret.txt")),
        "scanner must not follow symlink targets outside the root"
    );
    assert_eq!(
        error.reason,
        ChangedPathValidationErrorReason::SymlinkEscape
    );
}

#[test]
fn detector_reports_partial_status_when_entry_limit_is_hit() {
    let sandbox = TestSandbox::new("change-detection-partial");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/a.txt", b"a\n");
    sandbox.create_file_with_contents("project/b.txt", b"b\n");

    let detector = GeneratedChangeDetector::new(GeneratedChangeDetectionPolicy {
        max_entries: 1,
        max_changed_paths: 8,
    });
    let baseline = detector.capture_filesystem_baseline(&project);

    assert_eq!(baseline.entries.len(), 1);
    assert_eq!(baseline.status, ChangeDetectionStatus::Partial { limit: 1 });
}

#[test]
fn detector_suppresses_changed_paths_when_changed_path_limit_is_hit() {
    let sandbox = TestSandbox::new("change-detection-path-limit");
    let project = sandbox.project_session(1);
    let detector = GeneratedChangeDetector::new(GeneratedChangeDetectionPolicy {
        max_entries: 8,
        max_changed_paths: 1,
    });
    let baseline = detector.capture_filesystem_baseline(&project);
    sandbox.create_file_with_contents("project/a.txt", b"a\n");
    sandbox.create_file_with_contents("project/b.txt", b"b\n");

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(detected.status, ChangeDetectionStatus::Partial { limit: 1 });
    assert!(detected.changed_paths.is_empty());
}

#[test]
fn detector_suppresses_changed_paths_when_current_scan_fails() {
    let sandbox = TestSandbox::new("change-detection-failed-scan");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"metadata\n");
    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);
    fs::remove_dir_all(sandbox.path("project")).unwrap();

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(
        detected.status,
        ChangeDetectionStatus::Failed {
            reason: ChangeDetectionFailureReason::RootUnavailable,
        }
    );
    assert!(detected.changed_paths.is_empty());
}

#[test]
fn detector_rejects_cross_project_baselines_without_reporting_paths() {
    let project = project_session(1);
    let other_project = project_session(2);
    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&other_project);

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(
        detected.status,
        ChangeDetectionStatus::Failed {
            reason: ChangeDetectionFailureReason::CrossProjectBaseline,
        }
    );
    assert!(detected.changed_paths.is_empty());
}

#[test]
fn git_status_detection_reports_unavailable_or_unsupported_without_running_git() {
    let project = project_session(1);
    let detector = GeneratedChangeDetector::default();

    let unavailable = detector.detect_git_status_unavailable(&project);
    let unsupported = detector.detect_git_status_unsupported(&project);

    assert_eq!(unavailable.source, ChangeDetectionSource::GitStatus);
    assert_eq!(unavailable.status, ChangeDetectionStatus::Unavailable);
    assert!(unavailable.changed_paths.is_empty());
    assert_eq!(unsupported.source, ChangeDetectionSource::GitStatus);
    assert_eq!(unsupported.status, ChangeDetectionStatus::Unsupported);
    assert!(unsupported.changed_paths.is_empty());
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
        fs::create_dir(root.join("project")).unwrap();
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

    fn create_file_with_contents(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
        path
    }

    fn project_session(&self, sequence: u64) -> ProjectSession {
        let root_path = self.path("project");
        ProjectSession::new(
            ProjectId::for_test(sequence),
            format!("Project {sequence}"),
            root_path.clone(),
            fs::canonicalize(root_path).unwrap(),
        )
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
