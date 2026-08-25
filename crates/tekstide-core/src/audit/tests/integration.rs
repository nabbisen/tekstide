use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::agent::{
    AgentRunLaunchPlan, AgentRunLaunchRequest, AgentRunLaunchValidator, AiCliExecutable,
    AiCliExecutableProvenance, AiCliProfile, AiCliProfileSource, AiCliWorkspaceDiscoveryPolicy,
    VerifiedCwd,
};
use crate::approval::{
    AcceptedProposal, ApprovalCoordinator, CommandProposal, DecideOutcome, PROTOCOL_VERSION,
    ReceiveOutcome,
};
use crate::audit::integration::AuditRecordWriter;
use crate::audit::{
    AuditActionKind, AuditCoordinator, AuditEventFamily, AuditHealth, AuditHealthStatus,
    AuditIntegrationError, AuditObservationStatus, AuditOutcome, AuditQuery, AuditReasonCode,
    AuditStore, AuditStoreError, AuditStoreErrorReason, AuditSubjectKind,
    CommandDecisionActionKind, DurableAuditRecordV1, SafeCloseDecision,
};
use crate::domain::{
    AgentCompatibilityLevel, AgentRunId, AgentRunStatus, ApprovalDecision, ApprovalId,
    AuditOperationId, RiskLevel, TerminalId, TerminalKind, TerminalSession, TerminalStatus,
    Transcript,
};
use crate::project::{ProjectContentError, ProjectId, ProjectSession, WorkspaceTrust};
use crate::runtime::terminal::{
    BoundedRuntimeSummary, LinuxTerminalRuntime, TerminalRuntimeHandle, TerminationOutcome,
};

use super::support::TestAuditDirs;

#[test]
fn trust_grant_commits_authorization_before_mutation_and_applied_outcome() {
    let dirs = TestAuditDirs::new("integration-trust-grant");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let mut project = project_for(&dirs, 1);

    let result = AuditCoordinator::new(&mut store, &mut health)
        .grant_project_trust(&mut project)
        .unwrap();

    assert_eq!(project.trust_state(), WorkspaceTrust::Trusted);
    assert_eq!(result.audit_status, AuditObservationStatus::Persisted);
    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].outcome, AuditOutcome::Applied);
    assert_eq!(records[1].outcome, AuditOutcome::Authorized);
    assert_eq!(records[0].operation_id, records[1].operation_id);
    assert_eq!(health.status(), AuditHealthStatus::Healthy);
}

#[test]
fn required_failure_blocks_grant_while_observation_failure_preserves_safer_state() {
    let dirs = TestAuditDirs::new("integration-trust-failure");
    let mut project = project_for(&dirs, 1);
    let mut health = AuditHealth::default();
    let mut failing = RecordingWriter::fail_on(1);

    let error = AuditCoordinator::with_writer(&mut failing, &mut health)
        .grant_project_trust(&mut project)
        .unwrap_err();

    assert_eq!(project.trust_state(), WorkspaceTrust::Restricted);
    assert_eq!(
        error,
        AuditIntegrationError::RequiredAuditUnavailable(AuditStoreErrorReason::StorageFull)
    );
    assert_eq!(health.failure_count(), 1);

    let mut revoke_writer = RecordingWriter::fail_on(1);
    let result = AuditCoordinator::with_writer(&mut revoke_writer, &mut health)
        .revoke_project_trust(&mut project);

    assert_eq!(project.trust_state(), WorkspaceTrust::Revoked);
    assert_eq!(result.audit_status, AuditObservationStatus::Degraded);
    assert_eq!(health.failure_count(), 2);
    assert_eq!(revoke_writer.attempt_count, 1);
}

#[test]
fn post_authorization_failure_preserves_applied_trust_and_degrades_health_once() {
    let dirs = TestAuditDirs::new("integration-trust-applied-failure");
    let mut project = project_for(&dirs, 1);
    let mut health = AuditHealth::default();
    let mut writer = RecordingWriter::fail_on(2);

    let result = AuditCoordinator::with_writer(&mut writer, &mut health)
        .grant_project_trust(&mut project)
        .unwrap();

    assert_eq!(project.trust_state(), WorkspaceTrust::Trusted);
    assert_eq!(result.audit_status, AuditObservationStatus::Degraded);
    assert_eq!(writer.records.len(), 1);
    assert_eq!(writer.records[0].outcome, AuditOutcome::Authorized);
    assert_eq!(writer.attempt_count, 2);
    assert_eq!(health.failure_count(), 1);
}

#[test]
fn managed_launch_persists_authorized_started_and_terminated_runtime_truth() {
    let dirs = TestAuditDirs::new("integration-managed-launch");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let mut project = project_for(&dirs, 1);
    let plan = launch_plan_for(&project, Path::new("/bin/sh"));
    let mut runtime = LinuxTerminalRuntime::new();

    let launched = AuditCoordinator::new(&mut store, &mut health)
        .launch_managed_agent_run(&mut project, plan, &mut runtime)
        .unwrap();
    assert_eq!(launched.audit_status, AuditObservationStatus::Persisted);
    let handle =
        TerminalRuntimeHandle::new(launched.value.terminal_id().clone(), project.id().clone());
    runtime.write_input(&handle, b"exit 0\n").unwrap();
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(outcome, TerminationOutcome::Exited { exit_status: 0 });

    let mut other_project = project_for(&dirs, 2);
    let wrong_project_error = AuditCoordinator::new(&mut store, &mut health)
        .apply_managed_agent_terminal_outcome(&mut other_project, &launched.value, &outcome)
        .unwrap_err();
    assert_eq!(
        wrong_project_error,
        AuditIntegrationError::InvalidTypedContext
    );

    let applied = AuditCoordinator::new(&mut store, &mut health)
        .apply_managed_agent_terminal_outcome(&mut project, &launched.value, &outcome)
        .unwrap();
    assert_eq!(applied.audit_status, AuditObservationStatus::Persisted);

    let mut query = AuditQuery::latest(10);
    query.operation_id = Some(launched.value.operation_id().clone());
    let records = store.query(&query).unwrap().records;
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].record.outcome, AuditOutcome::Terminated);
    assert_eq!(records[1].record.outcome, AuditOutcome::Started);
    assert_eq!(records[2].record.outcome, AuditOutcome::Authorized);
    assert!(records.iter().all(|record| {
        record.record.project_id.as_ref() == Some(project.id())
            && record.record.agent_run_id.as_ref() == Some(launched.value.agent_run_id())
    }));
    assert_eq!(
        project
            .terminal_session(launched.value.terminal_id())
            .unwrap()
            .status(),
        TerminalStatus::Exited
    );
    let debug = format!("{records:?}");
    assert!(!debug.contains("private prompt sentinel"));
    assert!(!debug.contains("private command sentinel"));
    assert!(!debug.contains(project.root_path().to_string_lossy().as_ref()));
}

