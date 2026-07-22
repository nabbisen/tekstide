# RFC-013: Durable Audit Store and Local Audit Data Policy

Status: Proposed
Target milestone: M7
Date: 2026-07-22

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-roadmap-milestones-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md)

Depends on:

- [RFC-002](../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md)
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md)
- [RFC-008](../done/008-terminalsession-process-lifecycle.md)
- [RFC-009](../done/009-terminal-security-boundary.md)
- [RFC-010](../done/010-agentrun-launch-model-and-ai-cli-profiles.md)
- [RFC-011](../done/011-transcript-retention-and-local-data-policy.md)
- [RFC-012](../done/012-generated-change-review-foundations.md)

Blocks:

- M7 durable-audit completion;
- persistent security-decision claims for trust, launch, approval, paste, close, and blocked file access;
- a later rendered audit viewer and local-data management surface;
- release claims that security-relevant decisions survive process restart.

## Summary

RFC-013 defines Tekstide's first local durable audit store. It converts the existing in-memory `AuditEvent` vocabulary into a versioned, append-oriented SQLite record store under the Tekstide application state root.

The durable boundary stores allowlisted structured metadata only. It does not persist the current free-form `AuditEvent.summary`, exact commands, terminal output, transcript bytes, prompts, file contents, environment values, or shell history. Security-sensitive actions that require an audit record must fail closed before the action when the store is unavailable. Events that describe an already-observed fact may report degraded audit persistence without rewriting runtime truth.

RFC-013 also defines migration, bounded query, explicit purge, missing/corrupt-store recovery, and privacy behavior. It does not provide the final GUI audit viewer.

## Motivation

Tekstide already models audit events in memory, but those events disappear when the process exits. That is insufficient for trust changes, managed launch intent, approval decisions, safe-close choices, blocked security actions, and destructive confirmations.

Durability must not turn auditability into a second privacy problem. Existing summaries are convenient display strings supplied by callers and are not a safe persistence schema. A durable store needs explicit fields, bounded values, stable encodings, and a clear rule for what happens when persistence fails.

## Goals

- Persist security-relevant audit records under Tekstide-managed local state.
- Preserve project ownership and optional terminal, AgentRun, and approval references.
- Use a versioned schema with reviewed forward-only migrations.
- Make append operations transactional and retry-safe by event id.
- Distinguish audit-required actions from observations that cannot be undone.
- Keep persisted fields structured, allowlisted, bounded, and content-free.
- Provide bounded project/global queries suitable for a later viewer.
- Provide explicit project/global purge operations.
- Detect missing, unsupported, and corrupt store states without silently discarding evidence.
- Provide explicit recovery that quarantines corrupt database artifacts before creating a fresh store.
- Supply fixture, migration, corruption, privacy, and integration evidence for M7.

## Non-Goals

- Rendered audit viewer or local-data settings UI.
- Tamper-evident, cryptographically signed, or append-only-at-the-filesystem-level logs.
- Database encryption or SQLCipher.
- Secure deletion or forensic erasure guarantees.
- Cloud sync, remote export, telemetry, or multi-device audit aggregation.
- Storing exact commands, cwd paths, project root paths, prompts, file contents, diff hunks, transcript bytes, terminal output, environment names/values, or shell history.
- General command interception for Plain or Supervised terminals.
- Cross-process writers or a background audit daemon.
- Automatic age-based deletion in the initial M7 implementation.
- Reconstructing full ProjectSession, AgentRun, terminal, approval, transcript, or ChangeSet state from the audit store.
- Treating audit persistence as process lifecycle truth.

## Architecture Boundary

The existing domain vocabulary remains in `crates/tekstide-core/src/domain/audit.rs`. Durable persistence belongs in a sibling `audit/` subsystem under `tekstide-core`, with small modules for record conversion, path policy, schema/migrations, store operations, purge/recovery, and tests.

Expected shape:

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

The exact split may follow implementation pressure, but storage SQL and filesystem behavior must not accumulate in `domain/audit.rs` or `project/session.rs`.

RFC-013 does not require a new workspace crate. The store is currently a single application-owned backend with no independent public lifecycle. A later split into `tekstide-audit` is justified only if the backend gains multiple consumers, implementations, or release boundaries.

