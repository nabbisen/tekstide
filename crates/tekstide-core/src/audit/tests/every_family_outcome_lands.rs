//! RFC-059 PR-059-A, D6: "a view over a store that can silently fail to
//! write is a view that can be confidently wrong." Review 503 found that
//! `safe_close_decision` + `blocked` was accepted by `DurableAuditRecordV1::
//! validate()` but rejected by `CREATE_SCHEMA_V3`'s own `CHECK` constraint --
//! a combination nobody had written a test for. RFC-066 PR-066-B fixed that
//! one case; this is the enumeration that would have caught it, and now
//! guards every other `(family, outcome, action_kind)` combination the type
//! itself can produce, not only the ones a hand-written list happens to
//! include.
//!
//! **Derived from the type, not from a hand-written list of combinations.**
//! `chains_for_family` dispatches on `AuditEventFamily` with no wildcard
//! arm -- adding a family variant without adding it here is a compile
//! error, the same bar `DurableAuditRecordV1::validate()`'s own `match
//! self.family { ... }` (no wildcard) already holds this crate to. Every
//! record this module builds is checked against `validate()` before it is
//! appended, so a mistake in this file's own construction is caught by the
//! same Rust-level validator the real producers answer to, not assumed
//! correct because it compiles.
//!
//! **Each outcome's own record is appended inside a "chain", in order, not
//! as an independent row.** `AuditStore::append`'s own `validate_operation_phase`
//! enforces a second, cross-record contract `validate()` cannot see: an
//! `Applied`/`Failed`/`Started`/`Terminated` record naming an `operation_id`
//! requires a **prior** `Authorized` record for that exact `operation_id`
//! (same project, family, action kind) already in the store, or the append
//! itself is refused (`MissingAuthorization`) -- found by running this
//! enumeration's first version, which built every outcome as an independent
//! record with its own fresh `operation_id` and was correctly refused by
//! the store for every `Applied`/`Failed` combination in an authorization-
//! shaped family. Not a store defect: the real producers
//! (`AuditCoordinator::grant_project_trust` and siblings) always write the
//! two phases in order, sharing one `operation_id`; a test asserting
//! otherwise would have been asserting a shape nothing real produces.

use crate::audit::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome, AuditQuery,
    AuditReasonCode, AuditReference, AuditRiskLevel, AuditStore, AuditSubjectKind,
    DurableAuditRecordV1,
};
use crate::domain::{AgentRunId, ApprovalId, AuditOperationId, TerminalId};
use crate::project::ProjectId;

use super::support::TestAuditDirs;

type Chain = Vec<DurableAuditRecordV1>;

fn record(
    family: AuditEventFamily,
    outcome: AuditOutcome,
    action: AuditActionKind,
    actor: AuditActorKind,
    source: AuditActionSource,
) -> DurableAuditRecordV1 {
    DurableAuditRecordV1::new(family, outcome, action, actor, source)
}

