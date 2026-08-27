---
title: "RFC-044: Surface-Local Keyboard Affordances — implementation handoff"
rfc: "RFC-044"
rfc_file: "../../accepted/044-surface-local-keyboard-affordances.md"
source_rfc_status: "Accepted 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Two keyboard systems, one accountable

Source RFC: [RFC-044](../../accepted/044-surface-local-keyboard-affordances.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-044](../../accepted/044-surface-local-keyboard-affordances.md) | **Read "Decided on acceptance" first.** D1–D4 are settled, and the scope widened after it was proposed |
| 2 | [`what-advertising-keys-must-not-become.md`](./what-advertising-keys-must-not-become.md) | **Required.** The failure mode here is making surfaces worse while technically fixing them |
| 3 | `crates/tekstide/src/keyboard_help.rs` | `control_coverage` is the shape you are mirroring. Read it before designing a new one |
| 4 | [RFC-040](../../done/040-affordance-completion.md) | Did this in the other direction, for global actions |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Three slices, inventory first |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |

## The one-sentence version

Fourteen global bindings have a registry, a coverage test and generated help; roughly twenty-nine
surface-local keys have none of it — and at least one action has no keyboard route at all.

## What is already true, so you do not re-derive it

- **`advertised_bindings()` reads `KeybindingPolicy.rules`** — the global, `Ctrl`-prefixed
  bindings, all fourteen of them. The Help modal and `--help` are generated from it.
- **Eight surface-local handlers** — `handle_explorer_key`, `handle_editor_key`,
  `handle_approval_history_key`, `handle_change_review_key`, `handle_trust_settings_key`,
  `handle_project_board_row_key`, `handle_tab_strip_key`,
  `handle_project_board_path_field_key` — match roughly twenty-nine keys between them, in no
  registry.
- **`control_coverage(action: NavigationAction)`** is exhaustive and asks *"how does a mouse reach
  this?"*. `ControlCoverage` has `VisibleControl` and `KeyboardOnly { reason }`. **No `MouseOnly`.**
- **`CloseProjectTabPressed` has exactly one emitter**, the `×` button at `shell.rs:6084`.
  `FocusZone` is three variants, so `Tab` cycles zones, not widgets. Verified by the `0.15.0`
  release gate.

## The trap, stated once

**`control_coverage`'s domain is `NavigationAction`, and closing a project is not one.** The
exhaustive match that was supposed to guarantee affordance coverage could not see the control in
either direction, which is why four separate reviews have each had to find one of these by hand.

**If your registry inherits that domain, it inherits the blind spot.** D1's widening — to *surface
actions* — is the substance of the decision, not a detail of it.

## Traps this codebase has already set

- **Do not add bare keys to `KeybindingPolicy`.** `matching_global_action` compares a rendered
  binding string against `default_binding`; a bare `Enter` there becomes a global action and
  shadows every surface that handles `Enter` itself.
- **Do not enforce with a source scan.** RFC-042's first guard was one and the reviewer defeated it
  by respelling the same construct. If dispatch-through-registry proves impractical, say so in
  writing.
- **Every global binding requires `Ctrl`** — that is what makes a bare `a`/`r` reach a surface
  handler at all (RFC-034's own check). If you add a global binding without a modifier, you break
  every surface-local key silently.
- **Modal exclusivity is structural, not per-handler.** `RoutedInput::Surface` cannot be
  constructed without a `ModalAbsent` proof. You do not need a guard; do not add one.

## Live GUI evidence

Required for the access slice, and against a **`mktemp -d` fixture project with a fresh
`XDG_STATE_HOME`** — `ARCHITECTURE.md`'s rule, and the fresh state root is what keeps the Project
Board from rendering a real recent-projects list.

The walkthrough must show **closing a project entirely by keyboard**, since that is the defect
that widened this RFC's scope. State whether a real mouse click was sent, either way.

## Deferrals to state, not to solve

- Rebindable keys or a keymap file. RFC-023 owns configuration and is closed.
- Screen-reader support. `iced` has no accessibility bridge; unchanged, and out of scope for that
  reason and no other.
- The README keyboard-table check, which becomes possible once D1's registry exists and is a
  natural follow-up rather than part of this.
