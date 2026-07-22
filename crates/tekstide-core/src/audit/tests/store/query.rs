use crate::audit::{
    AuditEventFamily, AuditOutcome, AuditQuery, AuditStore, AuditStoreErrorReason,
    MAX_AUDIT_QUERY_LIMIT,
};
use crate::domain::AuditOperationId;
use crate::project::ProjectId;

use super::super::support::{TestAuditDirs, project_added, trust_authorized};

#[test]
fn query_is_descending_bounded_and_cursor_stable() {
    let dirs = TestAuditDirs::new("query-cursor");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let project_id = ProjectId::new_uuid();
    for _ in 0..3 {
        store.append(&project_added(project_id.clone())).unwrap();
    }

    let first = store.query(&AuditQuery::latest(2)).unwrap();
    assert_eq!(first.records.len(), 2);
    assert!(first.records[0].sequence > first.records[1].sequence);
    let cursor = first.next_before_sequence.unwrap();

    let mut next_query = AuditQuery::latest(2);
    next_query.before_sequence = Some(cursor);
    let second = store.query(&next_query).unwrap();
    assert_eq!(second.records.len(), 1);
    assert!(second.records[0].sequence < cursor);
}

#[test]
fn query_filters_project_family_outcome_and_operation() {
    let dirs = TestAuditDirs::new("query-filters");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let project_one = ProjectId::new_uuid();
    let project_two = ProjectId::new_uuid();
    let operation_id = AuditOperationId::new_uuid();
    store.append(&project_added(project_one.clone())).unwrap();
    store
        .append(&trust_authorized(project_two.clone(), operation_id.clone()))
        .unwrap();

    let mut query = AuditQuery::latest(10);
    query.project_id = Some(project_two);
    query.family = Some(AuditEventFamily::TrustChange);
    query.outcome = Some(AuditOutcome::Authorized);
    query.operation_id = Some(operation_id);
    let page = store.query(&query).unwrap();

    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].record.family, AuditEventFamily::TrustChange);
}

#[test]
fn query_rejects_zero_and_excessive_limits() {
    let dirs = TestAuditDirs::new("query-limits");
    let store = AuditStore::open(dirs.storage_path.clone()).unwrap();

    assert_eq!(
        store.query(&AuditQuery::latest(0)).unwrap_err().reason,
        AuditStoreErrorReason::InvalidQuery
    );
    assert_eq!(
        store
            .query(&AuditQuery::latest(MAX_AUDIT_QUERY_LIMIT + 1))
            .unwrap_err()
            .reason,
        AuditStoreErrorReason::InvalidQuery
    );
}

#[test]
fn undecodable_persisted_row_fails_the_page_without_claiming_database_corruption() {
    let dirs = TestAuditDirs::new("query-decode-failure");
    let record = project_added(ProjectId::new_uuid());
    {
        let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
        store.append(&record).unwrap();
    }

    let connection = rusqlite::Connection::open(dirs.storage_path.database_file()).unwrap();
    connection
        .execute(
            "UPDATE audit_events SET created_at = 'invalid-timestamp' WHERE event_id = ?1",
            [record.event_id.as_str()],
        )
        .unwrap();
    drop(connection);

    let store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    assert_eq!(
        store.query(&AuditQuery::latest(10)).unwrap_err().reason,
        AuditStoreErrorReason::DecodeFailed
    );
}
