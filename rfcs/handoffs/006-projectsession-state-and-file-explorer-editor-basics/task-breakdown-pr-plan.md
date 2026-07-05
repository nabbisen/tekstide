---
title: "RFC-006: ProjectSession State and File Explorer / Editor Basics — Task Breakdown / PR Plan"
rfc: "RFC-006"
rfc_file: "../../proposed/006-projectsession-state-and-file-explorer-editor-basics.md"
status: "Proposed developer handoff - do not implement before RFC acceptance"
target_milestone: "M3"
source_rfc_status: "Proposed"
created: "2026-07-04"
---

# RFC-006: ProjectSession State and File Explorer / Editor Basics — Task Breakdown / PR Plan

## Planning Assumptions

- This plan assumes RFC-006 is accepted before implementation begins.
- The work should be split into small reviewable PRs. Prefer PRs that can be tested without a complete GUI stack when the behavior is domain-level.
- Target milestone: **M3**.
- Prerequisites: RFC-002, RFC-003, RFC-004, RFC-005.

## PR Sequence Overview

- PR-006-A: Root-bound file access policy.
- PR-006-B: Bounded file explorer model.
- PR-006-C: Text document buffer.
- PR-006-D: Safe save and external change detection.
- PR-006-E: Content Mode integration.
- PR-006-F: QA evidence and closeout.

The sequence is intentionally ordered from core contracts to visible behavior to integration and hardening. Do not merge UI-only behavior before the underlying state transitions and policy decisions are testable.

## Detailed PR Breakdown

### PR-006-A — Root-Bound File Access Policy

Purpose:

- Establish the security-critical path model before explorer recursion, editor open, or save behavior exists.

Developer tasks:

- Add a `FileAccessTarget` equivalent carrying project id, selected relative path, selected absolute path, canonical target, canonical root, symlink status, and containment status.
- Add root canonicalization and containment checks.
- Add symlink classification for in-root symlinks and escaping symlinks.
- Add open/save path validation APIs that every later file operation must use.
- Add tests for in-root normal files, in-root symlinks, escaping symlinks, `..` traversal, missing files, unreadable paths, and cross-project isolation.

Review focus:

- No file operation bypasses root policy.
- Selected path vs canonical target semantics are explicit.
- Saving through symlinks cannot replace the symlink or write outside root.

### PR-006-B — Bounded File Explorer Model

Purpose:

- Provide a ProjectSession-root file tree read model without freezing on large trees.

Developer tasks:

- Add bounded or lazy scan behavior.
- Represent symlink labels, blocked escape nodes, unreadable nodes, stale nodes, and collapsed heavy directories.
- Collapse or bound `target`, `node_modules`, and `.git` internals by default.
- Add tests for large directory bounds, unreadable/stale nodes where practical, symlink labels, and root-only traversal.

Review focus:

- Explorer state is a read model, not an unbounded indexer.
- Errors are visible as node states rather than crashes.
- Heavy directories do not dominate startup behavior.

### PR-006-C — Text Document Buffer

Purpose:

- Add the minimal editable text document model without binding core state to a GUI editor crate.

Developer tasks:

- Add a UTF-8 `String`-backed `TextDocument` or `TextBuffer` boundary.
- Add editable open for valid UTF-8 files under the 4 MiB cap.
- Reject invalid UTF-8, NUL-containing files, and larger files for editable open with bounded user-facing errors or preview-only placeholders.
- Track dirty state, cursor position, scroll position, and active file identity.
- Add tests for valid UTF-8 open, invalid UTF-8 rejection, binary/NUL rejection, large-file cap, edit transitions, dirty state, and cross-project isolation.

Review focus:

- Buffer implementation is replacement-friendly.
- File contents are not logged or placed in release evidence.
- Dirty state remains scoped to the owning ProjectSession.

### PR-006-D — Safe Save and External Change Detection

Purpose:

- Make save correctness explicit before UI polish depends on it.

Developer tasks:

