---
title: "Terminal launch UX: implementation handoff"
owning_rfcs: "RFC-008 (lifecycle), RFC-015 (input/shell), RFC-017 (surface)"
status: "Implemented 2026-08-08 (da09009) — screenshot evidence pending, review requested"
created: "2026-08-08"
---

# Making the terminal reachable

## Why this slice exists

RFC-017 delivered a real, filtered, PTY-backed terminal and **no user can open one.** Every part of it is behind `TEKSTIDE_TERMINAL_DEMO`. The cycle review (`rfcs/delivery-plan.md`, 2026-08-08) measured 45 commits since `0.4.1` with zero releasable user-visible surface, and the owner approved closing that gap before RFC-018.

`rfcs/future-work.md` §Terminal / PTY Runtime has listed *"Add app/UI commands for launching, selecting, and closing terminals"* for some time. This is that item, scoped.

## I under-estimated this, and the correction matters for your planning

I told the owner this was *"plausibly one PR against RFC-008's existing lifecycle API — wiring a command to reviewed code rather than new runtime work."* **That was wrong**, and I found out by checking rather than by assuming. Two things are missing that the demo path never needed:

**1. `terminal_session_limit` is not enforced.** `ProjectSession::add_terminal_session` (`project/session.rs:239`) checks project membership and duplicate id, and nothing else. `ResourceLimits::default()` sets `terminal_session_limit: None`. The demo path launches exactly three sessions and stops, so this never mattered. A keybinding a user can hold down is a different thing: today it would spawn unbounded real shell processes.

**2. Nothing detects a shell exiting.** No `transition_terminal_status` call exists anywhere in `crates/tekstide/src`. `session_bar.rs` can *render* `TerminalStatus::Exited` but nothing ever sets it. So a user who types `exit` gets a dead PTY that still reports **"Running"** in trusted chrome, holds its visible slot forever, and keeps being polled.

Those two compound: without exit detection the limit becomes a dead end a user cannot escape without restarting the app, and with no limit there is no bound at all. **Both are in scope.** Do not treat this as a wiring task.

## Scope

### A — the launch command

`NavigationAction` has `CycleVisibleTerminalSession` but **no variant for opening a terminal**. `AppCommand` has four variants and none is terminal-related. So this slice adds:

- a `NavigationAction` variant for launching a terminal in the active project;
- the matching `AppCommand`;
- a `KeybindingRule` in `KeybindingPolicy::linux_mvp()`;
- the `app_command_for` mapping, and the `ApplicationShell::dispatch` arm.

**Pick the binding and say why.** `Ctrl+Alt+P` and `Ctrl+Alt+M` are taken as `Candidate`; `Ctrl+Shift+P` is `Reserved` for the command palette. Follow the existing `Ctrl+Alt+<letter>` shape unless you have a reason not to. **Do not silently collide** with a `Reserved` binding — `KeybindingStatus` exists to make that checkable, so check it mechanically rather than by reading.

Launching must switch the project into `TerminalImmersion` if it is not already there, or the user presses a key and nothing appears to happen.

### B — enforce `terminal_session_limit`

Enforce it in **`tekstide-core`**, in `add_terminal_session`, not at the call site. A limit enforced by the caller is a limit the next caller forgets.

Give `ResourceLimits::default()` a real bound rather than `None`. State the number you chose and the reasoning; it should be generous enough not to annoy and small enough to bound real processes. Refusal must be a typed error the shell can render, not a panic and not a silent no-op — **the user pressed a key and is owed a visible answer.**

### C — exit detection

The machinery exists in core (`termination.rs`: `wait_for_exit`, `try_child_outcome`, `TerminationOutcome`, `outcome_from_exit_status`). It is simply not called from the GUI.

Wire it into the existing poll path — `Message::TerminalDemoTick`'s handler already visits every pane every tick, which is where a non-blocking exit check belongs. On detecting exit:

- transition the session to `TerminalStatus::Exited` so the session bar stops lying;
- free the visible slot so another terminal can take it;
- stop polling that pane's PTY.

