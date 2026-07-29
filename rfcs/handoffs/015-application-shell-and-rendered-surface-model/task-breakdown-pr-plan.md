---
title: "RFC-015: Application Shell and Rendered Surface Model - Task Breakdown and PR Plan"
rfc: "RFC-015"
rfc_file: "../../proposed/015-application-shell-and-rendered-surface-model.md"
target_milestone: "M8"
created: "2026-07-29"
updated: "2026-07-29"
---

# RFC-015 Task Breakdown and PR Plan

Seven slices. PR-015-C is the security-critical one and has its own instruction document.

## PR-015-A — Design and handoff acceptance

Maintainer sign-off on the surface contract, layer model, input-routing architecture, and the two seams.

## PR-015-B — Window, layers, chrome, seams

Scope:

- `crates/tekstide` becomes a real `iced` application; the text harness is replaced.
- Layer composition: chrome / content / modal via `stack`/`opaque`.
- Top bar and status bar rendering real `ApplicationShell` state.
- `Theme` value with compiled default; **no hardcoded colours or font sizes**.
- i18n lookup seam with English default; **no hardcoded user-facing strings**.

Review gate:

- `tekstide-core` gains **no GUI dependency**.
- Seam enforcement demonstrated — ideally mechanically (a test that catches string/colour literals), otherwise by inspection with the limitation recorded.
- No shell-local state mirroring core state.

## PR-015-C — Input routing and focus model

**The security-critical slice.** Read [`pr-015-c-input-routing.md`](./pr-015-c-input-routing.md) first.

Scope:

- Three distinct input classes: `ShellInput`, `SurfaceInput`, `TextStream`.
- `TextStream` unconstructible outside the terminal surface module; `ShellInput` unconstructible by surfaces.
- Modal exclusivity: while a modal is active, surface and text-stream input are **not produced**.
- Global keybindings from `KeybindingPolicy`, not capturable by any surface.
- Focus cycle, focus trapping, and focus return on modal dismissal.

Review gate:

- **Guard-deletion resistance**: removing a guard condition produces a compile error, not a permissive runtime path.
- Compile-fail check proving `TextStream` cannot be constructed from shell or modal code.
- No post-dismissal delivery of keystrokes typed while a modal was open.
- Stale or cross-project `TerminalId` dropped rather than delivered.
- **Real focus-trap test**, discharging RFC-014 R6.

If the type-separation structure proves impossible in `iced`'s subscription model, **report it** and use the single-routing-function fallback described in §6 of the instruction document. Do not adopt guard ordering silently.

## PR-015-D — Project Board surface

Scope:

- The `Surface` contract, implemented by the Project Board.
- Project rows over `ApplicationShell` state: name, branch, trust, terminal count, AgentRun state, pending approvals, last activity.
- Attention ordering per UI/UX §6.5 where the model supports it.

Review gate:

- **`CountDisplay` fidelity**: `Unavailable` and `NotImplemented` never render as `0`. Test required.
- Surface holds no duplicated state.
- Surface cannot render trusted chrome or reach modal state.

## PR-015-E — Mode switching and Content-mode scaffolding

Scope:

- Content ↔ Terminal route switching, **no animation**.
- Sidebar and main-area scaffolding that RFC-017/019/020 plug into.
- Terminal sessions and AgentRuns unaffected by mode switching.

Review gate:

- No animation or interpolation in the switch path (`NFR-UX-005`), confirmed by inspection.
- Mode switch does not disturb running terminals or AgentRuns.
- Scaffolding exposes the surface contract without pre-empting later surface RFCs.

## PR-015-F — Measurement: discharge R1

Scope:

- Shell-internal measurement behind a flag.
- C2 typing latency, C4 mode switch, C5 warm start.
- Idle-CPU comparison proving the harness does not force redraw when inactive.

Review gate:

- **Non-contamination proven**, not assumed — the failure RFC-014 hit is the specific thing to rule out.
- Release builds, ≥1,000 samples, p50/p95/p99, machine identification, app-internal framing.
- Delivery-loss rates reported; survivorship-bias caveat applied.
- If a non-contaminating path is still unavailable: decomposed measurement plus honest re-recording of the residual. **Another all-zero figure is not an acceptable outcome.**

## PR-015-G — Closeout evidence

Scope: checklist, QA evidence, known limitations, answers to the RFC's open questions, and an explicit statement of R1's disposition and R6's discharge.

## Sequencing

B → C is strict. D and E both need B and C. F needs E. G needs all.

**PR-015-C blocks D and E.** If its structure cannot be achieved, stop and escalate before building surfaces on an input model that may need reworking — reworking routing after four surfaces exist is far more expensive than pausing here.
