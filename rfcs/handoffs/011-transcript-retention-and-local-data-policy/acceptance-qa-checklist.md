---
title: "RFC-011: Transcript Retention and Local Data Policy - Acceptance / QA Checklist"
rfc: "RFC-011"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Implemented with documented limitations"
target_milestone: "M6"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-21"
updated: "2026-07-21"
---

# RFC-011: Transcript Retention and Local Data Policy - Acceptance / QA Checklist

## Acceptance Status

RFC-011 implementation has been reviewed through PR-011-D and accepted with documented limitations.

## Policy Checklist

- [x] Transcript byte persistence requires local-only storage.
- [x] Transcript byte persistence requires bounded retention.
- [x] Bounded retention includes per-transcript, per-project, and app-wide aggregate accounting limits.
- [x] Local-data summaries expose total retained transcript bytes without content.
- [x] Transcript byte persistence requires purge support.
- [x] Per-run opt-out is available before AgentRun process start.
- [x] Search indexing remains disabled.
- [x] Redaction claims are limited to structured metadata.

## Path Checklist

- [x] Transcript state root must be absolute.
- [x] Transcript path must be under Tekstide-managed local state.
- [x] Transcript path must not be inside the project root.
- [x] Traversal and symlink cases cannot redirect storage into the project root.
- [x] Unsafe transcript paths reject or disable capture according to capture mode.

## Capture / Launch Checklist

- [x] Plain terminal sessions do not retain transcript bytes by default.
- [x] AgentRun opt-out writes no transcript bytes.
- [x] AgentRun enabled capture preflights storage before process start.
- [x] `RequiredLocalBounded` rejects launch when capture cannot be prepared.
- [x] Transcript metadata attaches to the correct AgentRun and TerminalSession after successful launch.
- [x] Terminal/runtime lifecycle remains process truth.

## Writer Checklist

- [x] Writer enforces byte limit.
- [x] Writer records truncation state.
- [x] Writer updates byte count and last-write metadata through explicit ProjectSession reconciliation.
- [x] Writer treats stored bytes as untrusted terminal output.
- [x] Writer summaries do not include transcript content.

## Purge Checklist

- [x] Purge by transcript id removes bytes.
- [x] Purge by AgentRun id removes related transcript bytes.
- [x] Purge by ProjectSession id removes related transcript bytes.
- [x] Purge is idempotent when bytes are already absent.
- [x] Purge never deletes project files.
- [x] Purge preserves content-free tombstone metadata by default.
- [x] AgentRun/TerminalSession transcript references remain attached to tombstones unless preserving them would retain sensitive metadata.

## Evidence Required

- [x] Design review response and any amendment.
- [x] Implementation review responses for each PR-011 slice.
- [x] Test command output.
- [x] Path containment evidence.
- [x] Capture/opt-out evidence.
- [x] Writer bound/truncation evidence.
- [x] Aggregate retention/accounting evidence.
- [x] Purge evidence.
- [x] Tombstone reference evidence.
- [x] Privacy summary evidence.
- [x] Migration note or "no migration" statement.
- [x] Known limitations.

## Documented Limitations

- Automatic project/app aggregate cleanup is not implemented; explicit purge scopes and metadata-only accounting are implemented.
- Transcript metadata reconciliation is explicit and caller-driven; there is no hidden background finalizer.
- Transcript capture occurs only when callers read terminal output through the existing bounded runtime read API; there is no hidden background terminal-output capture loop.
- RFC-011 does not claim durable audit, GUI review surfaces, generated-change review, search indexing, secure deletion, or redaction.
- Future persisted/restore paths must revalidate transcript storage references against the Tekstide state-root policy before making restored records purgeable.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [x] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Accepted with documented limitations after PR-011-A through PR-011-D implementation review.
```
