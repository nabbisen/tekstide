mod agent;
mod approval;
mod audit;
mod changeset;
mod identity;
mod ids;
mod ownership;
#[cfg(any(test, feature = "test-support"))]
mod shell_readiness;
mod terminal;
mod time;
mod transcript;

pub use agent::{
    AgentCompatibilityLevel, AgentRun, AgentRunOrigin, AgentRunStatus, AgentRunTransitionError,
    BlankClassificationLabel, RUN_CUSTOM_CLASSIFICATION_MAX_CHARS, RUN_NOTES_MAX_CHARS,
    RUN_PROMPT_SUMMARY_MAX_CHARS, RUN_RECORD_MAX_IDS_PER_KIND, RecordBounds, RunClassification,
    RunEnding, TranscriptAbsence,
};
pub use approval::{
    ApprovalDecision, ApprovalDecisionError, ApprovalRequest, RiskLevel, RiskReason,
};
pub use audit::{AuditEvent, AuditEventClass, AuditEventError};
pub use changeset::{
    ChangeAssociationConfidence, ChangeDetectionFailureReason, ChangeDetectionSource,
    ChangeDetectionStatus, ChangeSet, ChangeSetSummary, ReviewState, ReviewStateTransitionError,
};
pub use identity::ProjectId;
pub use ids::{
    AgentRunId, ApprovalId, AuditEventId, AuditOperationId, ChangeSetId, TerminalId, TranscriptId,
};
pub use ownership::OwnershipError;
#[cfg(any(test, feature = "test-support"))]
pub use shell_readiness::{
    SHELL_READINESS_MARKER, SHELL_READINESS_PROBE_COMMAND, SHELL_READINESS_TIMEOUT,
};
pub use terminal::{
    TerminalKind, TerminalSession, TerminalStatus, TerminalTransitionError, VisibleSlot,
};
pub use time::{DomainTimestamp, TimestampParseError};
pub use transcript::{Transcript, TranscriptLifecycleState, TranscriptOrigin, TruncationState};

#[cfg(test)]
mod tests;