/// RFC-017 PR-017-F: `plain_terminal_observation`'s first producer,
/// proven against a real, file-backed `AuditStore` (not a mock writer)
/// -- the same convention `trust_grant_commits_authorization_before_mutation_and_applied_outcome`
/// above uses. `record.validate()` is the direct proof this conforms to
/// the frozen family (`valid_plain_terminal`) rather than assuming a
/// hand-built record happens to satisfy it.
#[test]
fn plain_terminal_started_persists_a_valid_record() {
    let dirs = TestAuditDirs::new("plain-terminal-started");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::new_uuid();
    let terminal_id = TerminalId::new_uuid();

    let status = AuditCoordinator::new(&mut store, &mut health)
        .record_plain_terminal_started(project_id.clone(), terminal_id.clone());

    assert_eq!(status, AuditObservationStatus::Persisted);
    assert_eq!(health.status(), AuditHealthStatus::Healthy);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.family, AuditEventFamily::PlainTerminalObservation);
    assert_eq!(record.outcome, AuditOutcome::Started);
    assert_eq!(record.project_id, Some(project_id));
    assert_eq!(record.terminal_id, Some(terminal_id));
    assert!(
        record.reason_code.is_none(),
        "Started must carry no reason code, per valid_plain_terminal"
    );
    record
        .validate()
        .expect("a real producer's record must satisfy the frozen family's own predicate");
}

/// Terminal launch UX handoff: `plain_terminal_observation`'s second
/// producer, closing the PR-017-F known limitation that only `Started`
/// was ever reachable. Proves the `TerminationOutcome::Exited` ->
/// `AuditReasonCode::ProcessExited` mapping (the review gate's own named
/// case: "conforming to the frozen family, with the `reason_code` the
/// schema requires for non-`Started` outcomes") against a real store,
/// the same way `plain_terminal_started_persists_a_valid_record` does
/// for `Started`.
#[test]
fn plain_terminal_terminated_persists_a_valid_record_with_the_real_exit_reason() {
    let dirs = TestAuditDirs::new("plain-terminal-terminated");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::new_uuid();
    let terminal_id = TerminalId::new_uuid();

    let status = AuditCoordinator::new(&mut store, &mut health).record_plain_terminal_terminated(
        project_id.clone(),
        terminal_id.clone(),
        &TerminationOutcome::Exited { exit_status: 0 },
    );

    assert_eq!(status, AuditObservationStatus::Persisted);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.family, AuditEventFamily::PlainTerminalObservation);
    assert_eq!(record.outcome, AuditOutcome::Terminated);
    assert_eq!(record.project_id, Some(project_id));
    assert_eq!(record.terminal_id, Some(terminal_id));
    assert_eq!(record.reason_code, Some(AuditReasonCode::ProcessExited));
    record
        .validate()
        .expect("a real producer's record must satisfy the frozen family's own predicate");
}

/// The signalled/killed-after-timeout side of the same mapping --
/// `ManagedProcessLifecycle`'s own established `TerminationOutcome`-to-
/// `AuditReasonCode` precedent, reused rather than re-decided.
#[test]
fn plain_terminal_terminated_maps_a_signal_outcome_to_process_terminated() {
    let dirs = TestAuditDirs::new("plain-terminal-terminated-signal");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();

    let status = AuditCoordinator::new(&mut store, &mut health).record_plain_terminal_terminated(
        ProjectId::new_uuid(),
        TerminalId::new_uuid(),
        &TerminationOutcome::TerminatedBySignal {
            signal: crate::runtime::terminal::TerminationSignal::Sigterm,
        },
    );

    assert_eq!(status, AuditObservationStatus::Persisted);
    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    assert_eq!(
        records[0].record.reason_code,
        Some(AuditReasonCode::ProcessTerminated)
    );
}

/// `Failed`/`OrphanedUnknown` are ambiguous outcomes a plain, non-blocking
/// exit check cannot itself resolve into a confident reason -- matching
/// `ManagedProcessLifecycle`'s own precedent, nothing is written rather
/// than guessing a reason code.
#[test]
fn plain_terminal_terminated_is_not_required_for_an_ambiguous_outcome() {
    let dirs = TestAuditDirs::new("plain-terminal-terminated-ambiguous");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();

    let status = AuditCoordinator::new(&mut store, &mut health).record_plain_terminal_terminated(
        ProjectId::new_uuid(),
        TerminalId::new_uuid(),
        &TerminationOutcome::Failed {
            summary: BoundedRuntimeSummary::new("exited without exit code or signal"),
        },
    );

    assert_eq!(status, AuditObservationStatus::NotRequired);
    assert!(
        store
            .query(&AuditQuery::latest(10))
            .unwrap()
            .records
            .is_empty()
    );
}

/// RFC-018 PR-018-D: `paste_blocked`'s first producer, persisting a
/// record `valid_paste_blocked` itself accepts -- every field the
/// frozen family fixes (`action_kind`, `actor_kind`, `action_source`,
/// `reason_code`, `outcome`) checked explicitly, not only via
/// `validate()`, so a future change to `paste_blocked_record` that
/// drifted from the frozen contract fails here by name.
#[test]
fn paste_blocked_persists_a_valid_record_conforming_to_the_frozen_family() {
    let dirs = TestAuditDirs::new("paste-blocked");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::new_uuid();
    let terminal_id = TerminalId::new_uuid();

    let status = AuditCoordinator::new(&mut store, &mut health)
        .record_paste_blocked(project_id.clone(), terminal_id.clone());

    assert_eq!(status, AuditObservationStatus::Persisted);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.family, AuditEventFamily::PasteBlocked);
    assert_eq!(record.outcome, AuditOutcome::Blocked);
    assert_eq!(
        record.action_kind,
        crate::audit::AuditActionKind::TerminalPaste
    );
    assert_eq!(record.actor_kind, crate::audit::AuditActorKind::AppPolicy);
    assert_eq!(
        record.action_source,
        crate::audit::AuditActionSource::PolicyEngine
    );
    assert_eq!(record.reason_code, Some(AuditReasonCode::PastePolicy));
    assert_eq!(record.project_id, Some(project_id));
    assert_eq!(record.terminal_id, Some(terminal_id));
    record
        .validate()
        .expect("a real producer's record must satisfy the frozen family's own predicate");
}

/// **Ablation**: `valid_paste_blocked` requires `outcome == Blocked`
/// specifically -- the constraint RFC-018 names as the reason this
/// family can only ever record refusals, never a paste the user
/// confirms. Constructing the same record shape with `outcome` swapped
/// to a value this family does not accept proves the schema's own
/// validation is what enforces this, not merely that the producer
/// happens to always pass `Blocked`.
#[test]
fn paste_blocked_schema_rejects_any_outcome_other_than_blocked() {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::PasteBlocked,
        AuditOutcome::Applied,
        crate::audit::AuditActionKind::TerminalPaste,
        crate::audit::AuditActorKind::AppPolicy,
        crate::audit::AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(ProjectId::new_uuid());
    record.terminal_id = Some(TerminalId::new_uuid());
    record.reason_code = Some(AuditReasonCode::PastePolicy);

    assert!(
        record.validate().is_err(),
        "outcome != Blocked must fail valid_paste_blocked -- a paste the user confirms has no \
         valid encoding in this family, and this is the check that enforces it"
    );
}

