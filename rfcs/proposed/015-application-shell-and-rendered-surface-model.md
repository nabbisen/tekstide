# RFC-015: Application Shell and Rendered Surface Model

Status: Proposed
Target milestone: M8
Date: 2026-07-29

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md), [`delivery-plan.md`](../delivery-plan.md)

Depends on:

- [RFC-003](../done/003-information-architecture-and-ui-mode-model.md) — navigation and mode model
- [RFC-005](../done/005-application-shell-and-project-board.md) — `ApplicationShell`, Project Board state
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md) — content workspace models
- [RFC-009](../done/009-terminal-security-boundary.md) — trusted-UI separation requirement
- [RFC-014](./014-desktop-gui-substrate-and-terminal-rendering.md) — substrate decision (approved 2026-07-29)

Blocks:

- RFC-017 terminal renderer and immersion mode
- RFC-019 editor and explorer surfaces
- RFC-020 diff review and AgentRun report surfaces
- RFC-022 security dialogs and the audit producers that originate in them

## Summary

RFC-015 builds the application shell that every other rendered surface lives inside, and defines the **rendered-surface contract** those surfaces must satisfy.

Its single most important output is not the window — it is the **input-routing and layer model**. RFC-009 requires trusted dialogs to render outside terminal output, and the RFC-014 spike proved that property held only because its terminal was output-only and emitted no messages (recorded as residual risk R6). The real terminal must accept keystrokes. RFC-015 defines the routing architecture under which RFC-017 can accept terminal input *without* dissolving the trusted-UI boundary.

The shell is a **view over existing models**, not a new state layer. `ApplicationShell`, `AppRoute`, `KeybindingPolicy`, and the project/content models already exist and are tested. RFC-015 renders them and dispatches into them.

## Motivation

M8 is unblocked and nothing is rendered. `crates/tekstide/src/main.rs` is still a 40-line text harness that prints `shell.render_text()`.

Meanwhile four later RFCs each need a surface to render into, a way to receive input, and a guarantee that untrusted content cannot reach trusted chrome. Building those ad hoc, per surface, would produce four inconsistent answers to the same questions. RFC-015 answers them once.

## Goals

- Render the application shell: window, top bar, sidebar, main area, status bar, in both Content and Terminal modes.
- Define the **rendered-surface contract** every later surface RFC implements.
- Define the **layer model** that keeps trusted UI structurally separable from untrusted content.
- Define **input routing** with exactly one input sink at a time, such that RFC-017 can accept terminal keystrokes safely.
- Render the Project Board as the first real surface over existing `ApplicationShell` state.
- Establish the **i18n seam** (no hardcoded user-facing strings) without waiting for RFC-016.
- Establish the **theme and typography seam** without waiting for RFC-023.
- Discharge residual risk R1: verify mode-switch and typing latency with non-degenerate instrumentation.
- Establish the accessibility baseline, and record the screen-reader absence honestly.

## Non-Goals

- Terminal rendering. RFC-017 owns it; RFC-015 defines the surface and input contract it plugs into.
- Editor, explorer, diff, and AgentRun report surfaces. RFC-019 and RFC-020.
- Real security dialogs. RFC-022. RFC-015 provides only the **modal layer** they render into.
- Locale catalogs, pluralization, RTL policy. RFC-016 fills the seam this RFC creates.
- Configuration file loading. RFC-023 fills the theme/typography seam this RFC creates.
- Multi-window. Deferred by `REQ-PROJ` and external design §5.1.
- Mouse-driven workflows beyond ordinary click support. Keyboard is the product identity.

## Design Principles

1. **The shell is a view, not a model.** All state lives in `tekstide-core`. The shell renders `shell.state()` and dispatches `AppCommand`. It introduces no parallel state that could drift.
2. **Seams before implementations.** Where a later RFC owns a subsystem (i18n, configuration), RFC-015 creates the seam and a working default, so the shell is built ready rather than retrofitted.
3. **One input sink at a time.** Input routing is explicit and exclusive, never "whoever happens to handle the message."
4. **Trusted UI is a layer, not a widget.** Separation is structural — a different composition layer — not a matter of drawing something on top.
5. **Untrusted content renders as data.** Terminal output, transcripts, diffs, Git metadata, and file contents are never trusted chrome.

## Architecture

```
iced Message  ──►  update()  ──►  AppCommand  ──►  ApplicationShell::dispatch()
                                                            │
                     view()  ◄────────────────  shell.state()
```

The shell crate owns rendering and input; `tekstide-core` owns all state and policy. Keybindings come from the existing `KeybindingPolicy::linux_mvp()` — RFC-015 **uses** that policy and does not invent bindings.

