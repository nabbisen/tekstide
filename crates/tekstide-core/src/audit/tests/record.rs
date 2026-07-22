use crate::audit::{
    AUDIT_RECORD_SCHEMA_VERSION, AuditActionKind, AuditActionSource, AuditActorKind,
    AuditEventFamily, AuditOutcome, AuditReasonCode, AuditRecordValidationErrorReason,
    AuditReference, AuditRiskLevel, AuditSubjectKind, DurableAuditRecordV1,
};
use crate::domain::{AgentRunId, ApprovalId, AuditOperationId, TerminalId, TranscriptId};
use crate::project::ProjectId;

#[test]
fn retained_v1_families_accept_their_required_field_contracts() {
    for record in valid_family_records() {
        record
            .validate()
            .unwrap_or_else(|error| panic!("{:?} rejected: {error:?}", record.family));
    }
}

#[test]
fn invalid_actor_source_pair_is_rejected() {
    let mut record = project_added();
    record.actor_kind = AuditActorKind::Runtime;
    record.action_source = AuditActionSource::TrustedUi;

    let error = record.validate().unwrap_err();

    assert_eq!(
        error.reason,
        AuditRecordValidationErrorReason::InvalidActorSource
    );
}

#[test]
fn unsupported_schema_and_incomplete_subject_are_rejected() {
    let mut wrong_version = project_added();
    wrong_version.schema_version = AUDIT_RECORD_SCHEMA_VERSION + 1;
    assert_eq!(
        wrong_version.validate().unwrap_err().reason,
        AuditRecordValidationErrorReason::UnsupportedSchemaVersion
    );

    let mut incomplete_subject = transcript_purge();
    incomplete_subject.subject_ref = None;
    assert_eq!(
        incomplete_subject.validate().unwrap_err().reason,
        AuditRecordValidationErrorReason::IncompleteSubject
    );
}

#[test]
fn unrelated_entity_and_phase_combinations_are_rejected() {
    let mut project = project_added();
    project.terminal_id = Some(TerminalId::for_test(1));
    assert_invalid_family(project);

    let mut revoke = trust_revoke();
    revoke.operation_id = Some(AuditOperationId::for_test(1));
    assert_invalid_family(revoke);

    let mut managed = managed_authorized();
    managed.outcome = AuditOutcome::Started;
    managed.actor_kind = AuditActorKind::Runtime;
    managed.action_source = AuditActionSource::RuntimeObserver;
    assert_invalid_family(managed);

    let mut root = root_blocked();
    root.reason_code = Some(AuditReasonCode::RuntimeFailure);
    assert_invalid_family(root);

    let mut recovery = store_recovery();
    recovery.outcome = AuditOutcome::Failed;
    assert_invalid_family(recovery);

    let mut close = safe_close_authorized();
    close.subject_kind = Some(AuditSubjectKind::Transcript);
    close.subject_ref = AuditReference::new("transcript-opaque-id");
    assert_invalid_family(close);
}

#[test]
fn authorization_shapes_require_operation_ids() {
    let mut trust = trust_grant_authorized();
    trust.operation_id = None;
    assert_invalid_family(trust);

    let mut approval = command_approved_authorized();
    approval.operation_id = None;
    assert_invalid_family(approval);

    let mut launch = managed_authorized();
    launch.operation_id = None;
    assert_invalid_family(launch);

    let mut close = safe_close_authorized();
    close.operation_id = None;
    assert_invalid_family(close);
}

#[test]
fn audit_references_accept_identifiers_and_reject_content_or_paths() {
    assert_eq!(
        AuditReference::new("profile.codex-v1").unwrap().as_str(),
        "profile.codex-v1"
    );
    assert!(AuditReference::new(TranscriptId::for_test(1).as_str()).is_some());
    assert!(AuditReference::new("/workspace/private/file.rs").is_none());
    assert!(AuditReference::new("secret command --token value").is_none());
    assert!(AuditReference::new("line\nbreak").is_none());
    assert!(AuditReference::new("x".repeat(129)).is_none());
}

fn valid_family_records() -> Vec<DurableAuditRecordV1> {
    vec![
        project_added(),
        trust_grant_authorized(),
        trust_revoke(),
        command_requested(),
        command_approved_authorized(),
        managed_authorized(),
        managed_started(),
        managed_failed(),
        managed_terminated(),
        plain_terminal_started(),
        paste_blocked(),
        restricted_mode_blocked(),
        root_blocked(),
        safe_close_authorized(),
        safe_close_cancelled(),
        config_increase_authorized(),
        config_reduce_applied(),
        transcript_purge(),
        store_recovery(),
    ]
}

fn record(
    family: AuditEventFamily,
    outcome: AuditOutcome,
    action: AuditActionKind,
    actor: AuditActorKind,
    source: AuditActionSource,
) -> DurableAuditRecordV1 {
    DurableAuditRecordV1::new(family, outcome, action, actor, source)
}

fn project_added() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::ProjectAdded,
        AuditOutcome::Applied,
        AuditActionKind::ProjectAdd,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record
}

fn trust_grant_authorized() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::TrustChange,
        AuditOutcome::Authorized,
        AuditActionKind::TrustGrant,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.operation_id = Some(AuditOperationId::for_test(1));
    record
}

fn trust_revoke() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::TrustChange,
        AuditOutcome::Applied,
        AuditActionKind::TrustRevoke,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record
}

