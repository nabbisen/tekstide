use std::fmt;
use std::fs;
use std::time::Duration;

use rusqlite::{
    Connection, ErrorCode, OpenFlags, OptionalExtension, Row, Transaction, TransactionBehavior,
    params, types::Type,
};

use super::migration::{create_current_schema, prepare_existing_store, probe_existing_store};
use super::path::{AuditPathError, AuditStoragePath};
use super::record::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome,
    AuditReasonCode, AuditRecordValidationError, AuditReference, AuditRiskLevel, AuditSubjectKind,
    DurableAuditRecordV1,
};
use crate::domain::{
    AgentRunId, ApprovalId, AuditEventId, AuditOperationId, DomainTimestamp, TerminalId,
};
use crate::project::ProjectId;

pub const MAX_AUDIT_QUERY_LIMIT: u32 = 200;
const BUSY_TIMEOUT: Duration = Duration::from_secs(2);

pub struct AuditStore {
    connection: Connection,
    storage_path: AuditStoragePath,
}

impl AuditStore {
    pub fn open(storage_path: AuditStoragePath) -> Result<Self, AuditStoreError> {
        storage_path
            .validate_before_open()
            .map_err(AuditStoreError::path)?;
        let existed = storage_path.database_file().exists();
        let probed_version = existed
            .then(|| probe_existing_store(storage_path.database_file()))
            .transpose()?;

        fs::create_dir_all(storage_path.audit_dir())
            .map_err(|_| AuditStoreError::new(AuditStoreErrorReason::Io))?;
        storage_path
            .validate_before_open()
            .map_err(AuditStoreError::path)?;

        let mut connection = Connection::open_with_flags(
            storage_path.database_file(),
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(AuditStoreError::sqlite)?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(AuditStoreError::sqlite)?;
        if let Some(probed_version) = probed_version {
            // Keep migrations before configuration: SQLite table rebuilds require foreign-key
            // enforcement off, and WAL/journal settings must not precede identity validation.
            prepare_existing_store(&mut connection, probed_version)?;
            configure_connection(&connection)?;
        } else {
            configure_connection(&connection)?;
            create_current_schema(&mut connection)?;
        }

        Ok(Self {
            connection,
            storage_path,
        })
    }

    pub fn storage_path(&self) -> &AuditStoragePath {
        &self.storage_path
    }

    pub fn append(
        &mut self,
        record: &DurableAuditRecordV1,
    ) -> Result<AuditAppendOutcome, AuditStoreError> {
        record.validate().map_err(AuditStoreError::validation)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(AuditStoreError::sqlite)?;

        if let Some(existing) = load_by_event_id(&transaction, record.event_id.as_str())? {
            if existing.record == *record {
                return Ok(AuditAppendOutcome::AlreadyPresent {
                    sequence: existing.sequence,
                });
            }
            return Err(AuditStoreError::new(
                AuditStoreErrorReason::DuplicateEventConflict,
            ));
        }

        validate_operation_phase(&transaction, record)?;
        insert_record(&transaction, record)?;
        let sequence = transaction.last_insert_rowid();
        transaction.commit().map_err(AuditStoreError::sqlite)?;
        Ok(AuditAppendOutcome::Appended { sequence })
    }

    pub fn query(&self, query: &AuditQuery) -> Result<AuditRecordPage, AuditStoreError> {
        if query.limit == 0 || query.limit > MAX_AUDIT_QUERY_LIMIT {
            return Err(AuditStoreError::new(AuditStoreErrorReason::InvalidQuery));
        }

        let project_id = query
            .project_id
            .as_ref()
            .map(|project_id| project_id.as_str().to_owned());
        let family = query.family.map(|family| family.as_code().to_owned());
        let outcome = query.outcome.map(|outcome| outcome.as_code().to_owned());
        let operation_id = query
            .operation_id
            .as_ref()
            .map(|operation_id| operation_id.as_str().to_owned());

        let mut statement = self
            .connection
            .prepare(
                r#"
                SELECT sequence, event_id, schema_version, project_id, family, outcome,
                       operation_id, terminal_id, agent_run_id, approval_id, subject_kind,
                       subject_ref, action_kind, risk_level, actor_kind, action_source,
                       adapter_profile_ref, reason_code, created_at
                FROM audit_events
                WHERE (?1 IS NULL OR project_id = ?1)
                  AND (?2 IS NULL OR family = ?2)
                  AND (?3 IS NULL OR outcome = ?3)
                  AND (?4 IS NULL OR operation_id = ?4)
                  AND (?5 IS NULL OR sequence < ?5)
                ORDER BY sequence DESC
                LIMIT ?6
                "#,
            )
            .map_err(AuditStoreError::sqlite)?;
        let rows = statement
            .query_map(
                params![
                    project_id,
                    family,
                    outcome,
                    operation_id,
                    query.before_sequence,
                    i64::from(query.limit)
                ],
                decode_row,
            )
            .map_err(AuditStoreError::sqlite)?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(AuditStoreError::sqlite)?);
        }
        let next_before_sequence = (records.len() == query.limit as usize)
            .then(|| records.last().map(|record| record.sequence))
            .flatten();

