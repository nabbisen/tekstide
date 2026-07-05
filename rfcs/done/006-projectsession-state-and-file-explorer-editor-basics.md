# RFC-006: ProjectSession State and File Explorer / Editor Basics

Status: Implemented (core/shell baseline; GUI/runtime follow-ups deferred)  
Target milestone: M3  
Date: 2026-07-04

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-appendix-a-extensibility-plugin-v0.md`
- `tekstide-roadmap-milestones-v0.md`

Depends on:

- [RFC-002](./002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-003](./003-information-architecture-and-ui-mode-model.md)
- [RFC-004](./004-security-baseline-and-restricted-mode.md)
- [RFC-005](./005-application-shell-and-project-board.md)

Related scope gate:

- [RFC-001](../proposed/001-product-scope-mvp-and-non-goals.md)

These links reflect the current four-folder RFC policy. RFC-001 remains under `rfcs/proposed/` until implemented, even though review response 002 accepted it for foundation implementation planning.

## Summary

This RFC implements the basic per-project Content Mode: explorer, file open/edit/save, one main content surface, external change detection, and conservative root-bound file access.

## Motivation

Tekstide needs enough editor functionality to review and modify project files, but it should not become a full editor project before terminal/AgentRun workflow exists. This RFC defines the minimal useful editor layer.

## Goals

- Implement ProjectSession active workspace.
- Implement file explorer under project root.
- Implement basic text editor.
- Support open, edit, save.
- Detect external changes.
- Enforce root/symlink policy.
- Preserve the no-split Content Mode rule.

## Non-Goals

- Full IDE language intelligence.
- Complete Vim/Emacs emulation.
- Arbitrary multi-editor splits.
- Binary editing.
- Full search/replace suite.
- Automatic formatter/LSP in Restricted Mode.
- Terminal/PTY behavior.
- AgentRun launch, transcript capture, or diff workflow beyond placeholders.
- Plugin, remote/container, debugger, marketplace, cloud, or multi-window behavior.

## Detailed Design

## 1. Content Mode layout

```text
Explorer | MainSurface
StatusBar
```

MainSurface may be editor, placeholder Git view, AgentRun placeholder, diff placeholder, or trust summary.

## 2. File explorer

Explorer rules:

- root is ProjectSession root;
- ignored/common heavy directories may be collapsed or hidden by default (`target`, `node_modules`, `.git` internals) but user-configurable later;
- symlinks are visibly marked;
- symlinks escaping root are not followed silently;
- file operations are limited to root unless explicit future permission model allows otherwise.

### 2.1. Root-bound access model

All explorer, open, and save operations must resolve through a root-bound access policy before touching file contents.

The implementation should carry an equivalent of:

```text
FileAccessTarget {
  project_id,
  selected_relative_path,
  selected_absolute_path,
  canonical_path,
  root_canonical_path,
  symlink_status,
  containment_status,
}
```

Policy requirements:

- selected paths are interpreted relative to the ProjectSession root unless already represented as an internal resolved target;
- the project root is canonicalized before containment checks;
- the target canonical path must remain inside the canonical project root;
- in-root symlinks may be shown as symlinks and opened only when their canonical target remains inside root;
- escaping symlinks must be shown as blocked or unsafe and must not be followed silently;
- save paths must re-check root containment at save time;
- saving through a symlink must not replace the symlink itself with a regular file or write outside the project root;
- if symlink-safe atomic replacement cannot be guaranteed, the save must fail conservatively rather than risk data loss or root escape.

### 2.2. Explorer scan bounds

Explorer loading must be bounded or lazy so large trees do not freeze the application.

MVP behavior:

- hidden/heavy directories such as `target`, `node_modules`, and `.git` internals may start collapsed;
- unreadable, stale, or blocked nodes are represented as node states, not crashes;
- recursion depth, child count, or elapsed work must be bounded by implementation policy;
- full recursive indexing is not required for RFC-006.

## 3. Editor basics

MVP editor capabilities:

- load UTF-8 text file;
- display line numbers;
- edit text;
- save file;
- dirty indicator;
- external modification warning;
- basic undo/redo if feasible;
- configurable font size/family via global config later.

Syntax highlighting is desirable but lower priority than correctness and state preservation.

### 3.1. Text buffer decision

RFC-006 uses a simple UTF-8 `String`-backed text buffer wrapped behind a small internal `TextDocument` or `TextBuffer` boundary.

The core model must not depend on a GUI editor widget or editor crate. A rope or richer editor engine may replace the internal representation in a later RFC or implementation slice, but RFC-006 should not introduce that complexity.

### 3.2. File type and size policy

MVP editable file policy:

- UTF-8 text only;
- reject editable open when the initial scan finds invalid UTF-8 or NUL bytes;
- default editable file-size cap is 4 MiB;
- larger files show a bounded error or preview-only placeholder instead of crashing;
- file contents must not be dumped into logs, audit records, release evidence, or panic messages.

## 4. External change handling

When a file changes externally:

- if not dirty in Tekstide, reload or prompt based on safe default;
- if dirty, show conflict warning with choices: keep Tekstide version, reload external, compare;
- record non-security audit or activity event if useful.

RFC-006 uses a small shared state vocabulary so explorer, editor, Project Board summaries, and tests do not invent separate ad-hoc strings:

```text
FileBufferState = Clean | Dirty | ExternalChanged | Conflict | SaveError
SaveDecision = Saved | BlockedExternalChange | BlockedRootEscape | BlockedUnsafeSymlink | WriteFailed
```

The implementation may use different Rust names, but it must preserve these states and outcomes.

External-change detection uses a minimal snapshot model:

```text
FileSnapshot {
  canonical_path,
  modified_at,
  len,
  optional_content_hash,
}
```

MVP behavior:

- record a snapshot when a file is opened or saved;
- compare current disk metadata to the snapshot on focus, refresh, and save;
- if disk state is unchanged, save normally;
- if disk state changed and the buffer is dirty, enter a conflict state and do not overwrite silently;
- if disk state changed and the buffer is clean, reload or prompt according to the safest available UI path;
- save-time conflict detection must not depend on a real OS file watcher being reliable.

## 5. ProjectSession state

Per-project state includes:

- selected explorer path;
- open file path;
- cursor position;
- scroll position;
- dirty state;
- current main surface;
- mode.

Dirty state must remain scoped to the owning ProjectSession. It may update Project Board/runtime summaries as a count or attention hint, but it must not imply safe close completeness for files or providers outside RFC-006 coverage.

## 6. Save policy

Concrete MVP save policy:

- save only user-opened UTF-8 text files under the project root;
- before save, revalidate the target path and root containment;
- before save, compare the current disk snapshot with the loaded/saved snapshot;
- if the file changed externally, do not overwrite silently;
- on write error, preserve the in-memory buffer and keep dirty state;
- use temp-file-plus-rename where it is safe for the target path and platform;
- if atomic replacement is unsafe for a symlink or platform edge case, return a recoverable error rather than writing.

RFC-006 may expose a user confirmation for overwriting an externally changed file only if the confirmation identifies the project, target file, and external-change condition. If such UI is not implemented in the current surface, save must be blocked and the dirty buffer preserved.

RFC-006 does not guarantee full preservation of permissions, ownership, or extended attributes beyond normal user-space save semantics. If preservation cannot be reasoned about safely for a target, return a recoverable save error rather than silently weakening root or symlink safety.

## 7. Restricted Mode and automation

User-initiated open, edit, and save operations under the project root are allowed in Restricted Mode.

Restricted Mode still blocks or warns before workspace-local automation:

- LSP startup;
- formatter startup;
- scripts, hooks, and tasks;
- plugin loading;
- workspace AI profiles or prompts;
- environment-file loading such as `.env`;
- network behavior introduced by editor/explorer features.

## 8. Audit and privacy expectations

Ordinary user-initiated open and save operations are not required to become audit events in RFC-006.

Security-relevant file access decisions must route through an audit seam or be documented as audit-pending until RFC-012 provides the durable audit store. Examples include:

- symlink escape blocked;
- root escape blocked;
- save blocked because the target resolved outside root;
- destructive overwrite confirmation.

Errors and UI messages may show relevant file names or project-relative paths. Logs, fixtures, and release evidence must avoid dumping full project trees, absolute private paths where unnecessary, or file contents.

## User Experience Impact

The editor should be intentionally plain. Avoid visual clutter. Main content remains one focused surface.

## Security and Privacy Impact

- Root-bound policy is required.
- Symlink escape must be detected or conservatively blocked.
- Restricted Mode must not auto-run LSP/formatter on file open.
- Large/binary files should not crash the app.

## Test Plan

- File open/save tests.
- Dirty state tests.
- External modification tests.
- Symlink tests.
- Large file guard tests.

## Implementation Closeout

RFC-006 is implemented for the core/shell foundation stage.

Implemented:

- root-bound file access policy;
- bounded explorer read model;
- UTF-8 text document buffer;
- safe save and external-change detection;
- ProjectSession-owned Content Mode workspace;
- shell-visible explorer/text workflow evidence;
- dirty-state propagation to project/runtime summaries;
- one primary content surface without arbitrary splits.

Accepted with documented limitations:

- the current executable remains a CLI/text harness;
- GUI editor/tree widgets are deferred;
- one active text document only;
- file watcher, overwrite confirmation UI, durable audit persistence, and multi-document behavior are deferred;
- no LSP, formatter, task, plugin/profile, process, PTY, network, Git probing, or `.env` behavior is implemented.

Evidence:

- see [RFC-006 QA Evidence](../handoffs/006-projectsession-state-and-file-explorer-editor-basics/qa-evidence.md).

## Acceptance Criteria

- User can open a project and browse files.
- User can open, edit, and save a text file under root.
- Dirty state is visible.
- External modification is detected.
- Symlink escape does not silently open outside-root content.
- Content Mode does not introduce split panes.

## Risks and Mitigations

- Text editing widget complexity may expand. Mitigation: keep MVP editor minimal and prioritize AgentRun review workflows.

## Open Questions

- None for implementation start. RFC-006 selects a UTF-8 `String`-backed buffer behind an internal replacement-friendly boundary.

## Implementation Handoff Checklist

- Implement root policy before explorer recursion.
- Keep editor features minimal until PTY/AgentRun features are underway.
