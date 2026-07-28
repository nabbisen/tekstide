# RFC-013: Durable Audit Store and Local Data Policy - QA Evidence

Status: Implemented with documented limitations
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

Implemented and accepted by review response 101:

- Added `audit/purge.rs` as the explicit audit-retention boundary. The only purge scopes are one project and all audit records; no event-id, subject, or timestamp-range purge API exists.
- `AuditStore::purge_project_records` and `purge_all_records` execute one SQL `DELETE` in an immediate transaction. A receipt is returned only after commit, and repeating either operation succeeds with a zero deleted-record count.
- Project purge filters only by the validated project identifier. Tests prove target-project rows are removed while another project's rows and global rows remain.
- Global purge deletes every audit row while retaining the current schema. Tests prove no durable purge row is appended and a repeated global purge remains empty.
- Receipts are ephemeral value objects containing only scope kind, bounded deleted-record count, and journal-cleanup status. A successful return means the deletion transaction committed; project receipts contain no project, subject, event, or operation identifier.
- After commit, purge attempts `PRAGMA wal_checkpoint(TRUNCATE)`. Successful cleanup reports `Completed`; a pinned WAL reader reports `Deferred` without misrepresenting the already committed purge. Retrying after the reader closes completes cleanup.
- Purge never traverses or removes filesystem paths. Sentinel fixtures prove project files, transcript bytes, recent-project state, configuration, and recovery evidence remain byte-identical after global purge.
- `AuditStore::local_data_summary` reports retained row count and separate physical byte counts for the database, rollback journal, WAL, shared memory, and recovery files, plus total bytes and recovery-artifact count.
- Recovery accounting is a bounded, non-symlink-following scan capped at 4,096 entries and one directory level below `audit/recovery/`, matching the recovery bundle layout. Entry-limit and unavailable states are explicit rather than presented as complete totals.
- Recovery artifacts are counted as sensitive audit local data but are not deleted by project or global audit-record purge.

Observed gates on 2026-07-23:

- `cargo test -p tekstide-core audit::tests::purge -- --nocapture` passed; 6 tests passed, 0 failed.
- `cargo test -p tekstide-core audit::` passed; 63 tests passed, 0 failed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace --all-targets --all-features` passed; `tekstide-core` ran 365 tests with 0 failures and the other workspace targets ran 0 tests with 0 failures.

After response 101, the always-true `AuditPurgeReceipt::completed` field was removed. A returned `Ok` receipt itself means the deletion transaction committed; error paths return no receipt. The formatting, strict Clippy, full workspace test, and diff-hygiene gates above were rerun after this clarification.

Explicit limits:

- Purge is logical SQLite row deletion. It does not enable secure deletion, overwrite free pages, run `VACUUM`, or claim forensic erasure.
- Checkpoint/truncation can reduce live WAL retention but does not guarantee deletion from storage media or remove bytes already present in the main database's free pages.
- The database file is retained with its schema and may not shrink after purge.
- Local-data sizes are point-in-time logical file lengths, not allocated-block or storage-media measurements.
- Recovery evidence remains retained until a separately reviewed explicit recovery-artifact policy exists; ordinary audit-record purge never removes it.
- PR-013-H release-facing wording must not imply secure erasure: purged record bytes can remain in database free pages.
- PR-013-H release-facing wording must not imply tamper evidence or a durable record of purge operations; purge receipts are ephemeral by design.

## PR-013-G - Security-Event Integration

Implementation awaiting review:

- Added `audit/integration.rs` as the application coordinator boundary. It owns persistence ordering and bounded `AuditHealth`; domain collection setters and terminal output do not receive an `AuditStore`.
- Trust grants preflight a structured `authorized` record and commit it before changing `WorkspaceTrust`. The separate `applied` record reuses the operation id. Required-write failure leaves the project Restricted; an applied-observation failure leaves the already-applied Trusted state intact and marks audit health degraded.
- Trust revocation changes the project to Revoked before its observational append. Persistence failure cannot reverse the safer state and increments one bounded health failure without recursively creating another audit record.
- Managed/supervised AgentRun launch is split into side-effect-free/runtime-safe preflight and execution. The coordinator commits launch authorization after preflight and before process creation, then appends `started` with the actual project-owned AgentRun and TerminalSession ids or `failed` after launch failure.
- A returned `AuditedAgentLaunch` retains the application-generated operation id for later process outcome correlation. Observed exit/signal/kill outcomes update TerminalSession/AgentRun truth first and append `terminated` afterward; observational failure leaves the runtime state authoritative and marks health degraded. Orphaned/observer-failed states remain domain truth without being mislabeled as durable termination.
- AgentRun and terminal ownership are rechecked against the target ProjectSession before linked lifecycle outcomes are persisted. A cross-project handle is rejected before domain mutation or append.
- Plain launch plans are rejected by the managed/supervised coordinator before append or process creation. They are not relabeled as durably authorized.
- ProjectSession text open/save integration appends only a project id plus `root_escape` or `symlink_escape` after a typed `ProjectFileAccessPolicy::resolve_existing` block. Paths, filenames, and content are absent from the record type and privacy sentinels prove they do not appear in queried rows.
- Successful open/save operations produce no audit record. Other access, format, size, external-change, and write failures are not misclassified as root/symlink security events.
- Durable records are built directly from typed ProjectSession, launch-plan, runtime, and file-access context. No conversion reads or persists `AuditEvent.summary`, prompt summaries, terminal titles/output, executable/cwd paths, environment policy, transcript bytes, or generated-change content.

Runtime-integrated producers in this slice:

- trust grant authorization and applied outcome;
- trust revocation observation;
- managed/supervised AgentRun authorization, started/failed launch result, and observed exit/signal/kill termination;
- post-ProjectSession text open/save root or symlink access blocks.

Explicitly unsupported by this integration slice:

- project-added durability;
- Plain/manual terminal lifecycle observations;
- command approval request/decision outcomes;
- paste and restricted-feature blocks;
- safe-close/destructive decisions;
- sensitive configuration changes;
- transcript-purge records.

The audit-store recovery producer remains owned by PR-013-E. Project/global audit purge remains intentionally ephemeral under PR-013-F.

Observed gates on 2026-07-23:

- `cargo test -p tekstide-core audit::tests::integration -- --nocapture` passed; 10 tests passed, 0 failed.
- `cargo test -p tekstide-core audit::` passed; 73 tests passed, 0 failed.
- `cargo test --workspace --all-targets --all-features` passed; `tekstide-core` ran 375 tests with 0 failures and the other workspace targets ran 0 tests with 0 failures.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `git diff --check` passed.

Explicit limits:

- The integration coordinator is a core application service, not a rendered audit viewer or local-data UI.
- Command approval and safe-close matrix behavior remain unchecked because those producers are not wired; their record vocabulary alone is not implementation evidence.
- `AuditHealth` is bounded in-memory status and does not claim durable self-diagnostics, automatic recovery, or that a later successful append repairs an earlier audit gap.

Review response 102 accepted PR-013-G with one required visibility follow-up:

- `ProjectSession::apply_agent_terminal_outcome` is now crate-scoped, matching the trust and launch mutation boundaries. External callers handling managed/supervised lifecycle outcomes must use `AuditCoordinator::apply_managed_agent_terminal_outcome`; the direct domain mutation can no longer bypass its durable termination observation.
- Narrowing `apply_agent_terminal_outcome` to `pub(crate)` also removed the only public path for applying a Plain AgentRun terminal outcome. This is intentional and consistent with Plain lifecycle being an unimplemented durable producer. A future Plain path must be added deliberately — as a Plain-scoped wrapper that rejects managed/supervised runs — rather than by re-widening the crate-scoped method.
- The unexpected post-spawn invariant-failure case remains a known limitation: a live attached process can coexist with an incomplete authorization operation if an internal launched-id or ownership invariant fails after spawn.
- Managed/supervised profile identifiers must satisfy `AuditReference` syntax before launch. Profile-definition-time validation and a more specific error remain future usability work.
- Phase conflicts and store availability failures both conservatively degrade the in-memory health summary; health remains degraded for the process lifetime and does not claim current-store recovery state.

## PR-013-H - Closeout Evidence

Closeout awaiting review:

- Added root and `tekstide-core` package `NOTICE` files with the resolved `rusqlite 0.39.0` and `libsqlite3-sys 0.37.0` MIT notice plus the bundled SQLite 3.51.3 public-domain notice. The crate-local copy is included in the package that enables bundled SQLite; the root copy covers repository and binary distributions. This satisfies response 096's first bundled-native-code notice requirement without changing dependency features.
- Responses 092 through 095 accepted the design and amendments. Responses 096, 097, 100, 101, and 102 accepted PR-013-B/C, D, E, F, and G respectively after their required follow-ups.
- The current reviewed implementation evidence covers record/path validation, SQLite schema/append/query, migration fixtures, diagnostics and restart-safe recovery, purge/local-data accounting, and the integrated trust/managed-launch/root-block producers.
- `AuditRecovery` remains explicit and caller-driven. Application-owned SQLite handles must be closed before recovery; the type system does not discover or close them.
- The generated `NOTICE` does not change Tekstide's Apache-2.0 project license or claim copyright over public-domain SQLite.

Claimable RFC-013 properties:

- local structured audit records persist in a versioned SQLite store under the validated application state root;
- transactional append, exact retry, bounded query, operation correlation, phase cardinality, and explicit migration/recovery behavior have reviewed tests;
- managed/supervised launch authorization is audit-required before process creation, while trust revocation, root/symlink blocks, and observed runtime outcomes preserve safer/runtime truth on observational failure;
- project/global audit-row purge is explicit and idempotent, with bounded local-data accounting and no durable purge receipt;
- persisted record fields exclude commands, paths, output, prompts, environment data, transcript bytes, file/diff content, and free-form display summaries.

Release-facing non-claims and limitations:

- no rendered audit viewer or local-data settings UI;
- no encryption, tamper evidence, signed log, cloud sync, export, cross-process writer guarantee, automatic retention, or complete crash/power-loss guarantee;
- purge is logical row deletion, not secure erasure: deleted bytes can remain in SQLite free pages or storage media, and the database file can retain its prior size;
- purge operations intentionally leave no durable audit record, so local audit history is erasable without tamper-evident proof;
- recovery quarantines evidence and creates a fresh store but does not salvage records or automatically close live handles;
- audit health is bounded, in-memory, sticky after degradation, and does not prove current store availability or repair earlier gaps;
- unexpected post-spawn invariant failure can leave a live attached process with an incomplete authorization operation;
- managed/supervised profile ids must already satisfy the durable reference alphabet; profile-definition-time validation and a specific user-facing error are not implemented;
- project-added, Plain/manual terminal lifecycle, command approval, paste, restricted-feature, safe-close/destructive, sensitive-configuration, and transcript-purge producers are unimplemented, even though the v1 record vocabulary represents them;
- safe-close applied semantics remain design vocabulary only and are not an integrated runtime claim;
- current native build evidence is limited to `x86_64-unknown-linux-gnu`; Windows and macOS support are not established by RFC-013.

Observed closeout gates on 2026-07-23:

- `cargo package -p tekstide-core --list --allow-dirty` passed and listed the crate-local `NOTICE` in the package archive.
- `cargo tree -p tekstide-core -e features -i rusqlite` showed `rusqlite 0.39.0` with `bundled` and its implied `modern_sqlite` feature, rooted only through `tekstide-core`.
- `cargo test --workspace --all-targets --all-features` passed; `tekstide-core` ran 375 tests with 0 failures and the other workspace targets ran 0 tests with 0 failures.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo fmt --all -- --check` passed.
- `git diff --check` passed.