### Rendered-surface contract

A **Surface** is a rendered region that occupies the main area. Every later surface RFC implements this contract.

A surface declares:

- **identity** — which `AppRoute`/`ProjectOpenSurface` it serves;
- **view** — a pure function of core state to a widget tree; no interior mutable state;
- **input interest** — `None`, `Keyboard`, or `TextStream` (see Input Routing);
- **focus zones** — ordered regions participating in the shell's focus cycle;
- **status contribution** — what it publishes to the status bar.

Rules:

- A surface **must not** hold state that duplicates `tekstide-core`.
- A surface **must not** render trusted chrome. Only the modal layer may.
- A surface receives input **only** when it owns the input sink.
- Exactly one surface is active in the main area at a time (external design §3.4; no arbitrary splitting in MVP).

### Layer model

Composition is ordered and fixed:

| Layer | Contents | Trust |
| --- | --- | --- |
| **Chrome** | Top bar, status bar | Trusted |
| **Content** | Sidebar + active surface (editor, terminal, board, diff, report) | **Untrusted content** may appear here |
| **Modal** | Approval, trust, paste-confirmation, destructive, safe-close dialogs | Trusted, exclusive |

The modal layer renders via `stack`/`opaque` — the composition proven in the RFC-014 spike (C8) — and is **never** reachable from surface code. A surface cannot open, populate, or dismiss a modal; it may only emit a message that the shell interprets.

This is what makes RFC-009:212 structurally true rather than conventionally true.

### Input routing — the core of this RFC

Exactly one **input sink** is active at any moment, in strict precedence:

```
Modal layer  >  Terminal surface (when it holds text focus)  >  Shell focus cycle
```

Three message classes, deliberately distinct types:

| Class | Produced by | May route to |
| --- | --- | --- |
| `ShellInput` | Global keybindings from `KeybindingPolicy` | Shell only |
| `SurfaceInput` | Keyboard while a surface holds focus | The focused surface only |
| `TextStream` | Keystrokes destined for a PTY | **A terminal surface only** |

Binding rules:

1. **When the modal layer is active it is the exclusive sink.** `SurfaceInput` and `TextStream` are not produced at all — not produced-and-ignored. RFC-017 must not be able to route a keystroke to a PTY while a dialog is open.
2. **`TextStream` is constructible only by the terminal surface's own input handler** and carries the target `TerminalId`. It has no variant that can reach shell or modal state.
3. **Global keybindings always win** over surface input, so `Ctrl+Esc` mode switching and Project Board access cannot be captured by a surface — including a terminal.
4. **No surface may synthesize `ShellInput`.**

This is the direct answer to residual risk R6. In the spike the property held because terminal input did not exist; here it holds because terminal input is a *type that cannot address trusted state*.

### Mode switching

Content ↔ Terminal switching is a route change. No animation, no interpolation (`NFR-UX-005`). Terminal sessions and AgentRuns are unaffected by mode switching — already guaranteed at model level and must remain so.

### Theme and typography seam

RFC-015 defines a `Theme` value carrying colours, font families, and font sizes, with a compiled default. `NFR-UX-004` requires these be configurable; RFC-023 will supply them from configuration. Until then the default is used and the seam is honoured — **no widget hardcodes a colour or font size**.

`NFR-UX-002` is binding on every surface: **status must never rely on colour alone.** The spike's `[focused]` text prefix alongside a border is the reference pattern.

### i18n seam

Every user-facing string goes through a lookup function from the first line of shell code. RFC-016 supplies catalogs, locale selection, fallback, pluralization, and RTL policy; RFC-015 supplies the seam and an English default.

Retrofitting string extraction across a built UI is materially more expensive than writing it this way from the start, which is why this seam is in M8's first GUI RFC rather than deferred to RFC-016.

### Project Board surface

The first real surface, rendering existing `ApplicationShell` state: project rows with name, branch, trust state, terminal count, AgentRun state, pending approvals, last activity.

`CountDisplay` semantics must be preserved — a count that is `NotImplemented` or `Unavailable` renders as such, **never as zero**. That distinction was built deliberately in RFC-005 and the rendered surface is where it would most easily be lost.

## Discharging residual risk R1

RFC-014's C2/C3/C4 are unverified because `iced::window::frames()` forces continuous redraw once subscribed. RFC-015 must verify mode-switch and typing latency with instrumentation that does not contaminate what it measures.

Requirements:

- Instrumentation is **built into the shell** behind a measurement flag, not bolted on.
- It must not force redraw when inactive — verified by an idle-CPU comparison, as the spike did.
- If a non-contaminating input-to-frame path still cannot be found, measure **input-to-state-change** and **frame cost** separately, and report the decomposition rather than a degenerate combined figure.
- The survivorship-bias limitation recorded in RFC-014 R9 applies to any synthetic-input harness reused here.

