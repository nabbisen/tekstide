use std::fs;

use crate::audit::{AuditAppendOutcome, AuditQuery, AuditStore, AuditStoreErrorReason};
use crate::domain::{AuditEventId, AuditOperationId, DomainTimestamp};
use crate::project::ProjectId;

use super::super::support::{TestAuditDirs, managed_authorized, project_added};

#[test]
fn fresh_store_appends_and_reopens_durable_records() {
    let dirs = TestAuditDirs::new("append-reopen");
    let record = project_added(ProjectId::new_uuid());
    let sequence = {
        let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
        let AuditAppendOutcome::Appended { sequence } = store.append(&record).unwrap() else {
            panic!("first append must create a row");
        };
        sequence
    };

    let reopened = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let page = reopened.query(&AuditQuery::latest(10)).unwrap();

    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].sequence, sequence);
    assert_eq!(page.records[0].record, record);
}

#[test]
fn exact_retry_is_idempotent_and_conflicting_event_reuse_is_rejected() {
    let dirs = TestAuditDirs::new("append-idempotent");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let record = project_added(ProjectId::new_uuid());

    let first = store.append(&record).unwrap();
    let retry = store.append(&record).unwrap();
    assert_eq!(
        retry,
        AuditAppendOutcome::AlreadyPresent {
            sequence: match first {
                AuditAppendOutcome::Appended { sequence } => sequence,
                AuditAppendOutcome::AlreadyPresent { .. } => unreachable!(),
            }
        }
    );

    let mut conflict = record.clone();
    conflict.created_at = DomainTimestamp::from_utc_string("2026-07-22T00:00:00Z").unwrap();
    assert_eq!(
        store.append(&conflict).unwrap_err().reason,
        AuditStoreErrorReason::DuplicateEventConflict
    );
    assert_eq!(
        store.query(&AuditQuery::latest(10)).unwrap().records.len(),
        1
    );
}

#[test]
fn invalid_record_is_rejected_before_sensitive_values_reach_sqlite_files() {
    let dirs = TestAuditDirs::new("append-private-sentinel");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let sentinel = "secret command --token private-value";
    let mut invalid = project_added(ProjectId::new_uuid());
    invalid.subject_kind = Some(crate::audit::AuditSubjectKind::AppResource);
    invalid.subject_ref = crate::audit::AuditReference::new(sentinel);

    assert_eq!(
        store.append(&invalid).unwrap_err().reason,
        AuditStoreErrorReason::InvalidRecord
    );
    drop(store);

    for artifact in [
        dirs.storage_path.database_file().to_path_buf(),
        dirs.storage_path
            .database_file()
            .with_extension("sqlite3-wal"),
        dirs.storage_path
            .database_file()
            .with_extension("sqlite3-shm"),
    ] {
        if let Ok(bytes) = fs::read(artifact) {
            assert!(
                !bytes
                    .windows(sentinel.len())
                    .any(|window| window == sentinel.as_bytes())
            );
        }
    }
}

#[test]
fn sqlite_constraints_reject_invalid_shapes_references_and_duplicate_authorizations() {
    let dirs = TestAuditDirs::new("append-database-constraints");
    let authorization = managed_authorized(ProjectId::new_uuid(), AuditOperationId::new_uuid());
    {
        let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
        store.append(&authorization).unwrap();
    }

    let connection = rusqlite::Connection::open(dirs.storage_path.database_file()).unwrap();
    assert!(
        connection
            .execute(
                "UPDATE audit_events SET family = 'project_added' WHERE event_id = ?1",
                [authorization.event_id.as_str()],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE audit_events SET adapter_profile_ref = '/private/path' WHERE event_id = ?1",
                [authorization.event_id.as_str()],
            )
            .is_err()
    );

    let duplicate_event_id = AuditEventId::new_uuid();
    assert!(
        connection
            .execute(
                r#"
                INSERT INTO audit_events (
                    event_id, schema_version, project_id, family, outcome, operation_id,
                    terminal_id, agent_run_id, approval_id, subject_kind, subject_ref,
                    action_kind, risk_level, actor_kind, action_source, adapter_profile_ref,
                    reason_code, created_at
                )
                SELECT ?1, schema_version, project_id, family, outcome, operation_id,
                       terminal_id, agent_run_id, approval_id, subject_kind, subject_ref,
                       action_kind, risk_level, actor_kind, action_source, adapter_profile_ref,
                       reason_code, created_at
                FROM audit_events WHERE event_id = ?2
                "#,
                [duplicate_event_id.as_str(), authorization.event_id.as_str()],
            )
            .is_err()
    );
}