/// RFC-031 PR-031-A: `restricted_mode_blocked`'s first producer,
/// persisting a record `valid_restricted_mode_blocked` itself accepts.
/// **The test that matters more than presence**, per
/// `what-the-store-may-hold.md`: `subject_ref`/`subject_kind` asserted
/// `None` directly, not merely unset by inspection -- this is the
/// assertion that fails if a future "improvement" starts putting the
/// project's directory name in.
#[test]
fn restricted_mode_blocked_persists_a_valid_record_conforming_to_the_frozen_family() {
    let dirs = TestAuditDirs::new("restricted-mode-blocked");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::new_uuid();

    let status = AuditCoordinator::new(&mut store, &mut health)
        .record_restricted_mode_blocked(project_id.clone());

    assert_eq!(status, AuditObservationStatus::Persisted);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.family, AuditEventFamily::RestrictedModeBlocked);
    assert_eq!(record.outcome, AuditOutcome::Blocked);
    assert_eq!(
        record.action_kind,
        crate::audit::AuditActionKind::RestrictedFeature
    );
    assert_eq!(record.actor_kind, crate::audit::AuditActorKind::AppPolicy);
    assert_eq!(
        record.action_source,
        crate::audit::AuditActionSource::PolicyEngine
    );
    assert_eq!(record.reason_code, Some(AuditReasonCode::RestrictedMode));
    assert_eq!(record.project_id, Some(project_id));
    assert_eq!(
        record.terminal_id, None,
        "a refused launch never reaches a terminal"
    );
    assert_eq!(record.agent_run_id, None);
    assert_eq!(record.approval_id, None);
    assert_eq!(
        record.subject_kind, None,
        "what-the-store-may-hold.md: no path-shaped text belongs in this record"
    );
    assert_eq!(
        record.subject_ref, None,
        "what-the-store-may-hold.md: no path-shaped text belongs in this record"
    );
    record
        .validate()
        .expect("a real producer's record must satisfy the frozen family's own predicate");
}

/// **Ablation**: `valid_restricted_mode_blocked` requires
/// `outcome == Blocked` specifically, the same shape
/// `paste_blocked_schema_rejects_any_outcome_other_than_blocked` already
/// proves for a sibling family.
#[test]
fn restricted_mode_blocked_schema_rejects_any_outcome_other_than_blocked() {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::RestrictedModeBlocked,
        AuditOutcome::Failed,
        crate::audit::AuditActionKind::RestrictedFeature,
        crate::audit::AuditActorKind::AppPolicy,
        crate::audit::AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(ProjectId::new_uuid());
    record.reason_code = Some(AuditReasonCode::RestrictedMode);

    assert!(
        record.validate().is_err(),
        "outcome != Blocked must fail valid_restricted_mode_blocked -- this family exists only \
         to record a refusal"
    );
}

/// RFC-031 PR-031-B: `project_added`'s first producer, persisting a
/// record `valid_project_added` itself accepts. `subject_ref` asserted
/// `None` directly, the same required check
/// `restricted_mode_blocked_persists_a_valid_record_conforming_to_the_frozen_family`
/// applies to its own family.
#[test]
fn project_added_persists_a_valid_record_conforming_to_the_frozen_family() {
    let dirs = TestAuditDirs::new("project-added");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::new_uuid();

    let status =
        AuditCoordinator::new(&mut store, &mut health).record_project_added(project_id.clone());

    assert_eq!(status, AuditObservationStatus::Persisted);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.family, AuditEventFamily::ProjectAdded);
    assert_eq!(record.outcome, AuditOutcome::Applied);
    assert_eq!(
        record.action_kind,
        crate::audit::AuditActionKind::ProjectAdd
    );
    assert_eq!(record.actor_kind, crate::audit::AuditActorKind::User);
    assert_eq!(
        record.action_source,
        crate::audit::AuditActionSource::AppCommand
    );
    assert_eq!(record.project_id, Some(project_id));
    assert_eq!(record.terminal_id, None);
    assert_eq!(record.agent_run_id, None);
    assert_eq!(record.approval_id, None);
    assert_eq!(record.operation_id, None);
    assert_eq!(record.risk_level, None);
    assert_eq!(record.adapter_profile_ref, None);
    assert_eq!(record.reason_code, None);
    assert_eq!(
        record.subject_kind, None,
        "what-the-store-may-hold.md: no path-shaped text belongs in this record -- project_id \
         already identifies the project"
    );
    assert_eq!(record.subject_ref, None);
    record
        .validate()
        .expect("a real producer's record must satisfy the frozen family's own predicate");
}

/// **Ablation**: `valid_project_added` requires `outcome == Applied`
/// specifically and `no_optional_context` (every optional field unset),
/// the same shape the sibling families' own schema-rejection tests use.
#[test]
fn project_added_schema_rejects_any_outcome_other_than_applied() {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::ProjectAdded,
        AuditOutcome::Requested,
        crate::audit::AuditActionKind::ProjectAdd,
        crate::audit::AuditActorKind::User,
        crate::audit::AuditActionSource::AppCommand,
    );
    record.project_id = Some(ProjectId::new_uuid());

    assert!(
        record.validate().is_err(),
        "outcome != Applied must fail valid_project_added -- adding a project is not a two-phase \
         operation in this schema"
    );
}

/// RFC-023 PR-023-D: `config_policy_increase`'s producer, the same
/// two-linked-records shape `trust_grant_commits_authorization_before_mutation_and_applied_outcome`
/// proves for `grant_project_trust` -- `Authorized` then `Applied`,
/// sharing one `operation_id`, both real, persisted, queried-back
/// records rather than in-memory values.
#[test]
fn sensitive_config_policy_increase_persists_authorized_then_applied_sharing_one_operation_id() {
    let dirs = TestAuditDirs::new("config-policy-increase");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();

    let status =
        AuditCoordinator::new(&mut store, &mut health).record_sensitive_config_policy_increase();
    assert_eq!(status, AuditObservationStatus::Persisted);

    let mut records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2, "{records:?}");
    records.sort_by_key(|record| record.outcome == AuditOutcome::Applied);
    let (authorized, applied) = (&records[0], &records[1]);

    for record in [authorized, applied] {
        assert_eq!(record.family, AuditEventFamily::SensitiveConfigChanged);
        assert_eq!(
            record.action_kind,
            crate::audit::AuditActionKind::ConfigPolicyIncrease
        );
        assert_eq!(record.actor_kind, crate::audit::AuditActorKind::User);
        assert_eq!(
            record.action_source,
            crate::audit::AuditActionSource::TrustedUi
        );
        assert_eq!(record.reason_code, Some(AuditReasonCode::PolicyChanged));
        assert_eq!(
            record.project_id, None,
            "no project to attribute a global config change to"
        );
        assert_eq!(record.subject_kind, None);
        assert_eq!(
            record.subject_ref, None,
            "structurally impossible for this family -- see sensitive_config_changed_record's \
             own doc comment"
        );
        record
            .validate()
            .expect("a real producer's record must satisfy the frozen family's own predicate");
    }
    assert_eq!(authorized.outcome, AuditOutcome::Authorized);
    assert_eq!(applied.outcome, AuditOutcome::Applied);
    assert!(authorized.operation_id.is_some());
    assert_eq!(authorized.operation_id, applied.operation_id);
    assert_ne!(
        authorized.event_id, applied.event_id,
        "two distinct events, not the same record persisted twice"
    );
}

