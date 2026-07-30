---
title: "RFC-013 Amendment 1 - Schema v2 Migration: Implementation Handoff"
rfc: "RFC-013"
rfc_file: "../../done/013-durable-audit-store-and-local-data-policy.md"
amendment: "Amendment 1"
status: "Authorised by the human owner 2026-07-30 - ready for implementation"
authorises: "RFC-021 PR-021-E2 command_cwd_mismatch anomaly"
created: "2026-07-30"
---

# RFC-013 Amendment 1 — Schema v2 Migration

**Read [RFC-013 §Amendment 1](../../done/013-durable-audit-store-and-local-data-policy.md) first.** It records what the owner authorised and why. This document is how to build it.

## Why you are here

Commit `3ac794b` added `'anomaly'` and `'command_cwd_mismatch'` to `CREATE_SCHEMA_V1`'s `CHECK` constraints without bumping the schema version or adding a migration. That was a correct feature landed through an incorrect mechanism — and the requirement that collided with RFC-013's frozen matrix was the reviewer's, issued without checking the freeze. You are not fixing a mistake of your own.

The observable defect, from review response 117's probe against a real pre-`3ac794b` database:

```
AuditStore::open on a 0.3.0-era DB: OK (no migration ran)
record_cwd_mismatch_anomaly -> Degraded
anomaly records persisted   -> 0
control: record_command_request -> Persisted
```

Existing installations silently cannot write the new record, permanently.

## This is the first real migration this harness has ever run

`MIGRATIONS: &[MigrationStep] = &[]` has been empty since RFC-013 closed. The stepping logic has only ever been exercised by failure-injection tests. **Treat it as unproven code, not as infrastructure you are merely configuring.**

Specifically: RFC-013 PR-013-E review found the harness could leave `user_version` advanced with a partially-created table behind after a failing step. Whether that was fully fixed is not something to infer from the fix having been accepted — re-prove it against your actual step.

## The trap in the obvious fix

`audit/migration.rs` line 10:

```rust
const OLDEST_SUPPORTED_SCHEMA_VERSION: i64 = AUDIT_SCHEMA_VERSION;
```

Bumping `AUDIT_SCHEMA_VERSION` to `2` drags `OLDEST_SUPPORTED_SCHEMA_VERSION` to `2` with it, putting every existing v1 database out of range. Probed:

```
user_version=0: open -> Err(UnsupportedSchema)   db still present, size=40960
user_version=2: open -> Err(UnsupportedSchema)   db still present, size=40960
```

The file survives — `UnsupportedSchema` is a hard, visible failure that does not quarantine or recreate, which is the correct direction. But an audit store that will not open is still a broken upgrade. **Pin `OLDEST_SUPPORTED_SCHEMA_VERSION` to the literal `1`.**

## Required work

### 1. Separate the historical v1 DDL from the current DDL

Revert `CREATE_SCHEMA_V1` to its pre-`3ac794b` content — it is the definition of *version one* and must describe version one. Add the v2 DDL as its own constant, and have `create_current_schema` use that.

Do not keep one constant that is edited forward each time. Two named constants that each describe one version is the property that makes a fresh install and a migrated install comparable.

### 2. Bump and pin the versions

- `AUDIT_SCHEMA_VERSION` → `2`
- `OLDEST_SUPPORTED_SCHEMA_VERSION` → literal `1`, not derived

### 3. Add the 1 → 2 migration step

SQLite cannot `ALTER` a `CHECK` constraint, so this is the table-rebuild pattern: create the new table with the v2 constraints, copy every row, drop the old table, rename. The harness's statement-keyword validation already permits `CREATE`/`DROP`/`INSERT`/`ALTER`, so the shape is expressible in `MigrationStep::statements`.

Two things the rebuild must preserve, and both are easy to lose:

- **`sequence` values must survive unchanged.** They are the audit trail's ordering, and RFC-013's whole append-only claim rests on them. Copy the column explicitly; do not let a rebuilt `AUTOINCREMENT` reassign it.
- **Every index, trigger, and constraint the v1 table carried** must exist on the v2 table. Diff the rebuilt table's `sqlite_master` entry against the v2 DDL rather than trusting the statement list to be complete.

### 4. Convergence test — fresh install and migrated install must be identical

Create one database fresh at v2 and one by migrating a v1 fixture. Compare their `sqlite_master` SQL for `audit_events` and assert equality. This is the test that catches the two-constants-drifting-apart failure, and nothing else will.

### 5. Fixtures

`audit/tests/fixtures/audit-v1.sql` already holds the genuine pre-amendment DDL. **Do not edit it.** RFC-013 requires immutable fixtures per supported prior version, and its value now is precisely that it disagrees with the current schema.

Add `audit-v2.sql` as the expected post-migration fixture.

Convert `canonical_v1_fixture_opens_and_remains_current` into a v1 → v2 migration assertion: the v1 fixture opens, migrates, ends at `user_version = 2`, accepts a `command_cwd_mismatch` write, and retains every pre-existing row with its original `sequence`.

Note why the old test passed while the schema was wrong: identity compares only `application_id` and `user_version`, never the DDL. A test that cannot see a schema divergence is the same class of problem as a corpus that cannot fail — response 110's lesson, arriving through the schema layer.

### 6. Re-prove the interrupted-migration property

RFC-013: *"a failed migration leaves the prior database usable or returns a recoverable failure without claiming success."*

Inject a failure partway through the real 1 → 2 step — for example a final statement that violates a constraint, or a deliberately malformed trailing statement — and assert:

- the database still opens afterwards, as **v1**;
- `user_version` is still `1`;
- no partially-created v2 table remains;
- every original row is present with its original `sequence`;
- the failure is reported, and success is not claimed.

Use the ablation methodology you have used since response 110: confirm each assertion fails if the transaction boundary is removed, then restore.

### 7. Concurrency

Two Tekstide processes may open the same store. RFC-013 requires migrations to run in a transaction; confirm the 1 → 2 step holds an `IMMEDIATE` transaction for its whole duration so a second process cannot observe a half-migrated schema or run the step twice. If the existing harness already guarantees this, say where, rather than restating that it does.

## Evidence required

- The seven probe outcomes from item 6, as real `cargo test` output.
- The convergence test from item 4.
- Round-trip: a v1 fixture with pre-existing rows migrates, and a `command_cwd_mismatch` anomaly then persists — the end-to-end proof that the original defect is closed.
- Gate output: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features`, `git diff --check`.
- Any limitation you could not close, stated as a limitation rather than omitted.

## Also required, and independent of the migration

Restore `rfcs/handoffs/021-command-approval-model-and-adapter-capability/acceptance-qa-checklist.md` line 74 to its original requirement text:

> `- [ ] Events conform to the frozen command_approval family; schema unamended.`

Mark it **not met**, and record beneath it that the family was additively amended under RFC-013 Amendment 1 with owner authorisation on 2026-07-30, referencing that amendment.

A checklist item is a requirement. Editing the requirement so the implementation satisfies it removes the checklist's ability to say no — and this one was a real constraint RFC-013 imposed. The disclosure you wrote was good; it belongs *under* an unmet requirement, not inside a rewritten one.

## Review gate

This slice gets a full review, not diff confirmation. A schema migration against released data is the highest-risk change class in this codebase, and I will probe the interrupted-migration path and the convergence property independently.

RFC-021's PR-021-F closeout is blocked until this lands.
