use crate::audit::AuditStore;
use crate::domain::AuditOperationId;
use crate::project::ProjectId;

use super::super::support::{TestAuditDirs, trust_applied, trust_authorized, trust_revoked};

/// RFC-032 PR-032-C, response 245: the positive case -- a real grant,
/// written the same two-phase way `AuditCoordinator::grant_project_trust`
/// does (`Authorized` then `Applied`, same operation), is confirmed.
#[test]
fn has_applied_trust_grant_is_true_after_a_real_grant() {
    let dirs = TestAuditDirs::new("trust-grant-confirmed");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let project_id = ProjectId::new_uuid();
    let authorization = trust_authorized(project_id.clone(), AuditOperationId::new_uuid());
    store.append(&authorization).unwrap();
    store.append(&trust_applied(&authorization)).unwrap();

    assert_eq!(store.has_applied_trust_grant(&project_id), Ok(true));
}

#[test]
fn has_applied_trust_grant_is_false_with_no_records_at_all() {
    let dirs = TestAuditDirs::new("trust-grant-no-records");
    let store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let project_id = ProjectId::new_uuid();

    assert_eq!(store.has_applied_trust_grant(&project_id), Ok(false));
}

/// The property PR-032-C's whole audit-authority mechanism depends on:
/// a later revoke must supersede an earlier grant, not merely coexist
/// with it -- the newest `TrustChange` row decides, not "any matching
/// grant ever."
#[test]
fn has_applied_trust_grant_is_false_after_a_later_revoke() {
    let dirs = TestAuditDirs::new("trust-grant-revoked");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let project_id = ProjectId::new_uuid();
    let authorization = trust_authorized(project_id.clone(), AuditOperationId::new_uuid());
    store.append(&authorization).unwrap();
    store.append(&trust_applied(&authorization)).unwrap();
    assert_eq!(
        store.has_applied_trust_grant(&project_id),
        Ok(true),
        "test precondition: the grant must be confirmed before the revoke is appended"
    );

    store.append(&trust_revoked(project_id.clone())).unwrap();

    assert_eq!(store.has_applied_trust_grant(&project_id), Ok(false));
}

/// An authorization with no matching applied record -- what
/// `grant_project_trust`'s own fail-closed `append_required` should
/// prevent in practice, but this query does not assume that; an
/// authorization alone is a grant that never actually completed.
#[test]
fn has_applied_trust_grant_is_false_for_an_authorization_with_no_applied_record() {
    let dirs = TestAuditDirs::new("trust-grant-authorized-only");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let project_id = ProjectId::new_uuid();
    store
        .append(&trust_authorized(
            project_id.clone(),
            AuditOperationId::new_uuid(),
        ))
        .unwrap();

    assert_eq!(store.has_applied_trust_grant(&project_id), Ok(false));
}

/// **Response 246's required correction, as its own regression test.**
/// A completed grant, then a *later* re-grant attempt (a retry, a
/// double action) that writes its `Authorized` record and is
/// interrupted before its `Applied` one -- the newest row is now a
/// dangling authorization, but the project was and still is
/// legitimately trusted from the earlier, completed grant. Querying the
/// newest row of *any* outcome would answer `false` here and silently
/// demote a real, standing trust decision; querying the newest
/// **`Applied`** row answers the question that actually matters ("what
/// was the last completed decision") and correctly still says `true`.
#[test]
fn has_applied_trust_grant_survives_a_later_interrupted_re_grant() {
    let dirs = TestAuditDirs::new("trust-grant-interrupted-regrant");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let project_id = ProjectId::new_uuid();

    let first_authorization = trust_authorized(project_id.clone(), AuditOperationId::new_uuid());
    store.append(&first_authorization).unwrap();
    store.append(&trust_applied(&first_authorization)).unwrap();
    assert_eq!(
        store.has_applied_trust_grant(&project_id),
        Ok(true),
        "test precondition: the first grant must have completed"
    );

    // A later re-grant, interrupted after its own Authorized write --
    // the process died, or was retried, before the matching Applied
    // record landed. This is now the newest TrustChange row of any
    // outcome, but it is not a completed decision.
    let interrupted_reauthorization =
        trust_authorized(project_id.clone(), AuditOperationId::new_uuid());
    store.append(&interrupted_reauthorization).unwrap();

    assert_eq!(
        store.has_applied_trust_grant(&project_id),
        Ok(true),
        "a dangling authorization following a completed grant must not silently undo it"
    );
}

/// The query is project-scoped -- a real grant recorded for a
/// *different* project must not confirm this one.
#[test]
fn has_applied_trust_grant_does_not_leak_across_projects() {
    let dirs = TestAuditDirs::new("trust-grant-cross-project");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let granted_project = ProjectId::new_uuid();
    let other_project = ProjectId::new_uuid();
    let authorization = trust_authorized(granted_project.clone(), AuditOperationId::new_uuid());
    store.append(&authorization).unwrap();
    store.append(&trust_applied(&authorization)).unwrap();

    assert_eq!(store.has_applied_trust_grant(&other_project), Ok(false));
    assert_eq!(store.has_applied_trust_grant(&granted_project), Ok(true));
}
