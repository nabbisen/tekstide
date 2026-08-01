---
title: "RFC-015: Application Shell and Rendered Surface Model - Acceptance / QA Checklist"
rfc: "RFC-015"
rfc_file: "../../proposed/015-application-shell-and-rendered-surface-model.md"
status: "Proposed — PR-015-B/C/D/F/G accepted (0.4.0 released); PR-015-E (mode switching, focus indicator, C4) implemented 2026-08-01, pending review. RFC-015 stays in `rfcs/proposed/` until PR-015-E and NFR-PERF-002 are accepted and the architect performs the rfcs/done/ transition."
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-08-01"
---

# RFC-015 Acceptance / QA Checklist

**A checked box means evidence exists, not that the result was favourable.** A requirement that could not be met stays unchecked with a stated reason in `qa-evidence.md`.

## Architecture Checklist

- [x] `crates/tekstide` is a real GUI application; the text harness is replaced. `iced::application(boot, shell::update, shell::view)`, `main.rs`'s `print!("{}", shell.render_text())` removed.
- [x] `crates/tekstide-core` gains **no GUI dependency**. `cargo tree -p tekstide-core --edges normal | grep -i iced` returns nothing.
- [x] Shell renders `shell.state()` and dispatches `AppCommand`. Renders (PR-015-B) plus dispatches for real now (PR-015-C): `app_command_for(NavigationAction::OpenProjectBoard) -> AppCommand::OpenProjectBoard`, proven by `a_project_board_shell_input_dispatches_the_real_app_command` (route genuinely changes). Honestly partial: every other `NavigationAction` maps to `None` today — no default binding exists for most (RFC-023), and `OpenCommandPalette` has a binding but no feature to dispatch to yet. Not a placeholder; a true reflection of what's real.
- [x] **No shell-local state mirrors core state.** `shell::State` holds one `ApplicationShell` plus purely presentational fields (`catalog`, `theme`, `focus`, `modal`) — recorded by inspection per `implementation-handoff.md` §2's own examples, not mechanically checked.
- [x] Keybindings come from `KeybindingPolicy::linux_mvp()`, not invented. `matching_global_action` compares against `policy.rules`' real `default_binding` strings only; no binding is hardcoded independent of the policy. Proven for both bindings the policy currently ships (`a_global_keybinding_wins_over_a_focused_terminal`, `the_command_palette_binding_routes_to_the_shell`).

## Layer Model Checklist

- [x] Chrome / content / modal layers composed via `stack`/`opaque`. Screenshot: `evidence/pr-015-b/layer-composition-demo-modal-above-content.png`.
- [x] Surface code cannot open, populate, or dismiss a modal. `surface::board` (the first and only surface) has no path to `shell::State.modal` at all — it is never passed in; `board::view`'s signature is `(&ProjectBoardViewModel, &Catalog, &Theme) -> Element`. By construction, not a runtime check.
- [x] Surface code cannot render trusted chrome. `board::view`'s return value only ever fills `shell::content_area`'s content slot; it has no path to `top_bar`/`status_bar`.
- [x] Enforcement is module privacy, not convention. `state.modal`'s setter is private to `shell.rs`; `surface::board` is never given a `&mut State` or anything reaching `modal` — the same "cannot construct/cannot reach" shape as PR-015-C's input-class privacy, applied to the layer boundary.

## Input Routing Checklist (PR-015-C)

