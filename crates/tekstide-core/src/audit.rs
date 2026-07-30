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
    AuditObservationStatus, AuditedAgentLaunch, CommandDecisionActionKind,
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
    AuditRecoveryErrorReason, AuditRecoveryReceipt,
};
pub use store::{
    AuditAppendOutcome, AuditQuery, AuditRecordPage, AuditStore, AuditStoreError,
    AuditStoreErrorReason, MAX_AUDIT_QUERY_LIMIT, SequencedAuditRecord,
};

#[cfg(test)]
mod tests;
