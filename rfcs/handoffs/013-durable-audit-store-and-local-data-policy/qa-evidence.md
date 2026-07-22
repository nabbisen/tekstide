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

Pending implementation.

## PR-013-E - Corruption and Recovery Harness

Pending implementation.

## PR-013-F - Purge and Local-Data Summary

Pending implementation.

## PR-013-G - Security-Event Integration

Pending implementation.

## PR-013-H - Closeout Evidence

Pending implementation.

## Known Limitations

- Final limitations will be recorded from accepted implementation evidence.
- Until then, no durable-audit implementation claim is made.
