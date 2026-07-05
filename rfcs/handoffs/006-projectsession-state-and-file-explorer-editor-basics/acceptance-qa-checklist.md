---
title: "RFC-006: ProjectSession State and File Explorer / Editor Basics — Acceptance / QA Checklist"
rfc: "RFC-006"
rfc_file: "../../done/006-projectsession-state-and-file-explorer-editor-basics.md"
status: "Implemented with documented limitations"
target_milestone: "M3"
source_rfc_status: "Implemented"
created: "2026-07-04"
---

# RFC-006: ProjectSession State and File Explorer / Editor Basics — Acceptance / QA Checklist

## Acceptance Status

Closeout evidence is recorded in `qa-evidence.md`. This checklist is prepared for review as **accepted with documented limitations**: core state, root policy, explorer, text document, save/conflict handling, ProjectSession integration, shell-visible evidence, and security guardrails are covered by automated tests; the real GUI editor/tree widget, watcher, overwrite-confirmation UI, and multi-document behavior remain deferred.

## RFC Acceptance Criteria Traceability

- User can open a project and browse files: covered by `qa-evidence.md`.
- User can open, edit, and save a text file under root: covered by `qa-evidence.md`.
- Dirty state is visible: covered by `qa-evidence.md`.
- External modification is detected: covered by `qa-evidence.md`.
- Symlink escape does not silently open outside-root content: covered by `qa-evidence.md`.
- Content Mode does not introduce split panes: covered by `qa-evidence.md`.

For each criterion above, QA must record one of:

- automated test name;
- manual scenario evidence;
- release note limitation;
- RFC amendment reference.

## Automated Test Checklist

- [x] Unit tests cover normal state transitions for this RFC.
- [x] Unit tests cover invalid inputs and rejected transitions.
- [x] Cross-project isolation tests verify one ProjectSession cannot mutate another ProjectSession accidentally.
- [x] Persistence round-trip tests exist for every new schema-bearing entity. No new schema-bearing persisted entity is introduced; existing recent-project persistence tests remain covered.
- [x] Migration fixture exists if an existing schema changes. No schema migration is introduced.
- [x] Error path tests verify user-correctable errors are not treated as crashes.
- [x] Security policy tests verify Restricted Mode or trust policy behavior where relevant.
- [x] Regression tests cover the highest-risk bug class for this RFC.
- [x] Tests run without requiring a specific private machine setup, credentials, or network access unless the RFC explicitly accepts that dependency.

## RFC-006 Specific Required Evidence

- [x] Root policy records selected relative path, selected absolute path, canonical target, canonical root, symlink status, and containment status.
- [x] Valid UTF-8 file under root opens for editing.
- [x] Invalid UTF-8 file is rejected for editable open.
- [x] NUL-containing/binary-looking file is rejected for editable open.
- [x] File larger than the 4 MiB editable cap shows bounded error or preview-only behavior.
- [x] In-root symlink is labeled and opens only when its canonical target remains inside root.
- [x] Symlink escape is blocked and not followed silently.
- [x] Save-time root containment is revalidated.
- [x] Saving through an unsafe symlink/platform edge case fails conservatively.
- [x] Save failure preserves the in-memory buffer and keeps dirty state.
- [x] Shared state/outcome vocabulary covers clean, dirty, external changed, conflict, save error, saved, blocked external change, blocked root escape, blocked unsafe symlink, and write failed outcomes.
- [x] External clean-file modification reloads or prompts according to documented behavior. Core enters an external-changed prompt state; GUI reload choice is deferred.
- [x] External dirty-file modification enters conflict state and does not overwrite silently.
- [x] Any overwrite confirmation for an externally changed file identifies the project, target file, and external-change condition. No overwrite confirmation UI is implemented; save is blocked instead.
- [x] If overwrite confirmation UI is absent, save is blocked and the dirty buffer is preserved.
- [x] Save behavior does not claim full preservation of permissions, ownership, or extended attributes; uncertain cases return recoverable save errors.
- [x] Dirty state updates the owning ProjectSession and Project Board/runtime summary without overclaiming full safe-close coverage.
- [x] Content Mode remains one primary editor/content surface with no arbitrary split panes.
- [x] Restricted Mode allows user-initiated open/edit/save under root.
- [x] Restricted Mode does not start LSP, formatter, scripts, hooks, tasks, plugins, workspace AI profiles/prompts, `.env` loading, network behavior, or process execution.
- [x] Logs, fixtures, review packages, and release evidence do not dump file contents, full project trees, or unnecessary absolute private paths.

## Manual QA Scenarios