        Ok(AuditRecordPage {
            records,
            next_before_sequence,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditAppendOutcome {
    Appended { sequence: i64 },
    AlreadyPresent { sequence: i64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequencedAuditRecord {
    pub sequence: i64,
    pub record: DurableAuditRecordV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecordPage {
    pub records: Vec<SequencedAuditRecord>,
    pub next_before_sequence: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditQuery {
    pub project_id: Option<ProjectId>,
    pub family: Option<AuditEventFamily>,
    pub outcome: Option<AuditOutcome>,
    pub operation_id: Option<AuditOperationId>,
    pub before_sequence: Option<i64>,
    pub limit: u32,
}

impl AuditQuery {
    pub fn latest(limit: u32) -> Self {
        Self {
            project_id: None,
            family: None,
            outcome: None,
            operation_id: None,
            before_sequence: None,
            limit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditStoreErrorReason {
    Path,
    Io,
    Busy,
    ReadOnly,
    StorageFull,
    Corrupt,
    DecodeFailed,
    UnsupportedApplication,
    UnsupportedSchema,
    InvalidRecord,
    InvalidQuery,
    InvalidMigration,
    DuplicateEventConflict,
    MissingAuthorization,
    OperationConflict,
    PhaseConflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditStoreError {
    pub reason: AuditStoreErrorReason,
}

impl AuditStoreError {
    pub(super) fn new(reason: AuditStoreErrorReason) -> Self {
        Self { reason }
    }

    fn path(_error: AuditPathError) -> Self {
        Self::new(AuditStoreErrorReason::Path)
    }

    fn validation(_error: AuditRecordValidationError) -> Self {
        Self::new(AuditStoreErrorReason::InvalidRecord)
    }

    pub(super) fn sqlite(error: rusqlite::Error) -> Self {
        if let rusqlite::Error::FromSqlConversionFailure(_, _, source) = &error
            && source.downcast_ref::<AuditDecodeFailure>().is_some()
        {
            return Self::new(AuditStoreErrorReason::DecodeFailed);
        }
        let reason = match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
                AuditStoreErrorReason::Busy
            }
            Some(ErrorCode::ReadOnly) => AuditStoreErrorReason::ReadOnly,
            Some(ErrorCode::DiskFull) => AuditStoreErrorReason::StorageFull,
            Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => {
                AuditStoreErrorReason::Corrupt
            }
            _ => AuditStoreErrorReason::Io,
        };
        Self::new(reason)
    }
}

fn configure_connection(connection: &Connection) -> Result<(), AuditStoreError> {
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(AuditStoreError::sqlite)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(AuditStoreError::sqlite)?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(AuditStoreError::sqlite)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(AuditStoreError::sqlite)?;
    Ok(())
}

fn validate_operation_phase(
    transaction: &Transaction<'_>,
    record: &DurableAuditRecordV1,
) -> Result<(), AuditStoreError> {
    let Some(operation_id) = &record.operation_id else {
        return Ok(());
    };

    if record.outcome == AuditOutcome::Authorized {
        let existing: Option<String> = transaction
            .query_row(
                r#"
                SELECT event_id FROM audit_events
                WHERE operation_id = ?1 AND outcome = 'authorized'
                "#,
                [operation_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(AuditStoreError::sqlite)?;
        if existing.is_some() {
            return Err(AuditStoreError::new(
                AuditStoreErrorReason::OperationConflict,
            ));
        }
        return Ok(());
    }

    let authorization = transaction
        .query_row(
            r#"
            SELECT sequence, project_id, family, action_kind FROM audit_events
            WHERE operation_id = ?1 AND outcome = 'authorized'
            "#,
            [operation_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(AuditStoreError::sqlite)?
        .ok_or_else(|| AuditStoreError::new(AuditStoreErrorReason::MissingAuthorization))?;

    if authorization.1.as_deref() != record.project_id.as_ref().map(ProjectId::as_str)
        || authorization.2 != record.family.as_code()
        || authorization.3 != record.action_kind.as_code()
    {
        return Err(AuditStoreError::new(
            AuditStoreErrorReason::OperationConflict,
        ));
    }

    let mut phases = transaction
        .prepare(
            r#"
            SELECT outcome FROM audit_events
            WHERE operation_id = ?1 AND outcome != 'authorized'
            ORDER BY sequence ASC
            "#,
        )
        .map_err(AuditStoreError::sqlite)?;
    let existing = phases
        .query_map([operation_id.as_str()], |row| row.get::<_, String>(0))
        .map_err(AuditStoreError::sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AuditStoreError::sqlite)?;

    let valid = if record.family == AuditEventFamily::ManagedProcessLifecycle {
        valid_managed_phase(&existing, record.outcome)
    } else {
        existing.is_empty()
            && matches!(record.outcome, AuditOutcome::Applied | AuditOutcome::Failed)
    };
    if !valid {
        return Err(AuditStoreError::new(AuditStoreErrorReason::PhaseConflict));
    }
    Ok(())
}

fn valid_managed_phase(existing: &[String], next: AuditOutcome) -> bool {
    match (existing, next) {
        ([], AuditOutcome::Started | AuditOutcome::Failed) => true,
        ([started], AuditOutcome::Terminated) if started == "started" => true,
        _ => false,
    }
}

fn insert_record(
    transaction: &Transaction<'_>,
    record: &DurableAuditRecordV1,
) -> Result<(), AuditStoreError> {
    transaction
        .execute(
            r#"
            INSERT INTO audit_events (
                event_id, schema_version, project_id, family, outcome, operation_id,
                terminal_id, agent_run_id, approval_id, subject_kind, subject_ref,
                action_kind, risk_level, actor_kind, action_source, adapter_profile_ref,
                reason_code, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18
            )
            "#,
            params![
                record.event_id.as_str(),
                i64::from(record.schema_version),
                record.project_id.as_ref().map(ProjectId::as_str),
                record.family.as_code(),
                record.outcome.as_code(),
                record.operation_id.as_ref().map(AuditOperationId::as_str),
                record.terminal_id.as_ref().map(TerminalId::as_str),
                record.agent_run_id.as_ref().map(AgentRunId::as_str),
                record.approval_id.as_ref().map(ApprovalId::as_str),
                record.subject_kind.map(AuditSubjectKind::as_code),
                record.subject_ref.as_ref().map(AuditReference::as_str),
                record.action_kind.as_code(),
                record.risk_level.map(AuditRiskLevel::as_code),
                record.actor_kind.as_code(),
                record.action_source.as_code(),
                record
                    .adapter_profile_ref
                    .as_ref()
                    .map(AuditReference::as_str),
                record.reason_code.map(AuditReasonCode::as_code),
                record.created_at.as_str(),
            ],
        )
        .map_err(AuditStoreError::sqlite)?;
    Ok(())
}

fn load_by_event_id(
    connection: &Connection,
    event_id: &str,
) -> Result<Option<SequencedAuditRecord>, AuditStoreError> {
    connection
        .query_row(
            r#"
            SELECT sequence, event_id, schema_version, project_id, family, outcome,
                   operation_id, terminal_id, agent_run_id, approval_id, subject_kind,
                   subject_ref, action_kind, risk_level, actor_kind, action_source,
                   adapter_profile_ref, reason_code, created_at
            FROM audit_events
            WHERE event_id = ?1
            "#,
            [event_id],
            decode_row,
        )
        .optional()
        .map_err(AuditStoreError::sqlite)
}

fn decode_row(row: &Row<'_>) -> rusqlite::Result<SequencedAuditRecord> {
    decode_row_inner(row).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(AuditDecodeFailure))
    })
}

#[derive(Debug)]
struct AuditDecodeFailure;

impl fmt::Display for AuditDecodeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("audit record decode failed")
    }
}

impl std::error::Error for AuditDecodeFailure {}

fn decode_row_inner(row: &Row<'_>) -> Result<SequencedAuditRecord, ()> {
    let sequence = row.get(0).map_err(|_| ())?;
    let event_id =
        AuditEventId::from_persisted(row.get::<_, String>(1).map_err(|_| ())?).ok_or(())?;
    let schema_version = row.get::<_, i64>(2).map_err(|_| ())?;
    let schema_version = u32::try_from(schema_version).map_err(|_| ())?;
    let project_id = parse_optional(row.get(3).map_err(|_| ())?, ProjectId::from_persisted)?;
    let family = AuditEventFamily::from_code(&row.get::<_, String>(4).map_err(|_| ())?).ok_or(())?;
    let outcome = AuditOutcome::from_code(&row.get::<_, String>(5).map_err(|_| ())?).ok_or(())?;
    let operation_id = parse_optional(
        row.get(6).map_err(|_| ())?,
        AuditOperationId::from_persisted,
    )?;
    let terminal_id = parse_optional(row.get(7).map_err(|_| ())?, TerminalId::from_persisted)?;
    let agent_run_id = parse_optional(row.get(8).map_err(|_| ())?, AgentRunId::from_persisted)?;
    let approval_id = parse_optional(row.get(9).map_err(|_| ())?, ApprovalId::from_persisted)?;
    let subject_kind =
        parse_optional_code(row.get(10).map_err(|_| ())?, AuditSubjectKind::from_code)?;
    let subject_ref = parse_optional(row.get(11).map_err(|_| ())?, AuditReference::from_persisted)?;
    let action_kind =
        AuditActionKind::from_code(&row.get::<_, String>(12).map_err(|_| ())?).ok_or(())?;
    let risk_level = parse_optional_code(row.get(13).map_err(|_| ())?, AuditRiskLevel::from_code)?;
    let actor_kind =
        AuditActorKind::from_code(&row.get::<_, String>(14).map_err(|_| ())?).ok_or(())?;
    let action_source =
        AuditActionSource::from_code(&row.get::<_, String>(15).map_err(|_| ())?).ok_or(())?;
    let adapter_profile_ref =
        parse_optional(row.get(16).map_err(|_| ())?, AuditReference::from_persisted)?;
    let reason_code =
        parse_optional_code(row.get(17).map_err(|_| ())?, AuditReasonCode::from_code)?;
    let created_at = DomainTimestamp::from_utc_string(row.get::<_, String>(18).map_err(|_| ())?)
        .map_err(|_| ())?;

    let record = DurableAuditRecordV1 {
        event_id,
        schema_version,
        project_id,
        family,
        outcome,
        operation_id,
        terminal_id,
        agent_run_id,
        approval_id,
        subject_kind,
        subject_ref,
        action_kind,
        risk_level,
        actor_kind,
        action_source,
        adapter_profile_ref,
        reason_code,
        created_at,
    };
    record.validate().map_err(|_| ())?;
    Ok(SequencedAuditRecord { sequence, record })
}

fn parse_optional<T>(
    value: Option<String>,
    parser: impl FnOnce(String) -> Option<T>,
) -> Result<Option<T>, ()> {
    match value {
        Some(value) => parser(value).map(Some).ok_or(()),
        None => Ok(None),
    }
}

fn parse_optional_code<T>(
    value: Option<String>,
    parser: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<T>, ()> {
    match value {
        Some(value) => parser(&value).map(Some).ok_or(()),
        None => Ok(None),
    }
}
