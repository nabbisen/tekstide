use super::{
    AppStatePathProvider, RecentProject, RecentProjectAvailability, RecentProjectLoadOutcome,
    RecentProjectSave, RecentProjectState, RecentProjectStore, Timestamp,
    assess_recent_project_availability,
};
use crate::project::{ProjectId, WorkspaceTrust};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
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
    assert!(json.contains("\"trust_state\": \"Restricted\""));
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
      "trust_state": "Trusted",
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
    assert_eq!(state.projects[0].trust_state, WorkspaceTrust::Trusted);
}

/// RFC-032: `trust_state` is deliberately **not** covered by this test --
/// it is the one field allowed to be missing (`#[serde(default)]`, see
/// `RecentProject::trust_state`'s own doc comment), covered separately by
/// [`missing_trust_state_defaults_to_restricted`] below.
#[test]
fn rejects_missing_required_fields() {
    let json = r#"{
  "state_version": 1,
  "projects": [
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "tekstide",
      "root_path": "/selected/project",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "trust_state": "Restricted"
    }
  ]
}"#;

    let error = RecentProjectState::from_json(json).expect_err("missing field should be rejected");

    assert!(error.contains("canonical_root_path"));
}

/// RFC-032: the fail-closed default -- a pre-RFC-032 on-disk record has
/// no `trust_state` field at all (the field did not exist), and must
/// still parse, defaulting to `Restricted` rather than being rejected as
/// malformed. Correct as more than parser leniency: nothing could
/// genuinely have been trusted before this field existed.
#[test]
fn missing_trust_state_defaults_to_restricted() {
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

    let state = RecentProjectState::from_json(json)
        .expect("a record with no trust_state field must still parse");

    assert_eq!(state.projects[0].trust_state, WorkspaceTrust::Restricted);
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
      "trust_state": "Restricted"
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
      "trust_state": "Restricted"
    },
    {
      "project_id": "00000000-0000-4000-8000-000000000001",
      "display_name": "two",
      "root_path": "/selected/two",
      "canonical_root_path": "/canonical/two",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "trust_state": "Restricted"
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
      "trust_state": "Restricted"
    },
    {
      "project_id": "00000000-0000-4000-8000-000000000002",
      "display_name": "two",
      "root_path": "/selected/two",
      "canonical_root_path": "/canonical/shared",
      "last_opened_at": "2026-07-04T00:00:00Z",
      "last_activity": "2026-07-04T00:00:00Z",
      "trust_state": "Restricted"
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
      "trust_state": "Restricted"
    }
  ]
}"#;

    let error = RecentProjectState::from_json(json).expect_err("sequence id should be rejected");

    assert!(error.contains("project_id must be a UUID string"));
}

// The four tests below moved from `RecentProjectStore::load` to
// `load_or_recover` at RFC-051 PR-051-C, when `load` was deleted for having no
// caller outside them. Each keeps what it proved.

/// A first start is not a failure: no file, no recovery, nothing moved.
#[test]
fn missing_state_file_loads_empty_state() {
    let sandbox = TestSandbox::new("missing-state");
    let mut store =
        RecentProjectStore::new(AppStatePathProvider::from_state_dir(sandbox.path("state")));

    let loaded = store.load_or_recover();

    assert_eq!(loaded.state, RecentProjectState::default());
    assert_eq!(loaded.outcome, RecentProjectLoadOutcome::Loaded);
}

#[test]
fn corrupt_state_file_is_renamed_and_reported() {
    let sandbox = TestSandbox::new("corrupt-state");
    let state_dir = sandbox.create_dir("state");
    let state_file = state_dir.join("recent-projects.json");
    fs::write(&state_file, b"not json").unwrap();
    let mut store = RecentProjectStore::new(AppStatePathProvider::from_state_dir(&state_dir));

    let loaded = store.load_or_recover();

    match &loaded.outcome {
        RecentProjectLoadOutcome::Reset { message, .. } => assert!(!message.is_empty()),
        other => panic!("expected Reset with no backup to recover from, got {other:?}"),
    }
    assert!(!state_file.exists());
    assert!(state_dir.join("recent-projects.json.corrupt").exists());
}

/// RFC-050 PR-050-C: the board says where the unreadable list went, so the
/// outcome must carry the path the rename produced.
#[test]
fn a_corrupt_state_reports_where_it_was_moved() {
    let sandbox = TestSandbox::new("corrupt-state-moved-to");
    let state_dir = sandbox.create_dir("state");
    fs::write(state_dir.join("recent-projects.json"), b"not json").unwrap();
    let mut store = RecentProjectStore::new(AppStatePathProvider::from_state_dir(&state_dir));

    let loaded = store.load_or_recover();

    assert_eq!(
        loaded.outcome.moved_to(),
        Some(state_dir.join("recent-projects.json.corrupt").as_path())
    );
}

