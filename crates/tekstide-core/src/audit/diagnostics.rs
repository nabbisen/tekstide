use rusqlite::{Connection, OpenFlags};

use super::migration::probe_existing_store;
use super::path::AuditStoragePath;
use super::store::{AuditStoreError, AuditStoreErrorReason, decode_row};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditDiagnosticStatus {
    Missing,
    Healthy,
    Corrupt,
    InvalidRecords,
    UnsupportedApplication,
    UnsupportedSchema,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditDiagnosticsReport {
    pub status: AuditDiagnosticStatus,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuditDiagnostics;

impl AuditDiagnostics {
    pub fn run(self, storage_path: &AuditStoragePath) -> AuditDiagnosticsReport {
        if storage_path.validate_before_open().is_err() {
            return report(AuditDiagnosticStatus::Unavailable);
        }
        if !storage_path.database_file().exists() {
            return report(AuditDiagnosticStatus::Missing);
        }
        if let Err(error) = probe_existing_store(storage_path.database_file()) {
            return report(status_from_store_error(error));
        }

        match run_comprehensive_checks(storage_path) {
            Ok(status) => report(status),
            Err(error) => report(status_from_store_error(error)),
        }
    }
}

fn run_comprehensive_checks(
    storage_path: &AuditStoragePath,
) -> Result<AuditDiagnosticStatus, AuditStoreError> {
    let connection = Connection::open_with_flags(
        storage_path.database_file(),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(AuditStoreError::sqlite)?;

    let mut check = connection
        .prepare("PRAGMA integrity_check")
        .map_err(AuditStoreError::sqlite)?;
    let results = check
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(AuditStoreError::sqlite)?;
    for result in results {
        if result.map_err(AuditStoreError::sqlite)? != "ok" {
            return Ok(AuditDiagnosticStatus::Corrupt);
        }
    }
    drop(check);

    let mut records = connection
        .prepare(
            r#"
            SELECT sequence, event_id, schema_version, project_id, family, outcome,
                   operation_id, terminal_id, agent_run_id, approval_id, subject_kind,
                   subject_ref, action_kind, risk_level, actor_kind, action_source,
                   adapter_profile_ref, reason_code, created_at
            FROM audit_events ORDER BY sequence ASC
            "#,
        )
        .map_err(AuditStoreError::sqlite)?;
    let rows = records
        .query_map([], decode_row)
        .map_err(AuditStoreError::sqlite)?;
    for row in rows {
        if let Err(error) = row {
            let error = AuditStoreError::sqlite(error);
            return if error.reason == AuditStoreErrorReason::DecodeFailed {
                Ok(AuditDiagnosticStatus::InvalidRecords)
            } else {
                Err(error)
            };
        }
    }
    Ok(AuditDiagnosticStatus::Healthy)
}

fn status_from_store_error(error: AuditStoreError) -> AuditDiagnosticStatus {
    match error.reason {
        AuditStoreErrorReason::Corrupt => AuditDiagnosticStatus::Corrupt,
        AuditStoreErrorReason::DecodeFailed => AuditDiagnosticStatus::InvalidRecords,
        AuditStoreErrorReason::UnsupportedApplication => {
            AuditDiagnosticStatus::UnsupportedApplication
        }
        AuditStoreErrorReason::UnsupportedSchema => AuditDiagnosticStatus::UnsupportedSchema,
        _ => AuditDiagnosticStatus::Unavailable,
    }
}

fn report(status: AuditDiagnosticStatus) -> AuditDiagnosticsReport {
    AuditDiagnosticsReport { status }
}