/// **Ablation**: `valid_config_change`'s `ConfigPolicyIncrease` arm
/// requires `operation_id.is_some()`.
#[test]
fn sensitive_config_policy_increase_schema_rejects_a_missing_operation_id() {
    let record = DurableAuditRecordV1::new(
        AuditEventFamily::SensitiveConfigChanged,
        AuditOutcome::Authorized,
        crate::audit::AuditActionKind::ConfigPolicyIncrease,
        crate::audit::AuditActorKind::User,
        crate::audit::AuditActionSource::TrustedUi,
    );
    assert!(
        record.validate().is_err(),
        "ConfigPolicyIncrease without an operation_id must fail -- it is the one direction that \
         requires authorization to be traceable"
    );
}

/// RFC-023 PR-023-D: `config_policy_reduce`'s producer -- single stage,
/// no `operation_id`, `AppPolicy`/`PolicyEngine` rather than a user
/// actor, matching the asymmetry RFC-023's own §Audit pins: tightening
/// never needs authorization.
#[test]
fn sensitive_config_policy_reduce_persists_a_valid_record_conforming_to_the_frozen_family() {
    let dirs = TestAuditDirs::new("config-policy-reduce");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();

    let status =
        AuditCoordinator::new(&mut store, &mut health).record_sensitive_config_policy_reduce();
    assert_eq!(status, AuditObservationStatus::Persisted);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.family, AuditEventFamily::SensitiveConfigChanged);
    assert_eq!(record.outcome, AuditOutcome::Applied);
    assert_eq!(
        record.action_kind,
        crate::audit::AuditActionKind::ConfigPolicyReduce
    );
    assert_eq!(record.actor_kind, crate::audit::AuditActorKind::AppPolicy);
    assert_eq!(
        record.action_source,
        crate::audit::AuditActionSource::PolicyEngine
    );
    assert_eq!(record.reason_code, Some(AuditReasonCode::PolicyChanged));
    assert_eq!(record.operation_id, None);
    assert_eq!(record.project_id, None);
    assert_eq!(record.subject_kind, None);
    assert_eq!(record.subject_ref, None);
    record
        .validate()
        .expect("a real producer's record must satisfy the frozen family's own predicate");
}

/// **Ablation**: `valid_config_change`'s `ConfigPolicyReduce` arm
/// requires `outcome == Applied` and `operation_id.is_none()` --
/// checked here with a present `operation_id`, the direction-specific
/// half `sensitive_config_policy_increase_schema_rejects_a_missing_operation_id`
/// does not cover.
#[test]
fn sensitive_config_policy_reduce_schema_rejects_a_present_operation_id() {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::SensitiveConfigChanged,
        AuditOutcome::Applied,
        crate::audit::AuditActionKind::ConfigPolicyReduce,
        crate::audit::AuditActorKind::AppPolicy,
        crate::audit::AuditActionSource::PolicyEngine,
    );
    record.reason_code = Some(AuditReasonCode::PolicyChanged);
    record.operation_id = Some(crate::domain::AuditOperationId::new_uuid());

    assert!(
        record.validate().is_err(),
        "ConfigPolicyReduce with an operation_id must fail -- reduce never requires \
         authorization, so an operation_id here would imply a confirmation step that did not \
         happen"
    );
}

/// RFC-023's own required sentinel test (implementation-handoff.md §8,
/// modeled on RFC-012's own): the honest form it takes here is that
/// *neither producer method accepts a config value as a parameter at
/// all* -- `record_sensitive_config_policy_increase`/`_reduce` take no
/// arguments describing what changed, only that a change of a given
/// direction occurred. A distinctive, secret-shaped string is not
/// merely absent from the real, persisted, queried-back record; there
/// is no code path by which one could reach it, the same "inert by
/// construction" shape this pack has used throughout (`ConfigDiagnostic.message`,
/// `RestrictedDefaultTrust`). Proven against the real store round-trip
/// (write, persist, query, format), not by reading the source.
#[test]
fn no_config_value_can_reach_a_sensitive_config_changed_record() {
    let dirs = TestAuditDirs::new("config-policy-sentinel");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let sentinel = "sk-live-sentinel-config-value-should-never-appear-here";

    let mut coordinator = AuditCoordinator::new(&mut store, &mut health);
    coordinator.record_sensitive_config_policy_increase();
    coordinator.record_sensitive_config_policy_reduce();

    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    let debug = format!("{records:?}");
    assert!(!debug.contains(sentinel));
    assert!(!debug.contains("agent.profile"));
    assert!(!debug.contains("restricted_mode_blocks"));
}

#[test]
fn managed_launch_failure_is_recorded_after_authorization_without_process_attachment() {
    let dirs = TestAuditDirs::new("integration-managed-failure");
    let executable = dirs.base.join("temporary-agent");
    fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    fs::set_permissions(&executable, permissions).unwrap();
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let mut project = project_for(&dirs, 1);
    let plan = launch_plan_for(&project, &executable);
    fs::remove_file(&executable).unwrap();
    let mut runtime = LinuxTerminalRuntime::new();

    let error = AuditCoordinator::new(&mut store, &mut health)
        .launch_managed_agent_run(&mut project, plan, &mut runtime)
        .unwrap_err();

    assert!(matches!(error, AuditIntegrationError::AgentLaunch(_)));
    assert!(project.agent_runs().is_empty());
    assert!(project.terminal_sessions().is_empty());
    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].record.outcome, AuditOutcome::Failed);
    assert_eq!(records[1].record.outcome, AuditOutcome::Authorized);
    assert_eq!(
        records[0].record.operation_id,
        records[1].record.operation_id
    );
}

#[test]
fn managed_launch_does_not_create_process_when_authorization_cannot_persist() {
    let dirs = TestAuditDirs::new("integration-managed-audit-failure");
    let mut health = AuditHealth::default();
    let mut writer = RecordingWriter::fail_on(1);
    let mut project = project_for(&dirs, 1);
    let plan = launch_plan_for(&project, Path::new("/bin/sh"));
    let mut runtime = LinuxTerminalRuntime::new();

    let error = AuditCoordinator::with_writer(&mut writer, &mut health)
        .launch_managed_agent_run(&mut project, plan, &mut runtime)
        .unwrap_err();

    assert_eq!(
        error,
        AuditIntegrationError::RequiredAuditUnavailable(AuditStoreErrorReason::StorageFull)
    );
    assert!(project.agent_runs().is_empty());
    assert!(project.terminal_sessions().is_empty());
    assert_eq!(writer.attempt_count, 1);
    assert_eq!(health.failure_count(), 1);
}

