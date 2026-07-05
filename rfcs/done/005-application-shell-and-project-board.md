# RFC-005: Application Shell and Project Board

Status: Implemented  
Target milestone: M2  
Date: 2026-07-04

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-appendix-a-extensibility-plugin-v0.md`
- `tekstide-roadmap-milestones-v0.md`

Depends on:

- [RFC-001](./001-product-scope-mvp-and-non-goals.md)
- [RFC-002](./002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-003](./003-information-architecture-and-ui-mode-model.md)
- [RFC-004](./004-security-baseline-and-restricted-mode.md)

These links reflect the current four-folder RFC policy.

## Summary

This RFC defines the first implemented product shell for Tekstide `v0.1.0`: one desktop window with a global Project Board capable of adding, displaying, switching, and removing local ProjectSessions.

## Motivation

The Project Board is Tekstide's key differentiator over a single-project editor/terminal. It must be built before deep editor or terminal features so the entire application state model remains multi-project from the start.

## Goals

- Implement the outer application shell.
- Implement Project Board navigation.
- Add/open/remove local projects.
- Display project attention state.
- Implement minimal local recent-project persistence.
- Provide placeholders for terminals, AgentRuns, approvals, and review-ready changes.
- Preserve Linux-first implementation while keeping OS-specific shell, path, file-dialog, and config/state behavior behind seams.

## Non-Goals

- Implement full editor.
- Implement full terminal.
- Implement AI CLI launch.
- Implement plugin management.
- Implement remote/container projects.
- Implement multi-window support.
- Implement release-quality persistence beyond the recent-project seam.

## Detailed Design

## 1. Application shell

The MVP shell contains:

- top-level window;
- global command routing;
- Project Board surface;
- Active Project Workspace placeholder;
- global status strip;
- modal/dialog layer for add-project and warning flows.

The shell must be structured around `ProjectSession` ownership from RFC-002. It must not use an implicit global "current project" for state mutation.

The Linux MVP default Project Board layout is a compact row layout. Card layout is deferred unless implementation cost is negligible and it shares the same view model.

## 2. Project adding flow

Flow:

1. User invokes Add Project.
2. Tekstide opens a folder selection dialog or accepts a path from CLI.
3. Tekstide validates and canonicalizes the root path.
4. Tekstide creates a ProjectSession with trust state `Restricted`, unless a valid prior trust decision restores a different state according to RFC-004.
5. Tekstide records or queues audit event `project_added`. If RFC-012 storage is not available, the event goes through an audit seam/no-op collector.
6. Project appears on Project Board.

Trust-state transition:

```text
Path selected
  -> canonicalization pending: internal/transient Unknown
  -> new root with no prior trust decision: Restricted
  -> root with valid prior trust decision: restored trust state, subject to RFC-004
  -> canonicalization/root validation failure: no ProjectSession created
