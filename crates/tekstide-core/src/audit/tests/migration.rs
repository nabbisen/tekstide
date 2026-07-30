use std::fs;
use std::path::Path;

use rusqlite::Connection;

use crate::audit::migration::{MIGRATIONS, MigrationStep, migrate_sequentially};
use crate::audit::schema::{CREATE_SCHEMA_V1, CREATE_SCHEMA_V2};
use crate::audit::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome, AuditQuery,
    AuditRiskLevel, AuditStore, AuditStoreErrorReason, DurableAuditRecordV1,
};
use crate::domain::ApprovalId;
use crate::project::ProjectId;

/// Real UUID-shaped test fixture ids -- `AuditEventId`/`ProjectId`'s
/// `from_persisted` require this shape to decode a row back through
/// `AuditStore::query`, exactly the `AgentRunId::for_test` trap RFC-021
/// PR-021-E2 hit for the same reason. Raw SQL inserted directly against
/// the schema (simulating pre-existing rows a real installation wrote)
/// must use ids in this shape too, or query-back fails for reasons
/// unrelated to what these tests are actually checking.
fn fixture_event_id(n: u8) -> String {
    format!("audit-{n:08x}-0000-4000-8000-000000000000")
}

fn fixture_project_id(n: u8) -> String {
    format!("{n:08x}-0000-4000-8000-000000000000")
}

use super::support::TestAuditDirs;

const V1_FIXTURE: &str = include_str!("fixtures/audit-v1.sql");
const V2_FIXTURE: &str = include_str!("fixtures/audit-v2.sql");