#[test]
fn plain_agent_launch_is_not_relabelled_as_durably_authorized() {
    let dirs = TestAuditDirs::new("integration-plain-launch");
    let mut health = AuditHealth::default();
    let mut writer = RecordingWriter::fail_on(usize::MAX);
    let mut project = project_for(&dirs, 1);
    let plan = launch_plan_for_level(
        &project,
        Path::new("/bin/sh"),
        AgentCompatibilityLevel::Plain,
    );
    let mut runtime = LinuxTerminalRuntime::new();

    let error = AuditCoordinator::with_writer(&mut writer, &mut health)
        .launch_managed_agent_run(&mut project, plan, &mut runtime)
        .unwrap_err();

    assert_eq!(error, AuditIntegrationError::InvalidTypedContext);
    assert_eq!(writer.attempt_count, 0);
    assert!(project.agent_runs().is_empty());
    assert!(project.terminal_sessions().is_empty());
    assert_eq!(health.status(), AuditHealthStatus::Healthy);
}

#[test]
fn termination_truth_survives_observational_audit_failure() {
    let dirs = TestAuditDirs::new("integration-termination-audit-failure");
    let mut health = AuditHealth::default();
    let mut writer = RecordingWriter::fail_on(3);
    let mut project = project_for(&dirs, 1);
    let plan = launch_plan_for(&project, Path::new("/bin/sh"));
    let mut runtime = LinuxTerminalRuntime::new();
    let (launched, result) = {
        let mut coordinator = AuditCoordinator::with_writer(&mut writer, &mut health);
        let launched = coordinator
            .launch_managed_agent_run(&mut project, plan, &mut runtime)
            .unwrap();
        let handle =
            TerminalRuntimeHandle::new(launched.value.terminal_id().clone(), project.id().clone());
        runtime.write_input(&handle, b"exit 0\n").unwrap();
        let outcome = runtime
            .wait_for_exit(&handle, Duration::from_secs(5))
            .unwrap()
            .unwrap();
        let result = coordinator
            .apply_managed_agent_terminal_outcome(&mut project, &launched.value, &outcome)
            .unwrap();
        (launched, result)
    };

    assert_eq!(result.audit_status, AuditObservationStatus::Degraded);
    assert_eq!(
        project
            .terminal_session(launched.value.terminal_id())
            .unwrap()
            .status(),
        TerminalStatus::Exited
    );
    assert_eq!(writer.records.len(), 2);
    assert_eq!(writer.attempt_count, 3);
    assert_eq!(health.failure_count(), 1);
}

#[test]
fn orphaned_runtime_truth_is_not_mislabeled_as_durable_termination() {
    let dirs = TestAuditDirs::new("integration-orphaned-runtime");
    let mut health = AuditHealth::default();
    let mut writer = RecordingWriter::fail_on(usize::MAX);
    let mut project = project_for(&dirs, 1);
    let plan = launch_plan_for(&project, Path::new("/bin/sh"));
    let mut runtime = LinuxTerminalRuntime::new();
    let mut coordinator = AuditCoordinator::with_writer(&mut writer, &mut health);
    let launched = coordinator
        .launch_managed_agent_run(&mut project, plan, &mut runtime)
        .unwrap();
    let handle =
        TerminalRuntimeHandle::new(launched.value.terminal_id().clone(), project.id().clone());
    let orphaned = TerminationOutcome::OrphanedUnknown {
        summary: BoundedRuntimeSummary::new("private observer failure sentinel"),
    };

    let result = coordinator
        .apply_managed_agent_terminal_outcome(&mut project, &launched.value, &orphaned)
        .unwrap();

    assert_eq!(result.audit_status, AuditObservationStatus::NotRequired);
    assert_eq!(writer.attempt_count, 2);
    assert_eq!(
        project
            .terminal_session(launched.value.terminal_id())
            .unwrap()
            .status(),
        TerminalStatus::OrphanedUnknown
    );
    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Detached);
    runtime.write_input(&handle, b"exit 0\n").unwrap();
    runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .unwrap()
        .unwrap();
}

#[test]
fn open_and_save_symlink_blocks_persist_typed_reasons_without_paths() {
    let dirs = TestAuditDirs::new("integration-root-block");
    let project_root = dirs.base.join("project");
    let outside = dirs.base.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("secret-sentinel.txt"),
        "secret-content-sentinel",
    )
    .unwrap();
    symlink(
        outside.join("secret-sentinel.txt"),
        project_root.join("escape-sentinel"),
    )
    .unwrap();
    fs::write(project_root.join("editable.txt"), "before").unwrap();
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let mut project = project_for(&dirs, 1);

    let open_error = AuditCoordinator::new(&mut store, &mut health)
        .open_project_text_document(&mut project, "escape-sentinel")
        .unwrap_err();
    assert!(matches!(open_error, ProjectContentError::Open(_)));

    project.open_text_document("editable.txt").unwrap();
    project.replace_active_text("after").unwrap();
    fs::remove_file(project_root.join("editable.txt")).unwrap();
    symlink(
        outside.join("secret-sentinel.txt"),
        project_root.join("editable.txt"),
    )
    .unwrap();
    let save_error = AuditCoordinator::new(&mut store, &mut health)
        .save_project_text_document(&mut project)
        .unwrap_err();
    assert!(matches!(save_error, ProjectContentError::Save(_)));

    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| {
        record.record.family == AuditEventFamily::RootAccessBlocked
            && record.record.reason_code == Some(AuditReasonCode::SymlinkEscape)
            && record.record.project_id.as_ref() == Some(project.id())
            && record.record.terminal_id.is_none()
            && record.record.agent_run_id.is_none()
            && record.record.subject_ref.is_none()
    }));
    let debug = format!("{records:?}");
    assert!(!debug.contains("secret-sentinel"));
    assert!(!debug.contains("editable.txt"));
    assert!(!debug.contains("secret-content-sentinel"));
}

// --- RFC-021 PR-021-E2: command_approval family ---------------------------

/// `record_command_request` (`command_request`) is best-effort: nothing is
/// being authorized yet at this point (no execution to gate), so a write
/// failure degrades `AuditHealth` but is not surfaced as an error at all.
#[test]
fn command_request_is_best_effort_and_degrades_health_without_blocking() {
    let mut health = AuditHealth::default();
    let mut writer = RecordingWriter::fail_on(1);

    let result = AuditCoordinator::with_writer(&mut writer, &mut health).record_command_request(
        ProjectId::for_test(1),
        None,
        ApprovalId::new_uuid(),
        RiskLevel::Low,
    );

    assert_eq!(result, AuditObservationStatus::Degraded);
    assert_eq!(health.failure_count(), 1);
}