**This also closes a PR-017-F known limitation.** `plain_terminal_observation` could only ever emit `Started`, because *"process-exit detection isn't wired into the plain-terminal `poll()` loop"*. Once it is, emit the `Terminated` observation too — it is already valid in the frozen v1 schema (`valid_plain_terminal` accepts `Started | Failed | Terminated`, requiring a `reason_code` for the non-`Started` cases). **No schema amendment.**

**Use a non-blocking check.** `read_available_bounded_for`'s 10 ms `WouldBlock` sleep already blocks `iced`'s update thread (`rfcs/future-work.md` §Readiness-driven terminal I/O); do not add a second blocking call to the same handler. If the only available check blocks, stop and raise it rather than shipping it.

### D — the README privacy claim, for the third time

`README.md` §Local Data and Privacy currently says the audit store is created **only** under `TEKSTIDE_TERMINAL_DEMO`. **This slice makes that false**, because a user launching a terminal is a real producer call.

This exact claim has now been wrong twice and corrected twice — once when PR-017-F added the producer, once when the store was opened unconditionally. **Fix it in the same change**, not afterwards: say plainly that opening a terminal creates the audit database, where it lives, what it holds, and how to purge it.

### E — one ingress, not two

`state.terminal_demo` and `launch_terminal_demo_panes` exist for the env-gated demo. A real launch path must not become a **second** way to create and register a terminal.

**Fold the demo into the real path**: the demo becomes a caller that launches N terminals through the same function a keybinding calls. If that is not straightforward, say why rather than duplicating — a second construction path for PTY-backed sessions is the shape RFC-017 PR-017-B/C spent two slices proving *absent*.

The panes themselves (emulator grids) are legitimately shell-local. The **session list** is `tekstide-core`'s, and PR-017-C's contract — no shell state duplicating core — still holds.

## Explicitly out of scope

- **Terminating a running terminal from the UI.** With C in place, a user closes a terminal by typing `exit`, which is a real working path. The confirmation dialog and consequence text are `future-work.md`'s separate items and need RFC-022's dialog model.
- **Selecting or cycling between panes.** `CycleVisibleTerminalSession` exists as an action with no binding; PR-017-E scoped input to the `Primary` slot. Leave both as they are and say so, rather than half-wiring selection.
- **Anything touching the 10 ms sleep or the 64 KiB cap.** Those are one coupled change, recorded in `future-work.md`, and they belong to readiness-driven I/O.

## Review gate

- **A user can open a terminal, type in it, and see output** — screenshot, with the keystrokes real and individually dispatched, stating what it proves and does not.
- **The session limit is enforced in core and demonstrated**, including what the user sees on refusal. Ablate it.
- **Exit detection demonstrated**: type `exit`, session bar shows `Exited`, slot is freed and reusable by a new launch. Ablate it — a test that passes with the detection removed is the failure mode this project has hit repeatedly.
- **`Terminated` audit observation produced**, conforming to the frozen family, with the `reason_code` the schema requires for non-`Started` outcomes.
- **One creation path** — the demo and the keybinding go through the same function, shown by enumeration rather than asserted.
- **README privacy section updated in this commit**, not a follow-up.
- No new blocking call in the tick handler.
- Gates: `fmt`, `clippy -D warnings`, full test suite, `git diff --check`.

## What this unblocks

`0.5.0` becomes releasable with a claim that is actually true: a user can open a real, security-filtered terminal in a project. And RFC-018's adversarial evidence stops inheriting a developer env var — spoofing resistance proven against a terminal a user opened is materially stronger than against one only a flag can reach.

## Outcome, 2026-08-08

Implemented in `da09009`. Against the review gate above:

- **A user can open a terminal, type in it, and see output.** Real, tested end to end (`launch_terminal_shell_input_switches_to_terminal_immersion_and_launches_a_real_session`): a `Ctrl+Alt+T` `ShellInput` dispatched through the real `update` switches to `TerminalImmersion`, launches exactly one real pane rooted in the **actual project directory** (not a scratch temp dir, unlike the diagnostic demo/measurement paths), and the session is already `Running`/`Primary` by the time it returns — not left at `Starting` the way every launch path did before this handoff. **The screenshot itself is not yet taken** — a real GUI launch and synthetic `xdotool` input, which this session's standing convention asks to be confirmed before running rather than assumed carried over from RFC-017 PR-017-G's separate authorization. Raised in the accompanying review request.
- **The session limit is enforced in core and demonstrated, with what the user sees on refusal.** `ProjectSession::add_terminal_session` refuses at `terminal_session_limit` (`Some(3)`, was `None`) with `ProjectTerminalError::SessionLimitExceeded { limit }`; `terminal_session_limit_is_enforced_end_to_end_with_a_visible_notice` runs 3 real launches to the limit, confirms the 4th typed refusal, and confirms the catalog-driven notice text (`terminal-launch-refused`, `en.ftl`) names the real number. Ablated (`ablation_a_fourth_real_process_would_spawn_without_the_limit_check`): a 4th real shell spawns just fine at the OS level, confirming the application-level check is the only thing that stops it.
  - **Revised 2026-08-08 per review response 163**: the initial default (`Some(8)`) was reasoned only against process count. `Message::TerminalPollTick` polls every live pane sequentially, and each `poll()` carries `read_available_bounded_for`'s hardcoded 10ms `WouldBlock` sleep -- measured (response 163) linear at ~10.1ms/pane against the 50ms tick period, saturating at 5 panes. Revised to `Some(3)` (~30.2ms/tick, ~20ms headroom), reasoned against tick budget rather than process count, with that reasoning recorded on `ProjectResourceLimits::default` itself so it is revisited deliberately, not left arbitrary, once readiness-driven terminal I/O removes the per-poll sleep.
- **Exit detection demonstrated, ablated.** `a_real_session_exit_updates_status_frees_the_slot_and_is_reusable`: writes a real `exit\n` to a real pane, drives the real `TerminalPollTick` handler until the session reports `Exited`, confirms the slot is freed to `Hidden`, and confirms a subsequent launch reuses `Primary`. Ablated (`ablation_without_check_exit_a_dead_shell_still_reports_running`): with only `poll()` called (the pre-handoff behaviour), a shell that has genuinely exited at the OS level still reports `Running` — the exact lie the review gate named.
- **`Terminated` audit observation produced, conforming to the frozen family.** `AuditCoordinator::record_plain_terminal_terminated`, three new `tekstide-core` tests proving the real mapping (`Exited` → `ProcessExited`, a signal → `ProcessTerminated`, `Failed`/`OrphanedUnknown` → `NotRequired`, matching `ManagedProcessLifecycle`'s own established precedent rather than inventing a new one) and `record.validate()` against a real store. No schema amendment.
- **One creation path, shown by enumeration.** `terminal_pane_launch_has_exactly_two_named_production_callers` walks `shell.rs` for every `TerminalPane::launch(` call site and its enclosing function by name: `launch_terminal` (the one ingress `launch_terminal_demo_panes` and the real keybinding path both call) and `launch_measurement_terminal_pane` (deliberately separate, PR-017-G's own already-reviewed non-contamination reasoning) — any other count or name fails the test.
- **README privacy section updated in the same commit**, along with the feature-status paragraph and keybinding table that would otherwise have gone stale alongside it (found while fixing the one the gate named).
- **No new blocking call in the tick handler.** `TerminalPane::check_exit` is `wait_for_exit(handle, Duration::ZERO)`, traced against its own retry loop (documented on the method itself) to confirm it degrades to one non-blocking `try_wait()` rather than assumed.
- **Gates**: `fmt`, `clippy -D warnings` (workspace), full test suite (502 `tekstide-core` + 126 `tekstide`, 0 failures — 3 + 6 net new respectively), `git diff --check`. All passed.

**Not yet done**: the screenshot itself, and the non-contamination/GUI-only items that were never this handoff's to begin with. Everything else in the review gate is real, tested, and ablated where the gate asked for ablation.