- Browse and edit files under root: covered by shell/content tests listed in `qa-evidence.md`.
- Attempt to open symlink outside root and verify prompt/block behavior: covered by root/content tests listed in `qa-evidence.md`.
- Modify open file externally and verify conflict/refresh behavior: covered by content/shell tests listed in `qa-evidence.md`.
- Large text file has bounded loading behavior and does not crash UI: covered by editable cap tests.

Additional manual scenarios:

- [x] Start from a clean profile/config directory and verify first-run behavior. Covered by existing Project Board/recent-project tests; no new content persistence is introduced.
- [x] Restart the app and verify state is either restored or honestly reported as unavailable/interrupted. Runtime content buffers are not persisted; this limitation is documented in `qa-evidence.md`.
- [x] Exercise both keyboard and mouse paths for the primary action. Real GUI input paths are deferred; current shell harness exercises command seams.
- [x] Exercise empty state, loading state, normal state, error state, and recovery/cancel path. Loading UI and cancel UI are deferred; empty/normal/error/recovery states are covered by shell/core tests.
- [x] Verify user-facing text identifies the target project/session/run before any destructive action. RFC-006 does not implement destructive overwrite confirmation; save blocks instead.
- [x] Verify Project Board attention indicators remain correct after the workflow.

## Security and Privacy Checklist

- [x] No network call is introduced unless this RFC explicitly allows it.
- [x] No secret is written to config, audit log, release evidence, or test fixture.
- [x] User-initiated open, edit, and save under the project root work in Restricted Mode.
- [x] Workspace-local automation is blocked or warned in Restricted Mode, including LSP, formatter, scripts, hooks, tasks, plugins, workspace AI profiles/prompts, and `.env` loading.
- [x] Destructive actions require explicit user intent and identify consequences. RFC-006 avoids destructive overwrite confirmation and blocks conflicted saves.
- [x] Security-relevant file access decisions route through an audit seam, or an explicit no-op/audit-pending note is recorded if RFC-012 is not yet implemented.
- [x] Ordinary user file opens/saves are not broadly audit-logged before RFC-012 policy exists.
- [x] Managed/Supervised/Plain labels are shown honestly where terminal/AI CLI behavior is involved. RFC-006 does not add terminal/AI CLI behavior.
- [x] Symlink/path traversal behavior is tested where file access is involved.
- [x] Transcript or private output retention/purge behavior is tested where output capture is involved. RFC-006 does not add transcript/output capture.

## Performance and Reliability Checklist

- [x] UI remains responsive during the primary workflow. Current shell/core harness uses bounded operations; GUI responsiveness is deferred.
- [x] Long-running operations are asynchronous or bounded. Explorer scan and editable file size are bounded.
- [x] Large output, large file lists, or large diffs are bounded and show truncation/safe summary where required.
- [x] A failure in one project/session/run does not make other projects unusable.
- [x] App close/restart behavior is honest for running or interrupted work. RFC-006 does not add running process behavior; runtime content state is not persisted.
- [x] Temporary-file cleanup is verified where save behavior creates temporary files.

## UX Checklist

- [x] The user can tell what project is active.
- [x] The user can tell what state the workflow is in.
- [x] The user can recover from error or cancel without data loss. Error states preserve active/dirty buffers; cancel UI is deferred.
- [x] The UI avoids false confidence about unsupported capabilities.
- [x] Focus handling is unambiguous for keyboard input. Real GUI focus handling is deferred; command seams are explicit.
- [x] Labels and status badges match the current UI/UX baseline specification. Current labels are shell evidence labels, not final GUI copy.

Shell-rendered labels are acceptance evidence for core state and workflow visibility; final GUI wording/layout remains owned by later GUI implementation work.

## Release Evidence Required

Attach or link the following evidence before marking this RFC implemented:

- [x] Commit/PR list.
- [x] Test command output.
- [x] Manual QA notes.
- [x] Screenshots or text transcript for user-visible flows.
- [x] Security impact note.
- [x] Migration note or "no migration" statement.
- [x] Known limitations.
- [x] Follow-up RFCs/issues for deferred work.

## Risk-Specific QA

- Text editing widget complexity may expand. Mitigation: keep MVP editor minimal and prioritize AgentRun review workflows.

## Open Questions Before Implementation Completion

- None currently. RFC-006 selects a UTF-8 `String`-backed buffer behind a replacement-friendly internal boundary.

If any open question affects user-visible behavior, implementation cannot be marked complete until the question is answered, explicitly deferred, or converted into a documented assumption.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [x] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
See qa-evidence.md. Current closeout recommendation is accepted with documented limitations: the core/root/explorer/editor/save/conflict state is implemented and tested; the real GUI editor/tree widget, watcher, overwrite-confirmation UI, multi-document behavior, and durable audit persistence are deferred.
```