- Add a `FileSnapshot` equivalent with canonical path, modified timestamp, length, and optional content hash.
- Add shared state/outcome vocabulary equivalent to `FileBufferState` and `SaveDecision`.
- Record snapshots at open and successful save.
- Revalidate root containment and snapshot state before save.
- Use temp-file-plus-rename where safe for the target path and platform.
- Return conservative recoverable errors for symlink/platform edge cases where safe replacement cannot be guaranteed.
- Preserve buffer and dirty state on write error.
- Add conflict state for dirty buffer plus external disk change.
- Add clean-buffer reload or prompt behavior for external disk change.
- Block overwrite of externally changed files unless a confirmation identifies the project, target file, and external-change condition.
- Return recoverable save errors when permissions, ownership, extended attributes, symlink behavior, or platform replacement semantics cannot be reasoned about safely.
- Add tests for save success, write failure preservation, save-time root revalidation, external dirty conflict, external clean reload/prompt, and symlink save rejection where needed.

Review focus:

- No external change is overwritten silently.
- Save-time conflict detection does not depend on a watcher.
- Error states are recoverable and project-scoped.

### PR-006-E — Content Mode Integration

Purpose:

- Expose the editor/explorer workflow without breaking Project Board and RFC-003 navigation semantics.

Developer tasks:

- Wire Content Mode explorer and one main editor surface.
- Show active project, active file label, dirty indicator, unsupported-file messages, and conflict/error states.
- Propagate dirty-state count or attention hint to Project Board/runtime summary without overclaiming full safe-close coverage.
- Preserve no arbitrary editor splits.
- Ensure Restricted Mode allows user file open/edit/save under root but does not start LSP, formatter, hooks, tasks, plugins, AI profiles/prompts, `.env` loading, network, or process execution.
- Add visible evidence through shell/text rendering, GUI screenshot, or manual transcript appropriate to the current app surface.

Review focus:

- User can understand active project, active file, dirty/conflict status, and blocked path decisions.
- The UI does not imply unsupported automation, command approval, or safe-close completeness.
- Content Mode remains a single primary surface.

### PR-006-F — QA Evidence and Closeout

Purpose:

- Convert implementation into a release-ready RFC-006 slice.

Developer tasks:

- Run full formatting, linting, unit, integration, and scenario tests.
- Record evidence for symlink escape block, in-root symlink behavior, valid UTF-8 open, invalid UTF-8 rejection, binary/NUL rejection, large-file cap, save failure preservation, external clean reload/prompt, external dirty conflict, dirty Project Board/runtime summary update, no split panes, and no LSP/formatter/network/process/`.env` behavior.
- Add manual QA notes for open/edit/save/conflict workflows.
- Add security impact note and migration note or "no migration" statement.
- Add known limitations and follow-up RFC references.

Review focus:

- Evidence is sufficient for RFC-006 acceptance.
- Known limitations are explicit and do not contradict accepted RFC scope.
- No broad refactor or unrelated deferred feature entered the closeout.

## Suggested Review Gates

1. **Design gate:** RFC accepted and predecessor contracts stable.
2. **Core gate:** state transitions and policy behavior tested without GUI dependence.
3. **UX gate:** manual scenario proves user can understand current state and consequences.
4. **Security gate:** root-bound policy, Restricted Mode automation blocks, audit-pending decisions, and destructive confirmations reviewed where applicable.
5. **Release gate:** evidence bundle complete and known limitations documented.

## Work Items by Discipline

### Product / PM

- Confirm this RFC is still in scope for the target milestone.
- Confirm open questions from the RFC are either answered or converted into explicit implementation assumptions.
- Keep non-goals visible during review.

### Design / UX

- Validate empty, loading, error, confirmation, and recovery states.
- Validate keyboard-centric flow and focus order.
- Confirm labels are honest for trusted/restricted file access states and unsupported-file states.

### Engineering

- Implement the PR sequence above.
- Keep domain logic decoupled from GUI toolkit callbacks.
- Add tests before or with behavior.

### Security / QA

- Review trust boundaries and destructive paths.
- Run scenario tests listed in the QA checklist artifact.
- Check audit/release evidence before accepting completion.

## Implementation Stop Conditions

Pause and request RFC amendment if:

- the implementation needs network/cloud behavior not described in the RFC;
- command approval would require pretending to intercept arbitrary shell commands;
- file/process behavior crosses project boundaries;
- persistence requires storing secrets or sensitive data without purge/retention behavior;
- the UI needs arbitrary pane, plugin, remote, or multi-window behavior from deferred RFCs.