fn command_requested() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::CommandApproval,
        AuditOutcome::Requested,
        AuditActionKind::CommandRequest,
        AuditActorKind::AppPolicy,
        AuditActionSource::Adapter,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.agent_run_id = Some(AgentRunId::for_test(1));
    record.approval_id = Some(ApprovalId::for_test(1));
    record.risk_level = Some(AuditRiskLevel::High);
    record.adapter_profile_ref = AuditReference::new("profile:codex");
    record
}

fn command_approved_authorized() -> DurableAuditRecordV1 {
    let mut record = command_requested();
    record.outcome = AuditOutcome::Authorized;
    record.action_kind = AuditActionKind::CommandApprove;
    record.actor_kind = AuditActorKind::User;
    record.action_source = AuditActionSource::TrustedUi;
    record.operation_id = Some(AuditOperationId::for_test(2));
    record
}

fn managed_authorized() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::ManagedProcessLifecycle,
        AuditOutcome::Authorized,
        AuditActionKind::ManagedAgentLaunch,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.agent_run_id = Some(AgentRunId::for_test(1));
    record.operation_id = Some(AuditOperationId::for_test(3));
    record.adapter_profile_ref = AuditReference::new("profile:codex");
    record
}

fn managed_started() -> DurableAuditRecordV1 {
    let mut record = managed_authorized();
    record.outcome = AuditOutcome::Started;
    record.actor_kind = AuditActorKind::Runtime;
    record.action_source = AuditActionSource::RuntimeObserver;
    record.terminal_id = Some(TerminalId::for_test(1));
    record
}

fn managed_failed() -> DurableAuditRecordV1 {
    let mut record = managed_authorized();
    record.outcome = AuditOutcome::Failed;
    record.actor_kind = AuditActorKind::Runtime;
    record.action_source = AuditActionSource::RuntimeObserver;
    record.reason_code = Some(AuditReasonCode::RuntimeFailure);
    record
}

fn managed_terminated() -> DurableAuditRecordV1 {
    let mut record = managed_started();
    record.outcome = AuditOutcome::Terminated;
    record.reason_code = Some(AuditReasonCode::ProcessExited);
    record
}

fn plain_terminal_started() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::PlainTerminalObservation,
        AuditOutcome::Started,
        AuditActionKind::PlainTerminalLifecycle,
        AuditActorKind::Runtime,
        AuditActionSource::RuntimeObserver,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.terminal_id = Some(TerminalId::for_test(1));
    record
}

fn paste_blocked() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::PasteBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::TerminalPaste,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.terminal_id = Some(TerminalId::for_test(1));
    record.reason_code = Some(AuditReasonCode::PastePolicy);
    record
}

fn restricted_mode_blocked() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::RestrictedModeBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::RestrictedFeature,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.reason_code = Some(AuditReasonCode::RestrictedMode);
    record
}

fn root_blocked() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::RootAccessBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::RootAccess,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.reason_code = Some(AuditReasonCode::SymlinkEscape);
    record
}

fn safe_close_authorized() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::SafeCloseDecision,
        AuditOutcome::Authorized,
        AuditActionKind::SafeCloseTerminate,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.operation_id = Some(AuditOperationId::for_test(4));
    record.subject_kind = Some(AuditSubjectKind::AppResource);
    record.subject_ref = AuditReference::new("terminal:1");
    record
}

fn safe_close_cancelled() -> DurableAuditRecordV1 {
    let mut record = safe_close_authorized();
    record.outcome = AuditOutcome::Cancelled;
    record.operation_id = None;
    record.reason_code = Some(AuditReasonCode::UserCancelled);
    record
}

fn config_increase_authorized() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::SensitiveConfigChanged,
        AuditOutcome::Authorized,
        AuditActionKind::ConfigPolicyIncrease,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.operation_id = Some(AuditOperationId::for_test(5));
    record.reason_code = Some(AuditReasonCode::PolicyChanged);
    record
}

fn config_reduce_applied() -> DurableAuditRecordV1 {
    let mut record = config_increase_authorized();
    record.outcome = AuditOutcome::Applied;
    record.action_kind = AuditActionKind::ConfigPolicyReduce;
    record.operation_id = None;
    record
}

fn transcript_purge() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::TranscriptPurge,
        AuditOutcome::Completed,
        AuditActionKind::TranscriptPurge,
        AuditActorKind::AppPolicy,
        AuditActionSource::ExplicitCleanup,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.agent_run_id = Some(AgentRunId::for_test(1));
    record.subject_kind = Some(AuditSubjectKind::Transcript);
    record.subject_ref = AuditReference::new(TranscriptId::for_test(1).as_str());
    record
}

fn store_recovery() -> DurableAuditRecordV1 {
    let mut record = record(
        AuditEventFamily::AuditStoreRecovery,
        AuditOutcome::Completed,
        AuditActionKind::AuditStoreRecovery,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    record.subject_kind = Some(AuditSubjectKind::RecoveryBundle);
    record.subject_ref = AuditReference::new("recovery:1");
    record.reason_code = Some(AuditReasonCode::RecoveryCompleted);
    record
}

fn assert_invalid_family(record: DurableAuditRecordV1) {
    assert_eq!(
        record.validate().unwrap_err().reason,
        AuditRecordValidationErrorReason::InvalidFamilyFields
    );
}