- [x] `ShellInput`, `SurfaceInput`, `TextStream` exist as distinct types. `crates/tekstide/src/input.rs`, `input/terminal_surface.rs`.
- [x] **`TextStream` unconstructible outside the terminal surface module** — compile-fail check. Probed from `shell.rs`: `error[E0624]: associated function 'from_terminal_key' is private`.
- [x] `ShellInput` unconstructible by surfaces. Probed from `shell.rs`: `error[E0603]: tuple struct constructor 'ShellInput' is private`.
- [x] Modal active ⇒ surface and text-stream input **not produced** (not produced-then-ignored). `shell::subscription` calls a structurally different function (`modal_subscription`) that has no path to constructing `SurfaceInput`/`TextStream` — not a runtime check on produced values.
- [ ] No post-dismissal delivery of keystrokes typed while a modal was open. Not directly tested — no queuing mechanism exists in this slice's subscription design to begin with (each `subscription()` call is stateless per invocation), so there is nothing to queue and no in-flight keystroke to test draining. Recorded as "the design has no queue" rather than "tested and found absent."
- [x] Global keybindings not capturable by any surface, including a focused terminal. `a_global_keybinding_wins_over_a_focused_terminal` — `Ctrl+Alt+P` with a terminal nominally focused still routes to `ShellInput`.
- [x] Stale or cross-project `TerminalId` dropped, not best-effort delivered. `terminal_stream_targets_a_live_terminal`; negative path proven (`a_never_added_terminal_id_is_not_live`, `with_no_active_project_no_terminal_id_is_ever_live`). **Positive path ("a genuinely live terminal is accepted") is a disclosed testability gap** — see `qa-evidence.md` — `tekstide-core::AppState::project_mut` is `#[cfg(test)]`-gated to `tekstide-core`'s own test build, unreachable from `tekstide`'s tests, with zero production call sites yet (RFC-017's job).
- [x] Focus returns to the invoking element on modal dismissal. `dismissing_the_modal_clears_it_and_leaves_shell_focus_undisturbed` — falls out for free since `state.focus` is never touched while a modal is shown (proven by the focus-trap test), so nothing needs restoring.
- [x] **Guard-deletion resistance**: removing a guard is a compile error, not a permissive path. Probed directly: calling `non_modal_subscription(input::ModalAbsent(()), ...)` from `shell.rs`, bypassing `ModalAbsent::check` entirely, fails with `error[E0603]: tuple struct constructor 'ModalAbsent' is private`.
- [x] **Real focus-trap test** — discharges RFC-014 R6. `modal_focus_cycling_never_touches_the_shell_focus_cycle`: dispatches real `Message`s through `update`, asserts the modal's own focus cycles while `state.focus` never moves.

## Seam Checklist

- [x] No hardcoded user-facing strings; all through the i18n lookup. Mechanically checked, crate-tree-wide (response 128 Required): `no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate`, ablation-verified twice (once against `shell.rs`, once against `main.rs` after the fix broadened coverage to it).
- [x] No hardcoded colours or font sizes; all through `Theme`. Mechanically checked, crate-tree-wide: `no_raw_color_construction_anywhere_in_the_crate`, `no_raw_font_size_literal_anywhere_in_the_crate`, both ablation-verified.
- [x] Seam enforcement is mechanical where practical; otherwise the limitation is recorded. Heuristic source-text scans, not a full parse — recorded as a limitation in `qa-evidence.md`, not claimed as a complete guarantee. Scans walk `crates/tekstide/src` recursively rather than naming one file, so new source files are covered automatically.
- [x] English default and compiled theme default work without RFC-016/RFC-023. Screenshot: `evidence/pr-015-b/shell-chrome-over-real-state.png`; `theme::tests` cover the compiled `Theme` default directly.

## Project Board Checklist

- [x] `Surface` contract defined and implemented by the Project Board. Defined as concrete methods (`surface.rs`'s module doc), not a `trait Surface` — deliberate, with exactly one implementor; recorded as a decision, not an oversight.
- [x] Rows render name, branch, trust, terminal count, AgentRun state, pending approvals, last activity. `surface/board.rs::row_lines`; name/root path escaped via `text_safety`, branch/terminal/agent-run/approval/review/dirty-file counts via the catalog. "Last activity" is not a field `ProjectBoardRow` exposes — not rendered, not fabricated.
- [x] **`CountDisplay` fidelity: `Unavailable`/`NotImplemented` never render as `0`.** `unavailable_and_not_implemented_never_render_as_zero` (negative path) + `a_genuine_known_zero_count_does_render_as_zero` (positive path, so the rule is proven to discriminate real zero from fake). `CountDisplay::label()` never called — mechanically enforced (`no_count_display_or_attention_label_is_called_anywhere_in_the_crate`), both ablation-verified.

## Mode Switching Checklist

- [x] Content ↔ Terminal switching works. `NavigationAction::ToggleProjectMode` now has a real default binding (`Ctrl+Alt+M`, `tekstide-core::navigation`) dispatching `AppCommand::ToggleActiveProjectMode`; `a_toggle_project_mode_shell_input_dispatches_the_real_app_command` proves the real `ProjectMode` flips, and `evidence/pr-015-e/01-terminal-mode.png`/`02-content-mode.png` show the two real, distinct rendered states from a genuine keypress.
- [x] **No animation or interpolation** (`NFR-UX-005`). Confirmed by inspection: `AppCommand::ToggleActiveProjectMode`'s dispatch is a synchronous `tekstide-core` state mutation with no `iced::Task` delay, tween, or interpolation anywhere in the path.
- [x] Terminal sessions and AgentRuns unaffected by mode switching. Still a structural argument, not an observed one — no terminal surface exists yet (RFC-017) for the switch to disturb — stated as such rather than checked on an absence it cannot yet demonstrate.

## Measurement Checklist (R1 discharge)

- [x] Instrumentation built into the shell behind a flag. `crates/tekstide/src/measurement.rs`, `TEKSTIDE_MEASURE_CRITERION`/`TEKSTIDE_MEASURE_LOG`/`TEKSTIDE_MEASURE_TARGET`; `Measurement::from_env()` returns `None` unless explicitly set.
- [x] **Non-contamination proven** by idle-CPU comparison. 0 ticks/3s with the flag unset; 3 ticks/3s (~1%) with `Typing` active but idle — the disclosed, flag-gated `MeasurementTick` overhead, never present in a normal run.
- [x] Release builds only; no debug figures recorded. All figures from `cargo build --release`.
- [x] ≥1,000 samples; p50/p95/p99 reported. Typing: 1,000 input-to-state-change samples, 1,475 view-build-cost samples (both post-warmup-discard).
- [x] Machine identification recorded. Same machine as RFC-014's own measurements — see `qa-evidence.md`.
- [x] Latency described as **app-internal**, not end-to-end. Stated explicitly, and carried one level further: app-internal here means this app's own `update`/`view` functions specifically, not `iced`'s render pipeline.
- [x] Delivery-loss rates reported; survivorship-bias caveat applied. 0.00% loss (1,100 dispatched, 1,100 confirmed); R9 caveat recorded as moot for this run specifically (no dropped-sample population to bias), not omitted.
- [x] `NFR-PERF-001` warm start ≤ 800 ms. 14 warm samples: median 163.8ms. **Met, comfortably.**
- [x] `NFR-PERF-002` mode switch p95 ≤ 32 ms. PR-015-E: decomposed (input-to-state-change p95 29µs + view-build-cost p95 39µs = 68µs total), reusing PR-015-F's harness (`Criterion::ModeSwitch`) rather than a new mechanism. 1,100 dispatched, 1,100 confirmed (0.00% loss); idle-CPU re-proven for this criterion specifically (2 ticks/3s vs. 0 default). **Met by roughly 470×.**
- [x] `NFR-PERF-003` typing p95 ≤ 16 ms. Decomposed (input-to-state-change p95 42µs + view-build-cost p95 131µs = 173µs total) rather than a combined `frames()`-based figure — real, non-degenerate, **met by roughly two orders of magnitude.**
- [x] If still undischargeable: decomposed measurement plus explicit re-recording of the residual. Not needed here (both figures are real and well within budget), but the decomposition itself is the mechanism this bullet asks for, applied preemptively rather than only as a fallback after a degenerate result.

## Accessibility Checklist

- [x] Visible focus indicators on every focusable element. **Met by PR-015-E.** `Theme::border_focused` reinstated; `zone_style` changes border colour *and* width together, `focus_marker` adds a textual `"> "`/`"  "` channel (matching the modal's own convention) — three independent signals, not colour alone. Both `FocusZone` variants (`MainArea`, `Sidebar`) now render distinctly; `evidence/pr-015-e/02-content-mode.png` → `03-sidebar-focused.png` → `04-mainarea-refocused.png` (byte-identical to `02`) demonstrate a real two-zone cycle, not an assertion.
- [x] Focus indication does not rely on colour alone (`NFR-UX-002`). The modal's `"> "` marker and, since PR-015-E, `zone_style`'s border-width channel and `focus_marker`'s text are all non-colour signals paired with the colour change, not a replacement for checking it — `zone_style_changes_both_border_colour_and_width_when_focused` proves both channels move together.
- [x] Every shell workflow keyboard-reachable (`NFR-UX-001`). True for this slice's actual workflow surface: `FocusZone` cycling (trivial, one zone), modal focus-cycle/activate/dismiss, and `ShellInput` navigation actions are all keyboard-driven with no mouse-only path. Caveat: the Project Board's rows have no actions yet to reach either way (PR-015-D is presentational), so the surface this claim covers is still small.
- [x] **No partial or simulated screen-reader affordance** implying support that does not exist. Nothing simulated exists — `iced` has no accessibility tree wired in this build at all (RFC-014 R2, owner-accepted), so there is no partial affordance to misrepresent.
- [x] Screen-reader absence stated in evidence. `qa-evidence.md`'s Known Limitations: "Screen-reader support absent for the life of the `iced` substrate decision (RFC-014 R2, owner-accepted)."

## Evidence Required

- [x] Commit/PR list. PR-015-B: `773b410`, fix `cf047f0`. PR-015-C: `ac8b7be`, evidence `4a5af92`, fix `5b652e6`. PR-015-D: `0d0793d`, fix `7021c70`. PR-015-F: `6b06882`, fix `e834470`. Sequencing correction (architect): `7d203d3`.
- [x] Gate command output. Recorded per-slice in `qa-evidence.md`'s Implementation Evidence sections — `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features`, `git diff --check`, all passing at each commit.
- [x] Compile-fail check results for the input-class privacy properties. `qa-evidence.md` PR-015-C section; this file's Input Routing Checklist lines 34-35, 41 quote the exact `rustc` error codes (`E0624`, `E0603`).
- [x] Focus-trap test results. `modal_focus_cycling_never_touches_the_shell_focus_cycle` — see R6 disposition above and Input Routing Checklist line 42.
- [x] `CountDisplay` fidelity test results. `unavailable_and_not_implemented_never_render_as_zero` / `a_genuine_known_zero_count_does_render_as_zero` — see Project Board Checklist above.
- [x] Latency tables with methodology and idle-CPU comparison. `qa-evidence.md` PR-015-F section — C5/C2 tables, machine identification, idle-CPU comparison table.
- [x] Screenshots of both modes. `evidence/pr-015-e/01-terminal-mode.png` and `02-content-mode.png` — real `Ctrl+Alt+M` presses producing the two distinct rendered `ProjectMode` states, plus the focus-cycle set (`02` → `03` → `04`, the last byte-identical to `02`).
- [x] Known limitations. `qa-evidence.md`'s "Known Limitations" section (RFC-015-wide) plus each slice's own "Known Limitations (PR-015-X)" subsection.
- [x] Answers to the RFC's open questions. `qa-evidence.md`'s PR-015-G section, item 5.

## Final Acceptance Decision

- [x] **Accepted** — for `0.4.0`'s scope (PR-016-C, PR-015-B/C/D/F/G). Not a closure of RFC-015.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Blocked — input routing cannot be made structural.

Reviewer notes:

```text
PR-015-G accepted, high-capability model (architect), 2026-08-01. Reviewed unprompted:
the commit landed before a review request, and a closeout that gates a release does
not wait for one.

SCOPE OF THIS ACCEPTANCE. It covers 0.4.0's contents only. RFC-015 is NOT closed and
stays in rfcs/proposed/ per RFC-000, because PR-015-E and NFR-PERF-002 remain. The
closeout says so itself rather than needing to be told -- status line, "Date accepted:
Pending", and the deferred checklist lines all agree. That consistency is the thing I
checked first, since the sequencing correction it implements was mine and only landed
the day before.

ALL FOUR REQUIRED ITEMS DELIVERED.

1. What 0.4.0 may claim. The README's Current Status now describes the real shell and,
   more importantly, states what is absent -- no terminal surface, no editor, no mode
   switching, no dialogs -- and preserves command approval's honest framing
   (implemented, unreachable, cooperative rather than enforced). It described a
   pre-GUI state as recently as PR-015-F.

2. R1's disposition, with the caveat attached and not merely appended. The figure is
   stated as "the sum of the two measured streams' p95s, used as an upper-bound proxy",
   the non-pairing of the streams (n=1,000 vs 1,475) is explained rather than asserted,
   and the 688µs max is named rather than hidden behind percentiles. The
   app-internal-not-paint-to-screen boundary is drawn twice, in both the result and the
   limitations. A reader quoting this later has to work to quote it wrongly.

   R6 is discharged by a real test, not the structural argument that sufficed for the
   spike -- modal_focus_cycling_never_touches_the_shell_focus_cycle dispatches real
   Messages through update and asserts state.focus never moves.

   With R1 and R6 closed, the RFC-014 substrate decision record has no open items.

3. Unchecked lines each carry a reason. Seven substantive, and the causal chains are
   traced rather than gestured at -- the focus-indicator line names Theme::border_focused
   being cut in PR-015-B as the cause, which is honest about a decision this reviewer
   endorsed at the time.

4. What 0.4.1 carries: PR-015-E, C4, NFR-PERF-002, and RFC-015's own lifecycle
   transition.

CARRIED TO 0.4.1, NOT BLOCKING 0.4.0.

- Visible focus indicators. Not met at chrome level. Defensible now: FocusZone has one
  variant, chrome is non-interactive, and the modal does render focus visibly (the
  PR-015-C screenshots differ between focused buttons and 1 == 3 proves the cycle).
  It stops being defensible the moment PR-015-E adds a second zone -- a keyboard-driven
  shell where you cannot see what has focus is an accessibility defect, not a cosmetic
  one. Required for 0.4.1.
- Screenshots of both modes. Only Content Mode exists to screenshot.
- Post-dismissal keystroke queuing. No queue exists to drain; a property of the
  subscription shape rather than a tested behaviour.

WHAT REMAINS FOR 0.4.0 IS A RELEASE, NOT A SLICE -- and that is the owner's call.
```