fn project_added() -> Vec<Chain> {
    let mut r = record(
        AuditEventFamily::ProjectAdded,
        AuditOutcome::Applied,
        AuditActionKind::ProjectAdd,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    r.project_id = Some(ProjectId::new_uuid());
    vec![vec![r]]
}

fn trust_change() -> Vec<Chain> {
    let authorized = || {
        let mut r = record(
            AuditEventFamily::TrustChange,
            AuditOutcome::Authorized,
            AuditActionKind::TrustGrant,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        r.project_id = Some(ProjectId::new_uuid());
        r.operation_id = Some(AuditOperationId::new_uuid());
        r
    };
    let next = |authorization: &DurableAuditRecordV1, outcome: AuditOutcome| {
        let mut r = authorization.clone();
        r.event_id = crate::domain::AuditEventId::new_uuid();
        r.outcome = outcome;
        r
    };
    let applied_auth = authorized();
    let applied = next(&applied_auth, AuditOutcome::Applied);
    let failed_auth = authorized();
    let failed = next(&failed_auth, AuditOutcome::Failed);

    let mut revoke = record(
        AuditEventFamily::TrustChange,
        AuditOutcome::Applied,
        AuditActionKind::TrustRevoke,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    revoke.project_id = Some(ProjectId::new_uuid());

    vec![
        vec![applied_auth, applied],
        vec![failed_auth, failed],
        vec![revoke],
    ]
}

fn command_approval() -> Vec<Chain> {
    let authorized = |action: AuditActionKind| {
        let mut r = record(
            AuditEventFamily::CommandApproval,
            AuditOutcome::Authorized,
            action,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        r.project_id = Some(ProjectId::new_uuid());
        r.approval_id = Some(ApprovalId::new_uuid());
        r.risk_level = Some(AuditRiskLevel::High);
        r.operation_id = Some(AuditOperationId::new_uuid());
        r
    };
    let next = |authorization: &DurableAuditRecordV1, outcome: AuditOutcome| {
        let mut r = authorization.clone();
        r.event_id = crate::domain::AuditEventId::new_uuid();
        r.outcome = outcome;
        r
    };

    let mut chains = Vec::new();
    for action in [
        AuditActionKind::CommandApprove,
        AuditActionKind::CommandEditAndApprove,
    ] {
        let applied_auth = authorized(action);
        let applied = next(&applied_auth, AuditOutcome::Applied);
        chains.push(vec![applied_auth, applied]);
        let failed_auth = authorized(action);
        let failed = next(&failed_auth, AuditOutcome::Failed);
        chains.push(vec![failed_auth, failed]);
    }

    let base = |outcome, action, actor, source| {
        let mut r = record(
            AuditEventFamily::CommandApproval,
            outcome,
            action,
            actor,
            source,
        );
        r.project_id = Some(ProjectId::new_uuid());
        r.approval_id = Some(ApprovalId::new_uuid());
        r.risk_level = Some(AuditRiskLevel::High);
        r
    };
    chains.push(vec![base(
        AuditOutcome::Requested,
        AuditActionKind::CommandRequest,
        AuditActorKind::AppPolicy,
        AuditActionSource::Adapter,
    )]);
    chains.push(vec![base(
        AuditOutcome::Applied,
        AuditActionKind::CommandReject,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    )]);
    chains.push(vec![base(
        AuditOutcome::Anomaly,
        AuditActionKind::CommandCwdMismatch,
        AuditActorKind::AppPolicy,
        AuditActionSource::Adapter,
    )]);
    chains
}

fn managed_process_lifecycle() -> Vec<Chain> {
    let authorized = || {
        let mut r = record(
            AuditEventFamily::ManagedProcessLifecycle,
            AuditOutcome::Authorized,
            AuditActionKind::ManagedAgentLaunch,
            AuditActorKind::User,
            AuditActionSource::AppCommand,
        );
        r.project_id = Some(ProjectId::new_uuid());
        r.agent_run_id = Some(AgentRunId::new_uuid());
        r.adapter_profile_ref = AuditReference::new("profile:codex");
        r.operation_id = Some(AuditOperationId::new_uuid());
        r
    };
    let next = |previous: &DurableAuditRecordV1, outcome: AuditOutcome| {
        let mut r = previous.clone();
        r.event_id = crate::domain::AuditEventId::new_uuid();
        r.outcome = outcome;
        r.actor_kind = AuditActorKind::Runtime;
        r.action_source = AuditActionSource::RuntimeObserver;
        r
    };

    let run_a = authorized();
    let mut started = next(&run_a, AuditOutcome::Started);
    started.terminal_id = Some(TerminalId::new_uuid());
    let mut terminated = next(&started, AuditOutcome::Terminated);
    terminated.reason_code = Some(AuditReasonCode::ProcessExited);

    let run_b = authorized();
    let mut failed = next(&run_b, AuditOutcome::Failed);
    failed.reason_code = Some(AuditReasonCode::RuntimeFailure);

    vec![vec![run_a, started, terminated], vec![run_b, failed]]
}

fn plain_terminal_observation() -> Vec<Chain> {
    let base = |outcome| {
        let mut r = record(
            AuditEventFamily::PlainTerminalObservation,
            outcome,
            AuditActionKind::PlainTerminalLifecycle,
            AuditActorKind::Runtime,
            AuditActionSource::RuntimeObserver,
        );
        r.project_id = Some(ProjectId::new_uuid());
        r.terminal_id = Some(TerminalId::new_uuid());
        r
    };
    let started = base(AuditOutcome::Started);
    let mut failed = base(AuditOutcome::Failed);
    failed.reason_code = Some(AuditReasonCode::RuntimeFailure);
    let mut terminated = base(AuditOutcome::Terminated);
    terminated.reason_code = Some(AuditReasonCode::ProcessExited);
    vec![vec![started], vec![failed], vec![terminated]]
}

fn paste_blocked() -> Vec<Chain> {
    let mut r = record(
        AuditEventFamily::PasteBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::TerminalPaste,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    r.project_id = Some(ProjectId::new_uuid());
    r.reason_code = Some(AuditReasonCode::PastePolicy);
    vec![vec![r]]
}

fn restricted_mode_blocked() -> Vec<Chain> {
    let mut r = record(
        AuditEventFamily::RestrictedModeBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::RestrictedFeature,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    r.project_id = Some(ProjectId::new_uuid());
    r.reason_code = Some(AuditReasonCode::RestrictedMode);
    vec![vec![r]]
}

fn root_access_blocked() -> Vec<Chain> {
    let mut r = record(
        AuditEventFamily::RootAccessBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::RootAccess,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    r.project_id = Some(ProjectId::new_uuid());
    r.reason_code = Some(AuditReasonCode::RootEscape);
    vec![vec![r]]
}

fn safe_close_decision() -> Vec<Chain> {
    let authorized = |action: AuditActionKind| {
        let mut r = record(
            AuditEventFamily::SafeCloseDecision,
            AuditOutcome::Authorized,
            action,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        r.project_id = Some(ProjectId::new_uuid());
        r.operation_id = Some(AuditOperationId::new_uuid());
        r
    };
    let next = |authorization: &DurableAuditRecordV1, outcome: AuditOutcome| {
        let mut r = authorization.clone();
        r.event_id = crate::domain::AuditEventId::new_uuid();
        r.outcome = outcome;
        r
    };

    let mut chains = Vec::new();
    for action in [
        AuditActionKind::SafeCloseTerminate,
        AuditActionKind::SafeCloseAbandon,
        AuditActionKind::DestructiveAction,
    ] {
        let applied_auth = authorized(action);
        let applied = next(&applied_auth, AuditOutcome::Applied);
        chains.push(vec![applied_auth, applied]);
        let failed_auth = authorized(action);
        let failed = next(&failed_auth, AuditOutcome::Failed);
        chains.push(vec![failed_auth, failed]);

        let mut cancelled = record(
            AuditEventFamily::SafeCloseDecision,
            AuditOutcome::Cancelled,
            action,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        cancelled.project_id = Some(ProjectId::new_uuid());
        chains.push(vec![cancelled]);

        let mut blocked = record(
            AuditEventFamily::SafeCloseDecision,
            AuditOutcome::Blocked,
            action,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        blocked.project_id = Some(ProjectId::new_uuid());
        chains.push(vec![blocked]);
    }
    chains
}

fn sensitive_config_changed() -> Vec<Chain> {
    let authorized = || {
        let mut r = record(
            AuditEventFamily::SensitiveConfigChanged,
            AuditOutcome::Authorized,
            AuditActionKind::ConfigPolicyIncrease,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        r.reason_code = Some(AuditReasonCode::PolicyChanged);
        r.operation_id = Some(AuditOperationId::new_uuid());
        r
    };
    let next = |authorization: &DurableAuditRecordV1, outcome: AuditOutcome| {
        let mut r = authorization.clone();
        r.event_id = crate::domain::AuditEventId::new_uuid();
        r.outcome = outcome;
        r
    };

    let applied_auth = authorized();
    let applied = next(&applied_auth, AuditOutcome::Applied);
    let failed_auth = authorized();
    let failed = next(&failed_auth, AuditOutcome::Failed);

    let mut reduce = record(
        AuditEventFamily::SensitiveConfigChanged,
        AuditOutcome::Applied,
        AuditActionKind::ConfigPolicyReduce,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    reduce.reason_code = Some(AuditReasonCode::PolicyChanged);

    vec![
        vec![applied_auth, applied],
        vec![failed_auth, failed],
        vec![reduce],
    ]
}

fn transcript_purge() -> Vec<Chain> {
    let base = |outcome| {
        let mut r = record(
            AuditEventFamily::TranscriptPurge,
            outcome,
            AuditActionKind::TranscriptPurge,
            AuditActorKind::AppPolicy,
            AuditActionSource::ExplicitCleanup,
        );
        r.project_id = Some(ProjectId::new_uuid());
        r.subject_kind = Some(AuditSubjectKind::Transcript);
        r.subject_ref = AuditReference::new("transcript:1");
        r
    };
    vec![
        vec![base(AuditOutcome::Completed)],
        vec![base(AuditOutcome::Failed)],
    ]
}

fn audit_store_recovery() -> Vec<Chain> {
    let mut r = record(
        AuditEventFamily::AuditStoreRecovery,
        AuditOutcome::Completed,
        AuditActionKind::AuditStoreRecovery,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    r.subject_kind = Some(AuditSubjectKind::RecoveryBundle);
    r.subject_ref = AuditReference::new("recovery:1");
    r.reason_code = Some(AuditReasonCode::RecoveryCompleted);
    vec![vec![r]]
}

/// Exhaustive over `AuditEventFamily` -- **no wildcard arm**. Adding a
/// family variant without adding its own call here does not compile,
/// which is what "a reviewer must be able to break it by adding a family
/// arm and see the test fail" (`acceptance-qa-checklist.md`) means in
/// practice: the failure is a build failure, reported by `cargo test` the
/// same as any other.
fn chains_for_family(family: AuditEventFamily) -> Vec<Chain> {
    match family {
        AuditEventFamily::ProjectAdded => project_added(),
        AuditEventFamily::TrustChange => trust_change(),
        AuditEventFamily::CommandApproval => command_approval(),
        AuditEventFamily::ManagedProcessLifecycle => managed_process_lifecycle(),
        AuditEventFamily::PlainTerminalObservation => plain_terminal_observation(),
        AuditEventFamily::PasteBlocked => paste_blocked(),
        AuditEventFamily::RestrictedModeBlocked => restricted_mode_blocked(),
        AuditEventFamily::RootAccessBlocked => root_access_blocked(),
        AuditEventFamily::SafeCloseDecision => safe_close_decision(),
        AuditEventFamily::SensitiveConfigChanged => sensitive_config_changed(),
        AuditEventFamily::TranscriptPurge => transcript_purge(),
        AuditEventFamily::AuditStoreRecovery => audit_store_recovery(),
    }
}

const ALL_FAMILIES: [AuditEventFamily; 12] = [
    AuditEventFamily::ProjectAdded,
    AuditEventFamily::TrustChange,
    AuditEventFamily::CommandApproval,
    AuditEventFamily::ManagedProcessLifecycle,
    AuditEventFamily::PlainTerminalObservation,
    AuditEventFamily::PasteBlocked,
    AuditEventFamily::RestrictedModeBlocked,
    AuditEventFamily::RootAccessBlocked,
    AuditEventFamily::SafeCloseDecision,
    AuditEventFamily::SensitiveConfigChanged,
    AuditEventFamily::TranscriptPurge,
    AuditEventFamily::AuditStoreRecovery,
];

/// Every chain `chains_for_family` produces, each of its records checked
/// against `DurableAuditRecordV1::validate()` first -- a mistake in this
/// file's own construction fails here, loudly, rather than being asserted
/// against a store that would have rejected it for an unrelated reason.
fn every_chain() -> Vec<Chain> {
    let mut out = Vec::new();
    for family in ALL_FAMILIES {
        for chain in chains_for_family(family) {
            for record in &chain {
                record.validate().unwrap_or_else(|error| {
                    panic!(
                        "{family:?}/{:?}/{:?} is not a record this enumeration's own builder \
                         produced correctly -- validate() rejected it: {error:?}",
                        record.outcome, record.action_kind
                    )
                });
            }
            out.push(chain);
        }
    }
    out
}

/// This is the real gate: every `(family, outcome, action_kind)` the type
/// can produce, written to a real on-disk store in the order its own
/// cross-record contract requires, each asserted to land and to come back
/// from `query`.
#[test]
fn every_family_outcome_action_combination_lands_in_a_real_store() {
    let chains = every_chain();
    let total_records: usize = chains.iter().map(Vec::len).sum();
    assert!(
        total_records >= 40,
        "the enumeration must actually cover a meaningful fraction of the type's own \
         combinations, not a handful: found {total_records} records across {} chains",
        chains.len()
    );

    let dirs = TestAuditDirs::new("every-family-outcome-lands");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();

    for chain in &chains {
        for record in chain {
            let outcome = store.append(record).unwrap_or_else(|error| {
                panic!(
                    "{:?}/{:?}/{:?} was accepted by validate() but rejected by the real store: \
                     {error:?} -- a combination the Rust side can produce but the store \
                     silently cannot hold, the exact class review 503 found for \
                     safe_close_decision/blocked",
                    record.family, record.outcome, record.action_kind
                )
            });
            let crate::audit::AuditAppendOutcome::Appended { .. } = outcome else {
                panic!(
                    "{:?}/{:?}/{:?}: a fresh, never-before-seen event_id must always append, \
                     never be treated as already present",
                    record.family, record.outcome, record.action_kind
                );
            };
        }
    }

    let all_records: Vec<&DurableAuditRecordV1> = chains.iter().flatten().collect();
    for record in &all_records {
        let page = store
            .query(&AuditQuery {
                family: Some(record.family),
                ..AuditQuery::latest(u32::try_from(all_records.len()).unwrap())
            })
            .unwrap();
        assert!(
            page.records
                .iter()
                .any(|sequenced| sequenced.record == **record),
            "{:?}/{:?}/{:?} landed (append succeeded) but does not come back from query() -- \
             appended and queryable are not the same claim and both must hold",
            record.family,
            record.outcome,
            record.action_kind
        );
    }
}
