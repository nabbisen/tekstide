use std::path::Path;

use crate::agent::AgentRunLaunchPlan;
use crate::content::{SaveDecision, TextDocumentOpenError, TextDocumentSaveError};
use crate::domain::{
    AgentCompatibilityLevel, AgentRunId, AuditEventId, AuditOperationId, TerminalId,
};
use crate::project::root::{FileAccessBlockedReason, FileAccessError};
use crate::project::{ProjectAgentRuntimeLaunchError, ProjectContentError, ProjectSession};
use crate::runtime::terminal::{LinuxTerminalRuntime, TerminalRuntimeEvent, TerminationOutcome};

use super::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome,
    AuditReasonCode, AuditReference, AuditStore, AuditStoreError, AuditStoreErrorReason,
    DurableAuditRecordV1,
};

pub(crate) trait AuditRecordWriter {
    fn append_record(&mut self, record: &DurableAuditRecordV1) -> Result<(), AuditStoreError>;
}

impl AuditRecordWriter for AuditStore {
    fn append_record(&mut self, record: &DurableAuditRecordV1) -> Result<(), AuditStoreError> {
        self.append(record).map(|_| ())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AuditHealthStatus {
    #[default]
    Healthy,
    Degraded,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuditHealth {
    status: AuditHealthStatus,
    failure_count: u32,
    last_failure: Option<AuditStoreErrorReason>,
}

impl AuditHealth {
    pub fn status(&self) -> AuditHealthStatus {
        self.status
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    pub fn last_failure(&self) -> Option<AuditStoreErrorReason> {
        self.last_failure
    }

    fn record_failure(&mut self, reason: AuditStoreErrorReason) {
        self.status = AuditHealthStatus::Degraded;
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_failure = Some(reason);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditObservationStatus {
    NotRequired,
    Persisted,
    Degraded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditActionResult<T> {
    pub value: T,
    pub audit_status: AuditObservationStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditedAgentLaunch {
    agent_run_id: AgentRunId,
    terminal_id: TerminalId,
    operation_id: AuditOperationId,
    adapter_profile_ref: AuditReference,
    pub runtime_events: Vec<TerminalRuntimeEvent>,
}

impl AuditedAgentLaunch {
    pub fn agent_run_id(&self) -> &AgentRunId {
        &self.agent_run_id
    }

    pub fn terminal_id(&self) -> &TerminalId {
        &self.terminal_id
    }

    pub fn operation_id(&self) -> &AuditOperationId {
        &self.operation_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditIntegrationError {
    RequiredAuditUnavailable(AuditStoreErrorReason),
    InvalidTypedContext,
    AgentLaunch(ProjectAgentRuntimeLaunchError),
}

pub struct AuditCoordinator<'a> {
    writer: &'a mut dyn AuditRecordWriter,
    health: &'a mut AuditHealth,
}

impl<'a> AuditCoordinator<'a> {
    pub fn new(store: &'a mut AuditStore, health: &'a mut AuditHealth) -> Self {
        Self {
            writer: store,
            health,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_writer(
        writer: &'a mut dyn AuditRecordWriter,
        health: &'a mut AuditHealth,
    ) -> Self {
        Self { writer, health }
    }

    pub fn grant_project_trust(
        &mut self,
        project: &mut ProjectSession,
    ) -> Result<AuditActionResult<()>, AuditIntegrationError> {
        let operation_id = AuditOperationId::new_uuid();
        let mut authorization = trust_record(
            project,
            AuditOutcome::Authorized,
            AuditActionKind::TrustGrant,
        );
        authorization.operation_id = Some(operation_id.clone());
        self.append_required(&authorization)?;

        project.grant_trust("workspace trust granted");

        let mut applied = authorization;
        applied.event_id = AuditEventId::new_uuid();
        applied.outcome = AuditOutcome::Applied;
        applied.operation_id = Some(operation_id);
        let audit_status = self.append_observation(&applied);

        Ok(AuditActionResult {
            value: (),
            audit_status,
        })
    }

    pub fn revoke_project_trust(&mut self, project: &mut ProjectSession) -> AuditActionResult<()> {
        project.revoke_trust("workspace trust revoked");
        let record = trust_record(project, AuditOutcome::Applied, AuditActionKind::TrustRevoke);
        let audit_status = self.append_observation(&record);
        AuditActionResult {
            value: (),
            audit_status,
        }
    }

    pub fn launch_managed_agent_run(
        &mut self,
        project: &mut ProjectSession,
        mut plan: AgentRunLaunchPlan,
        runtime: &mut LinuxTerminalRuntime,
    ) -> Result<AuditActionResult<AuditedAgentLaunch>, AuditIntegrationError> {
        if plan.spec().compatibility_level() == AgentCompatibilityLevel::Plain {
            return Err(AuditIntegrationError::InvalidTypedContext);
        }
        let adapter_profile_ref = AuditReference::new(plan.spec().profile_id())
            .ok_or(AuditIntegrationError::InvalidTypedContext)?;
        let project_id = project.id().clone();
        let agent_run_id = plan.agent_run().id.clone();
        if plan.spec().project_id() != &project_id || plan.agent_run().project_id != project_id {
            return Err(AuditIntegrationError::InvalidTypedContext);
        }

        project
            .prepare_agent_run_launch(&mut plan)
            .map_err(AuditIntegrationError::AgentLaunch)?;

        let operation_id = AuditOperationId::new_uuid();
        let authorization = managed_process_record(
            project.id().clone(),
            agent_run_id.clone(),
            operation_id.clone(),
            adapter_profile_ref.clone(),
            AuditOutcome::Authorized,
        );
        self.append_required(&authorization)?;

        let launch = project.launch_prepared_agent_run_with_runtime(plan, runtime);
        let (launched_agent_run_id, runtime_events) = match launch {
            Ok(launch) => launch,
            Err(error) => {
                let mut failed = managed_process_record(
                    project.id().clone(),
                    agent_run_id,
                    operation_id,
                    adapter_profile_ref,
                    AuditOutcome::Failed,
                );
                failed.reason_code = Some(AuditReasonCode::RuntimeFailure);
                self.append_observation(&failed);
                return Err(AuditIntegrationError::AgentLaunch(error));
            }
        };

        if launched_agent_run_id != agent_run_id {
            return Err(AuditIntegrationError::InvalidTypedContext);
        }
        let terminal_id = project
            .agent_runs()
            .iter()
            .find(|run| run.id == launched_agent_run_id && run.project_id == *project.id())
            .and_then(|run| run.terminal_id.clone())
            .filter(|terminal_id| {
                project
                    .terminal_session(terminal_id)
                    .is_some_and(|terminal| terminal.project_id == *project.id())
            })
            .ok_or(AuditIntegrationError::InvalidTypedContext)?;

        let mut started = managed_process_record(
            project.id().clone(),
            launched_agent_run_id.clone(),
            operation_id.clone(),
            adapter_profile_ref.clone(),
            AuditOutcome::Started,
        );
        started.terminal_id = Some(terminal_id.clone());
        let audit_status = self.append_observation(&started);

        Ok(AuditActionResult {
            value: AuditedAgentLaunch {
                agent_run_id: launched_agent_run_id,
                terminal_id,
                operation_id,
                adapter_profile_ref,
                runtime_events,
            },
            audit_status,
        })
    }

    pub fn apply_managed_agent_terminal_outcome(
        &mut self,
        project: &mut ProjectSession,
        launch: &AuditedAgentLaunch,
        outcome: &TerminationOutcome,
    ) -> Result<AuditActionResult<()>, AuditIntegrationError> {
        let owns_links = project.agent_runs().iter().any(|run| {
            run.id == launch.agent_run_id
                && run.project_id == *project.id()
                && run.terminal_id.as_ref() == Some(&launch.terminal_id)
        }) && project
            .terminal_session(&launch.terminal_id)
            .is_some_and(|terminal| terminal.project_id == *project.id());
        if !owns_links {
            return Err(AuditIntegrationError::InvalidTypedContext);
        }

        project
            .apply_agent_terminal_outcome(&launch.agent_run_id, &launch.terminal_id, outcome)
            .map_err(AuditIntegrationError::AgentLaunch)?;

        let reason_code = match outcome {
            TerminationOutcome::Exited { .. } => AuditReasonCode::ProcessExited,
            TerminationOutcome::TerminatedBySignal { .. }
            | TerminationOutcome::KilledAfterTimeout { .. } => AuditReasonCode::ProcessTerminated,
            TerminationOutcome::OrphanedUnknown { .. } | TerminationOutcome::Failed { .. } => {
                return Ok(AuditActionResult {
                    value: (),
                    audit_status: AuditObservationStatus::NotRequired,
                });
            }
        };

        let mut terminated = managed_process_record(
            project.id().clone(),
            launch.agent_run_id.clone(),
            launch.operation_id.clone(),
            launch.adapter_profile_ref.clone(),
            AuditOutcome::Terminated,
        );
        terminated.terminal_id = Some(launch.terminal_id.clone());
        terminated.reason_code = Some(reason_code);
        let audit_status = self.append_observation(&terminated);

        Ok(AuditActionResult {
            value: (),
            audit_status,
        })
    }

    pub fn open_project_text_document(
        &mut self,
        project: &mut ProjectSession,
        selected_relative_path: impl AsRef<Path>,
    ) -> Result<AuditActionResult<()>, ProjectContentError> {
        match project.open_text_document(selected_relative_path) {
            Ok(()) => Ok(AuditActionResult {
                value: (),
                audit_status: AuditObservationStatus::NotRequired,
            }),
            Err(error) => {
                if let Some(reason) = root_block_reason(&error, project) {
                    let record = root_access_blocked_record(project, reason);
                    self.append_observation(&record);
                }
                Err(error)
            }
        }
    }

    pub fn save_project_text_document(
        &mut self,
        project: &mut ProjectSession,
    ) -> Result<AuditActionResult<SaveDecision>, ProjectContentError> {
        match project.save_active_text_document() {
            Ok(decision) => Ok(AuditActionResult {
                value: decision,
                audit_status: AuditObservationStatus::NotRequired,
            }),
            Err(error) => {
                if let Some(reason) = root_block_reason(&error, project) {
                    let record = root_access_blocked_record(project, reason);
                    self.append_observation(&record);
                }
                Err(error)
            }
        }
    }

    fn append_required(
        &mut self,
        record: &DurableAuditRecordV1,
    ) -> Result<(), AuditIntegrationError> {
        self.writer.append_record(record).map_err(|error| {
            self.health.record_failure(error.reason);
            AuditIntegrationError::RequiredAuditUnavailable(error.reason)
        })
    }

    fn append_observation(&mut self, record: &DurableAuditRecordV1) -> AuditObservationStatus {
        match self.writer.append_record(record) {
            Ok(()) => AuditObservationStatus::Persisted,
            Err(error) => {
                self.health.record_failure(error.reason);
                AuditObservationStatus::Degraded
            }
        }
    }
}

fn trust_record(
    project: &ProjectSession,
    outcome: AuditOutcome,
    action_kind: AuditActionKind,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::TrustChange,
        outcome,
        action_kind,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(project.id().clone());
    record
}

fn managed_process_record(
    project_id: crate::project::ProjectId,
    agent_run_id: AgentRunId,
    operation_id: AuditOperationId,
    adapter_profile_ref: AuditReference,
    outcome: AuditOutcome,
) -> DurableAuditRecordV1 {
    let (actor_kind, action_source) = if outcome == AuditOutcome::Authorized {
        (AuditActorKind::User, AuditActionSource::TrustedUi)
    } else {
        (AuditActorKind::Runtime, AuditActionSource::RuntimeObserver)
    };
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::ManagedProcessLifecycle,
        outcome,
        AuditActionKind::ManagedAgentLaunch,
        actor_kind,
        action_source,
    );
    record.project_id = Some(project_id);
    record.agent_run_id = Some(agent_run_id);
    record.operation_id = Some(operation_id);
    record.adapter_profile_ref = Some(adapter_profile_ref);
    record
}

fn root_access_blocked_record(
    project: &ProjectSession,
    reason: AuditReasonCode,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::RootAccessBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::RootAccess,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(project.id().clone());
    record.reason_code = Some(reason);
    record
}

fn root_block_reason(
    error: &ProjectContentError,
    project: &ProjectSession,
) -> Option<AuditReasonCode> {
    let access = match error {
        ProjectContentError::Open(TextDocumentOpenError::Access(access)) => Some(access),
        ProjectContentError::Save(
            TextDocumentSaveError::Access(access) | TextDocumentSaveError::RootEscape(access),
        ) => Some(access),
        _ => None,
    }?;
    if access.project_id != *project.id() {
        return None;
    }
    map_root_reason(access)
}

fn map_root_reason(access: &FileAccessError) -> Option<AuditReasonCode> {
    match access.reason {
        FileAccessBlockedReason::RootEscape => Some(AuditReasonCode::RootEscape),
        FileAccessBlockedReason::SymlinkEscape => Some(AuditReasonCode::SymlinkEscape),
        _ => None,
    }
}
