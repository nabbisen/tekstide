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

Pending implementation.

## PR-013-C - SQLite Schema, Append, and Query Store

Pending implementation.

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