## Storage Backend and Path

The initial backend is SQLite accessed through `rusqlite` with its `bundled` feature. The dependency must be declared in workspace dependencies. PR-013-C must record the exact crate/version/features, bundled SQLite version, license review, binary/build impact, and supported-target build evidence. SQLCipher, extension loading, and unrelated optional native features remain disabled.

SQLite is selected because transactions, uniqueness constraints, schema versioning, bounded queries, and corruption checks are core audit requirements. Implementing equivalent behavior over ad hoc JSON or JSONL would create a custom database and recovery protocol.

The conventional path is:

```text
<tekstide-state-root>/
  audit/
    audit.sqlite3
    recovery/
```

Path resolution must prove:

- the state root is absolute and resolves to an existing Tekstide-managed directory;
- the audit directory and database remain beneath the canonical state root;
- the database is not inside a ProjectSession root;
- existing parent components do not redirect through symlinks outside the state root;
- no project-controlled identifier becomes a path component;
- failures return bounded reason codes without project data.

Path exclusion is a lifecycle invariant, not only an audit-store open check. If a later-added or restored canonical ProjectSession root contains the existing application state root or audit database, project admission must reject it with a typed `AuditPathConflict`-equivalent result. Tekstide must not silently admit the project and place durable audit data under project-controlled content, nor silently relabel the project as durably audited.

The database, journal, WAL, shared-memory, and recovery artifacts are all sensitive local application data. The implementation must account for the complete SQLite file set when purging, recovering, or reporting size.

## Durable Record Model

The persistence DTO is separate from `AuditEvent`. A representative v1 record is:

```text
DurableAuditRecordV1
├─ sequence                 store-assigned monotonic integer
├─ event_id                 stable unique AuditEventId
├─ schema_version           record format version
├─ project_id optional
├─ class                    stable string code
├─ outcome                  stable string code
├─ operation_id optional    app-generated correlation id for one authorized attempt
├─ terminal_id optional
├─ agent_run_id optional
├─ approval_id optional
├─ subject_kind optional    allowlisted string code
├─ subject_ref optional     bounded opaque id, never content/path/command
├─ action_kind optional     reviewed allowlisted action code
├─ risk_level optional      stable risk category
├─ actor_kind               reviewed v1 actor code
├─ action_source            reviewed v1 source code
├─ adapter_profile_ref optional  bounded opaque application id
├─ reason_code optional     allowlisted bounded code
└─ created_at               UTC domain timestamp
```

The v1 schema must not contain a generic JSON metadata object or arbitrary persisted summary. New fields require a schema decision and migration rather than caller-defined key/value growth.

`AuditEvent.summary` remains an in-memory/display convenience until a later design replaces it. Conversion into a durable record ignores that field and derives display copy from stable codes at query/render time.

Stable class/outcome vocabulary must separate a pre-action authorization or recorded decision from an applied or observed outcome. A pre-action record may use `authorized` or `decision_recorded`; it must never say `started`, `terminated`, or `applied`. Where completion matters, the coordinator appends a separate applied/started/failed observation after the transition or runtime result.

Every outcome that completes a pre-action authorization carries the same application-generated `operation_id` as its authorization record. The operation id is a bounded stable identifier, never caller display text. The store must require exactly one earlier authorization per operation id. That authorization and every outcome must share the same project, event family, and action kind. An outcome cannot correlate to another outcome, a later record, another project, or an unrelated family. A different authorization event id reusing an operation id is an integrity conflict; an exact retry reuses the same event id and canonical record. An authorization with no outcome remains truthful incomplete/crash evidence and must not be synthesized as success during reopen.

Phase cardinality is event-family specific:

- trust grant, command approve/edit-and-approve, destructive/safe-close execution, and less-restrictive configuration operations permit exactly one `authorized` record and at most one terminal `applied` or `failed` outcome;
- a managed-process lifecycle operation permits exactly one `authorized` record, then at most one initial `started` or `failed` result;
- `failed` is terminal for that lifecycle operation;
- `started` may be followed by at most one `terminated` observation;
- `terminated` before `started`, `started` plus launch `failed`, repeated phases with different event ids, and contradictory terminal outcomes are integrity conflicts;
- exact retries of any phase use the same event id and canonical record and remain idempotent.

