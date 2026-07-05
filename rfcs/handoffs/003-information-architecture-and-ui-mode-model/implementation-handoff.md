# RFC-003 Implementation Handoff

Source RFC: [RFC-003](../../done/003-information-architecture-and-ui-mode-model.md)
Target: information architecture and navigation model

## Purpose

Make the Project Board and per-project dual-mode workspace the foundation for app shell implementation.

## Decisions To Preserve

- Project Board sits above active project workspace.
- Only one ProjectSession is active in the main workspace at a time.
- Background projects continue running and surface attention globally.
- Content Mode has one primary content surface in MVP.
- Terminal / Agent Immersion Mode shows at most two visible panes.
- AgentRun Detail and Diff Review are content surfaces.
- `Ctrl+Shift+P` is reserved for Command Palette.
- Project Board shortcut is configurable; Linux MVP candidate is `Ctrl+Alt+P`.
- `Ctrl+Shift+Esc` is not the Project Board default.

## Implementation Guidance

- Build navigation state separately from GUI widget callbacks.
- Add placeholder surfaces before full editor, terminal, diff, or AgentRun implementations.
- Treat keybindings as configurable candidates, not hard-coded cross-platform truth.
- Keep attention states text-labeled and not color-only.

## Stop Conditions

Pause for RFC amendment if implementation needs arbitrary editor splits, arbitrary terminal pane counts, plugin UI slots, multi-window support, or a Project Board shortcut that conflicts with platform conventions.
