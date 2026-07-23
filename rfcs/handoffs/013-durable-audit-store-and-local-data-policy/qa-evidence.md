# RFC-013: Durable Audit Store and Local Data Policy - QA Evidence

Status: Proposed; no implementation evidence recorded
Target milestone: M7
Date opened: 2026-07-22

## Evidence Policy

Record only commands and outputs observed during the relevant implementation review thread. Do not use design acceptance as implementation evidence.

Evidence in this file must not be used to claim a rendered audit viewer, encryption, tamper evidence, secure deletion, automatic retention, cloud sync, cross-process writing, exact command/output persistence, general command interception, or complete crash/power-loss safety unless later reviewed implementation explicitly supports those claims.

## Design Evidence

Review response 092 required amendment and re-review. The amendment records:

- security-direction ordering so audit failure cannot block protective authority reductions;
- distinct authorization/decision records and applied/observed outcomes;
- class-constrained approval action/risk/actor/source/adapter metadata with exact command/cwd still excluded;
- bundled SQLite, in-core audit module ownership, required managed/supervised launch authorization, project/global purge only, narrow root/symlink integration, bounded read-only startup probing, no-salvage recovery, and separate migration/recovery slices.

Review response 093 confirmed those amendments and required one further schema-contract amendment:

- application-generated operation ids now correlate authorization with later applied/started/failed/terminated outcomes without relying on adjacency;
- incomplete authorizations remain truthful after restart;
- the per-class matrix now exhaustively covers retained v1 families;
- audit-data purge remains ephemeral-only and generated-change metadata purge is deferred rather than represented speculatively;
- safe-close applied means action issued, not process exited.

Review response 094 confirmed the correlation and exhaustive-family direction, then required literal reconciliation:

- managed launch/start/failure/termination now use one `managed_process_lifecycle` family;
- each operation id permits exactly one authorization, with exact retry bound to the same event id and canonical record;
- phase cardinality rejects contradictory outcomes and termination before start;
- actor/source codes and valid pairs are exhaustive and shared by the DTO, matrix, handoff, and tests;
- project/global audit purge receipts are unconditionally ephemeral in v1 with no durable purge event.

Review response 095 accepted RFC-013 for implementation after the amendments from responses 092 through 094. This acceptance covers design only; implementation evidence remains pending.

## PR-013-B - Durable Record and Path Model

Implementation awaiting review:

- Added the `audit/` subsystem with a versioned `DurableAuditRecordV1`, stable string-code enums, bounded opaque references, typed validation failures, and an application-generated `AuditOperationId`.
- Added exhaustive Rust validation for all retained v1 event families, required/forbidden fields, actor/source pairs, authorization shape, and phase vocabulary.
- Added canonical state-root resolution, fixed audit/database/recovery paths, project-root exclusion, later-project compatibility checks, and rejection of existing symlinked audit paths.
- Durable records contain no free-form summary, command, cwd, path, output, prompt, environment, content, or arbitrary metadata field. `AuditEvent.summary` is not accepted by the durable DTO.
- A blanket conversion from legacy `AuditEvent` is intentionally not provided in this slice: several legacy classes lack the operation, risk, reason, subject, or adapter data required by the v1 matrix. PR-013-G must convert mature producers from typed domain context without parsing or persisting `AuditEvent.summary`.

Focused evidence observed on 2026-07-22:

- `cargo test -p tekstide-core audit::` passed; 36 tests passed, 0 failed.
- Record tests cover all retained families, invalid actor/source pairs, unsupported schema versions, incomplete subjects, unrelated ids/phases, required operation ids, safe-close subject restrictions, and path/content rejection for opaque references.
- Path tests cover valid containment, relative/missing roots, project containment conflicts, later-added project conflicts, project roots beneath state that do not contain audit state, invalid path types, symlink rejection, and content-free errors.

## PR-013-C - SQLite Schema, Append, and Query Store

Implementation awaiting review:

