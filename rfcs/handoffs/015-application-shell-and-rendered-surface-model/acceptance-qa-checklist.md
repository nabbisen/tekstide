---
title: "RFC-015: Application Shell and Rendered Surface Model - Acceptance / QA Checklist"
rfc: "RFC-015"
rfc_file: "../../proposed/015-application-shell-and-rendered-surface-model.md"
status: "Proposed — implementation in progress (PR-015-B landed 2026-07-31, not yet reviewed)"
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
- [ ] Shell renders `shell.state()` and dispatches `AppCommand`. Renders `shell.state()`/`shell.route()` (chrome only, this slice). **Dispatches nothing yet** — there is no interactivity in PR-015-B (`Message` is uninhabited); `AppCommand` dispatch begins with real input in PR-015-C. Left unchecked rather than claimed early.
- [x] **No shell-local state mirrors core state.** `shell::State` holds one `ApplicationShell` plus purely presentational fields (`catalog`, `theme`, `layer_composition_demo_modal_open`) — recorded by inspection per `implementation-handoff.md` §2's own examples, not mechanically checked.
- [ ] Keybindings come from `KeybindingPolicy::linux_mvp()`, not invented. Not applicable yet — this slice has no keybindings at all (no input routing until PR-015-C). Left unchecked rather than claimed vacuously true.

## Layer Model Checklist

- [x] Chrome / content / modal layers composed via `stack`/`opaque`. Screenshot: `evidence/pr-015-b/layer-composition-demo-modal-above-content.png`.
- [ ] Surface code cannot open, populate, or dismiss a modal. Not applicable yet — no surface code exists until PR-015-D. Left unchecked rather than claimed vacuously true.
- [ ] Surface code cannot render trusted chrome. Same as above — not applicable yet.
- [x] Enforcement is module privacy, not convention (for what exists in this slice). `layer_composition_demo_modal_open`'s setter is private to `shell.rs`; nothing outside it can open the modal. The real enforcement claim (surface code specifically cannot reach the modal layer) awaits PR-015-D's surface module.

## Input Routing Checklist (PR-015-C)

- [ ] `ShellInput`, `SurfaceInput`, `TextStream` exist as distinct types.
- [ ] **`TextStream` unconstructible outside the terminal surface module** — compile-fail check.
- [ ] `ShellInput` unconstructible by surfaces.
- [ ] Modal active ⇒ surface and text-stream input **not produced** (not produced-then-ignored).
- [ ] No post-dismissal delivery of keystrokes typed while a modal was open.
- [ ] Global keybindings not capturable by any surface, including a focused terminal.
- [ ] Stale or cross-project `TerminalId` dropped, not best-effort delivered.
- [ ] Focus returns to the invoking element on modal dismissal.
- [ ] **Guard-deletion resistance**: removing a guard is a compile error, not a permissive path.
- [ ] **Real focus-trap test** — discharges RFC-014 R6.

## Seam Checklist

- [x] No hardcoded user-facing strings; all through the i18n lookup. Mechanically checked: `shell_view_source_contains_no_raw_string_literal_passed_to_text`, ablation-verified.
- [x] No hardcoded colours or font sizes; all through `Theme`. Mechanically checked: `shell_view_source_contains_no_raw_color_construction`, `shell_view_source_contains_no_raw_font_size_literal`, both ablation-verified.
- [x] Seam enforcement is mechanical where practical; otherwise the limitation is recorded. Heuristic source-text scans, not a full parse — recorded as a limitation in `qa-evidence.md`, not claimed as a complete guarantee.
- [x] English default and compiled theme default work without RFC-016/RFC-023. Screenshot: `evidence/pr-015-b/shell-chrome-over-real-state.png`; `theme::tests` cover the compiled `Theme` default directly.

## Project Board Checklist

- [ ] `Surface` contract defined and implemented by the Project Board.
- [ ] Rows render name, branch, trust, terminal count, AgentRun state, pending approvals, last activity.
- [ ] **`CountDisplay` fidelity: `Unavailable`/`NotImplemented` never render as `0`.** Test required.

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
