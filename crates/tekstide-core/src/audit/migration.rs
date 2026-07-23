use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use super::schema::{AUDIT_APPLICATION_ID, AUDIT_SCHEMA_VERSION, CREATE_SCHEMA_V1};
use super::store::{AuditStoreError, AuditStoreErrorReason};

const BUSY_TIMEOUT: Duration = Duration::from_secs(2);
const OLDEST_SUPPORTED_SCHEMA_VERSION: i64 = AUDIT_SCHEMA_VERSION;
const MIGRATIONS: &[MigrationStep] = &[];

pub(super) fn probe_existing_store(database_file: &Path) -> Result<i64, AuditStoreError> {
    let connection = Connection::open_with_flags(
        database_file,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(AuditStoreError::sqlite)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(AuditStoreError::sqlite)?;

    let identity = read_identity(&connection)?;
    validate_identity(identity)?;
    verify_bounded_schema_read(&connection)?;
    Ok(identity.schema_version)
}

pub(super) fn prepare_existing_store(
    connection: &mut Connection,
    probed_version: i64,
) -> Result<(), AuditStoreError> {
    let identity = read_identity(connection)?;
    validate_identity(identity)?;
    if identity.schema_version != probed_version {
        return Err(AuditStoreError::new(
            AuditStoreErrorReason::UnsupportedSchema,
        ));
    }
    migrate_sequentially(
        connection,
        identity.schema_version,
        AUDIT_SCHEMA_VERSION,
        MIGRATIONS,
    )?;
    // Recheck the bounded read surface after the write-capable open and any migration.
    verify_bounded_schema_read(connection)
}

pub(super) fn create_current_schema(connection: &mut Connection) -> Result<(), AuditStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(AuditStoreError::sqlite)?;
    transaction
        .execute_batch(CREATE_SCHEMA_V1)
        .map_err(AuditStoreError::sqlite)?;
    transaction
        .pragma_update(None, "application_id", AUDIT_APPLICATION_ID)
        .map_err(AuditStoreError::sqlite)?;
    transaction
        .pragma_update(None, "user_version", AUDIT_SCHEMA_VERSION)
        .map_err(AuditStoreError::sqlite)?;
    transaction.commit().map_err(AuditStoreError::sqlite)
}

#[derive(Clone, Copy)]
pub(crate) struct MigrationStep {
    pub from_version: i64,
    pub to_version: i64,
    pub statements: &'static [&'static str],
}

pub(crate) fn migrate_sequentially(
    connection: &mut Connection,
    from_version: i64,
    target_version: i64,
    migrations: &[MigrationStep],
) -> Result<(), AuditStoreError> {
    if from_version == target_version {
        return Ok(());
    }
    if from_version > target_version {
        return Err(AuditStoreError::new(
            AuditStoreErrorReason::UnsupportedSchema,
        ));
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(AuditStoreError::sqlite)?;
    let mut current_version = from_version;
    while current_version < target_version {
        let step = migrations
            .iter()
            .find(|step| step.from_version == current_version)
            .filter(|step| step.to_version == current_version + 1)
            .ok_or_else(|| AuditStoreError::new(AuditStoreErrorReason::UnsupportedSchema))?;
        for statement in step.statements {
            validate_migration_statement(statement)?;
            transaction
                .execute(statement, [])
                .map_err(AuditStoreError::sqlite)?;
        }
        transaction
            .pragma_update(None, "user_version", step.to_version)
            .map_err(AuditStoreError::sqlite)?;
        current_version = step.to_version;
    }
    transaction.commit().map_err(AuditStoreError::sqlite)
}

fn validate_migration_statement(statement: &str) -> Result<(), AuditStoreError> {
    let keyword = statement
        .trim_start()
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .map(char::from)
        .collect::<String>()
        .to_ascii_uppercase();
    if !matches!(
        keyword.as_str(),
        "CREATE" | "ALTER" | "DROP" | "INSERT" | "UPDATE" | "DELETE"
    ) {
        return Err(AuditStoreError::new(
            AuditStoreErrorReason::InvalidMigration,
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SchemaIdentity {
    application_id: i64,
    schema_version: i64,
}

fn read_identity(connection: &Connection) -> Result<SchemaIdentity, AuditStoreError> {
    Ok(SchemaIdentity {
        application_id: connection
            .query_row("PRAGMA application_id", [], |row| row.get(0))
            .map_err(AuditStoreError::sqlite)?,
        schema_version: connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(AuditStoreError::sqlite)?,
    })
}

fn validate_identity(identity: SchemaIdentity) -> Result<(), AuditStoreError> {
    if identity.application_id != AUDIT_APPLICATION_ID {
        return Err(AuditStoreError::new(
            AuditStoreErrorReason::UnsupportedApplication,
        ));
    }
    if identity.schema_version < OLDEST_SUPPORTED_SCHEMA_VERSION
        || identity.schema_version > AUDIT_SCHEMA_VERSION
    {
        return Err(AuditStoreError::new(
            AuditStoreErrorReason::UnsupportedSchema,
        ));
    }
    Ok(())
}

fn verify_bounded_schema_read(connection: &Connection) -> Result<(), AuditStoreError> {
    let table_exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'audit_events')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(schema_read_error)?;
    if !table_exists {
        return Err(AuditStoreError::new(AuditStoreErrorReason::Corrupt));
    }
    connection
        .query_row(
            "SELECT sequence FROM audit_events ORDER BY sequence DESC LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(schema_read_error)?;
    Ok(())
}

fn schema_read_error(error: rusqlite::Error) -> AuditStoreError {
    let error = AuditStoreError::sqlite(error);
    if error.reason == AuditStoreErrorReason::Io {
        AuditStoreError::new(AuditStoreErrorReason::Corrupt)
    } else {
        error
    }
}
