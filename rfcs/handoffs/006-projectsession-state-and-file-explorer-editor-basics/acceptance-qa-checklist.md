---
title: "RFC-006: ProjectSession State and File Explorer / Editor Basics — Acceptance / QA Checklist"
rfc: "RFC-006"
rfc_file: "../../proposed/006-projectsession-state-and-file-explorer-editor-basics.md"
status: "Proposed developer handoff - do not implement before RFC acceptance"
target_milestone: "M3"
source_rfc_status: "Proposed"
created: "2026-07-04"
---

# RFC-006: ProjectSession State and File Explorer / Editor Basics — Acceptance / QA Checklist

## Acceptance Status

This checklist is complete only when every required item is checked, skipped items have an explicit rationale, and release evidence links to automated and manual results.

## RFC Acceptance Criteria Traceability

- User can open a project and browse files.
- User can open, edit, and save a text file under root.
- Dirty state is visible.
- External modification is detected.
- Symlink escape does not silently open outside-root content.
- Content Mode does not introduce split panes.

For each criterion above, QA must record one of:

- automated test name;
- manual scenario evidence;
- release note limitation;
- RFC amendment reference.

## Automated Test Checklist

- [ ] Unit tests cover normal state transitions for this RFC.
- [ ] Unit tests cover invalid inputs and rejected transitions.
- [ ] Cross-project isolation tests verify one ProjectSession cannot mutate another ProjectSession accidentally.
- [ ] Persistence round-trip tests exist for every new schema-bearing entity.
- [ ] Migration fixture exists if an existing schema changes.
- [ ] Error path tests verify user-correctable errors are not treated as crashes.
- [ ] Security policy tests verify Restricted Mode or trust policy behavior where relevant.
- [ ] Regression tests cover the highest-risk bug class for this RFC.
- [ ] Tests run without requiring a specific private machine setup, credentials, or network access unless the RFC explicitly accepts that dependency.

## RFC-006 Specific Required Evidence

- [ ] Root policy records selected relative path, selected absolute path, canonical target, canonical root, symlink status, and containment status.
- [ ] Valid UTF-8 file under root opens for editing.
- [ ] Invalid UTF-8 file is rejected for editable open.
- [ ] NUL-containing/binary-looking file is rejected for editable open.
- [ ] File larger than the 4 MiB editable cap shows bounded error or preview-only behavior.
- [ ] In-root symlink is labeled and opens only when its canonical target remains inside root.
- [ ] Symlink escape is blocked and not followed silently.
- [ ] Save-time root containment is revalidated.
- [ ] Saving through an unsafe symlink/platform edge case fails conservatively.
- [ ] Save failure preserves the in-memory buffer and keeps dirty state.
- [ ] Shared state/outcome vocabulary covers clean, dirty, external changed, conflict, save error, saved, blocked external change, blocked root escape, blocked unsafe symlink, and write failed outcomes.
- [ ] External clean-file modification reloads or prompts according to documented behavior.
- [ ] External dirty-file modification enters conflict state and does not overwrite silently.
- [ ] Any overwrite confirmation for an externally changed file identifies the project, target file, and external-change condition.
- [ ] If overwrite confirmation UI is absent, save is blocked and the dirty buffer is preserved.
- [ ] Save behavior does not claim full preservation of permissions, ownership, or extended attributes; uncertain cases return recoverable save errors.
- [ ] Dirty state updates the owning ProjectSession and Project Board/runtime summary without overclaiming full safe-close coverage.
- [ ] Content Mode remains one primary editor/content surface with no arbitrary split panes.
- [ ] Restricted Mode allows user-initiated open/edit/save under root.
- [ ] Restricted Mode does not start LSP, formatter, scripts, hooks, tasks, plugins, workspace AI profiles/prompts, `.env` loading, network behavior, or process execution.
- [ ] Logs, fixtures, review packages, and release evidence do not dump file contents, full project trees, or unnecessary absolute private paths.

## Manual QA Scenarios

- Browse and edit files under root.
- Attempt to open symlink outside root and verify prompt/block behavior.
- Modify open file externally and verify conflict/refresh behavior.
- Large text file has bounded loading behavior and does not crash UI.

Additional manual scenarios:

- [ ] Start from a clean profile/config directory and verify first-run behavior.
- [ ] Restart the app and verify state is either restored or honestly reported as unavailable/interrupted.
- [ ] Exercise both keyboard and mouse paths for the primary action.
- [ ] Exercise empty state, loading state, normal state, error state, and recovery/cancel path.
- [ ] Verify user-facing text identifies the target project/session/run before any destructive action.
- [ ] Verify Project Board attention indicators remain correct after the workflow.

## Security and Privacy Checklist

- [ ] No network call is introduced unless this RFC explicitly allows it.
- [ ] No secret is written to config, audit log, release evidence, or test fixture.
- [ ] User-initiated open, edit, and save under the project root work in Restricted Mode.
- [ ] Workspace-local automation is blocked or warned in Restricted Mode, including LSP, formatter, scripts, hooks, tasks, plugins, workspace AI profiles/prompts, and `.env` loading.
- [ ] Destructive actions require explicit user intent and identify consequences.
- [ ] Security-relevant file access decisions route through an audit seam, or an explicit no-op/audit-pending note is recorded if RFC-012 is not yet implemented.
- [ ] Ordinary user file opens/saves are not broadly audit-logged before RFC-012 policy exists.
- [ ] Managed/Supervised/Plain labels are shown honestly where terminal/AI CLI behavior is involved.
- [ ] Symlink/path traversal behavior is tested where file access is involved.
- [ ] Transcript or private output retention/purge behavior is tested where output capture is involved.

## Performance and Reliability Checklist

- [ ] UI remains responsive during the primary workflow.
- [ ] Long-running operations are asynchronous or bounded.
- [ ] Large output, large file lists, or large diffs are bounded and show truncation/safe summary where required.
- [ ] A failure in one project/session/run does not make other projects unusable.
- [ ] App close/restart behavior is honest for running or interrupted work.
- [ ] Temporary-file cleanup is verified where save behavior creates temporary files.

## UX Checklist

- [ ] The user can tell what project is active.
- [ ] The user can tell what state the workflow is in.
- [ ] The user can recover from error or cancel without data loss.
- [ ] The UI avoids false confidence about unsupported capabilities.
- [ ] Focus handling is unambiguous for keyboard input.
- [ ] Labels and status badges match the current UI/UX baseline specification.

## Release Evidence Required

Attach or link the following evidence before marking this RFC implemented:

- [ ] Commit/PR list.
- [ ] Test command output.
- [ ] Manual QA notes.
- [ ] Screenshots or text transcript for user-visible flows.
- [ ] Security impact note.
- [ ] Migration note or "no migration" statement.
- [ ] Known limitations.
- [ ] Follow-up RFCs/issues for deferred work.

## Risk-Specific QA

- Text editing widget complexity may expand. Mitigation: keep MVP editor minimal and prioritize AgentRun review workflows.

## Open Questions Before Implementation Completion

- None currently. RFC-006 selects a UTF-8 `String`-backed buffer behind a replacement-friendly internal boundary.

If any open question affects user-visible behavior, implementation cannot be marked complete until the question is answered, explicitly deferred, or converted into a documented assumption.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [ ] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text

```
