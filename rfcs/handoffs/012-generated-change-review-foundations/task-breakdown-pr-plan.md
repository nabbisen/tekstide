---
title: "RFC-012: Generated Change Review Foundations - Task Breakdown / PR Plan"
rfc: "RFC-012"
rfc_file: "../../proposed/012-generated-change-review-foundations.md"
status: "Proposed"
target_milestone: "M6"
created: "2026-07-21"
---

# RFC-012: Generated Change Review Foundations - Task Breakdown / PR Plan

Implementation must wait for RFC-012 design review acceptance.

## PR-012-A - Design and Handoff Acceptance

Goal:

- Review and accept generated-change review scope, detector boundaries, AgentRun association policy, and implementation sequencing.

Review focus:

- scope is headless/model-level and suitable for M6;
- durable audit is clearly deferred to RFC-013;
- rendered GUI review surfaces remain M8 scope;
- summaries remain content-free;
- implementation slices are reviewable.

## PR-012-B - ChangeSet Review Model

Goal:

- Extend ChangeSet/review vocabulary with detector state, association confidence, timestamps, bounded summaries, review transitions, artifact references, and AgentRun attachment helpers.

Review focus:

- review-state transitions are explicit and non-destructive;
- no test-pass, rollback, or authorship overclaim;
- ProjectSession review counts remain truthful.

## PR-012-C - Baseline and Path Detector Harness

Goal:

- Add root-contained baseline and changed-path detection using metadata-only filesystem and/or Git-status evidence.

Review focus:

- path containment;
- Git subprocess safety or safe-library evidence;
- detector unavailable/partial states;
- no file contents in summaries;
- no dependency on hidden background watchers.

## PR-012-D - AgentRun Review Integration

Goal:

- Connect baseline/detection to AgentRun lifecycle and ProjectSession ChangeSet collections.

Review focus:

- AgentRun association is conservative;
- detached/orphaned AgentRuns remain weak or ambiguous unless independently closed by reviewed runtime evidence;
- overlapping or ambiguous runs do not get false authorship claims;
- ReviewReady lifecycle remains truthful;
- TerminalSession/runtime remain process truth.

## PR-012-E - Closeout Evidence

Goal:

- Complete checklist, QA evidence, known limitations, and RFC lifecycle transition after implementation reviews accept RFC-012.

Review focus:

- every RFC-012 claim has observed model or harness evidence;
- durable audit, GUI review panes, hunk-level patching, rollback, search indexing, secure deletion, and redaction remain non-claims.