```

`Unknown` is allowed only as an internal/transient state before root validation and trust lookup complete. A newly added root with no valid prior trust decision must become `Restricted` before it is shown as an active ProjectSession.

Root validation requirements apply equally to dialog and CLI path input:

- path must exist;
- path must be a directory;
- path must be canonicalizable or explicitly rejected;
- non-directory file paths are rejected for RFC-005 unless a future open-file flow is defined;
- permission errors show a user-visible error and do not create ProjectSession;
- duplicate detection uses canonical root identity;
- symlink ambiguity fails closed unless the implementation provides a confirmation dialog that shows both the selected path and canonical target. Confirmation creates a ProjectSession for the canonical target only.

Duplicate handling:

- If the canonical path is already open, focus the existing ProjectSession.
- If path moved or canonicalization fails, show an error without creating a session.
- If a symlink or path normalization issue would make root identity ambiguous, fail closed or ask for explicit user action; do not silently create a conflicting ProjectSession.

## 3. Project card/row fields

Required MVP fields:

- project display name;
- short root path;
- trust badge;
- branch/status placeholder;
- terminal count;
- AgentRun count;
- pending approvals count;
- review count;
- last activity;
- attention badge.

Project rows/cards must use text labels or icon+label states; color alone is insufficient.

Counts for features not implemented by RFC-005 must distinguish real data from unavailable data:

```text
KnownCount(n)
Unavailable
NotImplemented
Unknown
```

Terminal, AgentRun, approval, and review counts may be omitted, dimmed as `--`, labeled `not available`, or fixture-backed in development/test builds. They must not be presented as meaningful runtime zeroes until the backing feature exists.

Branch/status is a placeholder in RFC-005. It must not invoke `git`, shell commands, hooks, LSPs, plugins, scripts, workspace AI profile loaders, or network operations. Until the Git/status RFC lands, the value is `Unavailable`, `NotImplemented`, omitted, or fixture-backed only in development/test builds.

## 4. Attention calculation

Priority order:

1. Risk/security warning.
2. Pending approval.
3. Review-ready generated changes.
4. Failed process/AgentRun.
5. Running AgentRun/process.
6. Dirty files.
7. Calm.

Attention calculation should be pure/testable domain or view-model logic, not embedded directly in widget callbacks.

## 5. Project switching

Switching active project:

- does not stop background terminals;
- does not cancel AgentRuns;
- restores the last open surface for that project, or a safe placeholder if the surface is not yet implemented;
- updates global keybinding context;
- records last opened timestamp.

Until terminal and AgentRun implementations land, placeholder counts and states may be fixture-backed in development/test builds, but production UI must use the count display model above and preserve the data shape without presenting unavailable features as meaningful zeroes.

Timestamp semantics:

- `last_opened_at` updates when the project is added or selected/opened from the Project Board.
- `last_activity` updates on meaningful project events such as attention-state changes, AgentRun lifecycle changes, process state changes, review-ready changes, dirty-file changes, or explicit user interaction.
- In RFC-005, `last_activity` may equal `last_opened_at` until later RFCs provide richer activity sources.

Use UTC timestamps internally. UI formatting may be local.

## 6. Removal/close project

Removing a ProjectSession from the board requires checking running processes and dirty files.

If either exists, open a project-specific safe-close dialog or route through a safe-close policy seam. If the full RFC-017 safe-close implementation is not available yet, the shell must fail safely by cancelling close or showing a not-yet-supported warning rather than silently discarding active state.

RFC-005 defines this interim close seam:

```text
CloseAssessment =
  | SafeToClose
  | NeedsConfirmation { reasons }
  | UnsupportedOrUnknown { reason }
