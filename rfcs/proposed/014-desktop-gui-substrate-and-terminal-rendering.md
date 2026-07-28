# RFC-014: Desktop GUI Substrate and Terminal Rendering Strategy

Status: Proposed
Target milestone: M8
Date: 2026-07-28

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md)

Depends on:

- [RFC-003](../done/003-information-architecture-and-ui-mode-model.md)
- [RFC-005](../done/005-application-shell-and-project-board.md)
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md)
- [RFC-007](../done/007-runtime-substrate-pty-feasibility.md)
- [RFC-009](../done/009-terminal-security-boundary.md)

Blocks:

- all M8 rendered surfaces: Project Board, Content Mode, Terminal / Agent Immersion Mode;
- rendered approval, paste-confirmation, trust, and safe-close dialogs;
- the audit producers deferred to M8 because they originate in dialogs;
- screenshot-backed trusted-UI spoofing evidence deferred from RFC-009;
- NFR performance verification for typing and terminal input latency.

## Summary

RFC-014 selects the desktop GUI substrate for Tekstide and, inseparably, the strategy for rendering terminal output inside it. These are one decision, not two: the terminal surface is the hardest constraint on substrate choice, and choosing a substrate without a terminal-rendering answer would defer the actual risk.

Following the RFC-007 precedent, this RFC does not decide on paper alone. It defines the decision criteria, narrows the candidate set, identifies the questions that cannot be answered without measurement, and specifies a bounded spike whose evidence closes the decision. A short decision record amends this RFC when the spike reports.

## Motivation

M8 is blocked. ROADMAP M8's review gates still list *"GUI runtime decision record or RFC, unless completed by the M4 feasibility gate"* — and it was not. RFC-007 states plainly:

> *"This is not a product-direction decision. Tekstide remains a one-window local workbench, and the roadmap still expects a desktop GUI surface... The TUI harness is chosen because it is the shortest credible path to proving the PTY loop."*

So the project has a tested headless core through RFC-013, a proven PTY loop, a reviewed terminal security boundary — and no decision about what renders any of it. No substrate crate appears anywhere in the repository.

This is now the largest open design decision in the project, and every remaining milestone depends on it.

## Goals

- Select the desktop GUI substrate with evidence, not preference.
- Select the terminal-rendering strategy in the same decision.
- Trace every criterion to an existing accepted requirement, so the choice is auditable rather than aesthetic.
- Preserve the RFC-009 security boundary rather than silently widening it.
- Specify a bounded spike that answers what paper analysis cannot.
- Produce a decision record naming the chosen direction and the rejected alternatives with reasons.

## Non-Goals

- Implementing production GUI surfaces. That is M8 proper, after this decision.
- Final visual design, theming, or icon work.
- Widget-level API design for Project Board, editor, or explorer.
- Replacing or reopening the RFC-009 accepted-sequence policy.
- Cross-platform build evidence beyond identifying it as required future work.
- Choosing a syntax-highlighting engine (deferred; RFC-006 left it optional).

## The Decision Is Not Open In One Direction

Before comparing candidates, one branch closes on existing accepted requirements rather than on measurement.

**A pure TUI cannot satisfy the accepted trusted-UI security boundary.**

RFC-009 requires, and review accepted, that *"approval, trust, paste-confirmation, and destructive-decision dialogs are rendered outside terminal output"* (`rfcs/done/009-terminal-security-boundary.md:212`). The threat model's T-026 mitigation is *"native approval UI outside terminal pane; clear visual boundary; terminal cannot open approvals."*

In a TUI, every surface is characters in one grid, drawn by the same renderer, in the host terminal's font and palette. "Outside terminal output" has no structural meaning there — a sufficiently crafted byte stream can imitate any box-drawn dialog Tekstide can draw. RFC-009 implemented the *model-level* boundary and explicitly deferred *"screenshot-backed spoofing evidence"* to the GUI milestone. A TUI cannot produce that evidence, because the property does not hold.

Two further accepted requirements point the same way:

- **i18n is mandatory.** Project rules require the GUI support multiple languages. A TUI inherits whatever text shaping the host terminal provides, with no control over complex-script rendering.
- **Accessibility.** `NFR-UX-001` and the UI/UX baseline require visible focus indicators, screen-reader labels for Project Board rows and approval actions, and focus trapping in dialogs. A TUI's accessibility story is the host terminal's, which Tekstide does not control.

**Consequence:** TUI is rejected as the product substrate on accepted-requirements grounds, and the spike does not need to evaluate it. This is a finding, not a preference — reopening it would require amending RFC-009 and the threat model, which is a larger decision than substrate selection.

A TUI remains legitimate as a *development harness*, exactly as RFC-007 used it.

## The Real Problem: Terminal Rendering Inside a GUI

