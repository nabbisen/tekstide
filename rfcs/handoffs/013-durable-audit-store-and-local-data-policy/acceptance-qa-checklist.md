---
title: "RFC-013: Durable Audit Store and Local Data Policy - Acceptance / QA Checklist"
rfc: "RFC-013"
rfc_file: "../../done/013-durable-audit-store-and-local-data-policy.md"
status: "Implemented with documented limitations; Amendment 1 landed, awaiting full review"
target_milestone: "M7"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-22"
updated: "2026-07-30"
---

# RFC-013: Durable Audit Store and Local Data Policy - Acceptance / QA Checklist

## Acceptance Status

Design was accepted by response 095. PR-013-B through PR-013-G implementation was accepted by responses 096, 097, 100, 101, and 102. Response 103 accepted PR-013-H as complete with documented limitations; its earlier requirement for a `crates/tekstide/NOTICE` file was withdrawn as a maintainer-challenged error and no such file was created.

**Amendment 1 (schema v2 migration), authorised by the human owner 2026-07-30, landed the same day — awaiting full review** (not diff confirmation, per the amendment's own instruction: a schema migration against released data is the highest-risk change class in this codebase). See the Amendment 1 checklist below.

## Design Checklist

- [x] SQLite backend with bundled-only linkage policy accepted.
- [x] Domain/storage module boundary accepted.
- [x] Structured durable record and privacy allowlist accepted.
- [x] Security-direction matrix accepted.
- [x] Pre-action authorization and post-action outcome semantics accepted.
- [x] Per-class field/invariant matrix accepted.
- [x] Operation-id authorization/outcome correlation contract accepted.
- [x] Exactly-one-authorization and per-family phase cardinality accepted.
- [x] Exhaustive actor/source codes and allowed pairs accepted.
- [x] Approval action/risk/actor/source/adapter fields and narrowed external-design claim accepted.
- [x] Retain-until-explicit-purge policy accepted.
- [x] Project/global purge scopes accepted.
- [x] Migration/versioning policy accepted.
- [x] Missing/corrupt/future-store behavior accepted.
- [x] Explicit quarantine/recreate recovery accepted.
- [x] GUI, encryption, tamper-evidence, secure-deletion, and automatic-retention non-claims accepted.

## Record and Path Checklist

- [x] Audit path is under canonical Tekstide state root.
- [x] Audit path is outside supplied project roots.
- [x] Later-added/restored project roots containing audit state are rejected explicitly.
- [x] Existing symlinks cannot redirect storage outside the state root.
- [x] No project-controlled id becomes a filesystem path component.
- [x] Durable DTO is versioned independently from Rust layout.
- [x] Stable enum values use string codes.
- [x] Action kind, risk, actor, source, and adapter/profile fields are allowlisted and class-constrained.
- [x] Every retained v1 event family has explicit required/forbidden fields and phase/outcome rules.
- [x] Operation ids are bounded application-generated ids, never caller display text.
- [x] Unknown actor/source codes and invalid actor/source pairs are rejected.
- [x] Free-form `AuditEvent.summary` is not persisted.
- [x] Exact/edited commands, cwd/project/file paths, output, content, prompts, environment data, display summaries, and arbitrary metadata are not persisted.
- [x] Persisted strings are bounded and reject controls/NUL.
- [x] Cross-project linked ids are rejected by store correlation and rechecked against ProjectSession ownership at runtime integration boundaries.

## Store Checklist

- [x] Fresh store creation sets application/schema identity.
- [x] Existing store identity/version is probed read-only before write-capable configuration or migration.
- [x] Append is transactional.
- [x] Exact retry by event id is idempotent.
- [x] Conflicting reuse of event id is rejected without overwrite.
- [x] Correlated outcomes require the sole earlier same-project authorization in the same family/action kind.
- [x] A different authorization event reusing an operation id is rejected.
- [x] Cross-project, wrong-family, outcome-to-outcome, and later-record correlation is rejected.
- [x] Interleaved operations do not depend on sequence adjacency.
- [x] Authorization without outcome survives reopen without synthesized success.
- [x] Contradictory outcomes and managed-process termination before start are rejected.
- [x] Managed-process started then terminated is accepted once per phase.
- [x] Query limits are required and capped.
- [x] Sequence cursor ordering is stable.
- [x] Busy/read-only/full-disk/I/O states are bounded and content-free.
- [x] Journal, synchronous, foreign-key, and busy-timeout settings are recorded.
- [x] One-writer/cross-process limitations are documented.
- [x] Bundled SQLite version, features, license, and build/binary impact are recorded.

## Migration and Recovery Checklist

- [x] Canonical v1 fixture exists.
- [x] Current schema reopen is tested.
- [x] Supported prior-version migrations are fixture-tested. V1 is the first schema, so no prior production version exists; the sequential harness is tested with synthetic steps.
- [x] Failed migration rolls back.
- [x] Future schema is rejected in v1 without writes.
- [x] Foreign application identity is not overwritten.
- [x] Missing store initializes safely.
- [x] Corrupt store open does not rename/delete/recreate automatically.
- [x] Startup health probing is bounded independently of retained row count.
- [x] Comprehensive integrity checks require explicit diagnostics/recovery.
- [x] Explicit recovery quarantines database and known companion artifacts.
- [x] Recovery manifest records moved/absent/failure state for each expected artifact.
- [x] Incomplete quarantine prevents fresh-store creation.
- [x] Fresh store after recovery records a content-free recovery event where possible.

## Purge and Retention Checklist

- [x] Normal startup/cleanup does not delete audit records.
- [x] Project purge affects only that project's records.
- [x] Global purge removes all audit rows.
- [x] Event-id/date-range purge remains deferred.
- [x] Purge is explicit, transactional, and idempotent.
- [x] Purge receipts do not defeat the requested scope.
- [x] Project/global audit-data purge receipts are ephemeral only; no durable purge event is appended.
- [x] Purge does not delete project files, transcripts, recent-project state, configuration, or recovery artifacts.
- [x] Database and companion artifacts are included in local-data size accounting.
- [x] Journal cleanup behavior is tested.
- [x] No secure-deletion claim is made.

## Integration Checklist

- [x] Authority-increasing actions integrated in PR-013-G persist authorization before mutation; destructive producers remain unsupported.
- [x] Authorization records do not claim applied/started/completed outcomes.
- [x] Outcome records carry the matching authorization operation id.
- [x] Authority-increasing persistence failure blocks the integrated action; destructive producers remain unsupported.
- [x] Authority-reducing/protective actions apply before observational persistence.
- [x] Protective persistence failure preserves the safer state.
- [x] Observational persistence failure preserves runtime/security truth.
- [x] Managed/supervised launch authorization is persisted before process creation and followed by started/failed truth.
- [ ] Safe-close applied outcome means action issued, not process exited.
- [x] Plain/manual terminal behavior is never relabeled as durably authorized.
- [x] Initial root/symlink integration is limited to typed post-ProjectSession open/save blocks without raw paths.
- [x] Audit degradation is visible in bounded in-memory health state.
- [x] Audit failure does not recursively audit itself.
- [x] Terminal output cannot create trusted durable records.
- [x] Runtime remains TerminalSession/AgentRun process truth.
- [x] Integrated and unsupported event producers are listed separately.

## Evidence Required

- [x] Design review response and amendments/rereviews.
- [x] Implementation review responses for PR-013-B through PR-013-G.
- [x] PR-013-H closeout review response.
- [x] Workspace dependency/linkage evidence.
- [x] Test command output.
- [x] Schema and migration fixture evidence.
- [x] Read-only identity/version probe evidence.
- [x] Append/idempotency/conflict evidence.
- [x] Correlation/interleaving/incomplete-authorization evidence.
- [x] Operation cardinality and lifecycle phase-order evidence.
- [x] Actor/source valid/invalid vocabulary evidence.
- [x] Path containment evidence.
- [x] Bounded query evidence.
- [x] Corrupt/missing/future-store evidence.
- [x] Recovery artifact evidence.
- [x] Recovery manifest/partial-quarantine evidence.
- [x] Purge isolation evidence.
- [x] Security-direction and authorization/outcome integration evidence.
- [x] Command/cwd/environment/display-summary/path privacy sentinel evidence.
- [x] Known limitations and release-claim assessment.

## Amendment 1 — Schema v2 Migration Checklist (2026-07-30)

**A checked box means evidence exists, not that the result was favourable.**

- [x] `CREATE_SCHEMA_V1` reverted to byte-for-byte match with the immutable `audit-v1.sql` fixture, and a permanent test (`create_schema_v1_constant_matches_the_immutable_fixture_exactly`) enforces this going forward.
- [x] `CREATE_SCHEMA_V2` added as its own named constant (not `CREATE_SCHEMA_V1` edited forward), sharing its table-body literal text with the `1 -> 2` migration's rebuild statement via one macro, so the two cannot independently drift.
- [x] `AUDIT_SCHEMA_VERSION` bumped to `2`.
- [x] `OLDEST_SUPPORTED_SCHEMA_VERSION` pinned to the literal `1`, not derived from `AUDIT_SCHEMA_VERSION` — the trap the amendment identified in advance, confirmed by probe before the fix.
- [x] `1 -> 2` migration step added, using the table-rebuild pattern (SQLite cannot `ALTER` a `CHECK` constraint).
- [x] `sequence` values survive the rebuild unchanged — explicit column lists on both sides of `INSERT ... SELECT`, proved against real pre-existing rows inserted via raw SQL, not through any Rust API.
- [x] Every index survives the rebuild — proved by the convergence test, not asserted.
- [x] **Convergence test**: a fresh v2 install and a migrated-from-v1 database produce byte-identical `sqlite_master` entries for `audit_events` (table + every index, including SQLite's own autoindexes).
- [x] `audit-v1.sql` fixture left unedited.
- [x] `audit-v2.sql` fixture added, generated from `CREATE_SCHEMA_V2` (not hand-transcribed), and itself verified to match the constant and to open as a valid current-version store.
- [x] `canonical_v1_fixture_opens_and_remains_current` converted into a genuine migration assertion (`v1_fixture_with_existing_rows_migrates_to_v2_preserving_sequence_and_accepts_the_new_anomaly`) rather than left proving only that a fixture opens.
- [x] **Round-trip, end-to-end defect closure**: a `command_cwd_mismatch` anomaly write, which silently degraded on a pre-amendment database (response 117's probe), now persists on a migrated database.
- [x] Interrupted-migration property re-proved against the **real** `1 -> 2` step (not only against the harness's pre-existing synthetic failure-injection tests): failure reported; `user_version` unchanged; no partial rebuild table; original table and rows intact with original `sequence`; database still genuinely v1 afterwards (a v2-only value is still rejected); no partial commit.
- [x] **Ablation**: the transaction-wrapped guarantee was temporarily removed from the real harness and the interrupted-migration test (plus a pre-existing test) were confirmed to fail before the guarantee was restored.
- [x] Concurrency: the real `1 -> 2` step holds one `IMMEDIATE` transaction for its entire duration, demonstrated directly (a second connection's write blocks/fails while the migration transaction is held).
- [x] RFC-021 `acceptance-qa-checklist.md` line 74 restored to its original requirement text and marked **not met**, with this amendment recorded as the disclosure beneath it rather than the requirement rewritten to match the implementation.
- [x] **The `sqlite_sequence` `AUTOINCREMENT` high-water mark survives the rebuild, separately from row values** (response 119 Required — the reviewer's own probe found this: a purged-then-migrated store reused a retired `sequence` number, since `DROP TABLE audit_events` deletes the mark along with the table). Fixed with a capture-before/restore-after pair of statements using `MAX` so the mark can only move forward. New test: `a_purged_then_migrated_store_does_not_reuse_a_retired_sequence_number`, ablation-verified.
- [x] Statement whitelist's permissiveness for `CREATE TABLE ... AS SELECT` (needed by the high-water-mark carry) documented directly on `validate_migration_statement`.
- [x] Gates: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (485 `tekstide-core`, 0 failures), `git diff --check`.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [x] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

Response 103 accepted RFC-013 as complete with documented limitations. Its original finding requiring a `crates/tekstide/NOTICE` file was withdrawn after maintainer challenge: the `tekstide` source package redistributes no third-party code, so the obligation is satisfied by the root `NOTICE` alone (covering repository checkouts and release tarballs) plus the rusqlite/libsqlite3-sys packages' own licenses for crates.io source consumers. RFC-013 has moved to `rfcs/done/`.

**Amendment 1 (2026-07-30) is a separate, additive reopening of this closed RFC and has not yet been reviewed.** The Final Acceptance Decision above describes the base RFC-013 scope only; Amendment 1's own acceptance is pending the full review its own checklist section requires.
