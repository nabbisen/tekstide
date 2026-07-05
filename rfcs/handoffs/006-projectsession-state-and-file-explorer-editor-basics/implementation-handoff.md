---
title: "RFC-006: ProjectSession State and File Explorer / Editor Basics — Implementation Handoff"
rfc: "RFC-006"
rfc_file: "../../done/006-projectsession-state-and-file-explorer-editor-basics.md"
status: "Implemented with documented limitations"
target_milestone: "M3"
source_rfc_status: "Implemented"
created: "2026-07-04"
---

# RFC-006: ProjectSession State and File Explorer / Editor Basics — Implementation Handoff

## Purpose

This handoff translates RFC-006 into developer-facing implementation guidance. It is intentionally scoped to **ProjectSession state, file explorer, and editor basics** and should be implemented only after the RFC is accepted or an explicit spike exception is recorded under RFC-000.

## Source RFC Summary

This RFC implements the basic per-project Content Mode: explorer, file open/edit/save, one main content surface, external change detection, and conservative root-bound file access.

## Dependencies and Sequencing

- Target milestone: **M3**
- Source RFC status: **Implemented with documented limitations**
- Required predecessor RFCs: RFC-002, RFC-003, RFC-004, RFC-005
- Implementation may begin only when predecessor interfaces are stable enough to avoid rework.
- If a predecessor is still draft, implement only a narrow spike or no-op seam and record the limitation in release evidence.

## Implementation Scope

The implementation must deliver the following user-visible or developer-visible result:

- User can open a project and browse files.
- User can open, edit, and save a text file under root.
- Dirty state is visible.
- External modification is detected.
- Symlink escape does not silently open outside-root content.
- Content Mode does not introduce split panes.

The implementation is successful only when these outcomes are visible through tests, manual QA, or release evidence. A hidden internal implementation without the RFC acceptance behavior is not complete.

## Recommended Module Boundaries

- `project::root`: root path policy, canonicalization, containment checks, and symlink classification.
- `explorer`: bounded/lazy file tree read model and visible node states.
- `content` or `editor`: `String`-backed UTF-8 text document buffer, dirty state, cursor/scroll/open-file state, save/conflict model.
- `watch`: optional file watcher adapter; save-time conflict detection must work from snapshots even without watcher reliability.
- `app`/`shell`: orchestration and visible evidence rendering only.

Keep these boundaries small and independently testable. UI modules should not own security policy, process lifecycle, persistence migration, or cross-project invariants. The core state transition layer should be testable without launching the GUI.

The first implementation slice should start with root policy and a file access target model. Do not start by wiring GUI widgets to raw paths.

## Key Design Decisions to Preserve

- The editor is basic but correct: open/edit/save under project root is more important than rich language features.
- All file operations must go through root-bound path policy.
- External changes are surfaced, not silently overwritten.

These decisions are higher priority than convenience shortcuts. If implementation pressure makes one of them difficult, pause and amend the RFC rather than silently weakening the behavior.

## Data Model and State Notes

- Carry explicit stable IDs through events, UI messages, persistence records, and audit entries.
- Avoid path strings as identity; paths may change, be symlinked, become unreadable, or collide across platforms.
- Represent selected path, absolute selected path, canonical target, canonical root, symlink status, and containment status explicitly before opening or saving content.
- Use a UTF-8 `String`-backed text buffer behind an internal `TextDocument` or `TextBuffer` boundary for RFC-006.
- Apply a 4 MiB default editable file-size cap, reject invalid UTF-8 and NUL-containing files for editable open, and show bounded errors or preview-only placeholders for unsupported files.
- Track a `FileSnapshot` equivalent containing canonical path, modified timestamp, length, and optional content hash at open/save time.
- Preserve a shared state vocabulary equivalent to `FileBufferState::{Clean, Dirty, ExternalChanged, Conflict, SaveError}` and `SaveDecision::{Saved, BlockedExternalChange, BlockedRootEscape, BlockedUnsafeSymlink, WriteFailed}`.
- Separate runtime handles from persisted metadata. Runtime process handles, file watchers, PTY handles, and GUI handles must not be serialized.
- Add schema-versioned fixtures whenever this RFC introduces persistent data.
- For post-MVP/deferred RFCs, preserve abstraction points but do not persist user-facing state until implementation is accepted.

