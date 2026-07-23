use std::fs;

use rusqlite::Connection;

use crate::audit::{
    AuditDiagnosticStatus, AuditDiagnostics, AuditQuery, AuditStore, AuditStoreErrorReason,
};
use crate::project::ProjectId;

use super::support::{TestAuditDirs, project_added};

#[test]
fn diagnostics_distinguish_missing_healthy_and_semantically_invalid_records() {
    let missing = TestAuditDirs::new("diagnostics-missing");
    assert_eq!(
        AuditDiagnostics.run(&missing.storage_path).status,
        AuditDiagnosticStatus::Missing
    );

    let healthy = TestAuditDirs::new("diagnostics-healthy");
    {
        let mut store = AuditStore::open(healthy.storage_path.clone()).unwrap();
        store.append(&project_added(ProjectId::new_uuid())).unwrap();
    }
    assert_eq!(
        AuditDiagnostics.run(&healthy.storage_path).status,
        AuditDiagnosticStatus::Healthy
    );

    let invalid = TestAuditDirs::new("diagnostics-invalid-record");
    let record = project_added(ProjectId::new_uuid());
    {
        let mut store = AuditStore::open(invalid.storage_path.clone()).unwrap();
        store.append(&record).unwrap();
    }
    let connection = Connection::open(invalid.storage_path.database_file()).unwrap();
    connection
        .execute(
            "UPDATE audit_events SET created_at = 'invalid-timestamp' WHERE event_id = ?1",
            [record.event_id.as_str()],
        )
        .unwrap();
    drop(connection);
    assert_eq!(
        AuditDiagnostics.run(&invalid.storage_path).status,
        AuditDiagnosticStatus::InvalidRecords
    );
    let store = AuditStore::open(invalid.storage_path.clone()).unwrap();
    assert_eq!(
        store.query(&AuditQuery::latest(10)).unwrap_err().reason,
        AuditStoreErrorReason::DecodeFailed
    );
}

#[test]
fn malformed_and_truncated_stores_are_corrupt_and_ordinary_open_preserves_evidence() {
    let malformed = TestAuditDirs::new("diagnostics-malformed");
    fs::create_dir_all(malformed.storage_path.audit_dir()).unwrap();
    fs::write(
        malformed.storage_path.database_file(),
        b"not a sqlite database: private-sentinel",
    )
    .unwrap();
    assert_corrupt_open_is_nonmutating(&malformed);

    let truncated = TestAuditDirs::new("diagnostics-truncated");
    {
        let store = AuditStore::open(truncated.storage_path.clone()).unwrap();
        drop(store);
    }
    let connection = Connection::open(truncated.storage_path.database_file()).unwrap();
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .unwrap();
    drop(connection);
    let mut bytes = fs::read(truncated.storage_path.database_file()).unwrap();
    bytes.truncate(bytes.len() / 2);
    fs::write(truncated.storage_path.database_file(), bytes).unwrap();
    assert_corrupt_open_is_nonmutating(&truncated);
}

#[test]
fn valid_identity_with_missing_required_table_is_classified_as_corrupt() {
    let dirs = TestAuditDirs::new("diagnostics-missing-table");
    fs::create_dir_all(dirs.storage_path.audit_dir()).unwrap();
    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    connection
        .execute_batch("PRAGMA application_id = 1414218069; PRAGMA user_version = 1;")
        .unwrap();
    drop(connection);

    assert_corrupt_open_is_nonmutating(&dirs);
}

fn assert_corrupt_open_is_nonmutating(dirs: &TestAuditDirs) {
    let before = artifact_snapshot(dirs);
    assert_eq!(
        AuditDiagnostics.run(&dirs.storage_path).status,
        AuditDiagnosticStatus::Corrupt
    );
    let error = match AuditStore::open(dirs.storage_path.clone()) {
        Ok(_) => panic!("corrupt store must not open"),
        Err(error) => error,
    };
    assert_eq!(error.reason, AuditStoreErrorReason::Corrupt);
    assert_eq!(artifact_snapshot(dirs), before);
}

fn artifact_snapshot(dirs: &TestAuditDirs) -> Vec<Option<Vec<u8>>> {
    [
        dirs.storage_path.database_file().to_path_buf(),
        dirs.storage_path.journal_file(),
        dirs.storage_path.wal_file(),
        dirs.storage_path.shared_memory_file(),
    ]
    .into_iter()
    .map(|path| fs::read(path).ok())
    .collect()
}
