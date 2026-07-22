---
title: "RFC-013: Durable Audit Store and Local Data Policy - Acceptance / QA Checklist"
rfc: "RFC-013"
rfc_file: "../../proposed/013-durable-audit-store-and-local-data-policy.md"
status: "Proposed"
target_milestone: "M7"
source_rfc_status: "Proposed"
created: "2026-07-22"
updated: "2026-07-22"
---

# RFC-013: Durable Audit Store and Local Data Policy - Acceptance / QA Checklist

## Acceptance Status

Design accepted for implementation by review response 095. Implementation and closeout acceptance remain pending.

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
- [ ] Cross-project linked ids are rejected.

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
- [ ] Corrupt store open does not rename/delete/recreate automatically.
- [x] Startup health probing is bounded independently of retained row count.
- [x] Comprehensive integrity checks require explicit diagnostics/recovery.
- [ ] Explicit recovery quarantines database and known companion artifacts.
- [ ] Recovery manifest records moved/absent/failure state for each expected artifact.
- [ ] Incomplete quarantine prevents fresh-store creation.
- [ ] Fresh store after recovery records a content-free recovery event where possible.

## Purge and Retention Checklist

- [ ] Normal startup/cleanup does not delete audit records.
- [ ] Project purge affects only that project's records.
- [ ] Global purge removes all audit rows.
- [ ] Event-id/date-range purge remains deferred.
- [ ] Purge is explicit, transactional, and idempotent.
- [ ] Purge receipts do not defeat the requested scope.
- [ ] Project/global audit-data purge receipts are ephemeral only; no durable purge event is appended.
- [ ] Purge does not delete project files, transcripts, recent-project state, configuration, or recovery artifacts.
- [ ] Database and companion artifacts are included in local-data size accounting.
- [ ] Journal cleanup behavior is tested.
- [ ] No secure-deletion claim is made.

## Integration Checklist

- [ ] Authority-increasing/destructive actions persist authorization before mutation.
- [ ] Authorization records do not claim applied/started/completed outcomes.
- [ ] Outcome records carry the matching authorization operation id.
- [ ] Authority-increasing/destructive persistence failure blocks the action.
- [ ] Authority-reducing/protective actions apply before observational persistence.
- [ ] Protective persistence failure preserves the safer state.
- [ ] Observational persistence failure preserves runtime/security truth.
- [ ] Managed/supervised launch authorization is persisted before process creation and followed by started/failed truth.
- [ ] Safe-close applied outcome means action issued, not process exited.
- [ ] Plain/manual terminal behavior is never relabeled as durably authorized.
- [ ] Initial root/symlink integration is limited to typed post-ProjectSession open/save blocks without raw paths.
- [ ] Audit degradation is visible in bounded in-memory health state.
- [ ] Audit failure does not recursively audit itself.
- [ ] Terminal output cannot create trusted durable records.
- [ ] Runtime remains TerminalSession/AgentRun process truth.
- [ ] Integrated and unsupported event producers are listed separately.

## Evidence Required

- [ ] Design review response and amendments/rereviews.
- [ ] Implementation review response for every PR-013 slice.
- [ ] Workspace dependency/linkage evidence.
- [ ] Test command output.
- [ ] Schema and migration fixture evidence.
- [ ] Read-only identity/version probe evidence.
- [ ] Append/idempotency/conflict evidence.
- [ ] Correlation/interleaving/incomplete-authorization evidence.
- [ ] Operation cardinality and lifecycle phase-order evidence.
- [ ] Actor/source valid/invalid vocabulary evidence.
- [ ] Path containment evidence.
- [ ] Bounded query evidence.
- [ ] Corrupt/missing/future-store evidence.
- [ ] Recovery artifact evidence.
- [ ] Recovery manifest/partial-quarantine evidence.
- [ ] Purge isolation evidence.
- [ ] Security-direction and authorization/outcome integration evidence.
- [ ] Command/cwd/environment/display-summary/path privacy sentinel evidence.
- [ ] Known limitations and release-claim assessment.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [ ] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Pending implementation and evidence.
```