Stable event-family and outcome vocabulary must cover the implemented or planned M7 sources:

- project added;
- trust change authorization and applied/revoked/failed outcomes;
- managed/supervised process lifecycle authorization and started/failed/terminated observations;
- plain terminal started/failed/terminated observations without managed authorization;
- command approval request, authorization, and applied/rejected/failed outcomes where adapter capability supports it;
- terminal paste blocked;
- restricted-mode feature blocked;
- root/symlink access blocked;
- safe-close or destructive confirmation decision;
- sensitive configuration policy changed;
- transcript metadata/bytes purged;
- audit-store recovery outcome.

Project/global audit-data purge produces an ephemeral receipt only in v1. It does not append a durable purge event into the scope the user requested to remove. Generated-change review metadata purge has no stable producer in RFC-012 and is deferred rather than represented speculatively.

Not every producer must be wired in the first storage PR. The acceptance checklist must distinguish represented vocabulary from runtime-integrated sources.

### Per-Class Field Invariants

Optional fields are class-constrained. The v1 validator and database constraints must reject unrelated combinations rather than accepting superficially valid records. Fields not listed as required or allowed for a row are forbidden.

The exhaustive v1 actor vocabulary is:

- `user` - a human action received through a trusted Tekstide surface;
- `app_policy` - a Tekstide policy/coordinator decision;
- `runtime` - a fact observed from a Tekstide-owned runtime boundary.

The exhaustive v1 action-source vocabulary is:

- `trusted_ui` - a future trusted app/native surface;
- `app_command` - Tekstide's own command dispatch, not a shell command string;
- `policy_engine` - an internal reviewed policy decision;
- `adapter` - a reviewed Tekstide adapter event, not arbitrary adapter text;
- `runtime_observer` - a Tekstide runtime observation;
- `explicit_cleanup` - an explicit local-data cleanup operation.

Allowed actor/source pairs are `user` with `trusted_ui` or `app_command`, `app_policy` with `policy_engine`, `adapter`, or `explicit_cleanup`, and `runtime` with `runtime_observer`. All other pairs and unknown codes are invalid. Terminal output, project content, external tool text, and caller display strings can never become actor or action-source codes.

| Event family | Required/allowed ownership, entity, and correlation fields | Required structured codes | Allowed actor/source | Valid phase/outcome direction |
| --- | --- | --- | --- | --- |
| Project added | project required; no entity or operation fields | action kind `project_add` | `user`/`trusted_ui`, `user`/`app_command`, or `app_policy`/`policy_engine` | `applied` observation only |
| Trust change | project required; operation required for grant authorization and its outcome; no entity fields | `trust_grant` or `trust_revoke` | `user`/`trusted_ui` or `app_policy`/`policy_engine` | grant `authorized` then correlated `applied`/`failed`; revocation `applied` observation only |
| Command approval | project and approval required; AgentRun allowed; operation required for approval authorization and its outcome | reviewed action kind and risk required; adapter/profile ref allowed | request: `app_policy`/`adapter`; decision: `user`/`trusted_ui` | `requested`; approve/edit `authorized` then correlated `applied`/`failed`; rejection `applied` observation only |
| Managed/supervised process lifecycle | project, AgentRun, and operation required; terminal allowed on authorization and required for `started`/`terminated` | launch action kind and adapter/profile ref required; bounded reason allowed for failure/termination | authorization: `user`/`trusted_ui`, `user`/`app_command`, or `app_policy`/`policy_engine`; outcomes: `runtime`/`runtime_observer` | exactly one `authorized`; then one `started` or `failed`; after `started`, at most one `terminated`; termination remains runtime truth |
| Plain/manual terminal observation | project and terminal required; AgentRun, approval, operation, risk, adapter/profile, and subject fields forbidden | terminal action kind | `runtime`/`runtime_observer` | `started`, `failed`, or `terminated` observation only; never managed authorization |
| Paste blocked | project required; terminal allowed; no operation or subject fields | paste action kind and bounded reason required | `app_policy`/`policy_engine` | `blocked` observation only |
| Restricted-mode feature blocked | project required; no entity, operation, or subject fields | restricted-feature action kind and bounded reason required | `app_policy`/`policy_engine` | `blocked` observation only |
| Root/symlink access blocked | project required; no entity, operation, or subject fields | root-access action kind and `root_escape`/`symlink_escape` reason required | `app_policy`/`policy_engine` | `blocked` observation only; raw paths forbidden |
| Safe-close/destructive decision | project required; operation required for execution authorization and outcome; bounded subject kind/ref allowed only for an app-owned resource id | reviewed close/destructive action kind; bounded reason allowed | `user`/`trusted_ui` or `user`/`app_command` | execution `authorized` then correlated `applied`/`failed`; cancellation `cancelled` observation only |
| Sensitive configuration change | project optional according to configuration scope; operation required for less-restrictive authorization and outcome; entity/subject fields forbidden | reviewed policy action kind and bounded reason required | `user`/`trusted_ui` or `app_policy`/`policy_engine` | less restrictive `authorized` then correlated `applied`/`failed`; more restrictive `applied` observation only |
| Transcript purge | project and subject kind/ref for TranscriptId required; AgentRun allowed; no operation field | action kind `transcript_purge` | `user`/`trusted_ui`, `user`/`app_command`, or `app_policy`/`explicit_cleanup` | `completed`/`failed` observation only |
| Audit-store recovery | project and domain entity ids forbidden; subject kind/ref for app-generated recovery id required; no operation field | action kind `audit_store_recovery` and bounded result | `user`/`trusted_ui` or `user`/`app_command` | durable `completed` observation only after quarantine and fresh-store creation; failed/partial recovery remains an ephemeral receipt because no writable trusted store exists |