- Added workspace-managed `rusqlite 0.39.0` with `default-features = false` and only the `bundled` feature requested directly. The lockfile selects `libsqlite3-sys 0.37.0` and bundled SQLite `3.51.3`.
- `rusqlite` and `libsqlite3-sys` declare MIT licensing; the bundled SQLite amalgamation is public-domain code according to the vendored package README and source notice.
- Bundled mode compiles and statically links the SQLite C amalgamation through the Rust build, adding native C compilation and increasing build/artifact cost. No clean before/after binary-size baseline was retained, so no quantitative binary-size claim is made.
- Current supported-target evidence is limited to `x86_64-unknown-linux-gnu` with Rust `1.97.1`; no Windows or macOS build claim is made in this slice.
- Fresh schema creation is transactional and sets application id `0x544b4155` plus schema/user version `1`.
- The connection uses WAL journal mode, `FULL` synchronous mode, foreign keys enabled, and a two-second busy timeout. Application coordination is expected to use one owned writer, but `AuditStore` does not enforce that convention: additional in-process or cross-process writers may open the database and are serialized or rejected by SQLite locking and the busy timeout.
- Appends use immediate transactions, prepared statements, event-id uniqueness, exact-retry comparison, and typed content-free failures. Rejected correlation/phase attempts leave no partial rows.
- SQLite CHECK constraints mirror the v1 family-field matrix and opaque-reference alphabet; a partial unique index enforces one authorization row per operation id at the database boundary.
- Operation correlation requires an earlier authorization with the same project, family, and action kind. Ordinary and managed lifecycle phase cardinality is explicit and does not depend on adjacency.
- Queries are capped at 200 records and support stable descending sequence cursors plus project, family, outcome, and operation filters.
- Persisted-row decode failure has a distinct `DecodeFailed` reason and cannot be mistaken for SQLite corruption. Query pages fail closed in full when any selected row is undecodable; PR-013-E must revisit that deliberate blast radius alongside corruption and recovery policy.

Observed gates on 2026-07-22:

- `cargo fmt --all --check` passed.
- `cargo check --workspace --all-targets` passed.
- `cargo test --workspace` passed; `tekstide-core` ran 338 tests with 0 failures, other workspace test targets and doc tests ran 0 tests with 0 failures.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `git diff --check` passed.
- Direct-SQL regression coverage proves the database rejects mismatched family fields, path-like opaque references, and duplicate authorization rows independently of Rust validation.
- Store tests cover fresh/open/reopen, commit visibility, exact retry, conflicting event reuse, rejected-attempt rollback, database constraints, bounded privacy sentinels, filter/cursor behavior, cross-project/wrong-family/later correlation rejection, interleaved operations, contradictory phases, termination ordering, exact phase retry, and incomplete authorization after reopen.

Deferred from this slice:

- PR-013-D owns immutable v1 fixtures, complete read-only identity/version probing evidence, migration sequencing, and migration rollback.
- PR-013-D/E must validate pre-existing WAL and shared-memory sidecar paths before relying on them.
- PR-013-E owns corruption classification and explicit recovery, including the whole-page decode-failure policy.
- PR-013-F owns purge and local-data summaries.
- PR-013-G owns runtime producer integration and audit-required action ordering.
- PR-013-H must add the project NOTICE entry for the first bundled third-party native code before release readiness.

Review response 096 accepted PR-013-B/C with two required follow-ups:

- Persisted-row decode failures now use a private rusqlite conversion marker and surface as `AuditStoreErrorReason::DecodeFailed`; malformed SQL no longer impersonates database corruption.
- The evidence now describes single-writer ownership as an unenforced application convention and records SQLite locking/busy-timeout behavior for additional writers.

Response 096 also accepted the absence of blanket legacy `AuditEvent` conversion, deferred module splitting until migration/recovery boundaries provide a real ownership split, and asked later slices to carry WAL/shared-memory sidecar validation plus a release NOTICE entry.

Observed after the response 096 follow-up on 2026-07-22:

- `cargo test -p tekstide-core audit::` passed; 37 tests passed, 0 failed.
- `cargo fmt --all --check` passed.
- `cargo check --workspace --all-targets` passed.
- `cargo test --workspace` passed; `tekstide-core` ran 339 tests with 0 failures, other workspace test targets and doc tests ran 0 tests with 0 failures.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `git diff --check` passed.

## PR-013-D - Schema Identity and Migration Harness

Implementation awaiting review:

- Added `crates/tekstide-core/src/audit/migration.rs` as the schema lifecycle boundary, reducing migration/identity responsibility in `store.rs`.
- Added the independent tracked SQL fixture `crates/tekstide-core/src/audit/tests/fixtures/audit-v1.sql`. It records the complete v1 schema, application id, user version, constraints, and indexes without deriving them from the production schema constant.
- Existing databases are opened read-only first and checked for application id, supported user version, required table presence, and one primary-key-limited row read. No row-count-dependent integrity scan runs during ordinary startup.
- After the read-write connection opens, identity/version are re-read and must match the read-only probe before migration or write-capable connection configuration.
- Foreign application ids, future versions, and versions older than the current supported floor are rejected before write-capable open. Tests snapshot the database, journal, WAL, and shared-memory artifacts and prove foreign/future rejection does not change them.
- Fresh database creation remains transactional and creates current application/schema identity only after path validation.
- The migration runner requires exact sequential `N -> N+1` steps and runs the complete chain in one immediate transaction. Missing/nonsequential steps fail without writes, and a later failing step rolls back earlier steps and the schema version.
- V1 is Tekstide's first durable-audit schema, so there is no supported historical production migration. Synthetic v1-to-v3 steps exercise the harness; the tracked v1 fixture is the immutable input baseline for the next real schema version.
- Fixed SQLite companion paths for rollback journal, WAL, and shared memory are now exposed by `AuditStoragePath`; pre-existing companions must be regular files and cannot be symlinks. Recovery behavior remains PR-013-E scope.

