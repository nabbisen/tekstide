mod diagnostics;
mod integration;
mod migration;
mod path;
mod purge;
mod record;
mod recovery;
mod schema;
mod store;

pub use diagnostics::{AuditDiagnosticStatus, AuditDiagnostics, AuditDiagnosticsReport};
pub use integration::{
    AuditActionResult, AuditCoordinator, AuditHealth, AuditHealthStatus, AuditIntegrationError,
    AuditObservationStatus, AuditRecoveryDisclosure, AuditedAgentLaunch, CommandDecisionActionKind,
    SafeCloseDecision,
};
pub use path::{
    AuditPathError, AuditPathErrorReason, AuditPathRequest, AuditPathResolver, AuditStoragePath,
};
pub use purge::{
    AuditJournalCleanupStatus, AuditLocalDataScanStatus, AuditLocalDataSummary, AuditPurgeReceipt,
    AuditPurgeScope, MAX_AUDIT_RECOVERY_SUMMARY_ENTRIES,
};
pub use record::{
    AUDIT_RECORD_SCHEMA_VERSION, AuditActionKind, AuditActionSource, AuditActorKind,
    AuditEventFamily, AuditOutcome, AuditReasonCode, AuditRecordValidationError,
    AuditRecordValidationErrorReason, AuditReference, AuditRiskLevel, AuditSubjectKind,
    DurableAuditRecordV1,
};
pub use recovery::{
    AuditArtifactKind, AuditArtifactStatus, AuditRecovery, AuditRecoveryEntry, AuditRecoveryError,
    AuditRecoveryErrorReason, AuditRecoveryOutcome, AuditRecoveryReceipt,
};
// RFC-047 PR-047-B: gated the same as its own definition -- a `pub use`
// of a `#[cfg(any(test, feature = "test-support"))]` item must carry
// the identical cfg, or a build compiling neither fails to find it.
#[cfg(any(test, feature = "test-support"))]
pub use recovery::corrupt_and_interrupt_recovery_for_test;
pub use store::{
    AuditAppendOutcome, AuditQuery, AuditRecordPage, AuditStore, AuditStoreError,
    AuditStoreErrorReason, MAX_AUDIT_QUERY_LIMIT, SequencedAuditRecord,
};

#[cfg(test)]
mod tests;
