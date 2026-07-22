use crate::audit::{
    AuditActionKind, AuditAppendOutcome, AuditEventFamily, AuditOutcome, AuditQuery, AuditStore,
    AuditStoreErrorReason,
};
use crate::domain::{AuditEventId, AuditOperationId};
use crate::project::ProjectId;

use super::super::support::{
    TestAuditDirs, managed_authorized, managed_failed, managed_started, managed_terminated,
    trust_applied, trust_authorized,
};

#[test]
fn correlated_outcome_requires_matching_authorization() {
    let dirs = TestAuditDirs::new("correlation-matching");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let authorization = trust_authorized(ProjectId::new_uuid(), AuditOperationId::new_uuid());
    let outcome = trust_applied(&authorization);

    assert_eq!(
        store.append(&outcome).unwrap_err().reason,
        AuditStoreErrorReason::MissingAuthorization
    );
    store.append(&authorization).unwrap();
    assert!(matches!(
        store.append(&outcome).unwrap(),
        AuditAppendOutcome::Appended { .. }
    ));
}

#[test]
fn operation_rejects_cross_project_wrong_family_and_duplicate_authorization() {
    let dirs = TestAuditDirs::new("correlation-conflicts");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let operation_id = AuditOperationId::new_uuid();
    let authorization = trust_authorized(ProjectId::new_uuid(), operation_id.clone());
    store.append(&authorization).unwrap();

    let mut cross_project = trust_applied(&authorization);
    cross_project.project_id = Some(ProjectId::new_uuid());
    assert_eq!(
        store.append(&cross_project).unwrap_err().reason,
        AuditStoreErrorReason::OperationConflict
    );

    let mut wrong_family = managed_authorized(
        authorization.project_id.clone().unwrap(),
        operation_id.clone(),
    );
    wrong_family.outcome = AuditOutcome::Started;
    wrong_family.actor_kind = crate::audit::AuditActorKind::Runtime;
    wrong_family.action_source = crate::audit::AuditActionSource::RuntimeObserver;
    wrong_family.terminal_id = Some(crate::domain::TerminalId::new_uuid());
    assert_eq!(
        store.append(&wrong_family).unwrap_err().reason,
        AuditStoreErrorReason::OperationConflict
    );

    let mut duplicate_authorization = authorization.clone();
    duplicate_authorization.event_id = AuditEventId::new_uuid();
    assert_eq!(
        store.append(&duplicate_authorization).unwrap_err().reason,
        AuditStoreErrorReason::OperationConflict
    );
    assert_eq!(
        store.query(&AuditQuery::latest(10)).unwrap().records.len(),
        1
    );
}

#[test]
fn managed_lifecycle_enforces_start_failure_and_termination_order() {
    let dirs = TestAuditDirs::new("managed-phase-order");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let authorization = managed_authorized(ProjectId::new_uuid(), AuditOperationId::new_uuid());
    let started = managed_started(&authorization);
    let terminated = managed_terminated(&started);
    let failed = managed_failed(&authorization);
    store.append(&authorization).unwrap();

    assert_eq!(
        store.append(&terminated).unwrap_err().reason,
        AuditStoreErrorReason::PhaseConflict
    );
    store.append(&started).unwrap();
    assert_eq!(
        store.append(&failed).unwrap_err().reason,
        AuditStoreErrorReason::PhaseConflict
    );
    store.append(&terminated).unwrap();

    let mut duplicate_termination = terminated.clone();
    duplicate_termination.event_id = AuditEventId::new_uuid();
    assert_eq!(
        store.append(&duplicate_termination).unwrap_err().reason,
        AuditStoreErrorReason::PhaseConflict
    );
    assert_eq!(
        store.query(&AuditQuery::latest(10)).unwrap().records.len(),
        3
    );
}

#[test]
fn failed_launch_is_terminal_and_exact_phase_retry_is_idempotent() {
    let dirs = TestAuditDirs::new("managed-failed-terminal");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let authorization = managed_authorized(ProjectId::new_uuid(), AuditOperationId::new_uuid());
    let failed = managed_failed(&authorization);
    store.append(&authorization).unwrap();
    store.append(&failed).unwrap();
    assert!(matches!(
        store.append(&failed).unwrap(),
        AuditAppendOutcome::AlreadyPresent { .. }
    ));

    let started = managed_started(&authorization);
    assert_eq!(
        store.append(&started).unwrap_err().reason,
        AuditStoreErrorReason::PhaseConflict
    );
}

#[test]
fn interleaved_operations_correlate_without_adjacency() {
    let dirs = TestAuditDirs::new("correlation-interleaved");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let first = trust_authorized(ProjectId::new_uuid(), AuditOperationId::new_uuid());
    let second = trust_authorized(ProjectId::new_uuid(), AuditOperationId::new_uuid());
    store.append(&first).unwrap();
    store.append(&second).unwrap();
    store.append(&trust_applied(&first)).unwrap();
    store.append(&trust_applied(&second)).unwrap();

    assert_eq!(
        store.query(&AuditQuery::latest(10)).unwrap().records.len(),
        4
    );
}

#[test]
fn authorization_without_outcome_remains_truthful_after_reopen() {
    let dirs = TestAuditDirs::new("authorization-incomplete");
    let authorization = managed_authorized(ProjectId::new_uuid(), AuditOperationId::new_uuid());
    {
        let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
        store.append(&authorization).unwrap();
    }

    let store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let page = store.query(&AuditQuery::latest(10)).unwrap();

    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].record.outcome, AuditOutcome::Authorized);
    assert_eq!(
        page.records[0].record.family,
        AuditEventFamily::ManagedProcessLifecycle
    );
    assert_eq!(
        page.records[0].record.action_kind,
        AuditActionKind::ManagedAgentLaunch
    );
}
