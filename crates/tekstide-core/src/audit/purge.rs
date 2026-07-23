use std::fs;
use std::io;
use std::path::Path;

use rusqlite::TransactionBehavior;

use super::store::{AuditStore, AuditStoreError, AuditStoreErrorReason};
use crate::project::ProjectId;

pub const MAX_AUDIT_RECOVERY_SUMMARY_ENTRIES: usize = 4_096;
const MAX_AUDIT_RECOVERY_SUMMARY_DEPTH: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditPurgeScope {
    Project,
    Global,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditJournalCleanupStatus {
    Completed,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditPurgeReceipt {
    pub scope: AuditPurgeScope,
    pub deleted_record_count: u64,
    pub journal_cleanup: AuditJournalCleanupStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditLocalDataScanStatus {
    Complete,
    EntryLimitReached,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditLocalDataSummary {
    pub retained_record_count: u64,
    pub database_bytes: u64,
    pub rollback_journal_bytes: u64,
    pub wal_bytes: u64,
    pub shared_memory_bytes: u64,
    pub recovery_bytes: u64,
    pub recovery_artifact_count: u64,
    pub total_bytes: u64,
    pub scan_status: AuditLocalDataScanStatus,
}

impl AuditStore {
    pub fn purge_project_records(
        &mut self,
        project_id: &ProjectId,
    ) -> Result<AuditPurgeReceipt, AuditStoreError> {
        self.purge_records(AuditPurgeScope::Project, Some(project_id))
    }

    pub fn purge_all_records(&mut self) -> Result<AuditPurgeReceipt, AuditStoreError> {
        self.purge_records(AuditPurgeScope::Global, None)
    }

    pub fn local_data_summary(&self) -> Result<AuditLocalDataSummary, AuditStoreError> {
        summarize_local_data_with_limit(self, MAX_AUDIT_RECOVERY_SUMMARY_ENTRIES)
    }

    fn purge_records(
        &mut self,
        scope: AuditPurgeScope,
        project_id: Option<&ProjectId>,
    ) -> Result<AuditPurgeReceipt, AuditStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(AuditStoreError::sqlite)?;
        let deleted = match project_id {
            Some(project_id) => transaction
                .execute(
                    "DELETE FROM audit_events WHERE project_id = ?1",
                    [project_id.as_str()],
                )
                .map_err(AuditStoreError::sqlite)?,
            None => transaction
                .execute("DELETE FROM audit_events", [])
                .map_err(AuditStoreError::sqlite)?,
        };
        transaction.commit().map_err(AuditStoreError::sqlite)?;

        Ok(AuditPurgeReceipt {
            scope,
            deleted_record_count: deleted as u64,
            journal_cleanup: checkpoint_and_truncate(&self.connection),
        })
    }
}

pub(crate) fn summarize_local_data_with_limit(
    store: &AuditStore,
    recovery_entry_limit: usize,
) -> Result<AuditLocalDataSummary, AuditStoreError> {
    let retained_record_count = store
        .connection
        .query_row("SELECT COUNT(*) FROM audit_events", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(AuditStoreError::sqlite)
        .and_then(|count| {
            u64::try_from(count).map_err(|_| AuditStoreError::new(AuditStoreErrorReason::Corrupt))
        })?;

    let mut status = AuditLocalDataScanStatus::Complete;
    let database_bytes = regular_file_len(store.storage_path.database_file(), &mut status);
    let rollback_journal_bytes = regular_file_len(&store.storage_path.journal_file(), &mut status);
    let wal_bytes = regular_file_len(&store.storage_path.wal_file(), &mut status);
    let shared_memory_bytes =
        regular_file_len(&store.storage_path.shared_memory_file(), &mut status);
    let (recovery_bytes, recovery_artifact_count, recovery_status) =
        summarize_recovery_files(store.storage_path.recovery_dir(), recovery_entry_limit);
    status = combine_status(status, recovery_status);

    let total_bytes = [
        database_bytes,
        rollback_journal_bytes,
        wal_bytes,
        shared_memory_bytes,
        recovery_bytes,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add)
    .unwrap_or_else(|| {
        status = AuditLocalDataScanStatus::Unavailable;
        u64::MAX
    });

    Ok(AuditLocalDataSummary {
        retained_record_count,
        database_bytes,
        rollback_journal_bytes,
        wal_bytes,
        shared_memory_bytes,
        recovery_bytes,
        recovery_artifact_count,
        total_bytes,
        scan_status: status,
    })
}

fn checkpoint_and_truncate(connection: &rusqlite::Connection) -> AuditJournalCleanupStatus {
    connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            let busy = row.get::<_, i64>(0)?;
            let log_frames = row.get::<_, i64>(1)?;
            let checkpointed_frames = row.get::<_, i64>(2)?;
            Ok((busy, log_frames, checkpointed_frames))
        })
        .map_or(AuditJournalCleanupStatus::Deferred, |result| {
            if result.0 == 0 && result.1 == result.2 {
                AuditJournalCleanupStatus::Completed
            } else {
                AuditJournalCleanupStatus::Deferred
            }
        })
}

fn regular_file_len(path: &Path, status: &mut AuditLocalDataScanStatus) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        _ => {
            *status = AuditLocalDataScanStatus::Unavailable;
            0
        }
    }
}

