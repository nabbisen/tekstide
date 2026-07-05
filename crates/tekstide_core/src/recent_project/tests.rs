use super::{
    AppStatePathProvider, RecentProject, RecentProjectAvailability, RecentProjectState,
    RecentProjectStore, Timestamp, assess_recent_project_availability,
};
use crate::project::ProjectId;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn serializes_versioned_recent_project_json() {
    let project = sample_project("/selected/project", "/canonical/project");
    let state = RecentProjectState {
        state_version: 1,
        projects: vec![project],
    };

    let json = state.to_json();

    assert!(json.contains("\"state_version\": 1"));
    assert!(json.contains("\"project_id\": \"00000000-0000-4000-8000-000000000001\""));
    assert!(json.contains("\"root_path\": \"/selected/project\""));
    assert!(json.contains("\"canonical_root_path\": \"/canonical/project\""));
    assert!(json.contains("\"last_trust_state_summary\": \"Restricted\""));
}

#[test]
fn parses_known_schema_and_ignores_unknown_fields() {
    let json = r#"{
  "state_version": 1,
  "unknown_top_level": "ignored",
  "projects": [
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "tekstide",
      "root_path": "/selected/project",
      "canonical_root_path": "/canonical/project",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "last_trust_state_summary": "Trusted",
      "future_field": "ignored",
      "future_flag": true,
      "future_null": null
    }
  ]
}"#;

    let state = RecentProjectState::from_json(json).expect("state should parse");

    assert_eq!(state.state_version, 1);
    assert_eq!(state.projects.len(), 1);
    assert_eq!(state.projects[0].display_name, "tekstide");
    assert_eq!(state.projects[0].last_trust_state_summary, "Trusted");
}

#[test]
fn rejects_missing_required_fields() {
    let json = r#"{
  "state_version": 1,
  "projects": [
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "tekstide",
      "root_path": "/selected/project",
      "canonical_root_path": "/canonical/project",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z"
    }
  ]
}"#;

    let error = RecentProjectState::from_json(json).expect_err("missing field should be rejected");

    assert!(error.contains("last_trust_state_summary"));
}

#[test]
fn rejects_invalid_timestamp() {
    let json = r#"{
  "state_version": 1,
  "projects": [
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "tekstide",
      "root_path": "/selected/project",
      "canonical_root_path": "/canonical/project",
      "last_opened_at": "not-a-timestamp",
      "last_activity": "2026-07-04T00:00:00Z",
      "last_trust_state_summary": "Restricted"
    }
  ]
}"#;

    let error = RecentProjectState::from_json(json).expect_err("timestamp should be rejected");

    assert!(error.contains("timestamp must use"));
}

#[test]
fn rejects_duplicate_project_id() {
    let json = r#"{
  "state_version": 1,
  "projects": [
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "one",
      "root_path": "/selected/one",
      "canonical_root_path": "/canonical/one",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "last_trust_state_summary": "Restricted"
    },
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "two",
      "root_path": "/selected/two",
      "canonical_root_path": "/canonical/two",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "last_trust_state_summary": "Restricted"
    }
  ]
}"#;

    let error = RecentProjectState::from_json(json).expect_err("duplicate ID should be rejected");

    assert!(error.contains("duplicate project_id"));
}

#[test]
fn rejects_duplicate_canonical_root_path() {
    let json = r#"{
  "state_version": 1,
  "projects": [
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "one",
      "root_path": "/selected/one",
      "canonical_root_path": "/canonical/shared",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "last_trust_state_summary": "Restricted"
    },
    {
      "project_id": "00000000-0000-4000-8000-000000000002",
      "display_name": "two",
      "root_path": "/selected/two",
      "canonical_root_path": "/canonical/shared",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "last_trust_state_summary": "Restricted"
    }
  ]
}"#;

    let error = RecentProjectState::from_json(json)
        .expect_err("duplicate canonical root should be rejected");

    assert!(error.contains("duplicate canonical_root_path"));
}

#[test]
fn rejects_non_uuid_project_id() {
    let json = r#"{
  "state_version": 1,
  "projects": [
    {
      "project_id": "project-0000000000000001",
      "display_name": "tekstide",
      "root_path": "/selected/project",
      "canonical_root_path": "/canonical/project",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "last_trust_state_summary": "Restricted"
    }
  ]
}"#;

    let error = RecentProjectState::from_json(json).expect_err("sequence id should be rejected");

    assert!(error.contains("project_id must be a UUID string"));
}

#[test]
fn missing_state_file_loads_empty_state() {
    let sandbox = TestSandbox::new("missing-state");
    let store =
        RecentProjectStore::new(AppStatePathProvider::from_state_dir(sandbox.path("state")));

    let state = store.load().expect("missing state should not fail");

    assert_eq!(state, RecentProjectState::default());
}