Budgets: `NFR-PERF-001` warm start ≤ 800 ms (spike measured 227.9 ms median — expected to hold), `NFR-PERF-002` mode switch p95 ≤ 32 ms, `NFR-PERF-003` typing p95 ≤ 16 ms.

## Accessibility baseline

- Visible focus indicators on every focusable element, not colour-dependent.
- Focus trapping in the modal layer, with a real test — not only the structural argument that sufficed for the spike.
- Keyboard reachability for every shell workflow (`NFR-UX-001`).
- **Screen-reader support is absent** and will remain so while `iced` has no accessibility bridge (RFC-014 R2, owner-accepted). Public documentation must state this. Do not add a partial or simulated accessibility affordance that implies otherwise.

## Data Model Impact

RFC-015 should add **no new state to `tekstide-core`**. If a rendering need appears to require new core state, that is a signal to check whether the model already expresses it — and if it genuinely does not, to raise it rather than adding a shell-local shadow copy.

New types live in the shell crate: `Surface` contract, layer composition, `Theme`, the three input-message classes, and the measurement harness.

## Implementation Plan

1. **PR-015-A** — design and handoff acceptance.
2. **PR-015-B** — window, layer composition, chrome, theme and i18n seams. No surfaces yet.
3. **PR-015-C** — input routing and focus model, including the three message classes and modal exclusivity. **The security-critical slice.**
4. **PR-015-D** — Project Board surface over `ApplicationShell` state, with `CountDisplay` fidelity.
5. **PR-015-E** — mode switching, Content-mode sidebar and main-area scaffolding for later surfaces.
6. **PR-015-F** — measurement: discharge R1 with non-contaminating instrumentation.
7. **PR-015-G** — closeout evidence.

PR-015-C is to this RFC what PR-014-C was to RFC-014: the slice where being subtly wrong produces something that looks correct. It gets a dedicated instruction document in the handoff pack.

## Test and Evidence Requirements

- **Routing tests:** modal active ⇒ no `SurfaceInput` or `TextStream` produced; `TextStream` cannot address shell or modal state; global keybindings not capturable by a surface.
- **Focus-trap test** — a real test this time, not a structural argument.
- **`CountDisplay` test:** unavailable and not-implemented counts never render as `0`.
- **Theme/i18n seam tests:** no hardcoded colour, font size, or user-facing string outside the seam. Enforce mechanically where practical.
- **Latency evidence** per R1, with methodology, release builds, p50/p95/p99, and machine identification.
- **Idle-CPU comparison** proving the measurement harness does not force redraw when inactive.
- Screenshots for the shell in both modes.

## Acceptance Criteria

- The shell renders both modes over existing core state with no duplicated state.
- The rendered-surface contract is documented and implemented by the Project Board surface.
- The layer model is structural: no surface can render trusted chrome or reach modal state.
- Input routing gives exactly one sink at a time, with the three classes type-distinct.
- **RFC-009:212 holds by construction under an input-accepting design** — R6 discharged.
- Theme and i18n seams are in place with working defaults and no hardcoded values.
- `CountDisplay` fidelity preserved.
- R1 discharged, or its residual state explicitly re-recorded with evidence of what was attempted.
- Accessibility baseline met; screen-reader absence stated, not obscured.

## Risks

- **Shell accumulates shadow state.** The most likely architectural drift. Mitigation: the no-new-core-state rule, and review attention on any shell-local field that mirrors a core value.
- **Input routing is subtly permissive.** Mitigation: PR-015-C gets its own instruction document, and I will probe it empirically as with RFC-013's SQL constraints and RFC-014's filter.
- **R1 proves undischargeable.** Possible; `iced` may offer no non-contaminating path. Mitigation: the decomposition fallback, and honest re-recording rather than another degenerate figure.
- **Seams get bypassed under delivery pressure.** A hardcoded string or colour is trivial to add and expensive to find later. Mitigation: mechanical enforcement where practical.
- **Scope creep into surfaces.** RFC-015 must not start rendering editors or terminals. Mitigation: explicit non-goals; the Project Board is the only surface in scope.

## Open Questions

1. Should the Project Board render as rows, cards, or width-responsive both? UI/UX §22 left this open; RFC-015 can settle it or defer to RFC-019's layout work.
2. Should `SurfaceInput` carry raw key events or pre-interpreted intents? Intents are safer and less flexible; raw is the reverse.
3. Should the measurement harness ship in release builds behind a flag, or be a separate build profile?
