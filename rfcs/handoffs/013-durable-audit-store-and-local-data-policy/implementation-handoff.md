---
title: "RFC-013: Durable Audit Store and Local Data Policy - Implementation Handoff"
rfc: "RFC-013"
rfc_file: "../../done/013-durable-audit-store-and-local-data-policy.md"
status: "Implemented with documented limitations"
target_milestone: "M7"
created: "2026-07-22"
---

# RFC-013: Durable Audit Store and Local Data Policy - Implementation Handoff

## Scope

Design review response 095 accepted the backend, privacy, failure, migration, purge, recovery, and schema contract for implementation. Begin with PR-013-B and preserve the accepted boundaries below.

The implementation must preserve runtime truth from RFC-008/RFC-010, terminal trust boundaries from RFC-009, transcript privacy from RFC-011, and generated-change non-claims from RFC-012. Audit records describe app-generated decisions and observations; they are not process supervision or full state recovery.

## Existing Anchors

- `crates/tekstide-core/src/domain/audit.rs` provides in-memory `AuditEvent` and `AuditEventClass` vocabulary.
- `crates/tekstide-core/src/project/session.rs` owns project-scoped in-memory audit collections and trust transitions.
- `crates/tekstide-core/src/project/recent/store.rs` demonstrates atomic local-state handling but is not a durable-audit backend.
- `crates/tekstide-core/src/transcript/path.rs` provides a reviewed state-root/project-root containment pattern.
- `crates/tekstide-core/src/domain/approval.rs` contains command and cwd data that must not cross into durable audit records.
- `crates/tekstide-core/src/security.rs` and `crates/tekstide-core/src/close.rs` provide policy and close-decision vocabulary.
- RFC-002 owns core entity ownership; RFC-004 owns baseline trust/audit policy.

## Module Boundary

Keep domain types and persistence concerns separate.

Preferred starting shape:

```text
crates/tekstide-core/src/
  domain/audit.rs
  audit.rs
  audit/
    path.rs
    record.rs
    schema.rs
    store.rs
    purge.rs
    recovery.rs
    tests/
```

Do not place SQLite connections, SQL, filesystem recovery, or migration logic in `domain/audit.rs` or `project/session.rs`. Split further only where implementation size or ownership makes the boundary useful.

## Hard Requirements

- Durable storage stays under the canonical Tekstide state root and outside every supplied project root.
- The storage DTO is explicit and versioned; Rust layout and serde derives are not the persistence contract.
- Never persist `AuditEvent.summary` or arbitrary caller-provided metadata.
- Never persist exact commands, edited commands, cwd/project/file paths, prompts, output, transcript bytes, diff hunks, environment names/values, executable paths, shell history, or external error strings.
- Use stable string codes for classes, outcomes, action kinds, risk, actors, action sources, and reasons; do not persist Rust discriminants or arbitrary `requested_action_kind` values.
- Enforce per-class field invariants so unrelated ids and codes cannot form a valid record.
- Use a bounded application-generated operation id to correlate every outcome that completes a pre-action authorization.
- Require exactly one authorization per operation id. Correlated outcomes must match that earlier same-project authorization by event family and action kind; never infer correlation from sequence adjacency.
- Treat a different authorization event id reusing an operation id as an integrity conflict; exact retry reuses the same event id and canonical record.
- Enforce family phase cardinality: ordinary authorized operations allow one `applied`/`failed` terminal outcome; managed-process lifecycle allows one `started`/`failed` initial outcome and, only after `started`, one `terminated` observation.
- Permit authorization without outcome as truthful incomplete/crash evidence; reopen must not synthesize success.
- Append is transactional and unique by event id.
- Exact retries are idempotent; conflicting reuse of an event id is rejected.
- Every query is bounded and uses a stable sequence cursor.
- Authority-increasing/destructive authorization failures fail closed before the action.
- Authority-reducing/protective actions apply first; persistence failure preserves the safer state and reports degraded audit health.
- Pre-action records say authorized/decision-recorded, never applied/started/completed; append a separate truthful outcome where required.
- Missing store creation is safe; corrupt or future-version stores are never silently overwritten.
- Recovery is explicit and quarantines the database plus known companion artifacts before fresh creation.
- Purge is explicit, scoped, transactional, and idempotent.
- No secure deletion, encryption, tamper-evidence, complete crash-proofing, GUI viewer, or cross-platform claim without separate evidence.

## Record Conversion Guidance

Introduce a storage-facing record type with validated bounded fields. Conversion should accept domain references and enum values, not free-form display strings.

The record validator should check:

- identifier shape and maximum length;
- timestamp shape;
- stable known class/outcome/reason codes;
- reviewed action-kind, risk, actor-kind, and action-source codes;
- exact actor/source vocabulary and allowed pairs;
- project ownership consistency across linked entities;
- subject kind/reference compatibility;
- per-class required/forbidden field combinations;
- operation-id shape and authorization/outcome phase compatibility;
- no NUL/control characters in persisted string fields;
- conservative maximum lengths before database access.

Display copy should be derived from codes when queried. Do not round-trip a rendered sentence through storage.

Approval records must preserve the allowlisted action category, risk level, actor/action source, and bounded adapter/profile reference when known. They must not persist exact/edited commands, cwd, environment summaries, or the arbitrary display/action strings from `ApprovalRequest`. RFC-013 v1 is intentionally not a complete command audit viewer.

Implement the RFC's per-class matrix exhaustively. V1 includes project-added, trust, command approval, managed/supervised launch and termination, plain terminal observation, paste/restricted/root blocks, safe-close/destructive decisions, sensitive configuration changes, transcript purge, and audit-store recovery. Project/global audit purge has an ephemeral receipt only, and generated-change metadata purge remains deferred.

