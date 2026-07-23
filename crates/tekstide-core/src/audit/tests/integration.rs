use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::Duration;

use crate::agent::{
    AgentRunLaunchPlan, AgentRunLaunchRequest, AgentRunLaunchValidator, AiCliExecutable,
    AiCliExecutableProvenance, AiCliProfile, AiCliProfileSource, AiCliWorkspaceDiscoveryPolicy,
};
use crate::audit::integration::AuditRecordWriter;
use crate::audit::{
    AuditCoordinator, AuditEventFamily, AuditHealth, AuditHealthStatus, AuditIntegrationError,
    AuditObservationStatus, AuditOutcome, AuditQuery, AuditReasonCode, AuditStore, AuditStoreError,
    AuditStoreErrorReason, DurableAuditRecordV1,
};
use crate::domain::{AgentCompatibilityLevel, AgentRunStatus, TerminalStatus};
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