#[test]
fn corrupt_state_file_is_renamed_and_reported() {
    let sandbox = TestSandbox::new("corrupt-state");
    let state_dir = sandbox.create_dir("state");
    let state_file = state_dir.join("recent-projects.json");
    fs::write(&state_file, b"not json").unwrap();
    let store = RecentProjectStore::new(AppStatePathProvider::from_state_dir(&state_dir));

    let error = store.load().expect_err("corrupt state should be reported");

    assert!(!error.to_string().is_empty());
    assert!(!state_file.exists());
    assert!(state_dir.join("recent-projects.json.corrupt").exists());
}

#[test]
fn corrupt_state_rename_does_not_overwrite_existing_corrupt_file() {
    let sandbox = TestSandbox::new("corrupt-state-collision");
    let state_dir = sandbox.create_dir("state");
    let state_file = state_dir.join("recent-projects.json");
    let first_corrupt = state_dir.join("recent-projects.json.corrupt");
    fs::write(&state_file, b"not json").unwrap();
    fs::write(&first_corrupt, b"older corrupt state").unwrap();
    let store = RecentProjectStore::new(AppStatePathProvider::from_state_dir(&state_dir));

    let _ = store.load().expect_err("corrupt state should be reported");

    assert_eq!(
        fs::read_to_string(first_corrupt).unwrap(),
        "older corrupt state"
    );
    assert!(state_dir.join("recent-projects.json.corrupt-1").exists());
}

#[test]
fn save_creates_linux_state_file() {
    let sandbox = TestSandbox::new("save-state");
    let state_dir = sandbox.path("xdg-state/tekstide");
    let store = RecentProjectStore::new(AppStatePathProvider::from_state_dir(&state_dir));
    let state = RecentProjectState {
        state_version: 1,
        projects: vec![sample_project("/selected/project", "/canonical/project")],
    };

    store.save(&state).expect("state should save");

    let saved = fs::read_to_string(state_dir.join("recent-projects.json")).unwrap();
    assert!(saved.contains("\"projects\""));
    assert!(!state_dir.join("recent-projects.json.tmp").exists());
}

#[test]
fn xdg_state_home_uses_exact_linux_state_path() {
    let provider = AppStatePathProvider::linux_from_env(
        Some(OsString::from("/tmp/xdg-state")),
        Some(OsString::from("/tmp/home")),
    )
    .expect("XDG path should be available");

    assert_eq!(
        provider.recent_projects_file(),
        PathBuf::from("/tmp/xdg-state/tekstide/recent-projects.json")
    );
}

#[test]
fn empty_xdg_state_home_falls_back_to_home_local_state() {
    let provider = AppStatePathProvider::linux_from_env(
        Some(OsString::from("")),
        Some(OsString::from("/tmp/home")),
    )
    .expect("HOME fallback should be available");

    assert_eq!(
        provider.recent_projects_file(),
        PathBuf::from("/tmp/home/.local/state/tekstide/recent-projects.json")
    );
}

#[test]
fn missing_home_returns_path_error_without_cwd_fallback() {
    let error = AppStatePathProvider::linux_from_env(None::<OsString>, None::<OsString>)
        .expect_err("missing HOME should not fall back to cwd");

    assert!(error.to_string().contains("HOME is unavailable"));
}

#[test]
fn availability_reports_folder_missing() {
    let sandbox = TestSandbox::new("folder-missing");
    let project = sample_project(
        sandbox.path("missing").display().to_string(),
        sandbox.path("missing").display().to_string(),
    );

    let availability = assess_recent_project_availability(&project);

    assert_eq!(availability, RecentProjectAvailability::FolderMissing);
}

#[test]
fn availability_reports_path_changed() {
    let sandbox = TestSandbox::new("path-changed");
    let selected = sandbox.create_dir("selected");
    let canonical = sandbox.path("old-canonical");
    let project = sample_project(
        selected.display().to_string(),
        canonical.display().to_string(),
    );

    let availability = assess_recent_project_availability(&project);

    assert_eq!(availability, RecentProjectAvailability::PathChanged);
}

fn sample_project(
    root_path: impl Into<PathBuf>,
    canonical_root_path: impl Into<PathBuf>,
) -> RecentProject {
    RecentProject::new(
        ProjectId::for_test(1),
        "tekstide",
        root_path,
        canonical_root_path,
        Timestamp::from_persisted("2026-07-04T00:00:00Z"),
        "Restricted",
    )
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
        let root = std::env::temp_dir().join(format!(
            "tekstide-recent-{name}-{}-{nonce}",
            std::process::id()
        ));
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
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
