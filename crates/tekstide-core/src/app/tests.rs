use super::AppState;
use crate::app::{AddProjectOutcome, RemoveProjectError};
use crate::close::{
    CloseAssessment, CloseReason, CloseReasonCode, CloseResourceProviderState, CloseResourceSummary,
};
use crate::domain::DomainTimestamp;
use crate::project::recent::{RecentProject, RecentProjectState, Timestamp};
use crate::project::root::{ProjectRootValidationError, SymlinkPolicy};
use crate::project::{ProjectId, ProjectRuntimeSummary};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn new_state_has_no_projects() {
    let state = AppState::default();

    assert!(state.projects().is_empty());
    assert!(state.active_project_id().is_none());
}

#[test]
fn first_project_becomes_active() {
    let mut state = AppState::default();

    let project_id =
        state.add_project_session("Tekstide", "/workspace/tekstide", "/workspace/tekstide");

    assert_eq!(state.active_project_id(), Some(&project_id));
    assert_eq!(
        state
            .active_project()
            .expect("first project should be active")
            .display_name(),
        "Tekstide"
    );
}

#[test]
fn switching_active_project_requires_existing_project_id() {
    let mut state = AppState::default();
    let first_id = state.add_project_session("First", "/workspace/first", "/workspace/first");
    let second_id = state.add_project_session("Second", "/workspace/second", "/workspace/second");

    assert!(state.switch_active_project(&second_id));
    assert_eq!(state.active_project_id(), Some(&second_id));

    let missing_id = ProjectId::for_test(999);
    assert!(!state.switch_active_project(&missing_id));
    assert_eq!(state.active_project_id(), Some(&second_id));
    assert_ne!(first_id, second_id);
}

#[test]
fn add_project_from_path_validates_and_restricts_before_display() {
    let sandbox = TestSandbox::new("add-valid-project");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();

    let outcome = state
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    let project_id = match outcome {
        AddProjectOutcome::Added(project_id) => project_id,
        AddProjectOutcome::FocusedExisting(_) => panic!("new project should be added"),
    };
    let project = state
        .project(&project_id)
        .expect("added project should be available");
    assert_eq!(project.display_name(), "project");
    assert_eq!(project.root_path(), &project_dir);
    assert_eq!(
        project.canonical_root_path(),
        &fs::canonicalize(&project_dir).unwrap()
    );
    assert_eq!(project.trust_state().label(), "Restricted");
}

#[test]
fn duplicate_canonical_root_focuses_existing_project() {
    let sandbox = TestSandbox::new("duplicate-canonical");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();

    let first = state
        .add_project_from_path(&project_dir)
        .expect("first add should succeed");
    let second = state
        .add_project_from_path(&project_dir)
        .expect("duplicate add should focus existing project");

    assert!(matches!(first, AddProjectOutcome::Added(_)));
    assert_eq!(
        second,
        AddProjectOutcome::FocusedExisting(first.project_id().clone())
    );
    assert_eq!(state.projects().len(), 1);
    assert_eq!(state.recent_projects().len(), 1);
    assert_eq!(state.active_project_id(), Some(first.project_id()));
}

#[test]
fn add_project_from_path_rejects_files() {
    let sandbox = TestSandbox::new("reject-file");
    let file_path = sandbox.create_file("file.txt");
    let mut state = AppState::default();

    let error = state
        .add_project_from_path(&file_path)
        .expect_err("file path should be rejected");

    assert_eq!(
        error,
        ProjectRootValidationError::NotDirectory { path: file_path }
    );
    assert!(state.projects().is_empty());
}

#[cfg(unix)]
#[test]
fn add_project_from_path_fails_closed_on_symlink_root() {
    let sandbox = TestSandbox::new("reject-symlink");
    let target_dir = sandbox.create_dir("target");
    let link_path = sandbox.path("link");
    std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();
    let mut state = AppState::default();

    let error = state
        .add_project_from_path(&link_path)
        .expect_err("symlink should require explicit confirmation");

    assert_eq!(
        error,
        ProjectRootValidationError::SymlinkAmbiguous {
            selected_path: link_path,
            canonical_path: fs::canonicalize(target_dir).unwrap(),
        }
    );
    assert!(state.projects().is_empty());
}

#[cfg(unix)]
#[test]
fn confirmed_symlink_root_creates_session_for_canonical_identity() {
    let sandbox = TestSandbox::new("confirm-symlink");
    let target_dir = sandbox.create_dir("target");
    let link_path = sandbox.path("link");
    std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();
    let mut state = AppState::default();

    let outcome = state
        .add_project_from_path_with_symlink_policy(&link_path, SymlinkPolicy::AllowCanonicalTarget)
        .expect("confirmed symlink should be added");

    let project = state.project(outcome.project_id()).unwrap();
    assert_eq!(project.root_path(), &link_path);
    assert_eq!(
        project.canonical_root_path(),
        &fs::canonicalize(target_dir).unwrap()
    );
}

#[test]
fn recent_project_state_uses_uuid_project_ids() {
    let sandbox = TestSandbox::new("uuid-recent");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();

    state
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    let recent_state = state.recent_project_state();
    let project_id = recent_state.projects[0].project_id.as_str();
    assert_eq!(project_id.len(), 36);
    assert_eq!(project_id.as_bytes()[14], b'4');
}

