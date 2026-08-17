---
title: "A default binding for OpenApprovalHistory — the last unreachable built surface"
status: "Scheduled 2026-08-17, awaiting implementation"
rfc_file: "../done/022-adapter-spawn-and-command-approval-surface.md"
target_milestone: "M11"
created: "2026-08-17"
---

# `OpenApprovalHistory` needs a default binding

## Why

RFC-022 built the `ApprovalHistory` surface — the approval queue, the expired-entry
disclosure, the "visibly unanswerable" constraint — and it **cannot be opened by anyone**.
`NavigationAction::OpenApprovalHistory` is `KeybindingStatus::Configurable` with a `None`
binding, and `app_command_for`'s mapping is the only route to
`ProjectOpenSurface::ApprovalHistory`. There is no button, no menu, no default key, and
configuration is RFC-023, which does not exist.

This is the same defect RFC-032 hit on `OpenTrustSettings` and fixed with `Ctrl+Alt+U`
(response 248, commit `12c645d`). **The architect closed RFC-022 as complete without
catching it**; the record is corrected in that RFC's own status and in `rfcs/README.md`
(`cde20af`), and this slice is the remedy that correction points at.

It is also the last one. Of the six `Configurable`/`None` actions, the other five
(`SwitchActiveProject`, `CycleVisibleTerminalSession`, `OpenDiffReview`,
`OpenSafeCloseDialog`, `OpenCommandPalette`) map to **no `AppCommand` at all** — a binding
would do nothing until the command and surface exist. `OpenApprovalHistory` is the only one
where a binding alone makes a built, tested surface reachable.

## Why this is small

Almost everything already exists. RFC-022 PR-022-E built the keyboard handling:

- `handle_approval_history_key` (`crates/tekstide/src/shell.rs:2022`) — already a
  `FocusZone::MainArea` consumer, already has arrow-key highlight movement and Enter
  activation, already guards on `open_surface`.
- `app_command_for` already maps `OpenApprovalHistory` →
  `AppCommand::OpenActiveProjectSurface(ProjectOpenSurface::ApprovalHistory)`.
- `content_mode_view` already has a real `ApprovalHistory` render arm.

**The only missing piece is the default binding.** Contrast with RFC-032, which had to
build the key handler as well.

## What to do

1. **Give `OpenApprovalHistory` a real `Candidate` binding** in
   `crates/tekstide-core/src/navigation.rs`, the same shape the seven working actions use.

   **Candidate: `Ctrl+Alt+H`** (History). Verify rather than assume it is free — the taken
   set today is `Ctrl+Alt+P`/`M`/`T`/`A`/`U`, `Ctrl+Shift+V`, `Ctrl+S`, and `Ctrl+Shift+P`
   is `Reserved` for a command palette that does not exist. **Check the collision
   mechanically against `KeybindingStatus`**, as RFC-032 did with
   `open_trust_settings_shortcut_is_a_candidate_that_collides_with_no_other_rule`, not by
   reading the table. Do not collide with a `Reserved` binding either.

2. **Re-run the surface's existing tests from a real key event.** RFC-032's lesson
   (response 248) was that a chain proof starting from a dispatched `AppCommand` starts one
   step after the step that did not exist. `arrow_keys_move_the_approval_history_highlight`
   already uses `send_main_area_key` for the surface's own keys; the *opening* of the
   surface is what should now come from `Ctrl+Alt+H` through `shell_input_for_test` rather
   than a dispatched command.

3. **Update `README.md`'s keyboard table**, which currently lists seven bindings and would
   otherwise be stale the moment this lands. The same file also says, under the "not yet"
   paragraph, that the approval-history surface "has no key bound to it, so it cannot
   currently be opened" — that sentence must go in the same commit.

## The gate

- The binding is `Candidate`, not `Configurable`, and the collision check is mechanical.
- Opening the surface is proven **from a real key press**, not a dispatched `AppCommand`.
- `README.md`'s keyboard table and its "cannot currently be opened" sentence both updated
  in the same commit as the binding.
- **State what this does and does not make reachable.** It makes the *surface* openable. It
  does **not** make command approval reachable — no shipping AI CLI speaks RFC-021's
  protocol, so `Managed` is still exercisable only by the reference adapter. An empty
  approval history is what a real user will see, and that is correct rather than a bug.
  RFC-022's own status says this; do not let this slice's evidence imply otherwise.

## Not in scope

- The other five `Configurable`/`None` actions. They need commands and surfaces first, and
  binding them would produce keys that silently do nothing — worse than dead.
- RFC-023. This is a default binding, not configuration.
