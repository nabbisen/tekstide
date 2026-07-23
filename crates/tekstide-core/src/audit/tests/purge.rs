use std::fs;

use crate::audit::purge::summarize_local_data_with_limit;
use crate::audit::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily,
    AuditJournalCleanupStatus, AuditLocalDataScanStatus, AuditOutcome, AuditPurgeScope, AuditQuery,
    AuditReasonCode, AuditReference, AuditStore, AuditSubjectKind, DurableAuditRecordV1,
};
use crate::project::ProjectId;

use super::support::{TestAuditDirs, project_added};

#[test]
fn project_purge_is_scoped_idempotent_and_ephemeral() {
    let dirs = TestAuditDirs::new("purge-project");
    let target = ProjectId::new_uuid();
    let retained = ProjectId::new_uuid();
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    store.append(&project_added(target.clone())).unwrap();
    store.append(&project_added(target.clone())).unwrap();
    store.append(&project_added(retained.clone())).unwrap();
    let mut global = DurableAuditRecordV1::new(
        AuditEventFamily::AuditStoreRecovery,
        AuditOutcome::Completed,
        AuditActionKind::AuditStoreRecovery,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    global.subject_kind = Some(AuditSubjectKind::RecoveryBundle);
    global.subject_ref = AuditReference::new("retained-global-bundle");
    global.reason_code = Some(AuditReasonCode::RecoveryCompleted);
    store.append(&global).unwrap();

    let first = store.purge_project_records(&target).unwrap();
    assert_eq!(first.scope, AuditPurgeScope::Project);
    assert_eq!(first.deleted_record_count, 2);
    assert_eq!(first.journal_cleanup, AuditJournalCleanupStatus::Completed);
    let second = store.purge_project_records(&target).unwrap();
    assert_eq!(second.deleted_record_count, 0);

    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .any(|record| record.record.project_id.as_ref() == Some(&retained))
    );
    assert!(
        records
            .iter()
            .any(|record| record.record.project_id.is_none())
    );
}

#[test]
fn global_purge_removes_all_rows_without_durable_receipt() {
    let dirs = TestAuditDirs::new("purge-global");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    store.append(&project_added(ProjectId::new_uuid())).unwrap();
    store.append(&project_added(ProjectId::new_uuid())).unwrap();

    let first = store.purge_all_records().unwrap();
    assert_eq!(first.scope, AuditPurgeScope::Global);
    assert_eq!(first.deleted_record_count, 2);
    assert!(
        store
            .query(&AuditQuery::latest(10))
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(store.purge_all_records().unwrap().deleted_record_count, 0);
}

#[test]
fn purge_preserves_unrelated_local_data_and_recovery_evidence() {
    let dirs = TestAuditDirs::new("purge-preserves-local-data");
    let project_file = dirs.base.join("project/source.txt");
    let transcript_file = dirs.base.join("state/transcripts/session.bin");
    let recent_file = dirs.base.join("state/recent-projects.json");
    let config_file = dirs.base.join("state/config.json");
    let recovery_file = dirs
        .storage_path
        .recovery_dir()
        .join("retained-bundle/audit.sqlite3");
    for (path, bytes) in [
        (&project_file, b"project".as_slice()),
        (&transcript_file, b"transcript".as_slice()),
        (&recent_file, b"recent".as_slice()),
        (&config_file, b"config".as_slice()),
        (&recovery_file, b"recovery-evidence".as_slice()),
    ] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    store.append(&project_added(ProjectId::new_uuid())).unwrap();
    store.purge_all_records().unwrap();

    for (path, expected) in [
        (&project_file, b"project".as_slice()),
        (&transcript_file, b"transcript".as_slice()),
        (&recent_file, b"recent".as_slice()),
        (&config_file, b"config".as_slice()),
        (&recovery_file, b"recovery-evidence".as_slice()),
    ] {
        assert_eq!(fs::read(path).unwrap(), expected);
    }
}

#[test]
fn local_data_summary_counts_database_companions_and_recovery_files() {
    let dirs = TestAuditDirs::new("purge-local-summary");
    let recovery_dir = dirs.storage_path.recovery_dir().join("bundle");
    fs::create_dir_all(&recovery_dir).unwrap();
    fs::write(recovery_dir.join("audit.sqlite3"), b"12345").unwrap();
    fs::write(recovery_dir.join("manifest.json"), b"123").unwrap();

    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    store.append(&project_added(ProjectId::new_uuid())).unwrap();
    let summary = store.local_data_summary().unwrap();

    assert_eq!(summary.retained_record_count, 1);
    assert!(summary.database_bytes > 0);
    assert_eq!(summary.rollback_journal_bytes, 0);
    assert!(summary.wal_bytes > 0);
    assert!(summary.shared_memory_bytes > 0);
    assert_eq!(summary.recovery_bytes, 8);
    assert_eq!(summary.recovery_artifact_count, 2);
    assert_eq!(summary.scan_status, AuditLocalDataScanStatus::Complete);
    assert_eq!(
        summary.total_bytes,
        summary.database_bytes
            + summary.rollback_journal_bytes
            + summary.wal_bytes
            + summary.shared_memory_bytes
            + summary.recovery_bytes
    );

    let bounded = summarize_local_data_with_limit(&store, 1).unwrap();
    assert_eq!(
        bounded.scan_status,
        AuditLocalDataScanStatus::EntryLimitReached
    );
}

#[test]
fn purge_checkpoints_and_truncates_wal_without_claiming_erasure() {
    let dirs = TestAuditDirs::new("purge-journal-cleanup");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    store.append(&project_added(ProjectId::new_uuid())).unwrap();
    assert!(fs::metadata(dirs.storage_path.wal_file()).unwrap().len() > 0);

    let receipt = store.purge_all_records().unwrap();

    assert_eq!(
        receipt.journal_cleanup,
        AuditJournalCleanupStatus::Completed
    );
    assert_eq!(fs::metadata(dirs.storage_path.wal_file()).unwrap().len(), 0);
    assert!(dirs.storage_path.database_file().exists());
}

#[test]
fn purge_reports_deferred_cleanup_while_wal_reader_is_active() {
    let dirs = TestAuditDirs::new("purge-journal-deferred");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    store.append(&project_added(ProjectId::new_uuid())).unwrap();
    let reader = rusqlite::Connection::open(dirs.storage_path.database_file()).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    assert_eq!(
        reader
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );

    let deferred = store.purge_all_records().unwrap();
    assert_eq!(
        deferred.journal_cleanup,
        AuditJournalCleanupStatus::Deferred
    );
    reader.execute_batch("COMMIT").unwrap();

    let completed = store.purge_all_records().unwrap();
    assert_eq!(
        completed.journal_cleanup,
        AuditJournalCleanupStatus::Completed
    );
}