#[test]
fn recent_project_state_exports_project_session_timestamps() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");
    let opened_at = DomainTimestamp::from_utc_string("2026-07-05T01:02:03Z").unwrap();
    let activity_at = DomainTimestamp::from_utc_string("2026-07-05T04:05:06Z").unwrap();
    let project = state.project_mut(&project_id).unwrap();
    project.mark_opened_at(opened_at);
    project.record_activity_at(activity_at);

    let recent_state = state.recent_project_state();
    let recent_project = &recent_state.projects[0];

    assert_eq!(
        recent_project.last_opened_at.as_str(),
        "2026-07-05T01:02:03Z"
    );
    assert_eq!(
        recent_project.last_activity.as_str(),
        "2026-07-05T04:05:06Z"
    );
}

#[test]
fn restored_recent_project_id_is_reused_when_project_is_added_again() {
    let sandbox = TestSandbox::new("reuse-restored-id");
    let project_dir = sandbox.create_dir("project");
    let canonical = fs::canonicalize(&project_dir).unwrap();
    let restored_id = ProjectId::for_test(42);
    let mut state = AppState::default();
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            restored_id.clone(),
            "project",
            project_dir.clone(),
            canonical,
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            "Trusted",
        )],
    });

    let outcome = state
        .add_project_from_path(&project_dir)
        .expect("restored project should validate");

    assert_eq!(outcome, AddProjectOutcome::Added(restored_id.clone()));
    assert_eq!(state.active_project_id(), Some(&restored_id));
    assert_eq!(state.projects().len(), 1);
    assert_eq!(state.recent_projects().len(), 1);
    assert_eq!(
        state.projects()[0].trust_state().label(),
        "Restricted",
        "display-only trust summary must not restore trust"
    );
}

#[test]
fn active_project_with_missing_close_provider_is_not_closed() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");

    let assessment = state
        .close_project(&project_id)
        .expect("assessment should be returned");

    assert_eq!(
        assessment,
        CloseAssessment::UnsupportedOrUnknown {
            reason: "active-resource state is unavailable".to_owned()
        }
    );
    assert!(state.project(&project_id).is_some());
}

#[test]
fn active_idle_project_closes_when_provider_proves_safe() {
    let sandbox = TestSandbox::new("close-idle");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();
    let project_id = state
        .add_project_from_path(&project_dir)
        .expect("project should add")
        .project_id()
        .clone();
    state
        .project_mut(&project_id)
        .unwrap()
        .set_runtime_summary(ProjectRuntimeSummary {
            close_resources: CloseResourceSummary {
                provider_state: CloseResourceProviderState::Complete,
                running_processes: 0,
                dirty_files: 0,
                pending_approvals: 0,
                review_ready_changes: 0,
            },
            ..ProjectRuntimeSummary::default()
        });

    let assessment = state
        .close_project(&project_id)
        .expect("safe close should remove session");

    assert_eq!(assessment, CloseAssessment::SafeToClose);
    assert!(state.project(&project_id).is_none());
    assert!(
        project_dir.exists(),
        "close must not delete workspace contents"
    );
    assert!(
        state
            .recent_projects()
            .iter()
            .any(|restored| restored.recent_project.project_id == project_id),
        "closing active session should preserve recent entry"
    );
}

#[test]
fn active_project_with_resources_needs_confirmation_and_stays_open() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");
    state
        .project_mut(&project_id)
        .unwrap()
        .set_runtime_summary(ProjectRuntimeSummary {
            close_resources: CloseResourceSummary {
                provider_state: CloseResourceProviderState::Complete,
                running_processes: 1,
                dirty_files: 2,
                pending_approvals: 0,
                review_ready_changes: 0,
            },
            ..ProjectRuntimeSummary::default()
        });

    let assessment = state
        .close_project(&project_id)
        .expect("assessment should be returned");

    assert_eq!(
        assessment,
        CloseAssessment::NeedsConfirmation {
            reasons: vec![
                CloseReason {
                    code: CloseReasonCode::RunningProcess,
                    message: "1 running process".to_owned(),
                },
                CloseReason {
                    code: CloseReasonCode::DirtyFile,
                    message: "2 dirty files".to_owned(),
                },
            ]
        }
    );
    assert!(state.project(&project_id).is_some());
}

#[test]
fn stale_recent_project_removal_only_removes_recent_metadata() {
    let sandbox = TestSandbox::new("remove-stale-recent");
    let missing_path = sandbox.path("missing");
    let recent_id = ProjectId::for_test(77);
    let mut state = AppState::default();
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            recent_id.clone(),
            "Missing",
            missing_path.clone(),
            missing_path,
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            "Restricted",
        )],
    });

    state
        .remove_recent_project(&recent_id)
        .expect("stale recent entry should be removable");

    assert!(state.recent_projects().is_empty());
    assert!(state.projects().is_empty());
}

#[test]
fn active_project_cannot_be_removed_as_recent_metadata() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");

    let error = state
        .remove_recent_project(&project_id)
        .expect_err("active project requires close flow");

    assert_eq!(error, RemoveProjectError::ProjectIsActive);
    assert!(state.project(&project_id).is_some());
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
        fs::write(&path, b"not a directory").unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
