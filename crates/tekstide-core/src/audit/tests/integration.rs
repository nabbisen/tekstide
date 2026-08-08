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
    AuditCoordinator, AuditEventFamily, AuditHealth, AuditHealthStatus, AuditIntegrationError,
    AuditObservationStatus, AuditOutcome, AuditQuery, AuditReasonCode, AuditStore, AuditStoreError,
    AuditStoreErrorReason, CommandDecisionActionKind, DurableAuditRecordV1,
};
use crate::domain::{
    AgentCompatibilityLevel, AgentRunId, AgentRunStatus, ApprovalDecision, ApprovalId, RiskLevel,
    TerminalId, TerminalStatus,
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

fn project_for(dirs: &TestAuditDirs, sequence: u64) -> ProjectSession {
    let root = dirs.base.join("project");
    ProjectSession::new(ProjectId::for_test(sequence), "Project", &root, &root)
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