Approval action kinds must map from caller input into reviewed codes. The current arbitrary `ApprovalRequest.requested_action_kind` string must not be persisted directly.

RFC-013 v1 deliberately narrows the approval-audit fields in the external design. It does not persist the historical exact-command, edited-command, or cwd fields and therefore must not claim a complete command audit viewer. After restart, v1 provides decision, action category, risk, actor/source, adapter/profile reference, and stable entity ids without reconstructing the command text.

## Privacy Boundary

Persisted audit records may contain:

- stable Tekstide ids;
- an application-generated operation id used only to correlate one authorization attempt and its outcomes;
- event class, outcome, subject kind, and bounded reason codes;
- project ownership id;
- timestamps;
- risk or compatibility categories represented as stable codes;
- allowlisted action kind, actor kind, and trusted action-source codes;
- an opaque profile/adapter/reference id only when it is already a bounded application identifier.

Persisted records must not contain:

- free-form `AuditEvent.summary` text;
- exact or edited commands;
- cwd or project-root paths;
- file paths or filenames;
- prompts, transcript snippets, terminal bytes, file contents, or diff hunks;
- environment variable names or values;
- executable paths, shell history, Git metadata, or error strings from external tools;
- arbitrary caller-supplied metadata.

Error reports and store-health summaries may expose store state, operation, schema version, SQLite result category, and affected record count. They must not echo SQL values, raw database pages, or persisted record content.

This is data minimization, not a complete redaction guarantee. Stable ids and event timing can still be sensitive local metadata.

## Append Semantics

Each append uses a transaction and a uniqueness constraint on `event_id`.

- A new event id and valid record append exactly once.
- Retrying the same event id with the same canonical record is an idempotent success reported as already present.
- Reusing an event id with different fields is an integrity conflict and must not overwrite the original.
- Invalid enum codes, timestamps, identifiers, oversized values, or cross-project links are rejected before SQL write.
- Correlated outcomes require the sole earlier authorization with the same operation id, project, family, and action kind.
- Operation phase cardinality rejects duplicate authorizations, contradictory outcomes, and managed-process termination before start.
- A successful append is reported only after transaction commit.
- No code may mutate or delete an existing audit record through the append API.

The initial store has one in-process writer boundary. Cross-process concurrency is unsupported and must produce a bounded busy/unavailable result rather than silently dropping an event.

## Security-Direction Audit Semantics

Audit failure cannot have one universal policy. Ordering follows whether the action increases authority/risk, reduces authority/risk, or records an already-observed fact.

