use std::path::Path;

use crate::agent::AgentRunLaunchPlan;
use crate::content::{SaveDecision, TextDocumentOpenError, TextDocumentSaveError};
use crate::domain::{
    AgentCompatibilityLevel, AgentRunId, ApprovalId, AuditEventId, AuditOperationId, RiskLevel,
    TerminalId,
};
use crate::project::root::{FileAccessBlockedReason, FileAccessError};
use crate::project::{
    ProjectAgentRuntimeLaunchError, ProjectContentError, ProjectId, ProjectSession,
    ProjectTranscriptError, ProjectTranscriptPurgeSummary,
};
use crate::runtime::terminal::{LinuxTerminalRuntime, TerminalRuntimeEvent, TerminationOutcome};

use super::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome,
    AuditReasonCode, AuditReference, AuditStore, AuditStoreError, AuditStoreErrorReason,
    AuditSubjectKind, DurableAuditRecordV1,
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

/// RFC-047 PR-047-B: what happened the one time this session a broken
/// store got fixed -- distinct from [`AuditHealth`]'s own
/// status/failure_count/last_failure, which track whether the store
/// works *right now*. This is the one-time disclosure D2 requires
/// ("tell the user the path of the quarantined file"): once a recovery
/// completes, the *current* state can go back to healthy, but a user
/// who had audit history moved aside still needs to be told where,
/// which is a fact about what happened, not about what is true now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditRecoveryDisclosure {
    /// An interrupted-but-safe migration was retried to completion.
    /// Nothing was quarantined -- there is no path to report.
    Resumed,
    /// The store was unreadable; the old database was moved aside and a
    /// fresh one started. `quarantine_dir` is where the old one is --
    /// §3 of the risk document: without this, auto-recovering without
    /// asking first would not be defensible.
    Recovered { quarantine_dir: std::path::PathBuf },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuditHealth {
    status: AuditHealthStatus,
    failure_count: u32,
    last_failure: Option<AuditStoreErrorReason>,
    last_recovery: Option<AuditRecoveryDisclosure>,
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

    /// RFC-047 PR-047-B. `None` until a recovery has actually happened
    /// this session; never cleared afterward by anything in this crate
    /// -- a durably-visible fact for the rest of the session, not a
    /// toast, matching §3's "the path is the condition, not the
    /// courtesy" (a disclosure a user could miss is not much of one).
    pub fn last_recovery(&self) -> Option<&AuditRecoveryDisclosure> {
        self.last_recovery.as_ref()
    }

    /// RFC-047 PR-047-A: `pub`, not `pub(crate)` -- until now the only
    /// callers were `AuditCoordinator`'s own write-failure paths, inside
    /// this module. The seam `open_audit_store` (`tekstide` crate) now
    /// needs is the same fact recorded the same way when the store does
    /// not *open* at all, onto the one `AuditHealth` a session
    /// accumulates rather than the fresh, immediately-dropped instance
    /// every one of its fourteen call sites used to construct.
    pub fn record_failure(&mut self, reason: AuditStoreErrorReason) {
        self.status = AuditHealthStatus::Degraded;
        self.failure_count = self.failure_count.saturating_add(1);
        self.last_failure = Some(reason);
    }

    /// RFC-047 PR-047-B: called only after a recovery is confirmed
    /// complete *and* its own `AuditStoreRecovery` durable record is
    /// confirmed written -- the store now genuinely works, so leaving
    /// `status` at `Degraded` would misreport the current state the
    /// same direction §2 of the risk document warns against for a
    /// permanently-healthy line, just inverted: **the indicator must
    /// track what is true now, not what was true a moment ago in
    /// either direction.** `last_recovery` is untouched by future
    /// failures -- it is history, not a live gauge, and does not get
    /// cleared by this call either (a second recovery this session
    /// replaces it with the newer fact, which is correct: "here's what
    /// most recently happened," not a log of every event).
    pub fn record_recovery(&mut self, disclosure: AuditRecoveryDisclosure) {
        self.status = AuditHealthStatus::Healthy;
        self.failure_count = 0;
        self.last_failure = None;
        self.last_recovery = Some(disclosure);
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

/// RFC-039 PR-039-C: `record_safe_close_decision`'s one parameter --
/// phase *two* of the two-phase shape `Closed` requires
/// (`record_safe_close_authorized` above is phase one, called earlier
/// with the same `operation_id`). Shaped so the `operation_id`/outcome
/// pairing `valid_safe_close` requires (`Authorized`/`Applied`/`Failed`
/// need one, `Cancelled` must have none) is enforced by construction
/// rather than left for the caller to get right by hand -- the same
/// reasoning behind this project's other recent move from a
/// runtime-checked invariant to one the type system makes
/// unrepresentable. `Cancelled` never has a phase-one call at all -- no
/// operation began, so there is nothing to authorize.
///
/// **`Closed::terminal_session_confirmed_empty`, renamed again from
/// `terminal_process_groups_confirmed_empty`** (itself renamed from
/// `fully_confirmed`, `safe-close-confirmation-honesty.md`) **by RFC-043
/// PR-043-C, and not only renamed this time -- rewired to a different,
/// more honest source of truth.** The old field was *computed*, in
/// `shell.rs`, by matching `Terminated`'s own outcome variant
/// (`Exited`/`TerminatedBySignal`/`KilledAfterTimeout` counted as
/// confirmed, `OrphanedUnknown`/`Failed` did not) -- an inference from a
/// *different* fact than the one this field claims. This value now
/// comes directly from [`crate::runtime::terminal::TerminalRuntimeEvent::SessionConfirmedEmpty`]'s
/// own `confirmed` field instead: RFC-043 D3's real, independent
/// re-enumeration of the *whole session*, taken immediately before
/// `request_terminate` returns, not derived from which signal ended up
/// terminating the leader.
///
/// **This is a strictly stronger claim than the old field made, not
/// merely a renamed version of the same one.** The old inference could
/// not see a backgrounded job (`cmd &`) that survived in its own
/// process group *inside* the same session -- `request_terminate` used
/// to signal only the leader's own group, so `Exited`/`TerminatedBySignal`/
/// `KilledAfterTimeout` on the leader said nothing about a sibling job.
/// RFC-043 D1/D2 changed what gets signaled and enumerated to the whole
/// **session**, and D3's `SessionConfirmedEmpty` is a real re-scan of
/// that same session -- a surviving backgrounded job is a session member
/// step 4 would find, making `confirmed` correctly `false` in exactly
/// the case the old field's name (and every rename before this one)
/// warned readers not to assume it covered.
///
/// **What remains outside the claim, by design, is now only D2's own
/// opt-out:** a process that left the session entirely (`nohup`,
/// `disown`, `setsid`) is invisible to this session's own enumeration on
/// purpose -- that is the user's deliberate detachment, not a gap this
/// field's honesty is compromised by. `false` also still covers the
/// mundane cases the old field did: a project-tracked terminal with no
/// live pane, or `request_terminate` erroring before emitting any
/// events at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SafeCloseDecision {
    Closed {
        operation_id: AuditOperationId,
        terminal_session_confirmed_empty: bool,
    },
    Cancelled,
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

    /// RFC-033 PR-033-D: `transcript_purge`'s first producer.
    /// `what-purge-must-remove.md`: "Record that a purge occurred and
    /// its scope. Never a path, never a byte count." `valid_transcript_purge`
    /// only permits `Completed`/`Failed` -- no `Authorized`/`Applied`
    /// pair the way `grant_project_trust` uses, and `operation_id` must
    /// be `None` -- so there is no schema-representable "refused
    /// because we could not pre-authorize it" state here, unlike
    /// granting trust. This is the family's own `valid_*` function
    /// already settling a question that looked like a judgement call
    /// (the task breakdown's own warning, per PR-023-D's precedent):
    /// the schema was designed for a record-after-the-fact family, not
    /// a pre-authorized one, because a purge is performed and then
    /// reported, not authorized in advance the way a future grant is.
    ///
    /// So `ProjectSession::purge_project_transcripts` (already real,
    /// already tested, untouched by this slice) runs first; this
    /// records what happened -- `Completed` on success, `Failed` on
    /// error -- the same "record what happened, do not gate on
    /// recording it" shape [`Self::revoke_project_trust`] already uses,
    /// for the same reason: the deletion this documents has already
    /// taken effect on the real filesystem by the time this runs, so a
    /// failed audit write cannot and should not roll it back.
    /// Best-effort (`append_observation`).
    ///
    /// `subject_ref` is the one place this record can state scope at
    /// all -- `valid_transcript_purge` forces `subject_kind:
    /// Some(Transcript)`, and the crate-wide `subject_kind.is_some() ==
    /// subject_ref.is_some()` invariant then forces `subject_ref:
    /// Some(..)` too, unlike `sensitive_config_changed_record`'s own
    /// family, where `subject_kind` is forced `None` and `subject_ref`
    /// is therefore structurally unable to hold anything at all. This
    /// slice only ever purges an entire project's transcripts (the task
    /// breakdown's own scope decision for PR-033-C), so the fixed
    /// literal `"project"` is the whole of what "scope" means today --
    /// a compile-time constant that names the purge's breadth without
    /// naming which transcript, the same "never a path" property
    /// `AuditReference`'s own bounded ASCII charset already enforces
    /// structurally for every family, not only this one.
    pub fn purge_project_transcripts(
        &mut self,
        project: &mut ProjectSession,
    ) -> AuditActionResult<Result<ProjectTranscriptPurgeSummary, ProjectTranscriptError>> {
        let project_id = project.id().clone();
        let outcome = project.purge_project_transcripts();
        let record = transcript_purge_record(
            project_id,
            if outcome.is_ok() {
                AuditOutcome::Completed
            } else {
                AuditOutcome::Failed
            },
        );
        let audit_status = self.append_observation(&record);
        AuditActionResult {
            value: outcome,
            audit_status,
        }
    }

    /// RFC-039 PR-039-C, `what-closing-a-project-must-not-lose.md` §4:
    /// `safe_close_decision` never had a producer before this slice --
    /// RFC-031 scoped it out for the exact reason the RFC-039 handoff
    /// names, that the dialog which would call it did not exist.
    ///
    /// Phase one of two, for the confirmed-and-closing path only --
    /// `Cancelled` (no live-work operation ever begins) skips this call
    /// entirely and goes straight to [`Self::record_safe_close_decision`].
    /// `AuditStore`'s own append path enforces the two-phase shape at the
    /// schema level for any record carrying an `operation_id`: a second
    /// phase (`Applied`/`Failed`) with no matching `Authorized` record
    /// already persisted for that same `operation_id` is rejected
    /// (`MissingAuthorization`), the same `ManagedProcessLifecycle`
    /// discipline [`Self::launch_managed_agent_run`] below already
    /// follows -- this is that same shape, not a new one. Unlike that
    /// producer's own `append_required` for its first phase, this one is
    /// best-effort (`append_observation`): closing this project's local
    /// session has no third-party-facing accountability property to
    /// protect (`purge_project_transcripts`'s own reasoning above
    /// applies unchanged), so gating a real termination sequence on
    /// whether the audit store happens to be open would cost the user
    /// the thing they asked for and buy nothing.
    pub fn record_safe_close_authorized(
        &mut self,
        project_id: ProjectId,
        operation_id: AuditOperationId,
    ) -> AuditObservationStatus {
        let mut record = DurableAuditRecordV1::new(
            AuditEventFamily::SafeCloseDecision,
            AuditOutcome::Authorized,
            AuditActionKind::SafeCloseTerminate,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        record.project_id = Some(project_id);
        record.operation_id = Some(operation_id);
        self.append_observation(&record)
    }

    /// Phase two: records the decision itself (closed, or cancelled).
    /// Each individual terminal's own termination is
    /// [`Self::record_plain_terminal_terminated`]'s job, already called
    /// once per terminal before this runs -- the same "two separate
    /// facts, two separate records" division `purge_project_transcripts`
    /// above draws between "the deletion happened" and "here is what was
    /// deleted." Best-effort (`append_observation`), for the same reason
    /// [`Self::record_safe_close_authorized`] is: by the time this runs
    /// the project has already been removed (or the user has already
    /// declined), so a transient audit-write failure cannot and must not
    /// roll either back.
    pub fn record_safe_close_decision(
        &mut self,
        project_id: ProjectId,
        decision: SafeCloseDecision,
    ) -> AuditObservationStatus {
        let record = safe_close_decision_record(project_id, decision);
        self.append_observation(&record)
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

    /// RFC-021 `command_request`: a proposal arrived. Best-effort --
    /// nothing is being authorized yet at this point (no execution to
    /// gate), so a failure here degrades `AuditHealth` but does not block
    /// receipt of the proposal.
    pub fn record_command_request(
        &mut self,
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        approval_id: ApprovalId,
        risk_level: RiskLevel,
    ) -> AuditObservationStatus {
        let record = command_approval_record(
            project_id,
            agent_run_id,
            approval_id,
            risk_level,
            AuditActionKind::CommandRequest,
            AuditOutcome::Requested,
            None,
            AuditActorKind::AppPolicy,
            AuditActionSource::Adapter,
        );
        self.append_observation(&record)
    }

    /// Authorizes a `command_approve`/`command_edit_and_approve` decision.
    /// **Required**, per the RFC's fail-closed matrix ("audit append
    /// failure for the authorization blocks execution", the same
    /// precedent as `grant_project_trust`'s trust-grant authorization):
    /// if this returns `Err`, the caller must not apply the decision or
    /// send it back over the channel.
    pub fn authorize_command_decision(
        &mut self,
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        approval_id: ApprovalId,
        risk_level: RiskLevel,
        action_kind: CommandDecisionActionKind,
    ) -> Result<AuditOperationId, AuditIntegrationError> {
        let operation_id = AuditOperationId::new_uuid();
        let record = command_approval_record(
            project_id,
            agent_run_id,
            approval_id,
            risk_level,
            action_kind.into(),
            AuditOutcome::Authorized,
            Some(operation_id.clone()),
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        self.append_required(&record)?;
        Ok(operation_id)
    }

    /// Best-effort follow-up after `authorize_command_decision` succeeded:
    /// records whether the decision was actually delivered back to the
    /// adapter (`Applied`) or not (`Failed`, e.g. the adapter had already
    /// disconnected). The decision itself is already authorized and final
    /// by this point regardless of which outcome this call records.
    #[allow(clippy::too_many_arguments)]
    pub fn record_command_decision_outcome(
        &mut self,
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        approval_id: ApprovalId,
        risk_level: RiskLevel,
        action_kind: CommandDecisionActionKind,
        operation_id: AuditOperationId,
        delivered: bool,
    ) -> AuditObservationStatus {
        let outcome = if delivered {
            AuditOutcome::Applied
        } else {
            AuditOutcome::Failed
        };
        let record = command_approval_record(
            project_id,
            agent_run_id,
            approval_id,
            risk_level,
            action_kind.into(),
            outcome,
            Some(operation_id),
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        self.append_observation(&record)
    }

    /// RFC-021 PR-021-E2 response 116 Required 2: a proposal's claimed
    /// `cwd` disagreed with `verified_cwd`. Best-effort, matching
    /// `record_command_request`'s treatment -- classification and storage
    /// have already used `verified_cwd` alone by the time this is called,
    /// so a write failure here degrades `AuditHealth` but blocks nothing;
    /// the anomaly is purely an observability signal, never a gate.
    pub fn record_cwd_mismatch_anomaly(
        &mut self,
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        approval_id: ApprovalId,
        risk_level: RiskLevel,
    ) -> AuditObservationStatus {
        let record = command_approval_record(
            project_id,
            agent_run_id,
            approval_id,
            risk_level,
            AuditActionKind::CommandCwdMismatch,
            AuditOutcome::Anomaly,
            None,
            AuditActorKind::AppPolicy,
            AuditActionSource::Adapter,
        );
        self.append_observation(&record)
    }

    /// `command_reject`: a single best-effort write, no `operation_id` --
    /// per the schema, rejection has no authorize-then-apply phase at all,
    /// since rejecting is always the safe direction and gates no
    /// execution. Blocking a rejection on an audit-write failure would be
    /// perverse: it would force an already-safe outcome into limbo.
    pub fn record_command_reject(
        &mut self,
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        approval_id: ApprovalId,
        risk_level: RiskLevel,
    ) -> AuditObservationStatus {
        let record = command_approval_record(
            project_id,
            agent_run_id,
            approval_id,
            risk_level,
            AuditActionKind::CommandReject,
            AuditOutcome::Applied,
            None,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
        );
        self.append_observation(&record)
    }

    /// Response 116 Q3's confirmation, recorded here rather than only in
    /// `qa-evidence.md` since this is exactly the property a future change
    /// to `AuditStore::append` must preserve: **this can never write a
    /// record and still return `Err`.** `AuditStore::append`
    /// (`audit/store.rs`) does all validation and insertion inside a
    /// single `rusqlite` transaction and only reports success after
    /// `transaction.commit()` itself succeeds; every error path (failed
    /// validation, a duplicate/conflict check, a failed `insert`, or a
    /// failed `commit`) returns before or without a successful commit, and
    /// an uncommitted `rusqlite::Transaction` rolls back on drop. So a
    /// caller retrying after `RequiredAuditUnavailable` (e.g. a retried
    /// `decide` after `AuditBlocked`) can never produce an orphaned
    /// `Authorized` record with no matching `Applied` -- the failed
    /// attempt left nothing behind to orphan.
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

    /// RFC-017 PR-017-F: `plain_terminal_observation`'s first producer.
    /// `project_id`/`terminal_id` are both real ids already assigned to
    /// an existing `TerminalSession`. **`Failed` is still never produced
    /// by this producer family**: `valid_plain_terminal` requires
    /// `terminal_id` for *every* outcome, so a launch failure -- which
    /// has no `TerminalSession` to name -- has no valid way to be
    /// recorded in this frozen schema at all. `Terminated` is
    /// [`Self::record_plain_terminal_terminated`], wired into the GUI's
    /// poll loop by the terminal-launch-UX handoff. Best-effort
    /// (`append_observation`), matching this family's own `Started` case
    /// never blocking the launch it observes.
    pub fn record_plain_terminal_started(
        &mut self,
        project_id: ProjectId,
        terminal_id: TerminalId,
    ) -> AuditObservationStatus {
        let record = plain_terminal_record(project_id, terminal_id, AuditOutcome::Started, None);
        self.append_observation(&record)
    }

    /// Terminal launch UX handoff: `plain_terminal_observation`'s second
    /// producer, closing the PR-017-F known limitation that only
    /// `Started` was ever reachable. `outcome` is the real
    /// `TerminationOutcome` a non-blocking exit check produced -- the
    /// same `TerminationOutcome`-to-`AuditReasonCode` mapping
    /// `apply_managed_agent_terminal_outcome` already established for
    /// `ManagedProcessLifecycle`, reused rather than re-decided: a clean
    /// exit is `ProcessExited`, a signal-caused end is
    /// `ProcessTerminated`. `OrphanedUnknown`/`Failed` are not audited
    /// here either, matching that same precedent's own choice --
    /// `NotRequired`, not a forced guess at a reason code for an outcome
    /// this ambiguous.
    pub fn record_plain_terminal_terminated(
        &mut self,
        project_id: ProjectId,
        terminal_id: TerminalId,
        outcome: &TerminationOutcome,
    ) -> AuditObservationStatus {
        let reason_code = match outcome {
            TerminationOutcome::Exited { .. } => AuditReasonCode::ProcessExited,
            TerminationOutcome::TerminatedBySignal { .. }
            | TerminationOutcome::KilledAfterTimeout { .. } => AuditReasonCode::ProcessTerminated,
            TerminationOutcome::OrphanedUnknown { .. } | TerminationOutcome::Failed { .. } => {
                return AuditObservationStatus::NotRequired;
            }
        };
        let record = plain_terminal_record(
            project_id,
            terminal_id,
            AuditOutcome::Terminated,
            Some(reason_code),
        );
        self.append_observation(&record)
    }

    /// RFC-018 PR-018-D: `paste_blocked`'s first and only producer.
    /// `reason_code` is fixed at `PastePolicy` -- `valid_paste_blocked`
    /// does not distinguish *which* `TerminalInputDecisionReason`
    /// `evaluate` returned (control-containing, wrong-project,
    /// wrong-terminal, or trusted-UI-active all collapse to the same
    /// `outcome == Blocked` record), so this producer takes no argument
    /// for it -- there is nothing for a caller to get wrong by passing
    /// the wrong one. **No pasted content, clipboard text, or command
    /// text is ever a parameter here**: the frozen schema has no field
    /// for any of them, so the type signature makes it impossible to
    /// pass one in, not merely a discipline not to. Best-effort
    /// (`append_observation`), matching every other producer in this
    /// file: an audit write failing must never fail the paste
    /// refusal it observes.
    pub fn record_paste_blocked(
        &mut self,
        project_id: ProjectId,
        terminal_id: TerminalId,
    ) -> AuditObservationStatus {
        let record = paste_blocked_record(project_id, terminal_id);
        self.append_observation(&record)
    }

    /// RFC-031 PR-031-A: `restricted_mode_blocked`'s first and only
    /// producer. Observes a refusal that already happened -- the same
    /// shape [`Self::record_paste_blocked`] uses, not a combined
    /// action-plus-audit method like [`Self::grant_project_trust`],
    /// since there is no domain mutation to perform here: a blocked
    /// launch changes nothing to roll into the same call.
    /// `what-the-store-may-hold.md`'s own required leave: `subject_ref`
    /// is `None` -- this project's canonical path is untrusted,
    /// attacker-influenceable text, and nothing here escapes it on
    /// read. `reason_code` is fixed at `RestrictedMode`, the one code
    /// this family's frozen schema allows -- RFC-004 blocks nine
    /// distinct features and this cannot say which; that coarseness is
    /// the accepted trade, recorded in evidence rather than left for a
    /// reader to assume finer granularity exists.
    pub fn record_restricted_mode_blocked(
        &mut self,
        project_id: ProjectId,
    ) -> AuditObservationStatus {
        let record = restricted_mode_blocked_record(project_id);
        self.append_observation(&record)
    }

    /// RFC-031 PR-031-B: `project_added`'s first and only producer.
    /// Observes a session that already exists -- `add_project_session`
    /// (`tekstide-core::app`) already created it by the time a caller
    /// can reach `AddProjectOutcome::Added`, so there is nothing to
    /// perform here either. `subject_ref` is `None`: `project_id`
    /// already identifies the project, and a project's own display
    /// name/path is exactly the untrusted, attacker-influenceable text
    /// `what-the-store-may-hold.md` says not to put in the store.
    pub fn record_project_added(&mut self, project_id: ProjectId) -> AuditObservationStatus {
        let record = project_added_record(project_id);
        self.append_observation(&record)
    }

    /// RFC-023 PR-023-D: `config_policy_increase`'s producer. Observes
    /// a change that has *already* been confirmed and applied by the
    /// time this is called -- there is no confirmation surface yet to
    /// call it from (M12 UI work), so, like `record_paste_blocked` and
    /// unlike `grant_project_trust`, this does not perform the change
    /// itself. Writes `Authorized` then `Applied` under one fresh
    /// `AuditOperationId`, the same two-stage shape
    /// `grant_project_trust` uses for the same reason: by the time
    /// either of these is called, the deliberate act (a confirmation
    /// dialog, in both cases) has already happened, so recording it as
    /// two linked stages in one call is complete and honest, not a
    /// fiction. Both writes are best-effort
    /// (`append_observation`, not `append_required`) -- this pack's own
    /// stated rule for every config producer: "audit-store availability
    /// is not guaranteed... a record that cannot be written must not
    /// break the action it was observing." `project_id` is always
    /// `None`: workspace configuration is not implemented (defaults +
    /// user-global only), so every config change today is global, with
    /// no project to attribute it to.
    pub fn record_sensitive_config_policy_increase(&mut self) -> AuditObservationStatus {
        let operation_id = AuditOperationId::new_uuid();
        let authorized = sensitive_config_changed_record(
            AuditActionKind::ConfigPolicyIncrease,
            AuditOutcome::Authorized,
            AuditActorKind::User,
            AuditActionSource::TrustedUi,
            Some(operation_id),
        );
        let _ = self.append_observation(&authorized);

        let mut applied = authorized;
        applied.event_id = AuditEventId::new_uuid();
        applied.outcome = AuditOutcome::Applied;
        self.append_observation(&applied)
    }

    /// RFC-023 PR-023-D: `config_policy_reduce`'s producer. Single
    /// stage -- `valid_config_change` fixes `outcome` to `Applied` and
    /// `operation_id` to `None` for this direction, matching RFC-023's
    /// own asymmetry: tightening never needs authorization, so there is
    /// no `Authorized` stage to write. `AppPolicy`/`PolicyEngine`, not
    /// `User`/`TrustedUi`: RFC-023 explicitly does not require a
    /// deliberate confirming act for this direction, so attributing it
    /// to a policy decision rather than a user click is accurate, not
    /// merely permitted by the schema's other allowed pairing.
    pub fn record_sensitive_config_policy_reduce(&mut self) -> AuditObservationStatus {
        let record = sensitive_config_changed_record(
            AuditActionKind::ConfigPolicyReduce,
            AuditOutcome::Applied,
            AuditActorKind::AppPolicy,
            AuditActionSource::PolicyEngine,
            None,
        );
        self.append_observation(&record)
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

/// [`AuditCoordinator::purge_project_transcripts`]'s own record --
/// `subject_ref: "project"` is a fixed literal, not derived from any
/// transcript's real identity or path; see that method's own doc
/// comment for why `subject_ref` cannot be `None` here and why
/// `"project"` is the whole of what this slice's own purge scope is.
fn transcript_purge_record(project_id: ProjectId, outcome: AuditOutcome) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::TranscriptPurge,
        outcome,
        AuditActionKind::TranscriptPurge,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(project_id);
    record.subject_kind = Some(AuditSubjectKind::Transcript);
    record.subject_ref =
        Some(AuditReference::new("project").expect("\"project\" is a valid AuditReference"));
    record
}

/// `action_kind` is always `SafeCloseTerminate`, not a choice between it
/// and `SafeCloseAbandon` -- this producer is only ever called on the
/// confirmation path (a project with live terminals or an active agent
/// run), and `SafeCloseTerminate` names exactly that: a close that had
/// something to terminate. `subject_kind`/`subject_ref` stay `None`
/// (`valid_safe_close` permits either `None` or `AppResource`); the
/// project already has its own field on this record, and this decision
/// covers the whole project's terminals together, not one at a time --
/// naming any single one here would be arbitrary.
fn safe_close_decision_record(
    project_id: ProjectId,
    decision: SafeCloseDecision,
) -> DurableAuditRecordV1 {
    let (outcome, operation_id) = match decision {
        SafeCloseDecision::Closed {
            operation_id,
            terminal_session_confirmed_empty,
        } => (
            if terminal_session_confirmed_empty {
                AuditOutcome::Applied
            } else {
                AuditOutcome::Failed
            },
            Some(operation_id),
        ),
        SafeCloseDecision::Cancelled => (AuditOutcome::Cancelled, None),
    };
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::SafeCloseDecision,
        outcome,
        AuditActionKind::SafeCloseTerminate,
        AuditActorKind::User,
        AuditActionSource::TrustedUi,
    );
    record.project_id = Some(project_id);
    record.operation_id = operation_id;
    record
}

/// `reason_code` is only ever `Some` for `Failed`/`Terminated` --
/// `valid_plain_terminal` requires the opposite (`None` for `Started`,
/// `Some` otherwise). `Started` (`record_plain_terminal_started`) and
/// `Terminated` (`record_plain_terminal_terminated`) both call this;
/// `Failed` still has no caller (see `record_plain_terminal_started`'s
/// own doc for why it structurally cannot).
fn plain_terminal_record(
    project_id: crate::project::ProjectId,
    terminal_id: TerminalId,
    outcome: AuditOutcome,
    reason_code: Option<AuditReasonCode>,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::PlainTerminalObservation,
        outcome,
        AuditActionKind::PlainTerminalLifecycle,
        AuditActorKind::Runtime,
        AuditActionSource::RuntimeObserver,
    );
    record.project_id = Some(project_id);
    record.terminal_id = Some(terminal_id);
    record.reason_code = reason_code;
    record
}

/// `valid_paste_blocked` fixes `outcome`, `action_kind`, `actor_kind`,
/// `action_source`, and `reason_code` all to a single combination --
/// unlike `plain_terminal_record`, there is nothing here for a caller
/// to choose, so this function takes no outcome/reason parameters at
/// all.
fn paste_blocked_record(
    project_id: crate::project::ProjectId,
    terminal_id: TerminalId,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::PasteBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::TerminalPaste,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(project_id);
    record.terminal_id = Some(terminal_id);
    record.reason_code = Some(AuditReasonCode::PastePolicy);
    record
}

/// `valid_restricted_mode_blocked` fixes every field but `project_id` --
/// same shape as `paste_blocked_record`, one fewer domain link (no
/// `terminal_id`: a refused agent-run launch never reaches a terminal
/// at all).
fn restricted_mode_blocked_record(project_id: crate::project::ProjectId) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::RestrictedModeBlocked,
        AuditOutcome::Blocked,
        AuditActionKind::RestrictedFeature,
        AuditActorKind::AppPolicy,
        AuditActionSource::PolicyEngine,
    );
    record.project_id = Some(project_id);
    record.reason_code = Some(AuditReasonCode::RestrictedMode);
    record
}

/// `valid_project_added` fixes `action_kind`/`outcome` and requires
/// `no_optional_context` -- nothing here to set beyond `project_id`.
/// `AuditActorKind::User` with `AuditActionSource::AppCommand`: the
/// real production caller is `boot()`'s CLI-argument path
/// (`crates/tekstide/src/main.rs`), which runs before any GUI widget
/// exists to make this `TrustedUi` -- closer to a startup directive
/// than an interactive click, and `AppCommand` is the schema's other
/// allowed source for a `User` actor (the same pairing
/// `ManagedAgentLaunch`'s own `Authorized` outcome accepts).
fn project_added_record(project_id: crate::project::ProjectId) -> DurableAuditRecordV1 {
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

/// `valid_config_change` forces `subject_kind: None`, and a separate,
/// crate-wide invariant (`DurableAuditRecordV1::validate`) forces
/// `subject_kind.is_some() == subject_ref.is_some()` -- so `subject_ref`
/// is not merely left `None` by convention here, it is **structurally
/// unable to hold anything** for this family. That matters for
/// `SecuritySensitiveField::AgentProfiles`: a profile name is
/// user-supplied text from the file, the same class as RFC-031's
/// `subject_ref` question and this RFC's own `key`-bounding work, and
/// the schema itself -- not a judgment call made here -- is what
/// prevents it from ever reaching the store. `project_id` is always
/// `None`, for the same reason `record_sensitive_config_policy_increase`'s
/// own doc states: no project to attribute a global config change to.
/// `operation_id` is the one field the two directions disagree on
/// (`Some` for increase, `None` for reduce), so it stays a parameter
/// rather than being fixed here.
fn sensitive_config_changed_record(
    action_kind: AuditActionKind,
    outcome: AuditOutcome,
    actor_kind: AuditActorKind,
    action_source: AuditActionSource,
    operation_id: Option<AuditOperationId>,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::SensitiveConfigChanged,
        outcome,
        action_kind,
        actor_kind,
        action_source,
    );
    record.reason_code = Some(AuditReasonCode::PolicyChanged);
    record.operation_id = operation_id;
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

/// The two decisions that need an `operation_id` (RFC-021's
/// authorize-then-apply phase). `CommandReject` is deliberately not a
/// variant here -- it never has an `operation_id`, so
/// `record_command_reject` does not take this type at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandDecisionActionKind {
    Approve,
    EditAndApprove,
}

impl From<CommandDecisionActionKind> for AuditActionKind {
    fn from(kind: CommandDecisionActionKind) -> Self {
        match kind {
            CommandDecisionActionKind::Approve => AuditActionKind::CommandApprove,
            CommandDecisionActionKind::EditAndApprove => AuditActionKind::CommandEditAndApprove,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn command_approval_record(
    project_id: ProjectId,
    agent_run_id: Option<AgentRunId>,
    approval_id: ApprovalId,
    risk_level: RiskLevel,
    action_kind: AuditActionKind,
    outcome: AuditOutcome,
    operation_id: Option<AuditOperationId>,
    actor_kind: AuditActorKind,
    action_source: AuditActionSource,
) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::CommandApproval,
        outcome,
        action_kind,
        actor_kind,
        action_source,
    );
    record.project_id = Some(project_id);
    record.agent_run_id = agent_run_id;
    record.approval_id = Some(approval_id);
    record.risk_level = Some(risk_level.into());
    record.operation_id = operation_id;
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
