---
title: "RFC-015: Application Shell and Rendered Surface Model - Acceptance / QA Checklist"
rfc: "RFC-015"
rfc_file: "../../proposed/015-application-shell-and-rendered-surface-model.md"
status: "Proposed — implementation pending"
target_milestone: "M8"
source_rfc_status: "Proposed"
created: "2026-07-29"
updated: "2026-07-29"
---

# RFC-015 Acceptance / QA Checklist

**A checked box means evidence exists, not that the result was favourable.** A requirement that could not be met stays unchecked with a stated reason in `qa-evidence.md`.

## Architecture Checklist

- [ ] `crates/tekstide` is a real GUI application; the text harness is replaced.
- [ ] `crates/tekstide-core` gains **no GUI dependency**.
- [ ] Shell renders `shell.state()` and dispatches `AppCommand`.
- [ ] **No shell-local state mirrors core state.**
- [ ] Keybindings come from `KeybindingPolicy::linux_mvp()`, not invented.

## Layer Model Checklist

- [ ] Chrome / content / modal layers composed via `stack`/`opaque`.
- [ ] Surface code cannot open, populate, or dismiss a modal.
- [ ] Surface code cannot render trusted chrome.
- [ ] Enforcement is module privacy, not convention.

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

- [ ] No hardcoded user-facing strings; all through the i18n lookup.
- [ ] No hardcoded colours or font sizes; all through `Theme`.
- [ ] Seam enforcement is mechanical where practical; otherwise the limitation is recorded.
- [ ] English default and compiled theme default work without RFC-016/RFC-023.

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