| Decision or event | Security direction | Required ordering | Persistence failure behavior |
| --- | --- | --- | --- |
| Grant workspace trust | Authority increasing | Persist authorization, apply grant, append applied/failed observation | Block grant if authorization cannot commit |
| Revoke workspace trust | Authority reducing | Apply revocation, then append observation | Preserve Revoked state; mark degraded audit |
| Approve/edit-and-approve command | Authority increasing | Persist authorization, apply decision, append applied/failed observation | Block approval if authorization cannot commit |
| Reject command | Authority reducing | Apply rejection, then append observation | Preserve Rejected state; mark degraded audit |
| Managed/supervised AgentRun launch | Authority/risk increasing | Persist launch authorization before process creation, then append started/failed observation | Do not create process if authorization cannot commit |
| Plain/manual terminal launch | Unsupported/degraded audit scope | Preserve honest Plain behavior; observation only where integrated | Never relabel as managed/supervised or durably authorized |
| Execute destructive or safe-close confirmation | Risk increasing | Persist execution authorization, execute, append applied/failed observation | Block execution if authorization cannot commit |
| Cancel close/destructive action | Protective | Apply cancellation, then append observation | Preserve cancellation; mark degraded audit |
| Make configuration less restrictive | Authority increasing | Persist authorization, apply, append applied/failed observation | Block change if authorization cannot commit |
| Make configuration more restrictive | Authority reducing | Apply restriction, then append observation | Preserve restrictive state; mark degraded audit |
| Paste/restricted/root/symlink block | Protective observation | Apply block first, then append observation | Preserve block; mark degraded audit |
| Process exit/crash/termination or launch failure | Observed fact | Preserve runtime fact, then append observation | Preserve runtime truth; mark degraded audit |

Authority-increasing and destructive actions persist an authorization or decision record before mutation. That record states only that Tekstide authorized the attempt. It does not claim the action completed.

After mutation or process creation, append a truthful `applied`, `started`, or `failed` observation where outcome evidence is required. If an unexpected post-authorization failure occurs, retain the authorization record and append a bounded failure observation when possible; never reinterpret authorization as completion.

A safe-close `applied` outcome means Tekstide issued the selected terminate/abandon action. It does not mean the process exited. Actual process termination remains a later runtime observation correlated to the managed/supervised launch operation where available.

Authority-reducing and protective actions apply the safer state first. Audit persistence failure must never leave a workspace Trusted, an approval Pending, a risky close accepted, or a policy less restrictive merely to preserve audit ordering.

For any observational append failure, the caller records bounded in-memory degraded-audit health and surfaces it to later app/UI integration.

Audit-store failure must not recursively try to audit itself.

## Query Model

Queries are read-only and bounded.

- Default ordering is descending store sequence, with sequence as a stable cursor.
- Every query requires a limit with a conservative maximum.
- Filters may include project id, class, outcome, linked entity id, and timestamp range.
- Queries return structured records or content-free summaries; they do not join transcript, command, file, or diff content.
- Unknown future record codes encountered by an older reader produce an unsupported-record result, not a guessed mapping.
- Query failures do not mutate or quarantine the store.

The final GUI viewer, search interaction, and export format remain later surface work.

## Retention and Purge

The initial retention policy is retain-until-explicit-purge. Normal startup, cache cleanup, transcript cleanup, and recent-project pruning must not delete audit records.

Required purge scopes:

- one project;
- all audit data.

Event-id and timestamp-range purge are deferred until the later local-data UI defines their user-intent semantics.

Purge behavior:

- requires an explicit caller action;
- is transactional and idempotent;
- reports bounded deleted-record counts and completion state;
- does not delete project files, transcripts, configuration, recent-project state, or unrelated local data;
- does not retain project or subject identifiers solely to prove that their audit data was purged;
- returns an ephemeral receipt for both project and global audit-data purge;
- appends no durable audit-data-purge event in v1, including outside the deleted project scope;
- attempts SQLite checkpoint/cleanup appropriate to the selected journal mode;
- makes no secure-deletion or forensic-erasure claim.

A future retention RFC may add automatic age/size policy. RFC-013 must not introduce hidden automatic audit deletion.

## Schema Versioning and Migration

The database uses both an application id and `PRAGMA user_version` (or an equivalently explicit metadata table) to distinguish Tekstide audit data and schema version.

Migration rules:

- schema creation and every migration run in a transaction;
- migrations are forward-only and sequential;
- the implementation ships immutable input/expected fixtures for every supported prior version;
- a failed migration leaves the prior database usable or returns a recoverable failure without claiming success;
- a schema newer than the running application or a foreign application id is rejected in v1 without writes;
- destructive migration requires a new reviewed RFC amendment;
- record enum encodings are stable strings, not Rust discriminant integers;
- domain types are converted through an explicit storage DTO rather than deriving persistence compatibility from Rust layout.

The initial implementation must include a v1 fixture even if no historical production schema exists. This proves the migration harness and gives the next schema change a durable baseline.

## Missing and Corrupt Store Recovery

Missing store state is normal on first run. Opening a missing store creates the current schema under the validated state root.

Corruption handling is conservative:

1. An existing store is first opened read-only for an identity/schema/read probe before any write-capable pragma or migration.
2. Ordinary startup uses bounded work independent of retained row count; comprehensive `quick_check`/`integrity_check` work is reserved for explicit diagnostics or recovery.
3. Corruption or an invalid application/schema identity returns a typed unavailable state.
4. Ordinary open does not rename, delete, overwrite, or recreate the database silently.
5. Explicit recovery closes connections and moves the database plus known journal/WAL/shared-memory companions into a unique directory under `audit/recovery/`.
6. Recovery writes a content-free manifest entry for each expected database, journal, WAL, and shared-memory artifact, including absent and moved outcomes.
7. Partial or failed quarantine is reported and does not create a fresh store.
8. Recovery creates a fresh current-version store only after complete quarantine succeeds.
9. The fresh store records a content-free recovery event when possible.
10. Recovery reports whether evidence was quarantined and where, without reading or displaying record contents.

Automatic salvage of partially readable rows is out of scope. A later forensic/export tool may inspect quarantined artifacts explicitly.

## Durability Settings

Implementation review must record the selected SQLite journal and synchronous settings and test the resulting artifact set. WAL with durable commit settings is acceptable, but the RFC does not claim that process commit equals storage-media persistence under every filesystem, kernel, power-loss, or hardware failure.

The store must use transactions, enforce foreign-key/check constraints where applicable, set a bounded busy timeout, and close/checkpoint cleanly. Implementation evidence should include abrupt-tail/corrupt fixtures rather than making theoretical crash-proof claims.

References used for the backend decision:

- SQLite pragma, integrity-check, application-id, user-version, journal, and synchronous behavior: <https://www.sqlite.org/pragma.html>
- `rusqlite` backend and bundled-linkage guidance: <https://github.com/rusqlite/rusqlite>

## Integration Boundaries

- `ProjectSession` may continue to expose in-memory audit events for current views, but durable storage must not be hidden inside arbitrary collection mutation.
- A coordinator/service owns the sequence according to the security-direction matrix: authorization before authority increase, protective mutation before observational persistence, and truthful post-action outcome records.
- AgentRun and TerminalSession lifecycle remains runtime truth. Audit records describe observations and decisions; replaying them must not resurrect processes.
- Transcript and generated-change stores remain separate data categories. Audit records may reference their stable ids but not their content or storage paths.
- Terminal output is untrusted and cannot directly create trusted audit records. Only app-generated policy/runtime events cross the durable boundary.

## Implementation Slices

### PR-013-A - Design and Handoff Acceptance

- Review backend, privacy, failure, migration, purge, and recovery decisions.

### PR-013-B - Durable Record and Path Model

- Add structured record conversion and validation.
- Add application-generated operation correlation and exhaustive per-class invariants.
- Add root-contained audit path resolution.
- Expand stable audit class/outcome vocabulary without persistence yet.

### PR-013-C - SQLite Schema, Append, and Query Store

- Add workspace-managed `rusqlite` dependency.
- Create v1 schema, transactional append, idempotency/conflict behavior, and bounded queries.
- Enforce authorization/outcome correlation across interleaved records without sequence-adjacency assumptions.
- Record selected SQLite linkage and durability settings.

### PR-013-D - Schema Identity and Migration Harness

- Add v1 fixtures, read-only identity/version probing, sequential migrations, and rollback behavior.

### PR-013-E - Corruption and Recovery Harness

- Add bounded startup health probing, explicit comprehensive diagnostics, corruption classification, artifact manifests, and quarantine/recreate recovery.