Observed gates on 2026-07-22:

- `cargo test -p tekstide-core audit::` passed; 44 tests passed, 0 failed.
- `cargo fmt --all --check` passed.
- `cargo check --workspace --all-targets` passed.
- `cargo test --workspace` passed; `tekstide-core` ran 346 tests with 0 failures, other workspace test targets and doc tests ran 0 tests with 0 failures.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `git diff --check` passed.

Explicit limits:

- No production migration is registered because no schema predates v1.
- Ordinary startup does not run `quick_check` or `integrity_check`; comprehensive diagnostics belong to PR-013-E.
- This slice does not classify malformed/truncated databases beyond existing typed SQLite errors, quarantine artifacts, write recovery manifests, recreate stores, or append recovery events.
- The canonical v1 fixture proves identity and compatibility/openability, not byte-for-byte schema equivalence with fresh schema creation.

Review response 097 accepted PR-013-D with one required migration-harness follow-up:

- Replaced free-form `execute_batch` migration steps with explicit single-statement lists.
- Added a conservative migration statement allowlist for `CREATE`, `ALTER`, `DROP`, `INSERT`, `UPDATE`, and `DELETE`; transaction control, journal-affecting PRAGMAs, `VACUUM`, comments before statements, and other unreviewed statement classes fail as `InvalidMigration`.
- Individual statements use `Transaction::execute`, which rejects appended second statements. A regression with `CREATE TABLE ...; COMMIT;` proves that embedded transaction control cannot leak schema changes or advance `user_version` after a later failure.
- Documented that existing-store migration must run before connection configuration because SQLite table rebuilds require foreign-key enforcement off and identity validation must precede WAL/journal changes.
- Documented that the second bounded schema read deliberately re-verifies the write-capable connection after identity checking and any migration.

Observed after the response 097 follow-up on 2026-07-22:

- `cargo test -p tekstide-core audit::tests::migration -- --nocapture` passed; 8 tests passed, 0 failed.
- `cargo fmt --all --check` passed.
- `cargo check --workspace --all-targets` passed.
- `cargo test --workspace` passed; `tekstide-core` ran 348 tests with 0 failures, other workspace test targets and doc tests ran 0 tests with 0 failures.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `git diff --check` passed.

## PR-013-E - Corruption and Recovery Harness

Implementation awaiting review:

- Added an explicit diagnostics boundary that first reuses the bounded read-only startup probe, then runs `PRAGMA integrity_check` and validates every durable row only when diagnostics are requested.
- Diagnostics classify missing, healthy, corrupt, semantically invalid-record, foreign-application, future/unsupported-schema, and unavailable stores without carrying paths or persisted content in the report.
- Malformed databases, truncated databases, and current-identity databases missing the required table are classified as corrupt. Ordinary `AuditStore::open` attempts fail without renaming, deleting, or recreating the database evidence.
- Persisted rows that fail the durable record decoder are classified as `InvalidRecords`, not SQLite corruption. Query pages continue to fail closed in full, and explicit recovery accepts this state without parsing or salvaging row content.
- Added explicit quarantine/recreate recovery. The caller contract requires application-owned SQLite handles to be closed before recovery begins.
- Recovery addresses exactly `audit.sqlite3`, `audit.sqlite3-journal`, `audit.sqlite3-wal`, and `audit.sqlite3-shm`; no directory glob is used.
- Every attempt creates one unique recovery bundle and a content-free manifest with a moved, absent, or failed result for each expected artifact.
- Recovery attempts all four known artifacts. Any failed move writes an incomplete manifest and prevents fresh-store creation.
- A complete quarantine creates the current schema and attempts to append one structured `audit_store_recovery` event referencing only the recovery-bundle identifier. Event persistence failure is represented by the ephemeral receipt rather than undoing successful quarantine/recreation.
- Recovery refuses missing, healthy, foreign-application, and unsupported future-schema stores without moving evidence.
- Recovery-directory and SQLite-artifact path checks reject symlink or unexpected file-type boundaries before recovery.