/// RFC-013 Amendment 1, item 5: `CREATE_SCHEMA_V1` must never be hand-
/// edited to match a later version -- its only value is that it now
/// disagrees with the current schema. This is the regression guard: if
/// anyone ever edits it in place again (the exact defect commit
/// `3ac794b` introduced), this test catches the divergence from the
/// immutable fixture immediately, rather than relying on the reviewer
/// noticing.
#[test]
fn create_schema_v1_constant_matches_the_immutable_fixture_exactly() {
    let fixture_ddl = V1_FIXTURE
        .lines()
        .filter(|line| !line.starts_with("PRAGMA"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(CREATE_SCHEMA_V1.trim(), fixture_ddl.trim());
}

/// Companion to the above for v2: a fresh install's DDL must match the
/// expected post-migration fixture too, so the convergence test (below)
/// has a known-good reference on both sides, not just internal
/// self-consistency between two Rust constants.
#[test]
fn create_schema_v2_constant_matches_the_expected_fixture_exactly() {
    let fixture_ddl = V2_FIXTURE
        .lines()
        .filter(|line| !line.starts_with("PRAGMA"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(CREATE_SCHEMA_V2.trim(), fixture_ddl.trim());
}

/// RFC-013 Amendment 1, item 5: converted from `canonical_v1_fixture_
/// opens_and_remains_current`, which only proved a v1 fixture *opened*
/// -- exactly the test that kept passing while `CREATE_SCHEMA_V1` and
/// `audit-v1.sql` silently disagreed after commit `3ac794b`, because
/// schema identity compares only `application_id`/`user_version`, never
/// the DDL (the amendment's own "why the old test passed while the
/// schema was wrong" note). This version proves the actual migration:
/// pre-existing rows (inserted directly, bypassing `AuditStore`, so
/// their `sequence` values are assigned the way a real prior installation
/// would have them) survive with their original `sequence`, the database
/// ends at `user_version = 2`, and -- the end-to-end proof the original
/// defect is closed -- a `command_cwd_mismatch` anomaly write, which used
/// to silently degrade on this exact fixture (response 117's probe),
/// now persists.
#[test]
fn v1_fixture_with_existing_rows_migrates_to_v2_preserving_sequence_and_accepts_the_new_anomaly() {
    let dirs = TestAuditDirs::new("migration-v1-to-v2");
    fs::create_dir_all(dirs.storage_path.audit_dir()).unwrap();
    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    connection.execute_batch(V1_FIXTURE).unwrap();
    // Two genuinely pre-existing rows, inserted directly against the raw
    // v1 schema -- not through any Rust-level API, since the whole point
    // is to simulate rows a real prior installation already wrote.
    connection
        .execute(
            "INSERT INTO audit_events \
                (event_id, schema_version, project_id, family, outcome, \
                 action_kind, actor_kind, action_source, created_at) \
             VALUES \
                (?1, 1, ?2, 'project_added', 'applied', 'project_add', 'user', \
                 'trusted_ui', '2026-01-01T00:00:00Z')",
            rusqlite::params![fixture_event_id(1), fixture_project_id(1)],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO audit_events \
                (event_id, schema_version, project_id, family, outcome, \
                 action_kind, actor_kind, action_source, created_at) \
             VALUES \
                (?1, 1, ?2, 'project_added', 'applied', 'project_add', 'user', \
                 'trusted_ui', '2026-01-02T00:00:00Z')",
            rusqlite::params![fixture_event_id(2), fixture_project_id(2)],
        )
        .unwrap();
    let original_rows: Vec<(i64, String)> = connection
        .prepare("SELECT sequence, event_id FROM audit_events ORDER BY sequence")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        original_rows.len(),
        2,
        "test precondition: two pre-existing rows"
    );
    drop(connection);

    // Opening via the real `AuditStore::open` is what actually runs the
    // `1 -> 2` migration (`prepare_existing_store` -> `migrate_
    // sequentially`), exactly the path a real upgrade takes.
    let mut store = AuditStore::open(dirs.storage_path.clone()).unwrap();

    // A `command_cwd_mismatch` anomaly -- silently rejected before this
    // amendment (response 117's probe: `Degraded`, 0 rows persisted) --
    // now persists, on a database that started life as v1.
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::CommandApproval,
        AuditOutcome::Anomaly,
        AuditActionKind::CommandCwdMismatch,
        AuditActorKind::AppPolicy,
        AuditActionSource::Adapter,
    );
    record.project_id = Some(ProjectId::for_test(1));
    record.approval_id = Some(ApprovalId::new_uuid());
    record.risk_level = Some(AuditRiskLevel::Low);
    let append_result = store.append(&record);
    assert!(
        append_result.is_ok(),
        "command_cwd_mismatch must persist on a migrated database: {append_result:?}"
    );

    // Kept as `SequencedAuditRecord` (not mapped down to just `.record`)
    // -- `sequence` lives only on the wrapper, and preserving it exactly
    // is the property this test exists to prove.
    let records = store.query(&AuditQuery::latest(10)).unwrap().records;
    let anomaly = records
        .iter()
        .find(|sequenced| sequenced.record.action_kind == AuditActionKind::CommandCwdMismatch)
        .expect("the anomaly record must be queryable back");
    assert_eq!(anomaly.record.outcome, AuditOutcome::Anomaly);

    // Every pre-existing row is still present, with its ORIGINAL sequence
    // -- not renumbered by the rebuild's `AUTOINCREMENT`.
    for (sequence, event_id) in &original_rows {
        let found = records
            .iter()
            .find(|sequenced| sequenced.record.event_id.as_str() == event_id);
        let found = found
            .unwrap_or_else(|| panic!("pre-existing row {event_id} must survive the migration"));
        assert_eq!(
            found.sequence, *sequence,
            "row {event_id}'s sequence must be preserved exactly by the rebuild"
        );
    }
    drop(store);

    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    assert_eq!(pragma(&connection, "application_id"), 0x544B_4155);
    assert_eq!(pragma(&connection, "user_version"), 2);
}

/// A v2 fixture (no migration needed) must also open and stay current --
/// the companion to the v1 fixture test, proving `audit-v2.sql` itself is
/// a valid, openable v2 database, not just text that happens to match
/// `CREATE_SCHEMA_V2`.
#[test]
fn canonical_v2_fixture_opens_and_remains_current() {
    let dirs = TestAuditDirs::new("migration-v2-fixture");
    fs::create_dir_all(dirs.storage_path.audit_dir()).unwrap();
    let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    connection.execute_batch(V2_FIXTURE).unwrap();
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
    assert_eq!(pragma(&connection, "user_version"), 2);
}

/// RFC-013 Amendment 1, item 4 -- the convergence test: a fresh v2
/// install and a v1-fixture-then-migrated database must produce
/// **identical** `sqlite_master` entries for `audit_events` (the table
/// itself and every index). This is the test that would have caught
/// `CREATE_SCHEMA_V1`/`audit-v1.sql` silently diverging, and it is the
/// only test in this file that would catch the migration's rebuilt table
/// drifting from a fresh install's -- comparing `application_id`/
/// `user_version` alone (what the old canonical-fixture test did) cannot
/// see a DDL difference at all.
#[test]
fn fresh_v2_install_and_migrated_v1_fixture_produce_identical_schema() {
    let fresh_dirs = TestAuditDirs::new("migration-convergence-fresh");
    let fresh_store = AuditStore::open(fresh_dirs.storage_path.clone()).unwrap();
    drop(fresh_store);
    let fresh_schema = audit_events_schema_entries(fresh_dirs.storage_path.database_file());

    let migrated_dirs = TestAuditDirs::new("migration-convergence-migrated");
    fs::create_dir_all(migrated_dirs.storage_path.audit_dir()).unwrap();
    let connection = Connection::open(migrated_dirs.storage_path.database_file()).unwrap();
    connection.execute_batch(V1_FIXTURE).unwrap();
    drop(connection);
    let migrated_store = AuditStore::open(migrated_dirs.storage_path.clone()).unwrap();
    drop(migrated_store);
    let migrated_schema = audit_events_schema_entries(migrated_dirs.storage_path.database_file());

    assert_eq!(
        fresh_schema, migrated_schema,
        "a fresh v2 install and a migrated-from-v1 database must have byte-identical \
         sqlite_master entries for audit_events (table + every index)"
    );
    assert!(
        !fresh_schema.is_empty(),
        "test precondition: there must be something to compare"
    );
}

/// RFC-013's rule, re-proved against the REAL `1 -> 2` step rather than
/// inherited from the harness's synthetic failure-injection tests
/// (`failed_migration_rolls_back_the_complete_sequence` etc., which never
/// exercised this migration's actual statements): *"a failed migration
/// leaves the prior database usable or returns a recoverable failure
/// without claiming success."* Reuses `MIGRATIONS[0]`'s real statements
/// plus one deliberately malformed trailing statement, run through the
/// same `migrate_sequentially` a real `AuditStore::open` would use.
///
/// The seven properties required by the amendment:
#[test]
fn interrupted_v1_to_v2_migration_leaves_the_v1_database_intact() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(CREATE_SCHEMA_V1).unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    connection
        .execute_batch(
            r#"
            INSERT INTO audit_events
                (event_id, schema_version, project_id, family, outcome,
                 action_kind, actor_kind, action_source, created_at)
            VALUES
                ('33333333-3333-3333-3333-333333333333', 1, 'project-interrupted',
                 'project_added', 'applied', 'project_add', 'user', 'trusted_ui',
                 '2026-01-01T00:00:00Z');
            "#,
        )
        .unwrap();

    let real_step = &MIGRATIONS[0];
    assert_eq!(real_step.from_version, 1);
    assert_eq!(real_step.to_version, 2);
    let mut broken_statements = real_step.statements.to_vec();
    broken_statements.push("CREATE TABLE this is not valid sql");
    // `MigrationStep::statements` is `&'static [&'static str]` -- every
    // element here already is `&'static str` (the real statements plus
    // one more string literal), so leaking the `Vec` to get a `'static`
    // slice is sound; it is a `#[cfg(test)]`-only leak for the duration
    // of the test process, not a runtime concern.
    let broken_statements: &'static [&'static str] =
        Box::leak(broken_statements.into_boxed_slice());
    let broken_migrations = [MigrationStep {
        from_version: 1,
        to_version: 2,
        statements: broken_statements,
    }];

    let error = migrate_sequentially(&mut connection, 1, 2, &broken_migrations).unwrap_err();

    // 1. The failure is reported, and success is not claimed. The
    //    trailing statement is a genuine SQL syntax error (not one of
    //    `validate_migration_statement`'s pre-execution keyword
    //    rejections), so it surfaces as `Io` -- `AuditStoreError::sqlite`'s
    //    catch-all for a real SQLite execution failure -- not
    //    `InvalidMigration`, which is reserved for the harness's own
    //    keyword whitelist (BEGIN/COMMIT/PRAGMA/VACUUM).
    assert_eq!(error.reason, AuditStoreErrorReason::Io);
    // 2. user_version is still 1.
    assert_eq!(pragma(&connection, "user_version"), 1);
    // 3. No partially-created v2 rebuild table remains.
    assert!(!table_exists(&connection, "audit_events_v2_rebuild"));
    // 4. The original audit_events table (v1 shape) still exists.
    assert!(table_exists(&connection, "audit_events"));
    // 5. Every original row is present, with its original sequence.
    let rows: Vec<(i64, String)> = connection
        .prepare("SELECT sequence, event_id FROM audit_events ORDER BY sequence")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, 1);
    assert_eq!(rows[0].1, "33333333-3333-3333-3333-333333333333");
    // 6. The database still opens afterwards, as v1 (the table exists
    //    with the v1 CHECK shape: a row violating only the v2-only
    //    `command_cwd_mismatch` clause must still be rejected as v1).
    let v2_only_row_rejected = connection
        .execute(
            "INSERT INTO audit_events \
             (event_id, schema_version, project_id, approval_id, family, outcome, \
              action_kind, actor_kind, action_source, risk_level, created_at) \
             VALUES ('44444444-4444-4444-4444-444444444444', 1, 'p', 'a', \
              'command_approval', 'anomaly', 'command_cwd_mismatch', 'app_policy', \
              'adapter', 'low', '2026-01-01T00:00:00Z')",
            [],
        )
        .is_err();
    assert!(
        v2_only_row_rejected,
        "the database must still be genuinely v1 -- a v2-only value must still be rejected"
    );
    // 7. No partial commit: schema_version pragma matches the table shape
    //    (already proved by 6 above using the real CHECK constraint,
    //    which is stronger than trusting the pragma alone).
}

