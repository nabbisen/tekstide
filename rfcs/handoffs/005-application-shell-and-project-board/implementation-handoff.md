# RFC-005 Implementation Handoff

Source RFC: [RFC-005](../../done/005-application-shell-and-project-board.md)
Target: first buildable application shell and Project Board slice

## Purpose

Implement Tekstide's first real product surface: a Linux-first one-window application shell with a global Project Board and multi-project state from the start.

## Required Tasks

1. Create application shell skeleton.
2. Create ProjectSession collection owned by app state.
3. Implement compact-row Project Board view model.
4. Implement first-run empty Project Board state.
5. Implement add-project dialog and CLI path flow with identical validation semantics.
6. Implement root validation and canonical duplicate detection.
7. Assign stable persisted `ProjectId` values.
8. Set new projects to `Restricted` after validation unless a valid prior trust decision applies.
9. Implement minimal versioned recent-project persistence.
10. Implement corrupt/missing recent-project recovery.
11. Implement text-labeled trust and attention states.
12. Implement placeholder-count display model: `KnownCount(n)`, `Unavailable`, `NotImplemented`, `Unknown`.
13. Implement pure/testable attention calculation.
14. Implement switch-project behavior.
15. Implement remove-project with `CloseAssessment`.
16. Add tests and release evidence.

## Decisions To Preserve

- Project Board is the global control plane.
- App state is multi-project from the first implementation slice.
- The UI must not present unavailable terminal/AgentRun/review/approval counts as meaningful zeroes.
- Adding a project must not run workspace automation.
- `Unknown` trust is internal/transient only; operational new projects are `Restricted` unless valid prior trust is restored.
- Recent-project state is local-only and may contain sensitive paths.
- `Ctrl+Shift+P` remains reserved for the Command Palette.
- `Ctrl+Alt+P` is the Linux MVP candidate for opening the Project Board.
- `last_trust_state_summary` is display-only cached metadata and must not grant trust.
- Authoritative trust restoration belongs to the RFC-004 trust-decision mechanism. If that mechanism is unavailable, restore projects as `Restricted`.
- Do not implement Git status probing or branch detection in RFC-005. Preserve the branch/status field shape only.

## Data Boundaries

Persist only minimal recent-project data:

- `state_version`
- `project_id`
- `display_name`
- `root_path`
- `canonical_root_path`
- `last_opened_at`
- `last_trust_state_summary`

Linux MVP stores this as versioned JSON under `$XDG_STATE_HOME/tekstide/recent-projects.json`, or `~/.local/state/tekstide/recent-projects.json` if `XDG_STATE_HOME` is unset. Access must go through an app-state path provider.

Do not persist:

- full ProjectSession runtime state;
- terminal state;
- AgentRun state;
- transcript state;
- dirty editor buffers;
- plugin state.

## CloseAssessment

Use this interim close seam before RFC-017:

```text
CloseAssessment =
  | SafeToClose
  | NeedsConfirmation { reasons }
  | UnsupportedOrUnknown { reason }
```

If risk cannot be evaluated, do not silently remove active state.

## Display States

Use these MVP labels for stale/error board states:

| Condition | User-visible label | Allowed action |
| --- | --- | --- |
| Selected root cannot be accessed | `Permission denied` | Retry, choose another folder, or cancel |
| Recent-project root no longer exists | `Folder missing` | Browse to new location or `Remove from recent` |
| Root exists but cannot be read | `Cannot read folder` | Retry after permission fix or `Remove from recent` |
| Canonical target differs unexpectedly | `Path changed` | Confirm canonical target or cancel |

`Remove from recent` removes only the local recent-project entry. It must not delete filesystem contents.

## Stop Conditions

Pause for RFC amendment if implementation needs Git/status probing, full editor, PTY terminal, AI CLI launch, plugin management, remote/container projects, multi-window support, final storage backend/migrations, or unsafe removal of unknown active resources.
