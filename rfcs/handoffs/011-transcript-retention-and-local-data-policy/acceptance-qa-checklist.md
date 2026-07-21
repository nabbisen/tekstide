---
title: "RFC-011: Transcript Retention and Local Data Policy - Acceptance / QA Checklist"
rfc: "RFC-011"
rfc_file: "../../proposed/011-transcript-retention-and-local-data-policy.md"
status: "Proposed"
target_milestone: "M6"
source_rfc_status: "Proposed"
created: "2026-07-21"
---

# RFC-011: Transcript Retention and Local Data Policy - Acceptance / QA Checklist

## Acceptance Status

This checklist is proposed. It becomes the implementation acceptance checklist only after RFC-011 design review accepts or amends the scope.

## Policy Checklist

- [ ] Transcript byte persistence requires local-only storage.
- [ ] Transcript byte persistence requires bounded retention.
- [ ] Bounded retention includes per-transcript, per-project, and app-wide aggregate limits.
- [ ] Local-data summaries expose total retained transcript bytes without content.
- [ ] Transcript byte persistence requires purge support.
- [ ] Per-run opt-out is available before AgentRun process start.
- [ ] Search indexing remains disabled.
- [ ] Redaction claims are limited to structured metadata.

## Path Checklist

- [ ] Transcript state root must be absolute.
- [ ] Transcript path must be under Tekstide-managed local state.
- [ ] Transcript path must not be inside the project root.
- [ ] Traversal and symlink cases cannot redirect storage into the project root.
- [ ] Unsafe transcript paths reject or disable capture according to capture mode.

## Capture / Launch Checklist

- [ ] Plain terminal sessions do not retain transcript bytes by default.
- [ ] AgentRun opt-out writes no transcript bytes.
- [ ] AgentRun enabled capture preflights storage before process start.
- [ ] `RequiredLocalBounded` rejects launch when capture cannot be prepared.
- [ ] Transcript metadata attaches to the correct AgentRun and TerminalSession after successful launch.
- [ ] Terminal/runtime lifecycle remains process truth.

## Writer Checklist

- [ ] Writer enforces byte limit.
- [ ] Writer records truncation state.
- [ ] Writer updates byte count and last-write metadata.
- [ ] Writer treats stored bytes as untrusted terminal output.
- [ ] Writer summaries do not include transcript content.

## Purge Checklist

- [ ] Purge by transcript id removes bytes.
- [ ] Purge by AgentRun id removes related transcript bytes.
- [ ] Purge by ProjectSession id removes related transcript bytes.
- [ ] Purge is idempotent when bytes are already absent.
- [ ] Purge never deletes project files.
- [ ] Purge preserves content-free tombstone metadata by default.
- [ ] AgentRun/TerminalSession transcript references remain attached to tombstones unless preserving them would retain sensitive metadata.

## Evidence Required

- [ ] Design review response and any amendment.
- [ ] Implementation review responses for each PR-011 slice.
- [ ] Test command output.
- [ ] Path containment evidence.
- [ ] Capture/opt-out evidence.
- [ ] Writer bound/truncation evidence.
- [ ] Aggregate retention/accounting evidence.
- [ ] Purge evidence.
- [ ] Tombstone reference evidence.
- [ ] Privacy summary evidence.
- [ ] Migration note or "no migration" statement.
- [ ] Known limitations.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [ ] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Pending design review.
```