Observed gates on 2026-07-22:

- `cargo test -p tekstide-core audit::` passed; 53 tests passed, 0 failed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace --all-targets --all-features` passed; `tekstide-core` ran 355 tests with 0 failures and the other workspace targets ran 0 tests with 0 failures.

Explicit limits:

- Recovery is an explicit caller-driven operation; it does not discover or close live SQLite handles itself.
- Recovery performs quarantine and fresh initialization only. It does not salvage records from corrupt evidence.
- Recovery bundles remain local data until a later explicit policy handles them; PR-013-F owns purge and local-data accounting behavior.
- No runtime security-event producer is wired to the durable store in this slice.

Review response 098 required a restart-safe incomplete-recovery guard and clean final-tree evidence:

- Recovery now creates and synchronizes one content-free `active-recovery.json` marker before the first artifact move. Ordinary `AuditStore::open` returns `RecoveryIncomplete` while that marker exists, including when the canonical database has already moved and would otherwise look like first run.
- `AuditRecovery::resume` reads the bounded marker, validates the exact recovery bundle, reconstructs moved/absent/remaining artifact state from exact paths, retries quarantine and manifest writing, and finishes fresh initialization explicitly.
- A complete manifest is synchronized before fresh initialization. The active marker is removed only after initialization and the best-effort recovery event step finish.
- Partial-move, manifest-write-failure, interrupted-move, and failed-fresh-initialization tests all prove ordinary open remains blocked and explicit resume can complete without silently treating the state as first run.
- The review-created `tmp_probe.rs` print-only probe was removed. Its unsafe scenario is retained as asserted recovery tests.

Observed after the response 098 follow-up on 2026-07-22:

- `cargo test -p tekstide-core audit::` passed; 56 tests passed, 0 failed.
- `cargo fmt --all -- --check` passed.
- `cargo test --workspace --all-targets --all-features` passed; `tekstide-core` ran 358 tests with 0 failures and the other workspace targets ran 0 tests with 0 failures.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

Review response 099 required fresh-store initialization to remain API-resumable when SQLite leaves regular partial-attempt artifacts:

- Fresh initialization now uses the exact recovery-owned `.audit.sqlite3.recovery-new` database path instead of creating the canonical database directly.
- After schema creation and best-effort recovery-event append, the temporary store switches from WAL to DELETE journal mode, closes, and must pass explicit healthy diagnostics with no journal/WAL/shared-memory companions.
- Only that complete single database file is atomically renamed to `audit.sqlite3`; the audit directory is synchronized before recovery finalization removes the active marker.
- A failed fresh attempt can leave only the exact temporary database/journal/WAL/shared-memory paths. Explicit resume, authorized by the active marker and complete manifest, removes those recovery-owned regular files and retries initialization without operator filesystem cleanup.
- If interruption occurs after atomic installation but before marker removal, resume recognizes the healthy canonical store, validates the complete manifest, avoids duplicating a matching recovery event, and finalizes the marker.
- If marker removal itself fails after installation, ordinary open remains blocked and a later resume idempotently finalizes the already healthy store.
- The initialization-failure regression injects regular temporary database, journal, WAL, and shared-memory files, proves ordinary open remains blocked with no canonical database, and proves `AuditRecovery::resume` cleans the temporary set and reaches a healthy store through the public API alone.
- A separate compatibility regression injects regular partial canonical database/journal/WAL/shared-memory artifacts after the complete manifest, proves ordinary open remains blocked, and proves public resume resolves the previously reported unsupported-application dead end without manual cleanup.
- Canonical-artifact cleanup exists only for recovery states produced by the earlier direct-canonical initialization implementation. It requires the active marker and complete matching manifest that prove original evidence is already quarantined; it is not a general store-reset capability.

Observed after the response 099 follow-up on 2026-07-23:

- `cargo test -p tekstide-core audit::` passed; 57 tests passed, 0 failed.
- `cargo fmt --all -- --check` passed.
- `cargo test --workspace --all-targets --all-features` passed; `tekstide-core` ran 359 tests with 0 failures and the other workspace targets ran 0 tests with 0 failures.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

## PR-013-F - Purge and Local-Data Summary

Pending implementation.

## PR-013-G - Security-Event Integration

Pending implementation.

## PR-013-H - Closeout Evidence

Pending implementation.

## Known Limitations

- Final limitations will be recorded from accepted implementation evidence.
- Until then, no durable-audit implementation claim is made.