```

Before RFC-017:

- normal recent-project removal is allowed only if the shell can prove there are no active resources owned by that ProjectSession;
- if placeholder/test process state indicates running work, removal must route through `CloseAssessment` and block or warn;
- if the implementation cannot evaluate a risk, it must not silently discard state.

## Data Model and Persistence Impact

RFC-005 implements minimal local recent-project persistence only.

Persisted fields:

```text
RecentProject {
  state_version,
  project_id,
  display_name,
  root_path,
  canonical_root_path,
  last_opened_at,
  last_trust_state_summary
}
```

`last_trust_state_summary` is display-only cached metadata. It must not grant trust. A project may restore a non-Restricted trust state only from the authoritative trust-decision mechanism defined by RFC-004. If that mechanism is unavailable, the project is restored as `Restricted`.

It does not persist:

- full ProjectSession runtime state;
- terminal state;
- AgentRun state;
- transcript state;
- dirty editor buffers;
- plugin state.

`ProjectId` is a generated stable UUID assigned when a project is first added and persisted in `RecentProject`. Canonical root path is used for duplicate detection, not as the sole ProjectId. If a path is removed and re-added after the recent-project entry is deleted, a new ProjectId may be assigned.

Linux MVP stores recent-project state as versioned JSON under `$XDG_STATE_HOME/tekstide/recent-projects.json`, or `~/.local/state/tekstide/recent-projects.json` if `XDG_STATE_HOME` is unset. This path is wrapped behind an app-state path provider so RFC-016 can revise platform policy or migration format later.

Corrupt or missing recent-project state must not crash startup. If practical, preserve or rename corrupt state for debugging and show an empty Project Board.

Persistence beyond this minimal recent-project store belongs to RFC-016. This RFC must not force a final storage backend or database migration framework.

## User Experience Impact

Project Board must be calm. Avoid live terminal output or scrolling logs. Show counts and attention states only.

The default Project Board shortcut is configurable. `Ctrl+Shift+P` remains reserved for the Command Palette, and `Ctrl+Alt+P` is the Linux MVP candidate for opening the Project Board.

First-run empty state:

```text
No projects yet.
[Add Project] [Open from path]
```

Recent projects whose directories no longer exist should show a stale/missing badge and allow removal from the recent list. Tekstide must not auto-delete stale recent entries without user action and must not create a ProjectSession until the path validates again.

MVP user-visible labels:

- `Permission denied`: Tekstide cannot access the selected folder.
- `Folder missing`: a recent-project folder no longer exists.
- `Cannot read folder`: the folder exists but cannot be read as a project root.
- `Path changed`: selected path resolves differently than expected and needs confirmation or re-add.
- `Remove from recent`: remove a stale recent-project entry without touching filesystem contents.

Display-state matrix:

| Condition | Board state | Allowed action |
| --- | --- | --- |
| Valid readable root | Project row with trust badge | Open/switch/remove |
| Permission denied | `Permission denied` | Retry, choose another folder, or remove from recent |
| Folder missing | `Folder missing` | Browse to new location or remove from recent |
| Unreadable folder | `Cannot read folder` | Retry after permission fix or remove from recent |
| Canonical target differs unexpectedly | `Path changed` | Confirm canonical target or cancel |

MVP sort order:

1. highest attention priority;
2. row kind, with active ProjectSessions before restored recent rows;
3. display name;
4. ProjectId.

This deterministic sort order is accepted for the RFC-005 CLI-harness implementation. Timestamp-aware ordering by recent activity and recent open is deferred until the GUI layer consumes persisted timestamp fields for release-quality Project Board UX polish.

Recommended view-model boundary:

```text
ProjectBoardViewModel {
  rows,
  active_project_id,
  empty_state,
  global_attention_summary
}
```

## Security and Privacy Impact

All newly added projects use transient `Unknown` only during validation and become `Restricted` before display unless a valid prior trust decision applies. Project Board must show trust state. No project-specific automation runs when adding a project.

Adding a project must not:

- auto-load `.env`;
- auto-start LSP;
- auto-load workspace plugins;
- auto-load workspace AI profiles or prompts;
- run project scripts or hooks;
- contact network services by Tekstide itself.

Project Board rendering must not invoke `git`, shell commands, LSPs, hooks, scripts, workspace plugins, workspace AI profile loaders, or network services to populate placeholder fields.

Recent-project state is local-only. It may contain sensitive project names and filesystem paths. Tekstide must not send this data to network services, and errors/logs should avoid dumping the full recent-project file unless the user intentionally exports diagnostics.

## Test Plan

- Unit tests for attention calculation.
- Unit/integration tests for add/switch/remove project.
- Path canonicalization and duplicate-root tests.
- Restricted-by-default tests for newly added projects.
- Tests for minimal recent-project persistence, restart restore, stale-path display, and corrupt/missing state recovery.
- UI/view-model tests that status is text-labeled and not color-only.

## Acceptance Criteria

- User can add at least five projects.
- User can switch between projects.
- Project Board shows trust state and placeholder counts.
- Duplicate project roots are handled gracefully.
- Newly added projects appear as `Restricted` after root validation unless a valid prior trust decision applies.
- Adding a project does not execute workspace-local automation.
- Removing a project with running placeholder/process state routes through safe-close policy or fails safely until safe close is implemented.
- Project Board route/keybinding does not consume `Ctrl+Shift+P`.
- Restarting Tekstide restores the recent-project list.
- Corrupt or missing recent-project state does not crash startup.
- Placeholder counts are not displayed as meaningful zeroes for unimplemented features.

## Risks and Mitigations

- Starting with placeholders may feel incomplete. Mitigation: explicitly label unavailable counts/features until their RFCs land.
- Early shell code may accidentally become single-project. Mitigation: require ProjectSession ownership and multi-project tests from the first shell slice.
- Safe-close dependencies may not be implemented yet. Mitigation: use a conservative close policy seam and never silently discard active process/dirty state.

## Open Questions

None for M2 acceptance. RFC-016 may later revise broader platform policy, storage backend, or migration format.

## Implementation Handoff Checklist

- Implement Project Board before terminal/editor details.
- Build project state as multi-project from first commit.
- Keep add-project flow Restricted-by-default.
- Keep attention calculation pure/testable.
- Use placeholders for later RFC-owned counts without implying those features are implemented.
- Implement compact row layout for Linux MVP.
- Implement minimal versioned recent-project persistence.
- Implement stable persisted ProjectId.
- Implement `CloseAssessment` seam for project removal.