fn summarize_recovery_files(
    recovery_dir: &Path,
    entry_limit: usize,
) -> (u64, u64, AuditLocalDataScanStatus) {
    let mut bytes = 0_u64;
    let mut files = 0_u64;
    let mut visited = 0_usize;
    let mut status = AuditLocalDataScanStatus::Complete;
    let root = match fs::read_dir(recovery_dir) {
        Ok(root) => root,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return (0, 0, status),
        Err(_) => return (0, 0, AuditLocalDataScanStatus::Unavailable),
    };

    for entry in root {
        let Ok(entry) = entry else {
            status = combine_status(status, AuditLocalDataScanStatus::Unavailable);
            continue;
        };
        if !visit_entry(
            &entry.path(),
            0,
            entry_limit,
            &mut visited,
            &mut bytes,
            &mut files,
            &mut status,
        ) {
            break;
        }
    }
    (bytes, files, status)
}

fn visit_entry(
    path: &Path,
    depth: usize,
    entry_limit: usize,
    visited: &mut usize,
    bytes: &mut u64,
    files: &mut u64,
    status: &mut AuditLocalDataScanStatus,
) -> bool {
    if *visited >= entry_limit {
        *status = combine_status(*status, AuditLocalDataScanStatus::EntryLimitReached);
        return false;
    }
    *visited += 1;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            *status = combine_status(*status, AuditLocalDataScanStatus::Unavailable);
            return true;
        }
    };
    if metadata.file_type().is_symlink() {
        *status = combine_status(*status, AuditLocalDataScanStatus::Unavailable);
    } else if metadata.is_file() {
        *bytes = bytes.checked_add(metadata.len()).unwrap_or_else(|| {
            *status = AuditLocalDataScanStatus::Unavailable;
            u64::MAX
        });
        *files = files.saturating_add(1);
    } else if metadata.is_dir() {
        if depth >= MAX_AUDIT_RECOVERY_SUMMARY_DEPTH {
            *status = combine_status(*status, AuditLocalDataScanStatus::Unavailable);
            return true;
        }
        let children = match fs::read_dir(path) {
            Ok(children) => children,
            Err(_) => {
                *status = combine_status(*status, AuditLocalDataScanStatus::Unavailable);
                return true;
            }
        };
        for child in children {
            let Ok(child) = child else {
                *status = combine_status(*status, AuditLocalDataScanStatus::Unavailable);
                continue;
            };
            if !visit_entry(
                &child.path(),
                depth + 1,
                entry_limit,
                visited,
                bytes,
                files,
                status,
            ) {
                return false;
            }
        }
    } else {
        *status = combine_status(*status, AuditLocalDataScanStatus::Unavailable);
    }
    true
}

fn combine_status(
    left: AuditLocalDataScanStatus,
    right: AuditLocalDataScanStatus,
) -> AuditLocalDataScanStatus {
    match (left, right) {
        (AuditLocalDataScanStatus::Unavailable, _) | (_, AuditLocalDataScanStatus::Unavailable) => {
            AuditLocalDataScanStatus::Unavailable
        }
        (AuditLocalDataScanStatus::EntryLimitReached, _)
        | (_, AuditLocalDataScanStatus::EntryLimitReached) => {
            AuditLocalDataScanStatus::EntryLimitReached
        }
        _ => AuditLocalDataScanStatus::Complete,
    }
}
