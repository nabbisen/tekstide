use crate::domain::{
    AgentRunId, ApprovalId, AuditEventId, AuditOperationId, DomainTimestamp, RiskLevel, TerminalId,
};
use crate::project::ProjectId;

pub const AUDIT_RECORD_SCHEMA_VERSION: u32 = 1;
const MAX_REFERENCE_LEN: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditEventFamily {
    ProjectAdded,
    TrustChange,
    CommandApproval,
    ManagedProcessLifecycle,
    PlainTerminalObservation,
    PasteBlocked,
    RestrictedModeBlocked,
    RootAccessBlocked,
    SafeCloseDecision,
    SensitiveConfigChanged,
    TranscriptPurge,
    AuditStoreRecovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditOutcome {
    Requested,
    Authorized,
    Applied,
    Failed,
    Started,
    Terminated,
    Blocked,
    Cancelled,
    Completed,
    /// RFC-021 PR-021-E2 response 116 Required 2: an informational
    /// observation, not a gate. Deliberately distinct from `Blocked` --
    /// `Blocked` in every other family here means the described action did
    /// not proceed (paste blocked, root access blocked); an `Anomaly`
    /// record (currently only `command_cwd_mismatch`) is recorded
    /// alongside an action that proceeded normally, using trusted data the
    /// anomaly itself has no bearing on. Reusing `Blocked` would overstate
    /// what happened.
    Anomaly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditActionKind {
    ProjectAdd,
    TrustGrant,
    TrustRevoke,
    CommandRequest,
    CommandApprove,
    CommandEditAndApprove,
    CommandReject,
    /// RFC-021 PR-021-E2 response 116 Required 2: a proposal's claimed
    /// `cwd` disagreed with the caller-supplied, already-verified
    /// `verified_cwd`. Never used for classification or containment (see
    /// `approval::coordinator`'s module doc) -- this is the one honest use
    /// `CommandProposal::cwd()` has: evidence that an adapter's claim was
    /// wrong, recorded best-effort as an `Anomaly`, alongside the
    /// classification that correctly used `verified_cwd` alone.
    CommandCwdMismatch,
    ManagedAgentLaunch,
    PlainTerminalLifecycle,
    TerminalPaste,
    RestrictedFeature,
    RootAccess,
    SafeCloseTerminate,
    SafeCloseAbandon,
    DestructiveAction,
    ConfigPolicyIncrease,
    ConfigPolicyReduce,
    TranscriptPurge,
    AuditStoreRecovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditRiskLevel {
    Low,
    Medium,
    High,
    Destructive,
}

/// RFC-021 PR-021-E2: `domain::RiskLevel` (the risk classifier's output)
/// and `AuditRiskLevel` (this schema's persisted column) are kept as
/// separate types deliberately -- one is `approval::risk`'s in-memory
/// vocabulary, the other is the frozen, on-disk `command_approval` audit
/// shape -- but their variants correspond exactly, so the conversion is a
/// straight match with no room for a silent severity change to slip in
/// unnoticed the way it would if the two just happened to share a type.
impl From<RiskLevel> for AuditRiskLevel {
    fn from(level: RiskLevel) -> Self {
        match level {
            RiskLevel::Low => AuditRiskLevel::Low,
            RiskLevel::Medium => AuditRiskLevel::Medium,
            RiskLevel::High => AuditRiskLevel::High,
            RiskLevel::Destructive => AuditRiskLevel::Destructive,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditActorKind {
    User,
    AppPolicy,
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditActionSource {
    TrustedUi,
    AppCommand,
    PolicyEngine,
    Adapter,
    RuntimeObserver,
    ExplicitCleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditSubjectKind {
    AppResource,
    Transcript,
    RecoveryBundle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditReasonCode {
    RootEscape,
    SymlinkEscape,
    RestrictedMode,
    PastePolicy,
    UserCancelled,
    RuntimeFailure,
    StorageFailure,
    ProcessExited,
    ProcessTerminated,
    PolicyChanged,
    RecoveryCompleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditReference(String);

impl AuditReference {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_REFERENCE_LEN
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
        {
            return None;
        }
        Some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn from_persisted(value: String) -> Option<Self> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableAuditRecordV1 {
    pub event_id: AuditEventId,
    pub schema_version: u32,
    pub project_id: Option<ProjectId>,
    pub family: AuditEventFamily,
    pub outcome: AuditOutcome,
    pub operation_id: Option<AuditOperationId>,
    pub terminal_id: Option<TerminalId>,
    pub agent_run_id: Option<AgentRunId>,
    pub approval_id: Option<ApprovalId>,
    pub subject_kind: Option<AuditSubjectKind>,
    pub subject_ref: Option<AuditReference>,
    pub action_kind: AuditActionKind,
    pub risk_level: Option<AuditRiskLevel>,
    pub actor_kind: AuditActorKind,
    pub action_source: AuditActionSource,
    pub adapter_profile_ref: Option<AuditReference>,
    pub reason_code: Option<AuditReasonCode>,
    pub created_at: DomainTimestamp,
}

impl DurableAuditRecordV1 {
    pub fn new(
        family: AuditEventFamily,
        outcome: AuditOutcome,
        action_kind: AuditActionKind,
        actor_kind: AuditActorKind,
        action_source: AuditActionSource,
    ) -> Self {
        Self {
            event_id: AuditEventId::new_uuid(),
            schema_version: AUDIT_RECORD_SCHEMA_VERSION,
            project_id: None,
            family,
            outcome,
            operation_id: None,
            terminal_id: None,
            agent_run_id: None,
            approval_id: None,
            subject_kind: None,
            subject_ref: None,
            action_kind,
            risk_level: None,
            actor_kind,
            action_source,
            adapter_profile_ref: None,
            reason_code: None,
            created_at: DomainTimestamp::now_utc(),
        }
    }

    pub fn validate(&self) -> Result<(), AuditRecordValidationError> {
        if self.schema_version != AUDIT_RECORD_SCHEMA_VERSION {
            return Err(self.error(AuditRecordValidationErrorReason::UnsupportedSchemaVersion));
        }
        if !valid_actor_source(self.actor_kind, self.action_source) {
            return Err(self.error(AuditRecordValidationErrorReason::InvalidActorSource));
        }
        if self.subject_kind.is_some() != self.subject_ref.is_some() {
            return Err(self.error(AuditRecordValidationErrorReason::IncompleteSubject));
        }

        let valid = match self.family {
            AuditEventFamily::ProjectAdded => self.valid_project_added(),
            AuditEventFamily::TrustChange => self.valid_trust_change(),
            AuditEventFamily::CommandApproval => self.valid_command_approval(),
            AuditEventFamily::ManagedProcessLifecycle => self.valid_managed_process(),
            AuditEventFamily::PlainTerminalObservation => self.valid_plain_terminal(),
            AuditEventFamily::PasteBlocked => self.valid_paste_blocked(),
            AuditEventFamily::RestrictedModeBlocked => self.valid_restricted_mode_blocked(),
            AuditEventFamily::RootAccessBlocked => self.valid_root_access_blocked(),
            AuditEventFamily::SafeCloseDecision => self.valid_safe_close(),
            AuditEventFamily::SensitiveConfigChanged => self.valid_config_change(),
            AuditEventFamily::TranscriptPurge => self.valid_transcript_purge(),
            AuditEventFamily::AuditStoreRecovery => self.valid_store_recovery(),
        };

        if valid {
            Ok(())
        } else {
            Err(self.error(AuditRecordValidationErrorReason::InvalidFamilyFields))
        }
    }

    fn valid_project_added(&self) -> bool {
        self.project_id.is_some()
            && self.action_kind == AuditActionKind::ProjectAdd
            && self.outcome == AuditOutcome::Applied
            && no_domain_links(self)
            && no_optional_context(self)
            && matches!(
                (self.actor_kind, self.action_source),
                (
                    AuditActorKind::User,
                    AuditActionSource::TrustedUi | AuditActionSource::AppCommand
                ) | (AuditActorKind::AppPolicy, AuditActionSource::PolicyEngine)
            )
    }

    fn valid_trust_change(&self) -> bool {
        if self.project_id.is_none()
            || !no_domain_links(self)
            || self.subject_kind.is_some()
            || self.risk_level.is_some()
            || self.adapter_profile_ref.is_some()
            || self.reason_code.is_some()
            || !matches!(
                (self.actor_kind, self.action_source),
                (AuditActorKind::User, AuditActionSource::TrustedUi)
                    | (AuditActorKind::AppPolicy, AuditActionSource::PolicyEngine)
            )
        {
            return false;
        }

        match self.action_kind {
            AuditActionKind::TrustGrant => {
                self.operation_id.is_some()
                    && matches!(
                        self.outcome,
                        AuditOutcome::Authorized | AuditOutcome::Applied | AuditOutcome::Failed
                    )
            }
            AuditActionKind::TrustRevoke => {
                self.operation_id.is_none() && self.outcome == AuditOutcome::Applied
            }
            _ => false,
        }
    }

    fn valid_command_approval(&self) -> bool {
        if self.project_id.is_none()
            || self.approval_id.is_none()
            || self.terminal_id.is_some()
            || self.subject_kind.is_some()
            || self.risk_level.is_none()
            || self.reason_code.is_some()
        {
            return false;
        }

        match self.action_kind {
            AuditActionKind::CommandRequest => {
                self.operation_id.is_none()
                    && self.outcome == AuditOutcome::Requested
                    && self.actor_kind == AuditActorKind::AppPolicy
                    && self.action_source == AuditActionSource::Adapter
            }
            AuditActionKind::CommandApprove | AuditActionKind::CommandEditAndApprove => {
                self.operation_id.is_some()
                    && matches!(
                        self.outcome,
                        AuditOutcome::Authorized | AuditOutcome::Applied | AuditOutcome::Failed
                    )
                    && self.actor_kind == AuditActorKind::User
                    && self.action_source == AuditActionSource::TrustedUi
            }
            AuditActionKind::CommandReject => {
                self.operation_id.is_none()
                    && self.outcome == AuditOutcome::Applied
                    && self.actor_kind == AuditActorKind::User
                    && self.action_source == AuditActionSource::TrustedUi
            }
            AuditActionKind::CommandCwdMismatch => {
                self.operation_id.is_none()
                    && self.outcome == AuditOutcome::Anomaly
                    && self.actor_kind == AuditActorKind::AppPolicy
                    && self.action_source == AuditActionSource::Adapter
            }
            _ => false,
        }
    }

    fn valid_managed_process(&self) -> bool {
        if self.project_id.is_none()
            || self.agent_run_id.is_none()
            || self.approval_id.is_some()
            || self.operation_id.is_none()
            || self.subject_kind.is_some()
            || self.risk_level.is_some()
            || self.adapter_profile_ref.is_none()
            || self.action_kind != AuditActionKind::ManagedAgentLaunch
        {
            return false;
        }

        match self.outcome {
            AuditOutcome::Authorized => {
                matches!(
                    (self.actor_kind, self.action_source),
                    (
                        AuditActorKind::User,
                        AuditActionSource::TrustedUi | AuditActionSource::AppCommand
                    ) | (AuditActorKind::AppPolicy, AuditActionSource::PolicyEngine)
                ) && self.reason_code.is_none()
            }
            AuditOutcome::Started => {
                self.terminal_id.is_some()
                    && self.actor_kind == AuditActorKind::Runtime
                    && self.action_source == AuditActionSource::RuntimeObserver
                    && self.reason_code.is_none()
            }
            AuditOutcome::Failed => {
                self.actor_kind == AuditActorKind::Runtime
                    && self.action_source == AuditActionSource::RuntimeObserver
                    && self.reason_code.is_some()
            }
            AuditOutcome::Terminated => {
                self.terminal_id.is_some()
                    && self.actor_kind == AuditActorKind::Runtime
                    && self.action_source == AuditActionSource::RuntimeObserver
                    && self.reason_code.is_some()
            }
            _ => false,
        }
    }

    fn valid_plain_terminal(&self) -> bool {
        self.project_id.is_some()
            && self.terminal_id.is_some()
            && self.agent_run_id.is_none()
            && self.approval_id.is_none()
            && self.operation_id.is_none()
            && self.subject_kind.is_none()
            && self.action_kind == AuditActionKind::PlainTerminalLifecycle
            && self.risk_level.is_none()
            && self.actor_kind == AuditActorKind::Runtime
            && self.action_source == AuditActionSource::RuntimeObserver
            && self.adapter_profile_ref.is_none()
            && matches!(
                self.outcome,
                AuditOutcome::Started | AuditOutcome::Failed | AuditOutcome::Terminated
            )
            && (self.outcome == AuditOutcome::Started || self.reason_code.is_some())
    }

    fn valid_paste_blocked(&self) -> bool {
        self.project_id.is_some()
            && self.agent_run_id.is_none()
            && self.approval_id.is_none()
            && self.operation_id.is_none()
            && self.subject_kind.is_none()
            && self.action_kind == AuditActionKind::TerminalPaste
            && self.risk_level.is_none()
            && self.actor_kind == AuditActorKind::AppPolicy
            && self.action_source == AuditActionSource::PolicyEngine
            && self.adapter_profile_ref.is_none()
            && self.reason_code == Some(AuditReasonCode::PastePolicy)
            && self.outcome == AuditOutcome::Blocked
    }

    fn valid_restricted_mode_blocked(&self) -> bool {
        self.project_id.is_some()
            && no_domain_links(self)
            && self.operation_id.is_none()
            && self.subject_kind.is_none()
            && self.action_kind == AuditActionKind::RestrictedFeature
            && self.risk_level.is_none()
            && self.actor_kind == AuditActorKind::AppPolicy
            && self.action_source == AuditActionSource::PolicyEngine
            && self.adapter_profile_ref.is_none()
            && self.reason_code == Some(AuditReasonCode::RestrictedMode)
            && self.outcome == AuditOutcome::Blocked
    }

    fn valid_root_access_blocked(&self) -> bool {
        self.project_id.is_some()
            && no_domain_links(self)
            && self.operation_id.is_none()
            && self.subject_kind.is_none()
            && self.action_kind == AuditActionKind::RootAccess
            && self.risk_level.is_none()
            && self.actor_kind == AuditActorKind::AppPolicy
            && self.action_source == AuditActionSource::PolicyEngine
            && self.adapter_profile_ref.is_none()
            && matches!(
                self.reason_code,
                Some(AuditReasonCode::RootEscape | AuditReasonCode::SymlinkEscape)
            )
            && self.outcome == AuditOutcome::Blocked
    }

    fn valid_safe_close(&self) -> bool {
        if self.project_id.is_none()
            || !no_domain_links(self)
            || !matches!(
                self.subject_kind,
                None | Some(AuditSubjectKind::AppResource)
            )
            || self.risk_level.is_some()
            || self.adapter_profile_ref.is_some()
            || self.actor_kind != AuditActorKind::User
            || !matches!(
                self.action_source,
                AuditActionSource::TrustedUi | AuditActionSource::AppCommand
            )
            || !matches!(
                self.action_kind,
                AuditActionKind::SafeCloseTerminate
                    | AuditActionKind::SafeCloseAbandon
                    | AuditActionKind::DestructiveAction
            )
        {
            return false;
        }

        match self.outcome {
            AuditOutcome::Authorized | AuditOutcome::Applied | AuditOutcome::Failed => {
                self.operation_id.is_some()
            }
            AuditOutcome::Cancelled => self.operation_id.is_none(),
            _ => false,
        }
    }

    fn valid_config_change(&self) -> bool {
        if !no_domain_links(self)
            || self.subject_kind.is_some()
            || self.risk_level.is_some()
            || self.adapter_profile_ref.is_some()
            || self.reason_code != Some(AuditReasonCode::PolicyChanged)
            || !matches!(
                (self.actor_kind, self.action_source),
                (AuditActorKind::User, AuditActionSource::TrustedUi)
                    | (AuditActorKind::AppPolicy, AuditActionSource::PolicyEngine)
            )
        {
            return false;
        }

        match self.action_kind {
            AuditActionKind::ConfigPolicyIncrease => {
                self.operation_id.is_some()
                    && matches!(
                        self.outcome,
                        AuditOutcome::Authorized | AuditOutcome::Applied | AuditOutcome::Failed
                    )
            }
            AuditActionKind::ConfigPolicyReduce => {
                self.operation_id.is_none() && self.outcome == AuditOutcome::Applied
            }
            _ => false,
        }
    }

    fn valid_transcript_purge(&self) -> bool {
        self.project_id.is_some()
            && self.terminal_id.is_none()
            && self.approval_id.is_none()
            && self.operation_id.is_none()
            && self.subject_kind == Some(AuditSubjectKind::Transcript)
            && self.action_kind == AuditActionKind::TranscriptPurge
            && self.risk_level.is_none()
            && self.adapter_profile_ref.is_none()
            && self.reason_code.is_none()
            && matches!(self.outcome, AuditOutcome::Completed | AuditOutcome::Failed)
            && matches!(
                (self.actor_kind, self.action_source),
                (
                    AuditActorKind::User,
                    AuditActionSource::TrustedUi | AuditActionSource::AppCommand
                ) | (
                    AuditActorKind::AppPolicy,
                    AuditActionSource::ExplicitCleanup
                )
            )
    }

    fn valid_store_recovery(&self) -> bool {
        self.project_id.is_none()
            && no_domain_links(self)
            && self.operation_id.is_none()
            && self.subject_kind == Some(AuditSubjectKind::RecoveryBundle)
            && self.action_kind == AuditActionKind::AuditStoreRecovery
            && self.risk_level.is_none()
            && self.adapter_profile_ref.is_none()
            && self.reason_code == Some(AuditReasonCode::RecoveryCompleted)
            && self.outcome == AuditOutcome::Completed
            && self.actor_kind == AuditActorKind::User
            && matches!(
                self.action_source,
                AuditActionSource::TrustedUi | AuditActionSource::AppCommand
            )
    }

    fn error(&self, reason: AuditRecordValidationErrorReason) -> AuditRecordValidationError {
        AuditRecordValidationError {
            event_id: self.event_id.clone(),
            reason,
        }
    }
}

fn valid_actor_source(actor: AuditActorKind, source: AuditActionSource) -> bool {
    matches!(
        (actor, source),
        (
            AuditActorKind::User,
            AuditActionSource::TrustedUi | AuditActionSource::AppCommand
        ) | (
            AuditActorKind::AppPolicy,
            AuditActionSource::PolicyEngine
                | AuditActionSource::Adapter
                | AuditActionSource::ExplicitCleanup
        ) | (AuditActorKind::Runtime, AuditActionSource::RuntimeObserver)
    )
}

fn no_domain_links(record: &DurableAuditRecordV1) -> bool {
    record.terminal_id.is_none() && record.agent_run_id.is_none() && record.approval_id.is_none()
}

fn no_optional_context(record: &DurableAuditRecordV1) -> bool {
    record.operation_id.is_none()
        && record.subject_kind.is_none()
        && record.risk_level.is_none()
        && record.adapter_profile_ref.is_none()
        && record.reason_code.is_none()
}

macro_rules! impl_code {
    ($type_name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
        impl $type_name {
            pub(crate) fn as_code(self) -> &'static str {
                match self {
                    $(Self::$variant => $code),+
                }
            }

            pub(crate) fn from_code(code: &str) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant)),+,
                    _ => None,
                }
            }
        }
    };
}

impl_code!(AuditEventFamily {
    ProjectAdded => "project_added",
    TrustChange => "trust_change",
    CommandApproval => "command_approval",
    ManagedProcessLifecycle => "managed_process_lifecycle",
    PlainTerminalObservation => "plain_terminal_observation",
    PasteBlocked => "paste_blocked",
    RestrictedModeBlocked => "restricted_mode_blocked",
    RootAccessBlocked => "root_access_blocked",
    SafeCloseDecision => "safe_close_decision",
    SensitiveConfigChanged => "sensitive_config_changed",
    TranscriptPurge => "transcript_purge",
    AuditStoreRecovery => "audit_store_recovery",
});

impl_code!(AuditOutcome {
    Requested => "requested",
    Authorized => "authorized",
    Applied => "applied",
    Failed => "failed",
    Started => "started",
    Terminated => "terminated",
    Blocked => "blocked",
    Cancelled => "cancelled",
    Completed => "completed",
    Anomaly => "anomaly",
});

impl_code!(AuditActionKind {
    ProjectAdd => "project_add",
    TrustGrant => "trust_grant",
    TrustRevoke => "trust_revoke",
    CommandRequest => "command_request",
    CommandApprove => "command_approve",
    CommandEditAndApprove => "command_edit_and_approve",
    CommandReject => "command_reject",
    CommandCwdMismatch => "command_cwd_mismatch",
    ManagedAgentLaunch => "managed_agent_launch",
    PlainTerminalLifecycle => "plain_terminal_lifecycle",
    TerminalPaste => "terminal_paste",
    RestrictedFeature => "restricted_feature",
    RootAccess => "root_access",
    SafeCloseTerminate => "safe_close_terminate",
    SafeCloseAbandon => "safe_close_abandon",
    DestructiveAction => "destructive_action",
    ConfigPolicyIncrease => "config_policy_increase",
    ConfigPolicyReduce => "config_policy_reduce",
    TranscriptPurge => "transcript_purge",
    AuditStoreRecovery => "audit_store_recovery",
});

impl_code!(AuditRiskLevel {
    Low => "low",
    Medium => "medium",
    High => "high",
    Destructive => "destructive",
});

impl_code!(AuditActorKind {
    User => "user",
    AppPolicy => "app_policy",
    Runtime => "runtime",
});

impl_code!(AuditActionSource {
    TrustedUi => "trusted_ui",
    AppCommand => "app_command",
    PolicyEngine => "policy_engine",
    Adapter => "adapter",
    RuntimeObserver => "runtime_observer",
    ExplicitCleanup => "explicit_cleanup",
});

impl_code!(AuditSubjectKind {
    AppResource => "app_resource",
    Transcript => "transcript",
    RecoveryBundle => "recovery_bundle",
});

impl_code!(AuditReasonCode {
    RootEscape => "root_escape",
    SymlinkEscape => "symlink_escape",
    RestrictedMode => "restricted_mode",
    PastePolicy => "paste_policy",
    UserCancelled => "user_cancelled",
    RuntimeFailure => "runtime_failure",
    StorageFailure => "storage_failure",
    ProcessExited => "process_exited",
    ProcessTerminated => "process_terminated",
    PolicyChanged => "policy_changed",
    RecoveryCompleted => "recovery_completed",
});

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditRecordValidationErrorReason {
    UnsupportedSchemaVersion,
    InvalidActorSource,
    IncompleteSubject,
    InvalidFamilyFields,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecordValidationError {
    pub event_id: AuditEventId,
    pub reason: AuditRecordValidationErrorReason,
}
