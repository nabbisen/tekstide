use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use super::schema::{
    AUDIT_APPLICATION_ID, AUDIT_SCHEMA_VERSION, CREATE_SCHEMA_V2, audit_events_v2_table_ddl,
};
use super::store::{AuditStoreError, AuditStoreErrorReason};

const BUSY_TIMEOUT: Duration = Duration::from_secs(2);
/// **Deliberately a literal, not derived from [`AUDIT_SCHEMA_VERSION`].**
/// RFC-013 Amendment 1's trap: `OLDEST_SUPPORTED_SCHEMA_VERSION =
/// AUDIT_SCHEMA_VERSION` looks like it just tracks "the only version we
/// support" today, but it silently drags the floor up with every future
/// bump -- the exact version that put every existing v1 database out of
/// range the moment `AUDIT_SCHEMA_VERSION` became `2`, verified by probe
/// in the amendment handoff (`user_version=0/2: open -> Err
/// (UnsupportedSchema)`, before this fix). Pinning it to `1` means the
/// oldest schema the `1 -> 2` migration below can actually migrate from
/// stays in range regardless of how high `AUDIT_SCHEMA_VERSION` climbs
/// later.
const OLDEST_SUPPORTED_SCHEMA_VERSION: i64 = 1;

/// RFC-013 Amendment 1's `1 -> 2` step: adds `command_cwd_mismatch`/
/// `anomaly` to the `command_approval` family. SQLite cannot `ALTER` a
/// `CHECK` constraint, so this is the table-rebuild pattern: create a v2
/// table under a temporary name, copy every row (explicitly listing
/// columns and, critically, `sequence` itself -- RFC-013's whole
/// append-only ordering claim rests on `sequence` surviving unchanged,
/// and an `AUTOINCREMENT` column left to default would silently
/// reassign it), drop the old table, rename the new one into place, and
/// recreate every index (`DROP TABLE` removes a table's indexes
/// automatically, so they do not need dropping explicitly).
///
/// The rebuild table's DDL is built from the exact same
/// [`audit_events_v2_table_ddl`] macro expansion [`CREATE_SCHEMA_V2`]
/// uses for a fresh install (only the table name differs), and SQLite's
/// `ALTER TABLE ... RENAME TO` rewrites the stored table name on rename
/// -- so a migrated database's `sqlite_master` entry for `audit_events`
/// ends up textually identical to a fresh v2 install's. The convergence
/// test in `audit::tests::migration` asserts this rather than assuming
/// it.
///
/// **The `AUTOINCREMENT` high-water mark is carried across the rebuild
/// separately from the rows themselves** (response 119 Required): `DROP
/// TABLE audit_events` deletes `audit_events`'s row in SQLite's own
/// `sqlite_sequence` table, and the rebuild table's row in
/// `sqlite_sequence` reflects only what was actually inserted into it --
/// if the source table had been purged of some rows before the upgrade
/// (`purge.rs`'s `DELETE FROM audit_events`), the counter would silently
/// reset below its true history, letting a later append reuse a
/// `sequence` value that used to identify a different, now-deleted
/// event. The values themselves surviving unchanged (the property named
/// explicitly in the amendment handoff) is not the same property as the
/// *counter* surviving unchanged -- this migration now preserves both.
/// The carry is captured into a throwaway table before anything else
/// runs and restored (via `MAX`, so it can only move forward, never
/// backward) after the rename and indexes are in place.
pub(crate) const MIGRATIONS: &[MigrationStep] = &[MigrationStep {
    from_version: 1,
    to_version: 2,
    statements: &[
        // Captured first, before the source table (and its
        // `sqlite_sequence` row) is touched at all. If `audit_events`
        // was never written to, it has no `sqlite_sequence` row yet
        // (SQLite creates that row lazily on first insert, not at
        // `CREATE TABLE` time) -- this then produces an empty carry
        // table, which the restore step below treats as "no floor to
        // enforce" via `COALESCE(MAX(seq), 0)`.
        "CREATE TABLE audit_events_seq_carry AS \
         SELECT seq FROM sqlite_sequence WHERE name = 'audit_events'",
        audit_events_v2_table_ddl!("audit_events_v2_rebuild"),
        "INSERT INTO audit_events_v2_rebuild (\
            sequence, event_id, schema_version, project_id, family, outcome, \
            operation_id, terminal_id, agent_run_id, approval_id, subject_kind, \
            subject_ref, action_kind, risk_level, actor_kind, action_source, \
            adapter_profile_ref, reason_code, created_at\
        ) SELECT \
            sequence, event_id, schema_version, project_id, family, outcome, \
            operation_id, terminal_id, agent_run_id, approval_id, subject_kind, \
            subject_ref, action_kind, risk_level, actor_kind, action_source, \
            adapter_profile_ref, reason_code, created_at \
        FROM audit_events",
        "DROP TABLE audit_events",
        "ALTER TABLE audit_events_v2_rebuild RENAME TO audit_events",
        // Each index statement's text is byte-identical to the
        // corresponding one in `CREATE_SCHEMA_V2` (real embedded
        // newline + 4-space indent, not a backslash line-continuation
        // collapsed to a single space) -- SQLite stores an index's
        // `CREATE INDEX` text in `sqlite_master.sql` verbatim as given,
        // unlike a table's, which gets rewritten on rename. A fresh
        // install and this migration must produce identical index text
        // too, or the convergence test's whole point is defeated by the
        // one part of it that ISN'T covered by the table-DDL macro.
        "CREATE INDEX audit_events_project_sequence\n    ON audit_events(project_id, sequence DESC)",
        "CREATE INDEX audit_events_operation_sequence\n    ON audit_events(operation_id, sequence ASC)",
        "CREATE UNIQUE INDEX audit_events_one_authorization_per_operation\n    ON audit_events(operation_id) WHERE outcome = 'authorized'",
        "CREATE INDEX audit_events_family_outcome_sequence\n    ON audit_events(family, outcome, sequence DESC)",
        // Restore the high-water mark -- `MAX` so this can only move the
        // mark forward (matching whatever the rebuild's own inserts
        // already advanced it to), never backward. Only reachable if the
        // rebuild table actually has a `sqlite_sequence` row itself
        // (true whenever it received at least one row; see the carry
        // step's own comment for the symmetric zero-row case).
        "UPDATE sqlite_sequence SET seq = MAX(seq, \
            (SELECT COALESCE(MAX(seq), 0) FROM audit_events_seq_carry)) \
         WHERE name = 'audit_events'",
        "DROP TABLE audit_events_seq_carry",
    ],
}];

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
        .execute_batch(CREATE_SCHEMA_V2)
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

/// Keyword-based, not statement-shape-based: this permits `CREATE TABLE
/// ... AS SELECT` (it only inspects the first word, `CREATE`) as well as
/// plain `CREATE TABLE (...)`. RFC-013 Amendment 1's `1 -> 2` step
/// depends on this specifically -- the `sqlite_sequence` high-water-mark
/// carry uses `CREATE TABLE audit_events_seq_carry AS SELECT ...` -- so a
/// future tightening of this whitelist to distinguish the two `CREATE
/// TABLE` shapes would silently break that migration step. Noted here so
/// nobody narrows this without checking `MIGRATIONS` first.
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
