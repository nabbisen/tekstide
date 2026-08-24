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
/// flow would compile and work with no record and no error.
///
/// Response 264's correction: the guarded property is "every *call* to
/// `add_project_from_path` writes an audit record," so the unit an
/// allow-list checks must be the call, not the file -- a per-file
/// presence check (as `only_this_module_opens_a_transcript_file_for_reading`
/// correctly uses for *its own*, different property) would let a second,
/// unreviewed call added inside `main.rs` itself pass silently. This test
/// instead asserts the exact call count per file: exactly one in each
/// allow-listed file, zero everywhere else -- so a second call anywhere,
/// an allow-listed file included, fails.
///
/// RFC-038 PR-038-A widened this from one file to two: `shell.rs`'s
/// `attempt_open_project_from_path_field` is the field's own real call
/// site, wiring `record_new_project_added` directly rather than
/// reusing `main.rs`'s `open_cli_project_path_and_record` (whose caller,
/// `boot()`, exits on `Err` -- catastrophic reached from a text field,
/// per `what-a-path-field-must-not-trust.md` §2). Kept a `HashMap` of
/// exact counts, not widened to a presence check, for the same reason
/// response 264 gave: a *third*, unreviewed call added inside either
/// already-allowed file must still fail this test.
///
/// RFC-038 PR-038-G widened `shell.rs`'s own count from one to two:
/// `choose_current_browsed_directory` is the folder browser's real call
/// site, the same "wire the record deliberately" shape
/// `attempt_open_project_from_path_field` already established, not a
/// second, unreviewed one -- both are named explicitly below rather
/// than the file simply reading `2`, so a reviewer checking this list
/// against the source does not have to first find both call sites
/// themselves.
///
/// RFC-038 PR-038-D widened `shell.rs`'s own count from two to three:
/// `reopen_recent_project` is the one-key-reopen feature's own real
/// call site, the same shape again -- named explicitly below with the
/// other two.
fn files_with_one_allowed_call_to_add_project_from_path()
-> std::collections::HashMap<&'static str, usize> {
    // main.rs: boot()'s CLI-argument loop, via open_cli_project_path_and_record.
    // shell.rs: attempt_open_project_from_path_field (the path field),
    //           choose_current_browsed_directory (the folder browser),
    //           and reopen_recent_project (the one-key reopen).
    std::collections::HashMap::from([("main.rs", 1), ("shell.rs", 3)])
}

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
fn add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else() {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);
    let allowed = files_with_one_allowed_call_to_add_project_from_path();

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
        let call_count = source.matches(".add_project_from_path(").count();
        let expected_call_count = allowed.get(relative.as_str()).copied().unwrap_or(0);

        assert_eq!(
            call_count, expected_call_count,
            "{relative} calls add_project_from_path {call_count} time(s), expected \
             {expected_call_count} -- every call site must wire its own audit record \
             deliberately, either by reusing open_cli_project_path_and_record or by calling \
             record_project_added_if_possible itself, not add a project with no record"
        );
    }
}
