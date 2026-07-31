---
title: "RFC-015: Application Shell and Rendered Surface Model - Acceptance / QA Checklist"
rfc: "RFC-015"
rfc_file: "../../proposed/015-application-shell-and-rendered-surface-model.md"
status: "Proposed — implementation in progress (PR-015-B, PR-015-C accepted [responses 128-131]; PR-015-D landed 2026-07-31, not yet reviewed)"
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-07-31"
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

- [ ] Content ↔ Terminal switching works.
- [ ] **No animation or interpolation** (`NFR-UX-005`).
- [ ] Terminal sessions and AgentRuns unaffected by mode switching.

## Measurement Checklist (R1 discharge)

- [ ] Instrumentation built into the shell behind a flag.
- [ ] **Non-contamination proven** by idle-CPU comparison.
- [ ] Release builds only; no debug figures recorded.
- [ ] ≥1,000 samples; p50/p95/p99 reported.
- [ ] Machine identification recorded.
- [ ] Latency described as **app-internal**, not end-to-end.
- [ ] Delivery-loss rates reported; survivorship-bias caveat applied.
- [ ] `NFR-PERF-001` warm start ≤ 800 ms.
- [ ] `NFR-PERF-002` mode switch p95 ≤ 32 ms.
- [ ] `NFR-PERF-003` typing p95 ≤ 16 ms.
- [ ] If still undischargeable: decomposed measurement plus explicit re-recording of the residual.

## Accessibility Checklist

- [ ] Visible focus indicators on every focusable element.
- [ ] Focus indication does not rely on colour alone (`NFR-UX-002`).
- [ ] Every shell workflow keyboard-reachable (`NFR-UX-001`).
- [ ] **No partial or simulated screen-reader affordance** implying support that does not exist.
- [ ] Screen-reader absence stated in evidence.

## Evidence Required

- [ ] Commit/PR list.
- [ ] Gate command output.
- [ ] Compile-fail check results for the input-class privacy properties.
- [ ] Focus-trap test results.
- [ ] `CountDisplay` fidelity test results.
- [ ] Latency tables with methodology and idle-CPU comparison.
- [ ] Screenshots of both modes.
- [ ] Known limitations.
- [ ] Answers to the RFC's open questions.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Blocked — input routing cannot be made structural.

Reviewer notes:

```text
Pending implementation.
```