/// The RFC's fail-closed matrix, applied to `command_approve`/`command_
/// edit_and_approve`: the authorization write is **required** -- a
/// failure here must propagate as `Err`, exactly like `grant_project_
/// trust`'s authorization. `command_reject` has no authorization phase at
/// all and must never block, even when every write fails -- rejecting is
/// always the safe direction, and blocking it on an audit failure would
/// force an already-safe outcome into limbo.
#[test]
fn command_approve_authorization_failure_blocks_and_command_reject_never_blocks() {
    let mut health = AuditHealth::default();
    let mut writer = RecordingWriter::fail_on(1);

    let error = AuditCoordinator::with_writer(&mut writer, &mut health)
        .authorize_command_decision(
            ProjectId::for_test(1),
            None,
            ApprovalId::new_uuid(),
            RiskLevel::High,
            CommandDecisionActionKind::Approve,
        )
        .unwrap_err();
    assert_eq!(
        error,
        AuditIntegrationError::RequiredAuditUnavailable(AuditStoreErrorReason::StorageFull)
    );
    assert_eq!(health.failure_count(), 1);

    let mut reject_writer = RecordingWriter::fail_on(1);
    let result = AuditCoordinator::with_writer(&mut reject_writer, &mut health)
        .record_command_reject(
            ProjectId::for_test(1),
            None,
            ApprovalId::new_uuid(),
            RiskLevel::Low,
        );
    assert_eq!(result, AuditObservationStatus::Degraded);
    assert_eq!(health.failure_count(), 2);
}

/// `command_approve`'s full authorize-then-apply shape, persisted to a
/// real store and read back: `Authorized` then `Applied`, sharing one
/// `operation_id`, both in the `command_approval` family.
#[test]
fn command_approve_persists_authorized_then_applied_with_matching_operation_id() {
    let dirs = TestAuditDirs::new("integration-command-approve");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let approval_id = ApprovalId::new_uuid();

    let operation_id = AuditCoordinator::new(&mut store, &mut health)
        .authorize_command_decision(
            ProjectId::for_test(1),
            None,
            approval_id.clone(),
            RiskLevel::Medium,
            CommandDecisionActionKind::Approve,
        )
        .unwrap();
    AuditCoordinator::new(&mut store, &mut health).record_command_decision_outcome(
        ProjectId::for_test(1),
        None,
        approval_id,
        RiskLevel::Medium,
        CommandDecisionActionKind::Approve,
        operation_id.clone(),
        true,
    );

    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].record.outcome, AuditOutcome::Applied);
    assert_eq!(records[1].record.outcome, AuditOutcome::Authorized);
    assert_eq!(records[0].record.operation_id, Some(operation_id.clone()));
    assert_eq!(records[1].record.operation_id, Some(operation_id));
    assert!(records.iter().all(|record| {
        record.record.family == AuditEventFamily::CommandApproval
            && record.record.terminal_id.is_none()
            && record.record.subject_kind.is_none()
    }));
}

/// RFC-021 PR-021-E2 response 116 Required 1: on `DecideOutcome::
/// AuditBlocked` for an edit-and-approve, the stored request must remain
/// exactly as it was before the edit -- `display_command`, `risk_level`,
/// and `risk_reasons` all still describing the *original* proposal, never
/// the edited argv that was never actually authorized. This is the one
/// property `approval::tests::coordinator` cannot reach on its own: that
/// module has no path to a fake failing `AuditRecordWriter` (the trait is
/// `pub(crate)` but lives in the private `audit::integration` module,
/// which is never re-exported, so it cannot be named -- or implemented for
/// a caller's own type -- from outside `audit` and its descendants). This
/// module IS a descendant of `audit`, so it can use `RecordingWriter` and
/// `AuditCoordinator::with_writer` exactly as every other required-vs-
/// best-effort proof in this file does, while calling into
/// `approval::ApprovalCoordinator` (a fully public, crate-external type)
/// to exercise the property that actually lives there.
#[test]
fn edit_and_approve_audit_block_leaves_the_stored_request_describing_the_original_proposal() {
    const PROJECT_ROOT: &str = "/home/user/project";
    const STATE_ROOT: &str = "/home/user/.local/share/tekstide";

    let mut health = AuditHealth::default();
    // Attempt 1 is `record_command_request` during `receive_proposal`
    // (must succeed, or there is no request to edit); attempt 2 is
    // `authorize_command_decision` during `decide_with_edited_argv` (must
    // fail, to exercise `AuditBlocked`).
    let mut writer = RecordingWriter::fail_on(2);

    let command_proposal = CommandProposal::decode(
        PROTOCOL_VERSION,
        "t".repeat(64),
        "proposal-1".to_string(),
        vec!["git".to_string(), "status".to_string()],
        PathBuf::from(PROJECT_ROOT),
        None,
        None,
    )
    .expect("test proposal must decode");
    let (accepted, _peer) = AcceptedProposal::for_test(command_proposal.clone());

    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let ReceiveOutcome::Created {
        request: original, ..
    } = coordinator.receive_proposal(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        &VerifiedCwd::for_test(PROJECT_ROOT),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        accepted,
        crate::approval::ApprovalQueueLimits::default(),
        &mut AuditCoordinator::with_writer(&mut writer, &mut health),
    )
    else {
        panic!("must create a request");
    };
    assert_eq!(original.risk_level, RiskLevel::Low);

    let outcome = coordinator.decide_with_edited_argv(
        &agent_run_id,
        command_proposal.proposal_id(),
        vec!["rm".to_string(), "-rf".to_string(), "/etc".to_string()],
        &VerifiedCwd::for_test(PROJECT_ROOT),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        &mut AuditCoordinator::with_writer(&mut writer, &mut health),
    );

    let DecideOutcome::AuditBlocked(error) = outcome else {
        panic!("must be blocked -- got {outcome:?}");
    };
    assert_eq!(
        error,
        AuditIntegrationError::RequiredAuditUnavailable(AuditStoreErrorReason::StorageFull)
    );
    // Nothing durable was written for the blocked authorization attempt --
    // only the one successful `command_request` write from receipt.
    assert_eq!(writer.records.len(), 1);

    let still_pending = coordinator
        .find(&agent_run_id, command_proposal.proposal_id())
        .expect("the request must still exist -- AuditBlocked does not remove it");
    assert_eq!(
        still_pending.decision,
        ApprovalDecision::Pending,
        "a blocked authorization must not apply the decision"
    );
    assert_eq!(still_pending.display_command, original.display_command);
    assert_eq!(still_pending.risk_level, original.risk_level);
    assert_eq!(still_pending.risk_reasons, original.risk_reasons);
}

