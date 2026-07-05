# RFC-003 Acceptance / QA Checklist

Source RFC: [RFC-003](../../done/003-information-architecture-and-ui-mode-model.md)

## Current Status

RFC-003 is closed for the foundation stage as a committed navigation/mode policy and state baseline. The full desktop GUI and real terminal/editor/AgentRun surfaces remain deferred.

Implemented evidence comes from:

- `577815d RFC-003 navigation/mode slice`

## UI Model Checks

| Check | Status | Evidence / note |
| --- | --- | --- |
| Project Board is reachable as the global control plane. | Satisfied | `ApplicationShell` starts on `AppRoute::ProjectBoard`; `OpenProjectBoard` returns to it. Covered by `shell_starts_on_project_board_route` and `command_router_changes_top_level_route`. |
| Active project workspace has Content Mode and Terminal / Agent Immersion Mode. | Satisfied at state/model level | `ProjectMode::{Content, TerminalImmersion}` is per-`ProjectSession`; shell commands toggle active project mode. Covered by `active_project_workspace_toggles_between_content_and_terminal_modes`. |
| Content Mode does not expose arbitrary editor splits. | Satisfied by absence / placeholder scope | `ProjectOpenSurface` is one primary surface enum; no arbitrary split model exists. Opening content surfaces forces `ProjectMode::Content`. Covered by `opening_content_surface_returns_to_content_mode_without_losing_active_project`. |
| Terminal / Agent Immersion Mode is limited to two visible panes. | Satisfied at policy/display level | `TerminalPanePolicy` caps visible panes at two for wide and narrow layouts; shell rendering uses that policy. Covered by `terminal_immersion_policy_limits_visible_panes_to_two` and `active_workspace_visible_panes_are_capped_by_navigation_policy`. |
| Background project attention can be surfaced globally. | Satisfied for Project Board read model | Project Board derives a global attention summary and sorts rows by attention. Covered by `attention_calculation_follows_priority_order`, `view_model_uses_runtime_summary_for_known_counts_and_attention`, and `rows_sort_by_attention_then_active_recent_status_then_name`. |
| Keybindings are configurable. | Satisfied as policy data | `KeybindingPolicy::linux_mvp` marks primary workflow bindings as `Configurable` unless reserved/candidate. Covered by `primary_navigation_workflows_have_keyboard_policy_entries`. Real keyboard event handling is deferred. |
| `Ctrl+Shift+P` is reserved for Command Palette. | Satisfied | `KeybindingPolicy::linux_mvp` reserves `Ctrl+Shift+P` for `OpenCommandPalette` and avoids `Ctrl+Shift+Esc`. Covered by `linux_mvp_keybinding_policy_reserves_command_palette_and_avoids_shift_escape`. |

## Evidence Required

| Evidence | Status | Notes |
| --- | --- | --- |
| Navigation state transition tests or equivalent design evidence. | Satisfied | Shell tests cover Project Board route, active workspace route requiring an active project, mode toggle, content surface opening, and no-active-project no-ops. |
| Wide/narrow layout notes for terminal mode. | Satisfied at policy level | `TerminalLayoutClass::{Wide, Narrow}` and `TerminalPanePolicy` currently share a two-visible-pane cap. Real responsive terminal layout is deferred until terminal surfaces exist. |
| Keyboard access notes. | Satisfied as policy data | `NavigationAction` enumerates primary workflows; `KeybindingPolicy::linux_mvp` reserves/candidates/configures bindings. Real keyboard event handling and user keybinding persistence are deferred. |
| UX/security note confirming no terminal output can masquerade as global approval UI. | Satisfied for current no-terminal scope | No terminal rendering exists yet, so terminal output cannot currently masquerade as approval UI. Carry-forward requirement: future terminal/AgentRun surfaces must keep approvals project-labeled and outside raw terminal output. |

## Gate Evidence

Observed after the RFC-003 navigation/mode slice:

```text
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

Latest observed test result:

```text
110 passed; 0 failed
```

Targeted no-execution/probing scan over RFC-003 touched files returned no matches for process launch, command execution, network clients, transcript byte writes, or PTY APIs.

The checklist closeout update is documentation-only; Rust gates were not rerun after this wording update.

## Closeout Decision

The current navigation/mode foundation is sufficient to close RFC-003 for the foundation stage. No extra placeholder/read-model slice is required before moving RFC-003 to `done/`.

## Deferred Future Work / Carry-Forward Requirements

- Future GUI implementation must use `KeybindingPolicy`/`NavigationAction` rather than hard-coding platform-specific shortcuts directly in widgets.
- Future terminal surface work must preserve the two-visible-pane MVP cap through `TerminalPanePolicy` or a reviewed successor.
- Future AgentRun, approval, and diff surfaces must be project-scoped and must not display another project's approval state without explicit project labeling.