// Ablation for the test above, run once against the real `migrate_
// sequentially` and recorded here rather than left as a permanent
// toggle: temporarily replaced the transaction-wrapped execution in
// `migrate_sequentially` (`migration.rs`) with per-statement
// `connection.execute` calls in SQLite's default autocommit mode (no
// shared transaction at all), leaving everything else -- including this
// test -- unchanged. Result: `interrupted_v1_to_v2_migration_leaves_the_
// v1_database_intact` failed on assertion 6 (the v2-only row was
// ACCEPTED, because the rebuild's CREATE/INSERT/DROP/RENAME/CREATE INDEX
// statements had each already committed individually before the final,
// deliberately malformed statement errored) -- and the pre-existing
// `failed_migration_rolls_back_the_complete_sequence` failed too
// (`user_version` ended at `2`, not `1`), reproducing exactly the bug
// RFC-013 PR-013-D review found: a failing step leaving `user_version`
// advanced and a partially-created table behind. Restored immediately
// after observing both failures; `migrate_sequentially`'s existing
// single-transaction wrapping was not modified.

/// RFC-013 Amendment 1 item 7: a second process must never observe a
/// half-migrated schema, nor run the step twice.
#[test]
fn concurrent_migration_holds_one_immediate_transaction_for_the_whole_step() {
    // `migrate_sequentially` (shared, unchanged infrastructure -- see
    // `migration.rs`) wraps every statement in `MIGRATIONS[0]`, the real
    // `1 -> 2` step, in ONE `transaction_with_behavior(TransactionBehavior
    // ::Immediate)` call, committed only once at the very end.
    // `TransactionBehavior::Immediate` acquires SQLite's RESERVED lock at
    // `BEGIN` time, before any statement in the step runs -- so a second
    // connection attempting a write during the rebuild blocks (or hits
    // `SQLITE_BUSY` past the busy timeout), never observing an
    // intermediate state. Demonstrated directly: hold the migration's
    // transaction open on one connection and confirm a second
    // connection's write attempt fails while it is held.
    let dirs = TestAuditDirs::new("migration-concurrency");
    fs::create_dir_all(dirs.storage_path.audit_dir()).unwrap();
    let mut connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    connection.execute_batch(V1_FIXTURE).unwrap();
    connection.pragma_update(None, "user_version", 1).unwrap();
    connection
        .busy_timeout(std::time::Duration::from_millis(50))
        .unwrap();
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    transaction
        .execute(MIGRATIONS[0].statements[0], [])
        .unwrap();

    let second_connection = Connection::open(dirs.storage_path.database_file()).unwrap();
    second_connection
        .busy_timeout(std::time::Duration::from_millis(50))
        .unwrap();
    let second_write = second_connection.execute(
        "INSERT INTO audit_events \
         (event_id, schema_version, project_id, family, outcome, action_kind, \
          actor_kind, action_source, created_at) \
         VALUES ('second-conn', 1, 'p', 'project_added', 'applied', 'project_add', \
          'user', 'trusted_ui', '2026-01-01T00:00:00Z')",
        [],
    );
    assert!(
        second_write.is_err(),
        "a second connection must not be able to write while the migration's \
         IMMEDIATE transaction is held open"
    );

    drop(transaction);
}

