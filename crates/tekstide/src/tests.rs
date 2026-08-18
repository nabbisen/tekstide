//! RFC-031 PR-031-B: `project_added`'s reachability and restore-vs-add
//! discrimination, proven against `open_cli_project_path_and_record` --
//! the real logic `boot()`'s CLI-argument loop reaches -- rather than
//! against `record_project_added_if_possible` directly, so a real
//! `AddProjectOutcome::Added`/`FocusedExisting` distinction from a real
//! `ApplicationShell` is what gates the record, not an assumption about
//! it.

use std::path::{Path, PathBuf};

use tekstide_core::audit::{AuditEventFamily, AuditQuery};
use tekstide_core::project::ProjectId;
use tekstide_core::shell::ApplicationShell;

use super::{open_cli_project_path_and_record, shell};

fn fresh_project_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tekstide-main-test-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn project_added_record_count(app_shell: &ApplicationShell, project_id: &ProjectId) -> usize {
    let audit_store =
        shell::open_real_audit_store(app_shell).expect("the real audit store must open");
    audit_store
        .query(&AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| record.project_id.as_ref() == Some(project_id))
        .filter(|record| record.family == AuditEventFamily::ProjectAdded)
        .count()
}

#[test]
fn opening_a_real_new_project_from_the_cli_path_writes_exactly_one_real_project_added_record() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("project-added-reachability");

    open_cli_project_path_and_record(&mut app_shell, &project_dir)
        .expect("a freshly created directory is a valid project root");
    let project_id = app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("adding a project must make it active");

    let audit_store =
        shell::open_real_audit_store(&app_shell).expect("the real audit store must open");
    let records: Vec<_> = audit_store
        .query(&AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| record.project_id.as_ref() == Some(&project_id))
        .filter(|record| record.family == AuditEventFamily::ProjectAdded)
        .collect();

    assert_eq!(
        records.len(),
        1,
        "exactly one ProjectAdded record must exist for a real, genuinely new project open: \
         {records:?}"
    );
    let record = &records[0];
    assert_eq!(
        record.subject_ref, None,
        "what-the-store-may-hold.md: no path-shaped text belongs in this record"
    );
    assert_eq!(record.outcome, tekstide_core::audit::AuditOutcome::Applied);
    assert_eq!(
        record.action_kind,
        tekstide_core::audit::AuditActionKind::ProjectAdd
    );
}

#[test]
fn reopening_the_same_project_path_focuses_it_instead_of_writing_a_second_record() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("project-added-focus-existing");

    open_cli_project_path_and_record(&mut app_shell, &project_dir)
        .expect("a freshly created directory is a valid project root");
    let project_id = app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("adding a project must make it active");
    assert_eq!(project_added_record_count(&app_shell, &project_id), 1);

    // Re-opening the exact same path a second time is what
    // `AddProjectOutcome::FocusedExisting` reflects -- nothing new
    // happened, so no second record may appear.
    open_cli_project_path_and_record(&mut app_shell, &project_dir)
        .expect("re-selecting an already-open project root must still succeed");

    assert_eq!(
        project_added_record_count(&app_shell, &project_id),
        1,
        "re-focusing an already-open project must not write a second ProjectAdded record"
    );
}

#[test]
fn restoring_recent_projects_on_boot_writes_no_project_added_record() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("project-added-restore-vs-add");

    open_cli_project_path_and_record(&mut app_shell, &project_dir)
        .expect("a freshly created directory is a valid project root");
    let project_id = app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("adding a project must make it active");
    assert_eq!(project_added_record_count(&app_shell, &project_id), 1);

    let recent_project_state = app_shell.recent_project_state();

    // A fresh boot, with no CLI arguments, only ever calls
    // `restore_recent_projects` -- never `add_project_from_path` -- so
    // this is the real shape `boot()` reaches when a project is merely
    // remembered, not newly opened.
    let mut restored_app_shell = ApplicationShell::new();
    restored_app_shell.restore_recent_projects(recent_project_state);

    assert_eq!(
        project_added_record_count(&restored_app_shell, &project_id),
        1,
        "restoring recent projects on boot must not write an additional ProjectAdded record; \
         the count must still be exactly the one written by the original real open"
    );
}

/// Response 263's required follow-up: `record_project_added_if_possible` is
/// called from the call site (`open_cli_project_path_and_record`), not
/// from `add_project_from_path` itself -- `AppState` holds no
/// `AuditCoordinator`, so the operation and the record cannot live
/// together the way `grant_project_trust`'s do. That makes auditing a
/// thing a future caller must remember: an interactive "Add Project"
/// flow would compile and work with no record and no error. This test
/// enumerates every production caller of `add_project_from_path` in this
/// crate and fails by name the moment a second one appears, the same
/// shape `only_this_module_opens_a_transcript_file_for_reading`
/// established.
const FILES_ALLOWED_TO_CALL_ADD_PROJECT_FROM_PATH: &[&str] = &["main.rs"];

fn crate_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("crate src dir must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn only_boot_calls_add_project_from_path_so_a_new_caller_cannot_silently_skip_the_audit_record() {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    for path in files {
        let relative = path
            .strip_prefix(crate_src_dir())
            .expect("file must be under src/")
            .to_str()
            .expect("path must be valid UTF-8")
            .to_string();

        if relative.contains("/tests/") || relative.ends_with("tests.rs") {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        let calls_add_project_from_path = source.contains(".add_project_from_path(");
        let is_allowed = FILES_ALLOWED_TO_CALL_ADD_PROJECT_FROM_PATH.contains(&relative.as_str());

        assert!(
            !calls_add_project_from_path || is_allowed,
            "{relative} calls add_project_from_path but is not in \
             FILES_ALLOWED_TO_CALL_ADD_PROJECT_FROM_PATH -- a new call site (for example an \
             interactive Add Project flow) must wire its own audit record deliberately, either \
             by reusing open_cli_project_path_and_record or by calling \
             record_project_added_if_possible itself, not add a project with no record"
        );
    }
}