### PR-013-F - Purge and Local-Data Summary

- Add project/global explicit purge, ephemeral receipt behavior, database artifact accounting, and content-free local-data summaries.

### PR-013-G - Security-Event Integration

- Wire the reviewed store boundary into bidirectional trust decisions, managed/supervised launch authorization/outcome, and narrow post-ProjectSession root/symlink blocks from `ProjectFileAccessPolicy::resolve_existing` through ProjectSession open/save operations.
- Prove the security-direction matrix, pre-action authorization versus post-action outcome, and degraded observational behavior.
- Keep unsupported producers visible as limitations.

### PR-013-H - Closeout Evidence

- Complete migration/privacy/recovery evidence, known limitations, and lifecycle transition.

## Acceptance Criteria

- Audit storage resolves under Tekstide local state and outside project roots.
- Durable records use an explicit versioned DTO and never persist free-form summaries.
- Append is transactional, retry-safe, and conflict-detecting by event id.
- Queries are bounded and cursor-stable.
- Authority-increasing/destructive authorization failures fail closed without claiming completion.
- Authority-reducing/protective and observation-after-fact failures preserve the safer/runtime state and expose degraded audit health.
- Approval records retain allowlisted action/risk/actor/source/adapter context while excluding exact command, cwd, environment, and display-summary content.
- Per-class field invariants reject invalid id/code combinations.
- Correlated outcomes reference the sole earlier same-project authorization in the same family/action, enforce phase cardinality, and leave incomplete authorizations truthful after reopen.
- Missing stores initialize safely.
- Future schemas and corrupt stores are not overwritten silently.
- Explicit recovery quarantines the complete known SQLite artifact set before creating a fresh store.
- v1 and migration fixtures are checked.
- Project/global purge is explicit, scoped, idempotent, and does not claim secure deletion.
- Security review confirms persisted fields exclude commands, paths, output, content, prompts, environment data, and arbitrary summaries.
- Unsupported producers, GUI viewer, encryption, tamper evidence, cross-process writing, automatic retention, and secure deletion remain documented limitations.

## Risks and Mitigations

- **Audit records leak sensitive text.** Use an allowlisted DTO and never persist free-form summaries or generic metadata.
- **Audit failure creates false safety claims.** Classify operations as required-before-action or observational and test both paths.
- **Custom persistence becomes fragile.** Use SQLite transactions and versioning rather than an ad hoc append-file protocol.
- **Corruption recovery destroys evidence.** Require explicit quarantine before fresh-store creation.
- **Purge leaves misleading guarantees.** Delete scoped rows and clean SQLite artifacts where practical, while explicitly rejecting secure-deletion claims.
- **Bundled SQLite complicates builds and artifacts.** Record exact bundled versions, features, license, binary/build impact, and supported-target evidence before broader platform claims.
- **The core crate grows further.** Keep domain vocabulary and storage modules separate; reconsider a crate split only when a real independent boundary appears.

## Resolved Design Decisions

- Use bundled SQLite for the first implementation and record its exact build/license impact in PR-013-C.
- Keep persistence in `tekstide-core/src/audit/` until a real independent consumer, backend, or release boundary justifies `tekstide-audit`.
- Make managed/supervised AgentRun launch authorization audit-required, followed by truthful started/failed observations. Plain/manual terminals remain unsupported or audit-degraded.
- Require project/global purge only for M7; defer event/date-range purge.
- Limit first root/symlink audit integration to typed post-ProjectSession blocks from `ProjectFileAccessPolicy::resolve_existing` and ProjectSession open/save paths, without persisting raw paths.
- Use explicit no-salvage quarantine/recreate recovery, with read-only identity probing and bounded startup work.
- Correlate every post-authorization outcome with a bounded application-generated operation id; incomplete authorizations remain truthful and are never promoted to success during reopen.
- Use one `managed_process_lifecycle` family with exactly one authorization, one `started`/`failed` initial result, and an optional later `terminated` observation only after start.
- Use only the exhaustive v1 actor/source codes and allowed pairs defined by the per-class contract.
- Keep the v1 event-family matrix exhaustive. Audit-data purge is ephemeral-only, and generated-change metadata purge remains deferred.