#[test]
fn corrupt_state_rename_does_not_overwrite_existing_corrupt_file() {
    let sandbox = TestSandbox::new("corrupt-state-collision");
    let state_dir = sandbox.create_dir("state");
    let state_file = state_dir.join("recent-projects.json");
    let first_corrupt = state_dir.join("recent-projects.json.corrupt");
    fs::write(&state_file, b"not json").unwrap();
    fs::write(&first_corrupt, b"older corrupt state").unwrap();
    let mut store = RecentProjectStore::new(AppStatePathProvider::from_state_dir(&state_dir));

    let _ = store.load_or_recover();

    assert_eq!(
        fs::read_to_string(first_corrupt).unwrap(),
        "older corrupt state"
    );
    assert!(state_dir.join("recent-projects.json.corrupt-1").exists());
}

// --- RFC-051: recovering the recent-project list ----------------------------

fn store_with_backup(name: &str) -> (TestSandbox, PathBuf, RecentProjectStore) {
    let sandbox = TestSandbox::new(name);
    let state_dir = sandbox.create_dir("state");
    let store = RecentProjectStore::new(AppStatePathProvider::from_state_dir(&state_dir));
    (sandbox, state_dir, store)
}

fn state_with_one_project() -> RecentProjectState {
    RecentProjectState {
        state_version: 1,
        projects: vec![sample_project("/selected/project", "/canonical/project")],
    }
}

/// RFC-051's whole point: **the ids come back.** A project's id is what a trust
/// grant is matched by and what its transcript directory is named after, so a
/// recovery that produced a list with new ids would repair nothing.
#[test]
fn a_corrupt_live_file_recovers_the_list_from_the_backup_with_ids_intact() {
    let (_sandbox, state_dir, mut store) = store_with_backup("recover-from-backup");
    // The order production uses, and the order D2′ requires: a load comes first,
    // and only a store that loaded may write the previous-good copy. A save
    // before any load writes no backup at all — which this test found by
    // failing when it skipped the load.
    assert_eq!(
        store.load_or_recover().outcome,
        RecentProjectLoadOutcome::Loaded
    );
    let saved = state_with_one_project();
    assert_eq!(
        store.save(&saved).expect("a real save"),
        RecentProjectSave::Written { backup: true }
    );
    fs::write(state_dir.join("recent-projects.json"), b"not json").unwrap();

    let loaded = store.load_or_recover();

    assert_eq!(
        loaded.state, saved,
        "the recovered list is the one that was saved, ids and all"
    );
    assert_eq!(
        loaded.state.projects[0].project_id,
        saved.projects[0].project_id
    );
    match loaded.outcome {
        RecentProjectLoadOutcome::Recovered { moved_to, .. } => assert_eq!(
            moved_to,
            Some(state_dir.join("recent-projects.json.corrupt")),
            "and the outcome names where the unusable file went"
        ),
        other => panic!("expected Recovered, got {other:?}"),
    }
}

/// §1, the defect this RFC exists for: an **unreadable** file was never
/// quarantined, and `boot()` saved an empty list over it.
#[test]
fn an_unreadable_live_file_is_quarantined_rather_than_overwritten() {
    let (_sandbox, state_dir, mut store) = store_with_backup("quarantine-unreadable");
    let state_file = state_dir.join("recent-projects.json");
    fs::write(&state_file, b"{\"state_version\":1,\"projects\":[]}").unwrap();
    let mut permissions = fs::metadata(&state_file).unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&state_file, permissions).unwrap();

    let loaded = store.load_or_recover();

    assert!(
        !state_file.exists(),
        "the unreadable file is moved aside, not left for the next save to overwrite"
    );
    let quarantined = state_dir.join("recent-projects.json.corrupt");
    assert!(quarantined.exists());
    assert!(
        matches!(loaded.outcome, RecentProjectLoadOutcome::Reset { .. }),
        "no backup existed, so this is the reset case: {:?}",
        loaded.outcome
    );
    let mut permissions = fs::metadata(&quarantined).unwrap().permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&quarantined, permissions).unwrap();
}

