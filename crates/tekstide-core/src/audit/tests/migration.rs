use std::fs;
use std::path::Path;

use rusqlite::Connection;

use crate::audit::migration::{MigrationStep, migrate_sequentially};
use crate::audit::{AuditQuery, AuditStore, AuditStoreErrorReason};

use super::support::TestAuditDirs;

const V1_FIXTURE: &str = include_str!("fixtures/audit-v1.sql");

#[test]
fn canonical_v1_fixture_opens_and_remains_current() {
    let dirs = TestAuditDirs::new("migration-v1-fixture");
    fs::create_dir_all(dirs.storage_path.audit_dir()).unwrap();
    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    connection.execute_batch(V1_FIXTURE).unwrap();
    drop(connection);

    let store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    assert!(
        store
            .query(&AuditQuery::latest(10))
            .unwrap()
            .records
            .is_empty()
    );
    drop(store);

    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    assert_eq!(pragma(&connection, "application_id"), 0x544B_4155);
    assert_eq!(pragma(&connection, "user_version"), 1);
}

#[test]
fn future_schema_and_foreign_application_are_rejected_without_writes() {
    assert_identity_rejected_without_writes(
        "migration-future",
        "PRAGMA user_version = 2",
        AuditStoreErrorReason::UnsupportedSchema,
    );
    assert_identity_rejected_without_writes(
        "migration-foreign",
        "PRAGMA application_id = 12345",
        AuditStoreErrorReason::UnsupportedApplication,
    );
}

#[test]
fn missing_store_initializes_current_identity() {
    let dirs = TestAuditDirs::new("migration-missing");
    assert!(!dirs.storage_path.database_file().exists());

    let store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    drop(store);

    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    assert_eq!(pragma(&connection, "application_id"), 0x544B_4155);
    assert_eq!(pragma(&connection, "user_version"), 1);
}

#[test]
fn migration_harness_applies_strictly_sequential_steps() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    let migrations = [
        MigrationStep {
            from_version: 1,
            to_version: 2,
            statements: &["CREATE TABLE migration_two (value INTEGER NOT NULL)"],
        },
        MigrationStep {
            from_version: 2,
            to_version: 3,
            statements: &["CREATE TABLE migration_three (value INTEGER NOT NULL)"],
        },
    ];

    migrate_sequentially(&mut connection, 1, 3, &migrations).unwrap();

    assert_eq!(pragma(&connection, "user_version"), 3);
    assert!(table_exists(&connection, "migration_two"));
    assert!(table_exists(&connection, "migration_three"));
}

#[test]
fn failed_migration_rolls_back_the_complete_sequence() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    let migrations = [
        MigrationStep {
            from_version: 1,
            to_version: 2,
            statements: &["CREATE TABLE should_roll_back (value INTEGER NOT NULL)"],
        },
        MigrationStep {
            from_version: 2,
            to_version: 3,
            statements: &["CREATE TABLE invalid syntax"],
        },
    ];

    assert!(migrate_sequentially(&mut connection, 1, 3, &migrations).is_err());

    assert_eq!(pragma(&connection, "user_version"), 1);
    assert!(!table_exists(&connection, "should_roll_back"));
}

#[test]
fn missing_or_nonsequential_migration_is_rejected_without_writes() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    let migrations = [MigrationStep {
        from_version: 1,
        to_version: 3,
        statements: &["CREATE TABLE must_not_exist (value INTEGER NOT NULL)"],
    }];

    let error = migrate_sequentially(&mut connection, 1, 3, &migrations).unwrap_err();

    assert_eq!(error.reason, AuditStoreErrorReason::UnsupportedSchema);
    assert_eq!(pragma(&connection, "user_version"), 1);
    assert!(!table_exists(&connection, "must_not_exist"));
}

#[test]
fn transaction_control_cannot_escape_the_migration_transaction() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    let migrations = [
        MigrationStep {
            from_version: 1,
            to_version: 2,
            statements: &["CREATE TABLE leaked (value INTEGER); COMMIT;"],
        },
        MigrationStep {
            from_version: 2,
            to_version: 3,
            statements: &["CREATE TABLE invalid syntax"],
        },
    ];

    assert!(migrate_sequentially(&mut connection, 1, 3, &migrations).is_err());

    assert_eq!(pragma(&connection, "user_version"), 1);
    assert!(!table_exists(&connection, "leaked"));
}

#[test]
fn transaction_and_journal_control_statements_are_rejected_before_execution() {
    const FORBIDDEN_STATEMENTS: &[&[&str]] = &[
        &["COMMIT"],
        &["BEGIN"],
        &["PRAGMA journal_mode = DELETE"],
        &["VACUUM"],
    ];
    for &statements in FORBIDDEN_STATEMENTS {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        let migrations = [MigrationStep {
            from_version: 1,
            to_version: 2,
            statements,
        }];

        let error = migrate_sequentially(&mut connection, 1, 2, &migrations).unwrap_err();

        assert_eq!(error.reason, AuditStoreErrorReason::InvalidMigration);
        assert_eq!(pragma(&connection, "user_version"), 1);
    }
}

fn assert_identity_rejected_without_writes(
    label: &str,
    identity_change: &str,
    expected: AuditStoreErrorReason,
) {
    let dirs = TestAuditDirs::new(label);
    fs::create_dir_all(dirs.storage_path.audit_dir()).unwrap();
    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    connection.execute_batch(V1_FIXTURE).unwrap();
    connection.execute_batch(identity_change).unwrap();
    drop(connection);
    let before = artifact_snapshot(dirs.storage_path.database_file());

    let error = match AuditStore::open(dirs.storage_path.clone()) {
        Ok(_) => panic!("invalid identity must be rejected"),
        Err(error) => error,
    };

    assert_eq!(error.reason, expected);
    assert_eq!(artifact_snapshot(dirs.storage_path.database_file()), before);
}

fn artifact_snapshot(database_file: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    ["", "-journal", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let path = database_file.with_file_name(format!(
                "{}{}",
                database_file.file_name().unwrap().to_string_lossy(),
                suffix
            ));
            (suffix.to_owned(), fs::read(path).ok())
        })
        .collect()
}

fn pragma(connection: &Connection, name: &str) -> i64 {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .unwrap()
}

fn table_exists(connection: &Connection, table: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get::<_, bool>(0),
        )
        .unwrap()
}
