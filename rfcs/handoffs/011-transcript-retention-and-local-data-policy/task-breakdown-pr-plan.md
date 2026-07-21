---
title: "RFC-011: Transcript Retention and Local Data Policy - Task Breakdown / PR Plan"
rfc: "RFC-011"
rfc_file: "../../proposed/011-transcript-retention-and-local-data-policy.md"
status: "Proposed"
target_milestone: "M6"
created: "2026-07-21"
---

# RFC-011: Transcript Retention and Local Data Policy - Task Breakdown / PR Plan

Implementation must wait for RFC-011 design review acceptance.

## PR-011-A - Transcript Policy and Path Model

Goal:

- Add transcript capture modes, retention states, local path resolver, and policy validation.

Expected files:

- `crates/tekstide-core/src/security.rs`
- `crates/tekstide-core/src/domain/transcript.rs`
- new transcript/path module if the boundary is large enough
- focused unit tests

Review focus:

- bounded-or-absent persistence policy;
- state-root containment;
- project-root exclusion;
- no content in summaries or errors.

## PR-011-B - Bounded Transcript Writer

Goal:

- Add local append/write harness with byte limits and truncation metadata.

Review focus:

- byte limit enforcement;
- metadata updates;
- untrusted terminal byte boundary;
- no search indexing;
- no background hidden behavior.

## PR-011-C - AgentRun Launch Integration

Goal:

- Wire transcript capture preference into RFC-010 AgentRun launch and terminal output capture.

Review focus:

- opt-out before process start;
- unsafe paths reject or disable capture according to mode;
- AgentRun and TerminalSession transcript references are attached truthfully;
- plain terminals still do not capture by default.

## PR-011-D - Purge and Local Data Summaries

Goal:

- Add transcript, AgentRun, and ProjectSession purge scopes plus metadata-only local data summaries.

Review focus:

- idempotent purge;
- no project-file deletion;
- purged/tombstone state;
- summaries exclude transcript content.

## PR-011-E - Closeout Evidence

Goal:

- Complete acceptance checklist, QA evidence, known limitations, and RFC lifecycle transition after review accepts implementation.

Review focus:

- every RFC-011 claim has observed test or harness evidence;
- durable audit, GUI review panes, redaction, search indexing, and generated-change UI remain non-claims unless separately implemented and reviewed.
