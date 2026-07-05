# RFC-003 Acceptance / QA Checklist

Source RFC: [RFC-003](../../proposed/003-information-architecture-and-ui-mode-model.md)

## UI Model Checks

- [ ] Project Board is reachable as the global control plane.
- [ ] Active project workspace has Content Mode and Terminal / Agent Immersion Mode.
- [ ] Content Mode does not expose arbitrary editor splits.
- [ ] Terminal / Agent Immersion Mode is limited to two visible panes.
- [ ] Background project attention can be surfaced globally.
- [ ] Keybindings are configurable.
- [ ] `Ctrl+Shift+P` is reserved for Command Palette.

## Evidence Required

- [ ] Navigation state transition tests or equivalent design evidence.
- [ ] Wide/narrow layout notes for terminal mode.
- [ ] Keyboard access notes.
- [ ] UX/security note confirming no terminal output can masquerade as global approval UI.
