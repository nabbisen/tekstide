//! RFC-027: crash detection and unsaved-buffer persistence. PR-027-A shipped crash
//! detection alone. PR-027-B adds the record itself: one file per dirty document, with
//! its own byte bounds and the purge that must cover it from the same slice that starts
//! writing it (D10).

mod instance;
mod policy;
mod record;

pub use instance::{DetectedCrash, InstanceMarker, InstanceStartup, instances_dir, start_instance};
pub use policy::{
    DEFAULT_RECOVERY_MAX_BYTES_PER_RECORD, DEFAULT_RECOVERY_MAX_BYTES_TOTAL,
    RecoveryRetentionLimits,
};
pub use record::{
    RECOVERY_RECORD_VERSION, RecoveryFileSnapshot, RecoveryPersistRefusal,
    RecoveryPersistRefusalReason, RecoveryRecord, RecoveryRecordTooLarge, RecoveryRecordWriteError,
    project_recovery_record_bytes, purge_project_recovery_records, read_project_recovery_records,
    record_file_name, records_dir, remove_recovery_record, write_recovery_record,
};

#[cfg(test)]
mod tests;
