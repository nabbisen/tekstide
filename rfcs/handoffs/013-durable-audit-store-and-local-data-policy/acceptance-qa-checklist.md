---
title: "RFC-013: Durable Audit Store and Local Data Policy - Acceptance / QA Checklist"
rfc: "RFC-013"
rfc_file: "../../done/013-durable-audit-store-and-local-data-policy.md"
status: "Implemented with documented limitations"
target_milestone: "M7"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-22"
updated: "2026-07-23"
---

# RFC-013: Durable Audit Store and Local Data Policy - Acceptance / QA Checklist

## Acceptance Status

Design was accepted by response 095. PR-013-B through PR-013-G implementation was accepted by responses 096, 097, 100, 101, and 102. Response 103 accepted PR-013-H as complete with documented limitations; its earlier requirement for a `crates/tekstide/NOTICE` file was withdrawn as a maintainer-challenged error and no such file was created.

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

## Final Acceptance Decision

- [ ] Accepted as complete.
- [x] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

Response 103 accepted RFC-013 as complete with documented limitations. Its original finding requiring a `crates/tekstide/NOTICE` file was withdrawn after maintainer challenge: the `tekstide` source package redistributes no third-party code, so the obligation is satisfied by the root `NOTICE` alone (covering repository checkouts and release tarballs) plus the rusqlite/libsqlite3-sys packages' own licenses for crates.io source consumers. RFC-013 has moved to `rfcs/done/`.