/// RFC-033 PR-033-D: `transcript_purge`'s required gate, per the
/// handoff -- record that a purge occurred and its scope, **never a
/// path, never a byte count**. The real transcript's own path and its
/// real byte count are both asserted absent from the persisted record's
/// own `Debug` text, not merely "no field happens to be named that" --
/// the same kind of direct proof
/// `local_data_summary_counts_retained_bytes_without_transcript_content`
/// already uses for a comparable secret-content concern.
#[test]
fn purge_persists_a_completed_record_naming_only_the_project_scope() {
    let dirs = TestAuditDirs::new("integration-transcript-purge");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let mut project = project_for(&dirs, 1);
    let real_bytes = b"a real transcript's real, sensitive content";
    attach_real_transcript(&mut project, &dirs, real_bytes);
    let transcript_path = dirs.base.join("state").join("transcript.log");

    let result =
        AuditCoordinator::new(&mut store, &mut health).purge_project_transcripts(&mut project);

    let summary = result
        .value
        .expect("a real, non-project-local transcript purges cleanly");
    assert_eq!(summary.purged_transcripts, 1);
    assert_eq!(summary.bytes_removed, real_bytes.len() as u64);
    assert_eq!(result.audit_status, AuditObservationStatus::Persisted);
    assert_eq!(health.status(), AuditHealthStatus::Healthy);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(
        records.len(),
        1,
        "single-phase family, per valid_transcript_purge's own Completed/Failed-only outcome"
    );
    let record = &records[0];
    assert_eq!(record.family, AuditEventFamily::TranscriptPurge);
    assert_eq!(
        record.action_kind,
        crate::audit::AuditActionKind::TranscriptPurge
    );
    assert_eq!(record.outcome, AuditOutcome::Completed);
    assert_eq!(record.project_id, Some(project.id().clone()));
    assert_eq!(record.subject_kind, Some(AuditSubjectKind::Transcript));
    assert_eq!(
        record
            .subject_ref
            .as_ref()
            .map(|reference| reference.as_str()),
        Some("project")
    );
    assert!(record.operation_id.is_none());
    assert!(record.terminal_id.is_none());
    assert!(record.agent_run_id.is_none());
    assert!(record.approval_id.is_none());
    assert!(record.risk_level.is_none());
    assert!(record.adapter_profile_ref.is_none());
    assert!(record.reason_code.is_none());
    // Every field `DurableAuditRecordV1` has is now asserted above --
    // the exhaustive check IS the "no byte count" proof: there is no
    // field left the count could have gone into. A `record_debug`
    // substring check for a *number* was tried and dropped: short byte
    // counts (e.g. `45`) collide with digits inside `event_id`'s own
    // UUID and `created_at`'s own timestamp often enough to make that
    // assertion flaky rather than meaningful.

    let record_debug = format!("{record:?}");
    assert!(
        !record_debug.contains(transcript_path.to_str().unwrap()),
        "the real transcript path must never reach the audit record: {record_debug}"
    );
    assert!(
        !record_debug.contains("sensitive content"),
        "no transcript content must ever reach the audit record: {record_debug}"
    );
}

/// The other outcome `valid_transcript_purge` permits: `Failed`, still
/// recorded rather than silently dropped -- the deletion itself refused
/// (`UnsafeProjectPath`, already tested at the model layer), and the
/// audit trail says so, not nothing.
#[test]
fn purge_failure_still_persists_a_failed_record() {
    let dirs = TestAuditDirs::new("integration-transcript-purge-failure");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let mut project = project_for(&dirs, 1);
    attach_project_local_transcript(&mut project, &dirs);

    let result =
        AuditCoordinator::new(&mut store, &mut health).purge_project_transcripts(&mut project);

    result
        .value
        .expect_err("a project-local transcript path must be refused, not purged");
    assert_eq!(result.audit_status, AuditObservationStatus::Persisted);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, AuditOutcome::Failed);
    assert_eq!(records[0].family, AuditEventFamily::TranscriptPurge);
}

/// Response 278's own reasoning, proven directly: the deletion already
/// took effect on the real filesystem by the time the record is built,
/// so a write failure for that one record must not be able to claim the
/// deletion itself did not happen.
#[test]
fn purge_write_failure_degrades_health_but_the_deletion_already_happened() {
    let dirs = TestAuditDirs::new("integration-transcript-purge-write-failure");
    let mut project = project_for(&dirs, 1);
    let real_bytes = b"real bytes that must actually be removed from disk";
    attach_real_transcript(&mut project, &dirs, real_bytes);
    let transcript_path = dirs.base.join("state").join("transcript.log");
    let mut health = AuditHealth::default();
    let mut failing = RecordingWriter::fail_on(1);

    let result = AuditCoordinator::with_writer(&mut failing, &mut health)
        .purge_project_transcripts(&mut project);

    let summary = result
        .value
        .expect("a degraded audit write must not roll back a deletion that already happened");
    assert_eq!(summary.bytes_removed, real_bytes.len() as u64);
    assert!(
        !transcript_path.exists(),
        "the real file must be gone from disk regardless of the audit write's own outcome"
    );
    assert_eq!(result.audit_status, AuditObservationStatus::Degraded);
    assert_eq!(health.failure_count(), 1);
}

/// RFC-039 PR-039-C, `what-closing-a-project-must-not-lose.md` §4:
/// `safe_close_decision`'s first producer pair, the confirmed-and-closed
/// path -- `record_safe_close_authorized` (phase one) then
/// `record_safe_close_decision` (phase two,
/// `terminal_process_groups_confirmed_empty: true` since every
/// terminated terminal reported a real exit, not an orphan)
/// maps to `Applied`, carrying the same `operation_id` phase one
/// authorized. `AuditStore`'s own schema enforces this pairing: a phase
/// two write with no matching `Authorized` record for its `operation_id`
/// is rejected outright (`missing_authorization_rejects_a_bare_closed_record`
/// below proves it), the same two-phase discipline
/// `ManagedProcessLifecycle` already has.
#[test]
fn safe_close_decision_persists_an_applied_record_with_its_operation_id() {
    let dirs = TestAuditDirs::new("integration-safe-close-applied");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::for_test(1);
    let operation_id = AuditOperationId::for_test(1);
    let mut coordinator = AuditCoordinator::new(&mut store, &mut health);

    let authorized_status =
        coordinator.record_safe_close_authorized(project_id.clone(), operation_id.clone());
    assert_eq!(authorized_status, AuditObservationStatus::Persisted);

    let status = coordinator.record_safe_close_decision(
        project_id.clone(),
        SafeCloseDecision::Closed {
            operation_id: operation_id.clone(),
            terminal_process_groups_confirmed_empty: true,
        },
    );

    assert_eq!(status, AuditObservationStatus::Persisted);
    assert_eq!(health.status(), AuditHealthStatus::Healthy);

    let records = store
        .query(&AuditQuery::latest(10))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2, "one record per phase");
    let applied = records
        .iter()
        .find(|record| record.outcome == AuditOutcome::Applied)
        .expect("the second phase's own record");
    assert_eq!(applied.family, AuditEventFamily::SafeCloseDecision);
    assert_eq!(applied.action_kind, AuditActionKind::SafeCloseTerminate);
    assert_eq!(applied.project_id, Some(project_id));
    assert_eq!(applied.operation_id, Some(operation_id));
    assert!(applied.subject_kind.is_none());
    assert!(applied.subject_ref.is_none());
    assert!(applied.terminal_id.is_none());
    assert!(applied.agent_run_id.is_none());
}

/// The uncertain half of the same decision: at least one terminated
/// terminal's own outcome was ambiguous (`OrphanedUnknown`/`Failed`),
/// so the project was still removed but the record says `Failed`, not
/// `Applied` -- honest that termination was not confirmed, matching
/// `record_plain_terminal_terminated`'s own refusal to guess a reason
/// code for the same two outcomes.
#[test]
fn safe_close_decision_persists_a_failed_record_when_termination_was_not_confirmed() {
    let dirs = TestAuditDirs::new("integration-safe-close-unconfirmed");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::for_test(2);
    let operation_id = AuditOperationId::for_test(2);
    let mut coordinator = AuditCoordinator::new(&mut store, &mut health);
    coordinator.record_safe_close_authorized(project_id.clone(), operation_id.clone());

    coordinator.record_safe_close_decision(
        project_id,
        SafeCloseDecision::Closed {
            operation_id,
            terminal_process_groups_confirmed_empty: false,
        },
    );

    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .any(|record| record.record.outcome == AuditOutcome::Failed)
    );
}

