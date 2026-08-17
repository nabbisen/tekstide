use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audit::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome,
    AuditPathRequest, AuditPathResolver, AuditReasonCode, AuditReference, AuditStoragePath,
    DurableAuditRecordV1,
};
use crate::domain::{AgentRunId, AuditOperationId, TerminalId};
use crate::project::ProjectId;

pub struct TestAuditDirs {
    pub base: PathBuf,
    pub storage_path: AuditStoragePath,
}

impl TestAuditDirs {
    pub fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-audit-store-{label}-{}-{unique}",
            std::process::id()
        ));
        let state_root = base.join("state");
        let project_root = base.join("project");
        fs::create_dir_all(&state_root).unwrap();
        fs::create_dir_all(&project_root).unwrap();
        let storage_path = AuditPathResolver
            .resolve(AuditPathRequest::new(
                &state_root,
                vec![project_root.clone()],
            ))
            .unwrap();
        Self { base, storage_path }
    }
}

impl Drop for TestAuditDirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

pub fn project_added(project_id: ProjectId) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::ProjectAdded,
        AuditOutcome::Applied,
        AuditActionKind::ProjectAdd,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    record.project_id = Some(project_id);
    record
}

pub fn trust_authorized(
    project_id: ProjectId,
    operation_id: AuditOperationId,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::TrustChange,
        AuditOutcome::Authorized,
        AuditActionKind::TrustGrant,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(project_id);
    record.operation_id = Some(operation_id);
    record
}

pub fn trust_applied(authorization: &DurableAuditRecordV1) -> DurableAuditRecordV1 {
    let mut record = authorization.clone();
    record.event_id = crate::domain::AuditEventId::new_uuid();
    record.outcome = AuditOutcome::Applied;
    record
}

/// The single-phase shape `AuditCoordinator::revoke_project_trust`
/// actually writes -- unlike `trust_authorized`/`trust_applied`, no
/// `operation_id` (`valid_trust_change` requires `TrustRevoke` records
/// to have none).
pub fn trust_revoked(project_id: ProjectId) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::TrustChange,
        AuditOutcome::Applied,
        AuditActionKind::TrustRevoke,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(project_id);
    record
}

pub fn managed_authorized(
    project_id: ProjectId,
    operation_id: AuditOperationId,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::ManagedProcessLifecycle,
        AuditOutcome::Authorized,
        AuditActionKind::ManagedAgentLaunch,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    record.project_id = Some(project_id);
    record.agent_run_id = Some(AgentRunId::new_uuid());
    record.operation_id = Some(operation_id);
    record.adapter_profile_ref = AuditReference::new("profile:codex");
    record
}

pub fn managed_started(authorization: &DurableAuditRecordV1) -> DurableAuditRecordV1 {
    let mut record = authorization.clone();
    record.event_id = crate::domain::AuditEventId::new_uuid();
    record.outcome = AuditOutcome::Started;
    record.actor_kind = AuditActorKind::Runtime;
    record.action_source = AuditActionSource::RuntimeObserver;
    record.terminal_id = Some(TerminalId::new_uuid());
    record
}

pub fn managed_failed(authorization: &DurableAuditRecordV1) -> DurableAuditRecordV1 {
    let mut record = authorization.clone();
    record.event_id = crate::domain::AuditEventId::new_uuid();
    record.outcome = AuditOutcome::Failed;
    record.actor_kind = AuditActorKind::Runtime;
    record.action_source = AuditActionSource::RuntimeObserver;
    record.reason_code = Some(AuditReasonCode::RuntimeFailure);
    record
}

pub fn managed_terminated(started: &DurableAuditRecordV1) -> DurableAuditRecordV1 {
    let mut record = started.clone();
    record.event_id = crate::domain::AuditEventId::new_uuid();
    record.outcome = AuditOutcome::Terminated;
    record.reason_code = Some(AuditReasonCode::ProcessExited);
    record
}
