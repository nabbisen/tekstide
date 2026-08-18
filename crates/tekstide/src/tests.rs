//! RFC-031 PR-031-B: `project_added`'s reachability and restore-vs-add
//! discrimination, proven against `open_cli_project_path_and_record` --
//! the real logic `boot()`'s CLI-argument loop reaches -- rather than
//! against `record_project_added_if_possible` directly, so a real
//! `AddProjectOutcome::Added`/`FocusedExisting` distinction from a real
//! `ApplicationShell` is what gates the record, not an assumption about
//! it.

use std::path::PathBuf;

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