/// The two-phase enforcement itself, proven rather than assumed: a
/// `Closed` record with no preceding `record_safe_close_authorized` call
/// for its own `operation_id` is refused, not silently accepted --
/// `AuditStore`'s own schema-level guard against a phase-two write with
/// nothing to authorize it (`AuditStoreErrorReason::MissingAuthorization`,
/// surfaced here only as `Degraded` since `append_observation` never
/// exposes the reason to its caller, matching every other best-effort
/// producer in this file).
#[test]
fn missing_authorization_rejects_a_bare_closed_record() {
    let dirs = TestAuditDirs::new("integration-safe-close-missing-authorization");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::for_test(4);
    let operation_id = AuditOperationId::for_test(4);

    let status = AuditCoordinator::new(&mut store, &mut health).record_safe_close_decision(
        project_id,
        SafeCloseDecision::Closed {
            operation_id,
            terminal_process_groups_confirmed_empty: true,
        },
    );

    assert_eq!(status, AuditObservationStatus::Degraded);
    assert_eq!(health.status(), AuditHealthStatus::Degraded);
    assert!(
        store
            .query(&AuditQuery::latest(10))
            .unwrap()
            .records
            .is_empty(),
        "a rejected write must not persist anything"
    );
}

/// The declined half of `safe_close_decision`: `valid_safe_close`
/// requires `Cancelled` to carry **no** `operation_id` -- there was no
/// termination operation, because the user never authorized one.
#[test]
fn safe_close_decision_persists_a_cancelled_record_with_no_operation_id() {
    let dirs = TestAuditDirs::new("integration-safe-close-cancelled");
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let mut health = AuditHealth::default();
    let project_id = ProjectId::for_test(3);

    let status = AuditCoordinator::new(&mut store, &mut health)
        .record_safe_close_decision(project_id.clone(), SafeCloseDecision::Cancelled);

    assert_eq!(status, AuditObservationStatus::Persisted);
    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    assert_eq!(records.len(), 1);
    let record = &records[0].record;
    assert_eq!(record.outcome, AuditOutcome::Cancelled);
    assert_eq!(record.project_id, Some(project_id));
    assert!(
        record.operation_id.is_none(),
        "a cancelled close has no termination operation to name"
    );
}

fn project_for(dirs: &TestAuditDirs, sequence: u64) -> ProjectSession {
    let root = dirs.base.join("project");
    ProjectSession::new(ProjectId::for_test(sequence), "Project", &root, &root)
}

/// RFC-033 PR-033-D: a real transcript, with real bytes on a real file,
/// attached to `project` -- the minimum `purge_project_transcripts`
/// needs something real to delete. `storage_path` lives under
/// `dirs.base.join("state")`, outside the project root `project_for`
/// uses, so a normal call is not refused by `UnsafeProjectPath`;
/// `attach_project_local_transcript` below is the deliberate opposite,
/// for proving the `Failed` outcome is still recorded.
fn attach_real_transcript(project: &mut ProjectSession, dirs: &TestAuditDirs, bytes: &[u8]) {
    let transcript_path = dirs.base.join("state").join("transcript.log");
    fs::create_dir_all(transcript_path.parent().unwrap()).unwrap();
    fs::write(&transcript_path, bytes).unwrap();

    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Supervised,
        "Agent",
        dirs.base.join("project"),
        "agent-cli",
    );
    let transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        None,
        &transcript_path,
        "local-bounded-agent-run",
    );
    project.add_terminal_session(terminal).unwrap();
    project.add_transcript(transcript).unwrap();
}

/// The deliberate `UnsafeProjectPath` trigger: a transcript record whose
/// own `storage_path` resolves *inside* the project's canonical root --
/// `transcript_path_is_project_local`'s own refusal condition, already
/// tested at the model layer
/// (`transcript_purge_never_deletes_project_files`,
/// `crates/tekstide-core/src/project/tests/transcripts.rs`). Used here
/// only to prove the coordinator still records `Failed` rather than
/// silently dropping the audit trail when the underlying purge itself
/// refuses.
fn attach_project_local_transcript(project: &mut ProjectSession, dirs: &TestAuditDirs) {
    let project_root = dirs.base.join("project");
    let transcript_path = project_root.join("inside-project.log");
    fs::write(
        &transcript_path,
        b"must not be treated as a real transcript store",
    )
    .unwrap();

    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Supervised,
        "Agent",
        &project_root,
        "agent-cli",
    );
    let transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        None,
        &transcript_path,
        "local-bounded-agent-run",
    );
    project.add_terminal_session(terminal).unwrap();
    project.add_transcript(transcript).unwrap();
}

fn launch_plan_for(project: &ProjectSession, executable: &Path) -> AgentRunLaunchPlan {
    launch_plan_for_level(project, executable, AgentCompatibilityLevel::Supervised)
}

fn launch_plan_for_level(
    project: &ProjectSession,
    executable: &Path,
    compatibility_level: AgentCompatibilityLevel,
) -> AgentRunLaunchPlan {
    let mut profile = AiCliProfile::new(
        "builtin-ai",
        "Built-in AI",
        AiCliProfileSource::BuiltIn,
        AiCliExecutable::Absolute {
            path: executable.to_path_buf(),
            provenance: AiCliExecutableProvenance::SystemPathReviewed,
        },
        compatibility_level,
    );
    profile.workspace_discovery_policy = AiCliWorkspaceDiscoveryPolicy::DisabledByLaunch {
        evidence: "reviewed project-config discovery disable flag".to_owned(),
    };
    let request = AgentRunLaunchRequest::new(
        project.id().clone(),
        profile.id.clone(),
        "private prompt sentinel",
    );
    let validation = AgentRunLaunchValidator
        .validate(project, &profile, &request)
        .unwrap();
    AgentRunLaunchPlan::from_validation(validation, "private command sentinel").unwrap()
}

struct RecordingWriter {
    records: Vec<DurableAuditRecordV1>,
    fail_on_attempt: usize,
    attempt_count: usize,
}

impl RecordingWriter {
    fn fail_on(attempt: usize) -> Self {
        Self {
            records: Vec::new(),
            fail_on_attempt: attempt,
            attempt_count: 0,
        }
    }
}

impl AuditRecordWriter for RecordingWriter {
    fn append_record(&mut self, record: &DurableAuditRecordV1) -> Result<(), AuditStoreError> {
        self.attempt_count += 1;
        if self.attempt_count == self.fail_on_attempt {
            return Err(AuditStoreError::new(AuditStoreErrorReason::StorageFull));
        }
        self.records.push(record.clone());
        Ok(())
    }
}