The only valid actor codes are `user`, `app_policy`, and `runtime`. The only valid source codes are `trusted_ui`, `app_command`, `policy_engine`, `adapter`, `runtime_observer`, and `explicit_cleanup`. Valid pairs are exactly those listed in the RFC. Unknown codes and invalid pairs fail validation. Terminal/project/external content and display strings cannot supply these fields.

PR-013-B validates record shape, exact actor/source codes, allowed pairs, and phase vocabulary without requiring an open store. PR-013-C enforces relational correlation and phase cardinality: an outcome's operation id must resolve to the sole earlier authorization in the same project and family/action kind, and cannot resolve to another outcome or later record.

## SQLite Guidance

- Declare `rusqlite` in root workspace dependencies and consume it through `.workspace = true`.
- Enable only the reviewed `bundled` feature; do not enable SQLCipher, extension loading, or unrelated optional native features.
- Record exact `rusqlite`/SQLite versions, features, license review, build/binary impact, and supported-target evidence.
- Use an application id and explicit schema version.
- Run schema creation/migration in transactions.
- Use prepared statements and typed parameters only.
- Select and document journal mode, synchronous setting, foreign-key behavior, and busy timeout.
- Keep one application-owned connection/writer boundary initially.
- Treat busy, locked, I/O, full-disk, read-only, corrupt, and unsupported-schema states as bounded typed errors.
- Never include SQL parameter values or raw SQLite messages in user-facing summaries without sanitization.

## Security-Direction Integration Guidance

Create a coordinator/service rather than hiding persistence inside collection setters.

Authority-increasing/destructive flow:

1. Validate the requested domain transition.
2. Generate an operation id and build a validated authorization/decision-recorded durable record.
3. Commit the authorization record.
4. Apply the already-validated transition.
5. Append a separate applied/started/failed observation carrying the same operation id where outcome evidence is required.
6. If step 4 unexpectedly fails, retain the authorization and expose/append bounded failure truth; never reinterpret authorization as completion.

Authority-reducing/protective flow:

1. Validate and apply the safer state first.
2. Attempt durable append.
3. On failure, update bounded in-memory audit-health state.
4. Never reverse the safer state to make audit persistence succeed.

Observation-after-fact flow follows the same append/degradation behavior after preserving runtime or security truth.

The required matrix is:

| Persist authorization before action | Apply safer/observed state before append |
| --- | --- |
| trust grant | trust revocation |
| approve/edit-and-approve | command rejection |
| managed/supervised AgentRun launch | close/destructive cancellation |
| destructive/safe-close execution | restrictive policy change |
| less-restrictive policy change | paste/restricted/root/symlink block |
| | process exit/crash/termination or launch failure |

Audit-store failure must never recursively audit itself.

A safe-close `applied` outcome records that Tekstide issued the selected terminate/abandon action. Process exit remains a separate runtime observation; audit state must not infer termination from the close outcome.

## Migration and Fixture Guidance

- Commit a canonical v1 database fixture and expected query projection, or a deterministic fixture builder accepted by review.
- Open an existing store read-only for identity/version probing before any write-capable pragma or migration.
- Test fresh creation, current schema reopen, each supported prior-version migration, failed migration rollback, foreign application id, and future schema version.
- Keep migration functions sequential and independently testable.
- Do not edit an already-released migration in place.
- Destructive migration requires an RFC amendment and explicit backup/recovery policy.

## Recovery Guidance

Ordinary open uses a bounded identity/schema/read probe whose work is independent of retained row count. It must not quarantine automatically. Comprehensive `quick_check`/`integrity_check` work belongs to explicit diagnostics/recovery.

Explicit recovery must:

- close active connections;
- identify the database and known `-journal`, `-wal`, and `-shm` companions without broad globs;
- move present artifacts into one unique state-root recovery directory;
- write a content-free manifest result for each expected database, `-journal`, `-wal`, and `-shm` artifact, including absent artifacts;
- abort fresh creation if quarantine is incomplete;
- initialize the current schema only after quarantine succeeds;
- return a content-free recovery receipt;
- record a recovery event in the fresh store when possible.

Do not parse or salvage corrupt row content in RFC-013.

## Path Lifecycle Guidance

Audit path safety must also be checked when a project is added or restored after the store exists. Reject a canonical project root that contains the application state root or audit database with a typed path-conflict result. Do not silently admit the project with audit data beneath project control, and do not silently disable durable audit while retaining an audited label.

## Purge Guidance

- Project purge deletes only rows owned by that project.
- Global purge deletes all audit rows without touching schema unless the reviewed API explicitly recreates the store.
- Event-id and timestamp-range purge remain deferred.
- Both project and global audit-data purge return ephemeral receipts only and append no durable purge event in v1.
- Purge must not delete transcripts, project files, recent-project state, configuration, or recovery artifacts.
- Do not retain project/subject identifiers solely as purge tombstones.
- Never retain a durable audit-data purge receipt outside the deleted scope; a later RFC requires a new matrix row and privacy design to add one.
- Account for database and companion files in local-data size summaries.
- Checkpoint/cleanup according to the selected journal mode, but retain the no-secure-deletion limitation.

## Review Request Requirements

Every implementation review request for RFC-013 must reference:

- `rfcs/done/013-durable-audit-store-and-local-data-policy.md` after lifecycle closeout;
- this handoff pack;
- RFC-002 and RFC-004;
- the specific upstream integration RFCs touched by the slice, such as RFC-008, RFC-009, RFC-010, RFC-011, or RFC-012;
- prior RFC-013 design/re-review responses and relevant implementation responses.

Implementation review request filenames must include `implementation`.