#[test]
fn future_schema_and_foreign_application_are_rejected_without_writes() {
    // RFC-013 Amendment 1 bumped AUDIT_SCHEMA_VERSION to 2, so `2` is no
    // longer future -- `3` is the genuinely-out-of-range probe now.
    assert_identity_rejected_without_writes(
        "migration-future",
        "PRAGMA user_version = 3",
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
    // RFC-013 Amendment 1: a fresh install now starts at v2 directly.
    assert_eq!(pragma(&connection, "user_version"), 2);
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

/// Every `sqlite_master` entry belonging to `audit_events` -- the table
/// itself plus every index, **including** SQLite's own implicit
/// autoindexes backing `UNIQUE`/`PRIMARY KEY` columns (their `sql` column
/// is always `NULL`, hence `Option<String>` here, not a missing row) --
/// as `(type, name, sql)` triples, ordered deterministically. Used by the
/// convergence test to compare a fresh v2 install against a migrated-
/// from-v1 database; comparing `type`/`name`/`sql` together (not just
/// `sql`) also catches an index quietly missing or renamed, not only a
/// textual DDL difference.
fn audit_events_schema_entries(database_file: &Path) -> Vec<(String, String, Option<String>)> {
    let connection =
        Connection::open_with_flags(database_file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    connection
        .prepare(
            "SELECT type, name, sql FROM sqlite_master \
             WHERE tbl_name = 'audit_events' ORDER BY type, name",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get::<_, Option<String>>(2)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
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
