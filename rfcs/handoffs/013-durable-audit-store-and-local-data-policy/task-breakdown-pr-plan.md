---
title: "RFC-013: Durable Audit Store and Local Data Policy - Task Breakdown / PR Plan"
rfc: "RFC-013"
rfc_file: "../../proposed/013-durable-audit-store-and-local-data-policy.md"
status: "Proposed"
target_milestone: "M7"
created: "2026-07-22"
---

# RFC-013: Durable Audit Store and Local Data Policy - Task Breakdown / PR Plan

Design review response 095 accepted RFC-013 for implementation. PR-013-B may begin.

## PR-013-A - Design and Handoff Acceptance

Goal:

- Review and accept the bundled backend, data minimization, security-direction matrix, authorization/outcome distinction, class invariants, migration, purge, recovery, and implementation sequencing.

Review focus:

- SQLite is justified against a custom file protocol;
- bundled SQLite linkage and its build/license evidence are explicit;
- persisted records exclude free-form and sensitive content;
- authority-increasing actions fail closed while protective actions cannot be blocked by audit failure;
- pre-action authorization is distinct from applied/observed outcome;
- approval records remain useful through allowlisted action/risk/actor/source/adapter fields;
- purge and recovery do not overclaim secure deletion or silent repair;
- implementation slices remain independently reviewable.

## PR-013-B - Durable Record and Path Model

Goal:

- Add structured durable-record vocabulary, conversion/validation, audit-store health/error categories, and root-contained storage-path resolution without opening a database.

Review focus:

- no free-form summary persistence;
- stable string code mapping;
- per-class required/forbidden fields;
- exhaustive coverage for every retained v1 event family;
- allowlisted action/risk/actor/source/adapter conversion;
- operation-id shape and authorization/outcome phase vocabulary;
- exact actor/source code vocabulary and allowed-pair validation;
- field bounds and control-character rejection;
- ownership/link validation;
- state-root containment and project-root exclusion;
- later-added/restored project roots cannot contain existing audit state;
- no manifest dependency change yet unless needed for reviewed model serialization.

Expected evidence:

- record conversion, exhaustive class-invariant, operation-correlation shape, actor/source valid/invalid code, and sensitive-field sentinel tests;
- wrong-family/outcome-as-authorization/cross-project correlation shape rejection tests where model validation can decide without storage;
- path containment, symlink, relative-root, inside-project, and later-project-conflict tests;
- bounded error-summary tests.

## PR-013-C - SQLite Schema, Append, and Query Store

Goal:

- Add `rusqlite` with only its reviewed `bundled` feature, v1 schema, transactional append, retry/conflict behavior, and bounded query API.

Review focus:

- workspace dependency policy;
- exact `rusqlite`/SQLite versions, bundled linkage, license, binary/build impact, and supported-target implications;
- application id and schema version;
- transaction and constraint behavior;
- exact retry versus conflicting id reuse;
- operation correlation requires an earlier same-project compatible authorization and does not rely on adjacency;
- exactly one authorization exists per operation id;
- managed-process lifecycle and ordinary authorization phase cardinality are explicit;
- bounded cursor queries;
- busy/read-only/full-disk/I/O categories do not leak content;
- journal/synchronous/foreign-key/busy-timeout settings are explicit.

Expected evidence:

- fresh/open/reopen tests;
- append commit, duplicate retry, conflict, rollback, and ordering tests;
- valid correlation, cross-project/wrong-family/later-reference rejection, interleaved operation, idempotent correlated retry, and authorization-without-outcome reopen tests;
- different-event duplicate authorization, contradictory outcome, termination-before-start, and started-then-terminated tests;
- bounded filter/cursor tests;
- workspace build/check/clippy/test gates.

## PR-013-D - Schema Identity and Migration Harness

Goal:

- Add read-only identity/version probing, immutable fixtures, sequential migration, and rollback behavior.

Review focus:

- immutable v1 fixture baseline;
- read-only probe before any write-capable pragma or migration;
- failed migration rollback;
- future version and foreign application id are rejected without writes.

Expected evidence:

- current and prior fixture tests;
- future-version and foreign-database tests;
- missing-store creation test.

## PR-013-E - Corruption and Recovery Harness

Goal:

- Add bounded startup health probing, explicit comprehensive diagnostics, corruption classification, and explicit quarantine/recreate recovery.

Review focus:

- startup probe work is independent of retained row count;
- comprehensive checks run only through explicit diagnostics/recovery;
- ordinary open does not mutate corrupt evidence;
- explicit recovery handles the complete known SQLite artifact set without broad globs;
- manifest records moved/absent/failure state for every expected artifact;
- partial quarantine prevents fresh-store creation;
- recovery diagnostics remain content-free;
- no automatic salvage claim.

Expected evidence:

- malformed/corrupt/truncated database tests;
- bounded-startup-probe evidence;
- recovery bundle tests for database, journal, WAL, and shared-memory companions;
- partial quarantine and manifest tests.

## PR-013-F - Purge and Local-Data Summary

Goal:

- Add explicit project/global purge, ephemeral receipts, artifact-aware size summaries, and journal cleanup behavior.

Review focus:

- scope isolation;
- project/global scopes only; event/date-range purge remains deferred;
- both scopes return ephemeral receipts and append no durable audit-data-purge event;
- idempotency;
- no unrelated local-data deletion;
- no project/subject tombstone defeating purge intent;
- no secure-deletion claim;
- recovery artifacts are not silently deleted by ordinary purge.

Expected evidence:

- project isolation and global purge tests;
- repeated purge tests;
- unrelated transcript/recent/config fixture preservation;
- bounded count/size summary tests;
- journal-mode-specific cleanup evidence.

## PR-013-G - Security-Event Integration

Goal:

- Wire bidirectional trust, managed/supervised launch authorization/outcome, and narrow post-ProjectSession root/symlink block producers through a coordinator and expose audit health/degradation.

Review focus:

- authority-increasing/destructive actions persist authorization before mutation;
- persistence failure blocks only authority-increasing/destructive actions;
- authority-reducing/protective actions apply first and remain effective when persistence fails;
- pre-action records never claim applied/started/completed outcomes;
- outcome records carry the authorization operation id and incomplete authorization remains truthful;
- managed/supervised launches append started/failed truth after authorization;
- root/symlink blocks store only project id and typed reason, never paths;
- terminal output cannot create trusted records;
- AgentRun/TerminalSession runtime truth remains authoritative;
- no exact command, cwd, output, path, prompt, environment, transcript, or diff content reaches storage;
- unsupported producers remain explicit.

Expected evidence:

- trust grant/revocation ordering and failure tests;
- command approve/reject and close execute/cancel matrix tests where integrated;
- managed/supervised launch authorization plus started/failed tests;
- safe-close applied semantics do not imply process exit;
- `ProjectFileAccessPolicy::resolve_existing` ProjectSession open/save block tests;
- command/cwd/environment/display-summary/path sentinel tests across durable rows and diagnostics.

## PR-013-H - Closeout Evidence

Goal:

- Complete the checklist, QA evidence, security/privacy assessment, known limitations, and RFC lifecycle transition after implementation reviews accept RFC-013.

Review focus:

- every durable-audit claim has observed fixture/harness evidence;
- represented and runtime-integrated event sources are distinguished;
- migration and recovery evidence is complete;
- GUI viewer, encryption, tamper evidence, secure deletion, automatic retention, cross-process writing, and unsupported event producers remain non-claims.