Choosing "a GUI toolkit" is the easy half. The hard half is that Terminal / Agent Immersion Mode is roughly half the product, and rendering it inside a GUI means owning a terminal emulator: a cell grid with attributes, a VT state machine, scrollback, selection, font metrics and DPI-aware sizing, cursor shapes, and reflow on resize.

RFC-005's split policy already depends on this — vertical split is permitted only when each pane preserves a configured minimum column count *"after font metrics and DPI scaling are applied."* That requirement is unimplementable without real font metrics from the chosen substrate.

### The RFC-009 tension

Mature terminal-emulation crates exist and adopting one is plausible. But RFC-009 deliberately defined a **conservative** accepted subset, pinned the exact grammar, and made every unsupported family inert — OSC 52 clipboard, title mutation, hyperlinks, DCS/PM/APC, mouse/focus reporting, keyboard-protocol extensions, identity queries, status and cursor-position reports. A general-purpose emulator accepts a far wider set by design, because its job is compatibility, not containment.

This is the central architectural question of RFC-014, and it must not be resolved implicitly by a dependency choice:

- **Option A — Emulator as engine, RFC-009 as filter.** Adopt an emulation crate for the grid/parser core, but interpose the RFC-009 policy so unsupported families never reach it. Retains compatibility work; requires proving the filter is not bypassable.
- **Option B — Own the grid, keep RFC-009 as the parser.** Extend the existing reviewed parser into a cell-grid model and render that. Preserves the security boundary exactly; more implementation work and slower toward broad shell compatibility.
- **Option C — Emulator unfiltered, amend RFC-009.** Accept the wider set and re-review the security boundary. Honest, but reopens an accepted, evidence-backed decision and should not be chosen for convenience.

**Provisional lean: Option A**, with the filter placed before the emulator and tested as an isolation boundary. Option B is the safer choice if the spike shows the filter is leaky or the emulator's API does not permit interposition. Option C requires explicit maintainer sign-off and a threat-model amendment; it is not a default.

## Decision Criteria

Every criterion traces to an accepted requirement. The spike must produce evidence for each.

| # | Criterion | Source | How judged |
| --- | --- | --- | --- |
| C1 | Terminal grid rendering at the accepted subset | RFC-009 | Renders the pinned accepted families correctly; unsupported families visibly inert |
| C2 | Typing latency p95 ≤ 16 ms, p99 ≤ 33 ms | `NFR-PERF-003` | Measured in an editor surface under load |
| C3 | Terminal input latency p95 ≤ 16 ms with bounded background output | `NFR-PERF-004` | Measured with a flooding background process |
| C4 | Mode switch p95 ≤ 32 ms, no animation | `NFR-PERF-002`, external design §4.5 | Measured Content ↔ Terminal toggle |
| C5 | Warm start to first usable window ≤ 800 ms | `NFR-PERF-001` | Measured |
| C6 | Baseline memory substantially below Electron-class IDEs | `NFR-RES-002` | Measured idle RSS; recorded as a baseline, not a pass/fail threshold |
| C7 | Font metrics and DPI scaling exposed | RFC-005 split policy | Can compute per-pane column counts |
| C8 | Trusted UI structurally separable from terminal content | RFC-009:212, T-026 | Dialogs render outside the terminal surface; screenshot evidence producible |
| C9 | Keyboard-first: focus model, focus trapping, no mouse dependency | `NFR-UX-001`, UI/UX §18 | All primary workflows keyboard-reachable |
| C10 | i18n text shaping | Project rules | Renders a non-Latin script correctly |
| C11 | Accessibility affordances | `NFR-UX-002`, UI/UX §18 | Focus indicators; screen-reader labelling path identified |
| C12 | Cross-platform path to Linux/Windows/macOS | `NFR-PORT-001` | Linux proven; no known blocker for the others |
| C13 | Licence compatible with Apache-2.0 distribution | Project rules | Verified against each dependency, per the `NOTICE` discipline established in RFC-013 |
| C14 | Maintenance posture | vice-manager concern | Release cadence, breaking-change history, bus factor, native-dependency weight |

C13 and C14 are not technical gates but are decision-relevant: RFC-013 established that a new dependency carries a notice and supply-chain obligation (T-033).

## Candidate Set

The spike evaluates **pure-Rust GUI toolkits**, because `NFR-RES-002`'s "substantially lower than Electron" positioning and the project's lightweight framing argue against a webview-backed shell, and because the existing codebase is dependency-light by deliberate choice.

Candidates to evaluate against C1-C14:

1. **`iced`** — Elm-architecture, wgpu-backed, pure Rust. Named in the maintainer's stated expertise, which is material for C14: substrate familiarity is a real maintenance factor on a small team.
2. **One retained-mode alternative** — for comparison on C1, C7, and C9, since terminal rendering and precise font metrics are the discriminating constraints.
3. **Terminal-emulation crate(s)** — evaluated for the Option A/B/C question above, independently of the toolkit choice.

The spike must record what it *could not* determine, rather than asserting capabilities it did not exercise.

## Spike Specification

Bounded, disposable, and quarantined exactly as `tekstide-pty-spike` was under RFC-007 — a new `crates/tekstide-gui-spike` with `publish = false`, not wired into the product.

Minimum evidence:

1. A window rendering a **static Content Mode shell**: sidebar, main area, status bar, at the accepted layout proportions.
2. A **terminal pane** driven by the existing `LinuxTerminalRuntime`, rendering the RFC-009 accepted subset, with at least three unsupported families shown inert.
3. A **modal dialog rendered outside the terminal surface**, with a screenshot demonstrating the visual boundary under adversarial terminal output that imitates a dialog. This is the C8 evidence RFC-009 deferred.
4. **Latency measurements** for C2, C3, C4, C5 with methodology recorded, plus idle RSS for C6.
5. **Font metrics** sufficient to compute per-pane column counts for the RFC-005 split policy.
6. A **non-Latin script** rendered in both editor and terminal surfaces (C10).
7. **Keyboard-only** navigation through the shell, including focus trapping in the dialog (C9, C11).
8. Licence and dependency-weight inventory for every crate introduced (C13, C14).

The spike must not implement product behavior, persist state, or touch the audit, transcript, or project stores.

## Implementation Plan

1. **PR-014-A: RFC and criteria acceptance.** This document; maintainer sign-off on criteria, candidate set, and the TUI rejection.
2. **PR-014-B: Spike harness.** Quarantined crate, window, static Content Mode shell, keyboard focus model.
3. **PR-014-C: Terminal surface.** PTY-backed pane honoring the RFC-009 boundary; Option A/B interposition proven or falsified.
4. **PR-014-D: Trusted-UI evidence.** Dialog outside terminal surface; adversarial-output screenshots.
5. **PR-014-E: Measurement.** C2-C7, C10 evidence with recorded methodology.
6. **PR-014-F: Decision record.** Amend this RFC with the chosen substrate, terminal strategy, rejected alternatives with reasons, and the maintenance/licence assessment. Requires maintainer sign-off before M8 implementation begins.

PR-014-B through PR-014-E are developer implementation and measurement work. PR-014-A and PR-014-F are design and decision work.

## Test and Evidence Requirements

- Latency figures with stated methodology, sample size, and machine identification — not single observations.
- Screenshot artifacts for C8, retained in the spike's QA evidence.
- Explicit statement of every criterion the spike could **not** evaluate, and why.
- Licence inventory naming each new transitive native dependency, following the RFC-013 `NOTICE` precedent.
- Confirmation that the spike introduces no product-code dependency until the decision record is accepted.

## Acceptance Criteria

- Substrate and terminal-rendering strategy are chosen together, with evidence against C1-C14.
- The RFC-009 accepted-sequence boundary is preserved, or its amendment is an explicit reviewed decision rather than a side effect.
- Trusted-UI separation is demonstrated with screenshot evidence, closing the RFC-009 deferral.
- Performance criteria are measured, not asserted; any missed budget is recorded as a limitation with a mitigation path.
- Rejected alternatives are named with reasons.
- No product code depends on the chosen substrate before the decision record is accepted.

## Risks

- **Terminal rendering is underestimated.** It is the single largest technical risk in M8. Mitigation: PR-014-C exists specifically to falsify the plan early, while the spike is still disposable.
- **A dependency choice silently widens the security boundary.** Mitigation: the Option A/B/C question is explicit and its resolution is a reviewed acceptance criterion.
- **Substrate lock-in.** GUI substrate is among the most expensive decisions to reverse. Mitigation: keep product code behind the existing core/runtime boundary, which has held well through RFC-008 to RFC-013.
- **Performance budgets prove unreachable.** Mitigation: measure before committing; if a budget is missed, amend the NFR with evidence rather than quietly shipping past it.
- **Spike becomes production by accident.** This happened to no one here yet, but `tekstide-pty-spike` remains quarantined and should stay the model. Mitigation: `publish = false`, no product dependency, deleted or archived after the decision record.

## Open Questions

1. Does the maintainer accept the TUI rejection on accepted-requirements grounds, or should it remain in the evaluated set?
2. Should the second retained-mode candidate be named now, or selected by the spike author after a bounded survey?
3. If a performance budget is missed by a small margin, is the preference to amend the NFR or to change substrate?
4. Should syntax highlighting be pulled into the spike's evaluation, or remain deferred as RFC-006 left it?