/// §1's second half: when the quarantine rename **fails**, the file we could not
/// read is still there — so the session persists nothing at all rather than
/// writing over it.
#[test]
fn a_failed_quarantine_withholds_every_save_for_the_session() {
    let (_sandbox, state_dir, mut store) = store_with_backup("quarantine-rename-fails");
    let state_file = state_dir.join("recent-projects.json");
    let original = b"the user's unreadable file";
    fs::write(&state_file, original).unwrap();
    let mut file_permissions = fs::metadata(&state_file).unwrap().permissions();
    file_permissions.set_mode(0o000);
    fs::set_permissions(&state_file, file_permissions).unwrap();
    // A read-only directory is what makes the rename fail: the file cannot be
    // moved out of it, so there is nowhere to quarantine it to.
    let mut dir_permissions = fs::metadata(&state_dir).unwrap().permissions();
    dir_permissions.set_mode(0o500);
    fs::set_permissions(&state_dir, dir_permissions).unwrap();

    let loaded = store.load_or_recover();
    let saved = store
        .save(&state_with_one_project())
        .expect("no error, a refusal");

    assert!(matches!(
        loaded.outcome,
        RecentProjectLoadOutcome::Reset { moved_to: None, .. }
    ));
    assert_eq!(
        saved,
        RecentProjectSave::Withheld,
        "a user's unreadable file is worth more than our empty list"
    );

    let mut dir_permissions = fs::metadata(&state_dir).unwrap().permissions();
    dir_permissions.set_mode(0o700);
    fs::set_permissions(&state_dir, dir_permissions).unwrap();
    let mut file_permissions = fs::metadata(&state_file).unwrap().permissions();
    file_permissions.set_mode(0o600);
    fs::set_permissions(&state_file, file_permissions).unwrap();
    assert_eq!(
        fs::read(&state_file).unwrap(),
        original,
        "and the file itself is untouched"
    );
}

/// §2/D2′: a session that started empty after a failed load must not make its
/// first save the backup — that would overwrite the only good copy with the
/// empty list, which is this RFC's own failure one file further along.
#[test]
fn a_session_that_started_empty_after_a_failed_load_writes_no_backup() {
    let (_sandbox, state_dir, mut store) = store_with_backup("no-backup-after-failed-load");
    let good = state_with_one_project();
    store.save(&good).expect("a first save writes both files");
    let backup = state_dir.join("recent-projects.json.bak");
    fs::write(state_dir.join("recent-projects.json"), b"not json").unwrap();
    fs::write(&backup, b"not json either").unwrap();

    let loaded = store.load_or_recover();
    assert!(matches!(
        loaded.outcome,
        RecentProjectLoadOutcome::Reset { .. }
    ));
    let saved = store
        .save(&RecentProjectState::default())
        .expect("the live file still saves");

    assert_eq!(saved, RecentProjectSave::Written { backup: false });
    assert_eq!(
        fs::read_to_string(&backup).unwrap(),
        "not json either",
        "the backup is left exactly as it was, not replaced with the empty list"
    );
}

/// §3: whole file or nothing. A backup that cannot be parsed is a reset, and
/// **nothing is salvaged from it** — a half-parsed list can resurrect a wrong
/// path-to-id mapping, and a wrong id re-attaches somebody's trust grant to the
/// wrong folder.
#[test]
fn a_corrupt_backup_is_a_reset_and_nothing_is_salvaged_from_it() {
    let (_sandbox, state_dir, mut store) = store_with_backup("corrupt-backup");
    fs::write(state_dir.join("recent-projects.json"), b"not json").unwrap();
    // Valid JSON, one valid-looking project, and one entry that is not: the
    // shape a salvaging implementation would half-read.
    fs::write(
        state_dir.join("recent-projects.json.bak"),
        br#"{"state_version":1,"projects":[{"project_id":"not-a-uuid"}]}"#,
    )
    .unwrap();

    let loaded = store.load_or_recover();

    assert!(
        loaded.state.projects.is_empty(),
        "nothing is taken from a backup that did not parse whole: {:?}",
        loaded.state
    );
    assert!(matches!(
        loaded.outcome,
        RecentProjectLoadOutcome::Reset { .. }
    ));
}

/// A normal start is the common case, and it must stay silent: loaded, with
/// nothing moved anywhere.
#[test]
fn a_readable_live_file_loads_and_reports_nothing_moved() {
    let (_sandbox, _state_dir, mut store) = store_with_backup("normal-start");
    let saved = state_with_one_project();
    store.save(&saved).unwrap();

    let loaded = store.load_or_recover();

    assert_eq!(loaded.state, saved);
    assert_eq!(loaded.outcome, RecentProjectLoadOutcome::Loaded);
    assert_eq!(loaded.outcome.moved_to(), None);
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
        WorkspaceTrust::Restricted,
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