Response 103 accepted closeout as complete with documented limitations. That response originally required adding `crates/tekstide/NOTICE`; a maintainer challenge showed the finding was wrong, and it was withdrawn. Third-party notices for bundled SQLite reach binary recipients through the release tarball's root `NOTICE`. crates.io source consumers receive rusqlite and libsqlite3-sys licenses from those crates' own packages; Tekstide's published packages declare dependencies rather than redistributing that code. No `crates/tekstide/NOTICE` was created. RFC-013 is implemented with documented limitations and has moved to `rfcs/done/`.

Response 103 separately left `crates/tekstide-core/NOTICE` as a discretionary "keep or delete, not load-bearing either way" call, distinct from the withdrawn `crates/tekstide` finding. Maintainer decision on 2026-07-28: consolidate to a single root `NOTICE` as the source of truth. `crates/tekstide-core/NOTICE` is removed. The same reasoning that applies to `tekstide` applies here — the `tekstide-core` package does not redistribute rusqlite, libsqlite3-sys, or SQLite source itself, so its crates.io source consumers receive those licenses from the dependency packages, and the repo-root `NOTICE` carried in the release tarball covers binary distribution.

Observed after the 2026-07-28 consolidation:

- `cargo package -p tekstide-core --list --allow-dirty` no longer lists a `NOTICE` file.
- `cargo fmt --all -- --check` passed.
- `git diff --check` passed.

## Known Limitations

- The PR-013-H release-facing non-claims above are the accepted RFC-013 limitation set.
- Unsupported producers must be implemented and reviewed before their represented schema vocabulary becomes a runtime durability claim.
