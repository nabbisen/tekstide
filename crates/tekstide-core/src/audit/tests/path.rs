use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audit::{AuditPathErrorReason, AuditPathRequest, AuditPathResolver};

#[test]
fn audit_path_resolves_under_state_root_and_outside_projects() {
    let temp = TestDirs::new("valid");

    let path = AuditPathResolver
        .resolve(AuditPathRequest::new(
            &temp.state_root,
            vec![temp.project_root.clone()],
        ))
        .unwrap();

    assert!(path.database_file().starts_with(path.state_root()));
    assert!(
        path.database_file()
            .ends_with(Path::new("audit/audit.sqlite3"))
    );
    assert!(path.recovery_dir().ends_with(Path::new("audit/recovery")));
    assert!(path.journal_file().ends_with("audit.sqlite3-journal"));
    assert!(path.wal_file().ends_with("audit.sqlite3-wal"));
    assert!(path.shared_memory_file().ends_with("audit.sqlite3-shm"));
    assert!(!path.database_file().starts_with(&temp.project_root));
}

#[test]
fn audit_path_rejects_relative_or_missing_state_root() {
    let relative = AuditPathResolver
        .resolve(AuditPathRequest::new("relative-state", Vec::new()))
        .unwrap_err();
    assert_eq!(relative.reason, AuditPathErrorReason::StateRootNotAbsolute);

    let temp = TestDirs::new("missing-state");
    let missing = temp.base.join("missing");
    let error = AuditPathResolver
        .resolve(AuditPathRequest::new(missing, Vec::new()))
        .unwrap_err();
    assert_eq!(error.reason, AuditPathErrorReason::InvalidStateRoot);
}

#[test]
fn audit_path_rejects_project_that_contains_state_or_audit_directory() {
    let temp = TestDirs::new("project-contains-state");

    let error = AuditPathResolver
        .resolve(AuditPathRequest::new(
            &temp.state_root,
            vec![temp.base.clone()],
        ))
        .unwrap_err();

    assert_eq!(
        error.reason,
        AuditPathErrorReason::ProjectContainsAuditState
    );

    let path = AuditPathResolver
        .resolve(AuditPathRequest::new(&temp.state_root, Vec::new()))
        .unwrap();
    fs::create_dir_all(path.audit_dir()).unwrap();
    let later_error = path
        .ensure_project_root_compatible(path.audit_dir())
        .unwrap_err();
    assert_eq!(
        later_error.reason,
        AuditPathErrorReason::ProjectContainsAuditState
    );
}

#[test]
fn project_inside_state_root_is_allowed_when_it_does_not_contain_audit_state() {
    let temp = TestDirs::new("project-inside-state");
    let project = temp.state_root.join("workspace");
    fs::create_dir_all(&project).unwrap();

    let path = AuditPathResolver
        .resolve(AuditPathRequest::new(&temp.state_root, vec![project]))
        .unwrap();

    assert!(path.database_file().starts_with(&temp.state_root));
}

#[test]
fn existing_non_directory_audit_path_is_rejected() {
    let temp = TestDirs::new("audit-file");
    fs::write(temp.state_root.join("audit"), b"not a directory").unwrap();

    let error = AuditPathResolver
        .resolve(AuditPathRequest::new(&temp.state_root, Vec::new()))
        .unwrap_err();

    assert_eq!(error.reason, AuditPathErrorReason::AuditPathTypeInvalid);
}

#[cfg(unix)]
#[test]
fn existing_symlinked_audit_directory_is_rejected() {
    use std::os::unix::fs::symlink;

    let temp = TestDirs::new("audit-symlink");
    let target = temp.base.join("audit-target");
    fs::create_dir_all(&target).unwrap();
    symlink(&target, temp.state_root.join("audit")).unwrap();

    let error = AuditPathResolver
        .resolve(AuditPathRequest::new(&temp.state_root, Vec::new()))
        .unwrap_err();

    assert_eq!(error.reason, AuditPathErrorReason::AuditPathIsSymlink);
}

#[cfg(unix)]
#[test]
fn existing_symlinked_sqlite_companion_is_rejected() {
    use std::os::unix::fs::symlink;

    let temp = TestDirs::new("companion-symlink");
    let audit_dir = temp.state_root.join("audit");
    let target = temp.base.join("outside-wal");
    fs::create_dir_all(&audit_dir).unwrap();
    fs::write(&target, b"not a real wal").unwrap();
    symlink(&target, audit_dir.join("audit.sqlite3-wal")).unwrap();

    let error = AuditPathResolver
        .resolve(AuditPathRequest::new(&temp.state_root, Vec::new()))
        .unwrap_err();

    assert_eq!(error.reason, AuditPathErrorReason::AuditPathIsSymlink);
}

#[test]
fn path_errors_do_not_echo_local_paths() {
    let secret_path = "/private/project/customer-name";
    let error = AuditPathResolver
        .resolve(AuditPathRequest::new(secret_path, Vec::new()))
        .unwrap_err();

    assert!(!format!("{error:?}").contains(secret_path));
}

struct TestDirs {
    base: PathBuf,
    state_root: PathBuf,
    project_root: PathBuf,
}

impl TestDirs {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-audit-{label}-{}-{unique}",
            std::process::id()
        ));
        let state_root = base.join("state");
        let project_root = base.join("project");
        fs::create_dir_all(&state_root).unwrap();
        fs::create_dir_all(&project_root).unwrap();
        Self {
            base,
            state_root,
            project_root,
        }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