## Error Handling Requirements

- Errors must be recoverable and visible to the user when they affect trust, process state, file state, transcript state, or generated changes.
- Failed operations must not partially mutate unrelated ProjectSessions.
- Cross-project failures must not cascade. A broken project, missing Git binary, crashed provider, invalid config, or failed terminal must not make the Project Board unusable.
- Any operation that could lose data, terminate processes, expose secrets, or grant trust must have explicit user-facing confirmation where required by the RFC.
- Save failures must preserve the in-memory buffer and keep dirty state.
- External disk changes must not be overwritten silently; dirty buffers enter a conflict state.
- Clean buffers may reload or prompt on external change according to the safest available UI path.
- Overwriting an externally changed file may be offered only through a confirmation that identifies the project, target file, and external-change condition. If that UI is absent, block save and preserve the dirty buffer.
- RFC-006 does not guarantee full preservation of permissions, ownership, or extended attributes beyond normal user-space save semantics. If preservation cannot be reasoned about safely for a target, return a recoverable save error.

## Security and Privacy Requirements

- Enforce Restricted Mode and trust checks before loading workspace-local automation, profiles, plugins, task definitions, or environment files.
- User-initiated open, edit, and save of files under the project root are allowed in Restricted Mode.
- Restricted Mode must still block or warn before LSP startup, formatter startup, scripts, hooks, tasks, plugins, workspace AI profiles/prompts, `.env` loading, or other workspace-local automation.
- Do not add network behavior unless the RFC explicitly permits it.
- Do not store secrets in logs, audit events, config, or transcripts beyond unavoidable terminal output chosen by the user/tool; where unavoidable, provide purge or retention controls according to RFC-010/RFC-016.
- Do not dump file contents, full project trees, or unnecessary absolute private paths into logs, test fixtures, review packages, or release evidence.
- Where the RFC affects terminal/AI behavior, label sessions as Managed, Supervised, or Plain and avoid implying unsupported command interception.
- Route security-relevant file access decisions through an audit seam or document them as audit-pending until RFC-012. Examples: symlink escape blocked, root escape blocked, save blocked due to outside-root target, and destructive overwrite confirmation.
- Do not force broad audit logging of ordinary user file opens/saves before the audit persistence policy exists.

## UX Requirements

- The user must always be able to identify the active project and whether any process, AgentRun, approval, dirty file, or review item needs attention.
- Keyboard-centric operation must be preserved, but mouse access must exist for destructive confirmations and review flows.
- Blocking dialogs must show exact target project/session/run and exact consequences.
- Empty, loading, missing, and error states must be designed as first-class states, not debug text.

## Observability and Release Evidence

Every PR implementing this RFC must include:

- unit/integration tests added or updated;
- manual QA scenarios run;
- screenshots or text logs where UI behavior is essential;
- security-impact note;
- migration note if local state/config/data schema changes;
- known limitations and follow-up RFC/issue references.

## Non-Goals and Guardrails

- Do not implement full syntax/language intelligence, multiple editor splits, or deep binary file support.

Additionally, do not implement unrelated deferred RFCs while touching this area. The presence of a useful abstraction seam is acceptable; shipping the deferred feature is not.

## Global Implementation Conventions

These instructions apply to this RFC unless the RFC explicitly says otherwise.

- Keep the implementation local-first. Do not introduce cloud services, telemetry, automatic uploads, or account systems.
- Treat every workspace root as untrusted until a persisted trust decision says otherwise.
- Keep project boundaries explicit in APIs. No state mutation may rely on an implicit "current project" without carrying the relevant `ProjectId` or equivalent stable identifier.
- Make security-relevant transitions auditable: process launches, trust changes, approvals, transcript purges, safe-close decisions, and destructive actions.
- Prefer small, testable modules over large GUI callback files. UI code should render state and emit commands; domain state transitions should live in testable core logic.
- Do not implement deferred plugin, registry, remote/container, debugger, or multi-window behavior unless the relevant RFC is later accepted.
- Preserve compatibility labeling. Never imply that Tekstide can intercept commands for plain terminals or unsupported AI CLIs.
- Include release evidence with every implementation PR: tests run, manual scenarios run, known limitations, migration notes, and security impact.
