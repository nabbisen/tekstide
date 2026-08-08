# RFC-014: Desktop GUI Substrate and Terminal Rendering Strategy

Status: Implemented with documented limitations — closed 2026-08-01. Substrate decision (`iced` 0.14 + Option A) accepted by the owner 2026-07-29; **R1 and R6 discharged by RFC-015** across `0.4.0`/`0.4.1`, so this decision record has no open items. R2 (no accessibility bridge) and R9 (survivorship bias) are owner-accepted standing findings. **R4-R7 are carried scope for RFC-017**, which re-establishes them under real product conditions.

**The spike crate (`crates/tekstide-gui-spike`, `publish = false`) outlives this RFC on purpose** — see §"When the spike crate is deleted" below. — spike complete; substrate decision approved by the human owner 2026-07-29 (see §PR-014-F)
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

---

# Decision Record (PR-014-F)

Date: 2026-07-29
Author: high-capability model (architect)
Status: **Approved by the human owner, 2026-07-29**
Evidence base: PR-014-B through PR-014-E, reviewed and accepted in responses 105, 106, 107, and 108.

**Approved by the human owner on 2026-07-29.** Substrate selection is a multi-year maintenance commitment reserved to the human owner; this section records the recommendation, the evidence behind it, and the owner's decision.

**Owner's basis for the decision.** The owner's own evaluation considered a wider candidate set than the spike's shortlist — including `dioxus` and `tauri`, neither of which the spike assessed — and independently reached the same conclusion. `dioxus` and `tauri` are webview-adjacent, which this RFC had already excluded on `NFR-RES-002` grounds ("baseline memory substantially lower than Electron-based IDEs"); the owner's consideration reached that outcome separately rather than inheriting it. The decision therefore rests on both the spike evidence below and a broader independent comparison.

## D1. Recommended decision

**Substrate: `iced` 0.14.**
**Terminal rendering: Option A** — `alacritty_terminal` as the emulator, with the RFC-009 accepted-sequence policy interposed as a `vte::ansi::Handler` wrapper in front of it.

## D2. Evidence against the criteria

| # | Criterion | Result |
| --- | --- | --- |
| C1 | Terminal grid renders the RFC-009 accepted subset; unsupported families inert; styled spans | **Verified.** Real PTY-backed shell; OSC 52 / title / OSC 8 confirmed inert, window title independently checked unchanged; multi-colour spans in one line via `Term::renderable_content()` |
| C2 | Typing latency p95 ≤ 16 ms | **Not verified** — see R1 |
| C3 | Terminal input latency under flood p95 ≤ 16 ms | **Not verified** — see R1 |
| C4 | Mode switch p95 ≤ 32 ms; no animation | **Not verified** — see R1. No-animation confirmed separately by code inspection |
| C5 | Warm start ≤ 800 ms | **Met.** Median 227.9 ms, max 255.5 ms over 14 warm runs. Uncontaminated |
| C6 | Idle RSS baseline | **Recorded.** 178,124 kB after 60 s idle, one project + one terminal. Uncontaminated |
| C7 | Font metrics for per-pane column count | **Verified.** 7.8 logical px/glyph at 13 px monospace via iced's own `Paragraph` primitive, exercised at the real 1.2× fractional scale |
| C8 | Trusted UI structurally separable; screenshot evidence | **Verified.** Genuine dialog via `stack`/`opaque` alongside an adversarial imitation in one frame. **Closes the RFC-009 `screenshot-backed spoofing evidence` deferral** |
| C9 | Keyboard reachability; focus trapping | **Verified** for the spike surfaces. Caveat R6 applies |
| C10 | Non-Latin script rendering | **Verified with disclosed gaps.** Editor reorders bidi correctly; terminal grid does not, and lacks wide-cell CJK — see R7 |
| C11 | Accessibility affordances | **Partially met.** Focus indicators verified and non-colour-reliant. **Screen-reader path absent — see R2** |
| C12 | Cross-platform path | **Not attempted** (non-goal). Blocker identified is `LinuxTerminalRuntime`, not the GUI substrate |
| C13 | Licence inventory | **Verified.** `iced` MIT; `alacritty_terminal` Apache-2.0; `vte` Apache-2.0 OR MIT; transitives inventoried. No bundled native C introduced |
| C14 | Maintenance posture | **Recorded.** See R3 for dependency weight |

Ten of fourteen criteria verified or met. Three unverified (C2-C4, one root cause). One partially met (C11). One deliberately not attempted (C12).

## D3. Rejected alternatives

| Alternative | Reason |
| --- | --- |
| **Pure TUI** | Rejected on accepted-requirements grounds, not preference. RFC-009:212 requires approval/trust/paste/destructive dialogs rendered *outside terminal output*; that property cannot hold when every surface is characters in one grid. i18n and accessibility point the same way |
| **`slint`** | C13 licence. `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` — GPL is virally incompatible with Apache-2.0 redistribution, and `LicenseRef-` denotes bespoke non-OSI terms |
| **`gpui`** | C14 maintenance. Last crates.io release 0.2.2 on 2025-10-22, ~9 months stale; 172 K downloads vs `egui`'s 20.4 M. Rejected *despite having the strongest terminal precedent* (Zed ships a production terminal on it) — precedent shows a framework can do the job today, maintenance posture shows whether it still will in three years |
| **`xilem`** | Explicitly alpha; no stable release |
| **`relm4`/GTK** | Native GTK4 C dependency, in tension with the pure-Rust framing that already excludes webview shells |
| **`egui`** | Not rejected on merit — evaluated as the comparison candidate under a deliberately narrow C1/C7 probe. **See R2: it is materially better than `iced` on accessibility, and that was never compared** |
| **Terminal Option B** (own grid on the RFC-009 parser) | Not needed. Option A proved implementable and non-falsified |
| **Terminal Option C** (unfiltered emulator + amend RFC-009) | Not adopted. Would have reopened an accepted, evidence-backed security boundary |

## D4. Why Option A rather than a byte-level filter

The handoff specified byte-level interposition with a stateful filter mirroring `TerminalSecurityParser`. The spike falsified that as the right approach: `alacritty_terminal` delegates classification to the same `vte` grammar it is built on, so wrapping `vte::ansi::Handler` yields two properties **by construction** rather than by testing —

- **P3 (classification parity):** filter and emulator share one classifier; there is no second implementation to drift.
- **P4 (stream-position independence):** `Processor` holds parse state across `advance()` calls, so sequences split across PTY reads are reassembled by the same code the real terminal uses.

Verified in vendored `vte-0.15.0`: all 71 `Handler` methods have default bodies and every default is an empty no-op, so "blocked by omission" is fail-closed by the trait's own design. A future `vte` adding a method gets the no-op automatically — a new capability cannot appear by silent forwarding.

The handoff's original instruction would have created the very dual-parser drift risk it was written to prevent.

## D5. Residual risks

Numbered for carry-forward. **R1 and R2 require owner acknowledgement.**

**R1 — Latency budgets unverified (C2/C3/C4).** ~~*Owner acknowledgement required.*~~ **Acknowledged by the owner 2026-07-29 and DISCHARGED 2026-08-01** — RFC-015 PR-015-F measured C2/C5 in `0.4.0` and PR-015-E measured C4 in `0.4.1`, all non-degenerate and inside budget. C3 (`NFR-PERF-004`, terminal input latency under flood) is RFC-017's, not open here.
`iced` 0.14's only application-level frame-observation hook, `window::frames()`, forces continuous compositor-driven redraw once subscribed (~57 Hz, ~2.7 % of one core, with nothing animating). Input-to-frame therefore measures frame availability, not rendering cost. All samples read 0 µs — a degenerate result, not a pass.
*Impact:* the substrate's responsiveness against `NFR-PERF-002/003/004` is unknown. It may be excellent; it is unproven.
*Mitigation:* verify in M8 with instrumentation built into the real shell, where a non-contaminating path is available. **Do not amend the NFRs** — nothing was measured, so there is nothing to calibrate against.

**R2 — No accessibility bridge exists in `iced` 0.14.** ~~*Owner decision required.*~~ **Decided by the owner 2026-07-29**: `iced` accepted knowing this, on the reasoning that i18n is mandatory with verified evidence while screen-reader support is a "should". **Not closed — watched.** The release checklist carries a standing per-release check (`cargo tree -p tekstide | grep -i accesskit`); a hit reopens this. Recorded 2026-08-01 at the owner's direction that accessibility is a social need, not a nice-to-have.
Verified: zero matches for `accesskit`/`a11y`/`accessibility` in vendored `iced`/`iced_winit`, and `accesskit` is absent from the resolved graph. **`egui` 0.35 depends on `accesskit` 0.24.1 as a required, non-optional dependency.**
*Impact:* `NFR-UX-001` (keyboard accessibility) is satisfiable and satisfied. UI/UX §18's *"screen-reader labels should be provided for Project Board rows, approval actions, and process state"* is **not achievable** on this substrate unless `iced` adds a bridge upstream. Accessibility cannot be retrofitted onto a toolkit with no bridge.
*Honest attribution:* the spike did not surface this comparison because **my own handoff scoped the second candidate to a narrow C1/C7 probe**, excluding accessibility. That scoping error is mine, not the implementer's.
***Accepted by the human owner, 2026-07-29.*** Accepted on the grounds that i18n is a **mandatory** project requirement with verified evidence on this substrate (C10: the editor surface performs full Unicode bidi reordering with correct shaping, via `cosmic-text`/`swash`/`unicode-bidi`), whereas screen-reader labelling is a **"should"** in UI/UX §18 with no end-to-end verification on any candidate. `NFR-UX-001` keyboard accessibility is satisfied and unaffected.

*Obligations arising from this acceptance:*
1. **Public claims must state the limitation** — README, documentation, and release notes must say Tekstide is keyboard-accessible and has **no screen-reader support**, with the same discipline applied to audit-producer coverage.
2. **Watch `iced` upstream for an `accesskit` bridge.** If one lands, adoption becomes ordinary implementation work rather than a substrate question.
3. **Keep the core/runtime boundary clean.** It has held from RFC-008 through RFC-013 and is what would make a future substrate change survivable.

*Reviewer note on provenance:* the spike did not compare accessibility because **the architect's handoff scoped the second candidate to a narrow C1/C7 probe**. That scoping error is recorded here so the decision's evidentiary limits are visible, not hidden.

**R3 — Dependency weight.** `Cargo.lock` 50 → 406 packages (+356 across PR-014-B/C), including `wgpu`, `glow`, `cosmic-text`, `fontdb`. Unremarkable for a GPU-accelerated Rust GUI, but materially larger than the single native dependency that justified threat-model entry T-033. PR-014-F records it; a supply-chain assessment belongs in M8.

**R4 — `linefeed()` admits VT and FF.** Constraint-forced: `vte` collapses `C0::LF | C0::VT | C0::FF` into one `linefeed()` call, so the filter cannot separate them at this boundary. `tekstide-core`'s byte-level classifier blocks VT/FF. RFC-017 must decide explicitly whether admitting them is acceptable or whether a byte-level pre-filter is needed for those two codes.

**R5 — `clear_screen` forwarded mode-blind.** `CSI 3 J` (`ClearMode::Saved`) erases scrollback. Faithful to RFC-009:131 as written, and `tekstide-core`'s parser is equally mode-blind — so this is pre-existing policy breadth, not a filter divergence. Unlike R4 this is a *choice*: the mode is available as a parameter. RFC-017 should decide whether terminal output may wipe user scrollback.

**R6 — The focus-trap property does not transfer.** It currently holds because the spike's terminal is output-only and emits no messages. The real terminal must accept keyboard input; RFC-017 must re-establish the property under that condition, with a different argument and probably a real test. **This evidence must not be cited as covering the input-accepting case.**

**R7 — Terminal i18n fidelity is lower than the editor's.** Editor reorders bidi correctly; the terminal grid does not (correct — real terminals generally do not) and lacks wide-cell CJK (a genuine gap; `alacritty_terminal` supports it, the spike's minimal renderer does not consume it). RFC-016 and RFC-017 both need this asymmetry as input.

**R8 — `default-features` unconstrained**, now plus the `advanced` feature. RFC-013 established `default-features = false` discipline for `rusqlite`. If `iced` becomes a product dependency, its feature surface and transitive native licences need the same review.

**R9 — Measurement harness carries survivorship bias.** Percentiles computed over confirmed-received samples only; if delivery loss correlates with the app being busy, dropped samples may be the slow ones. Irrelevant to a degenerate result, material for M8 reuse.

## D6. Spike disposition

`crates/tekstide-gui-spike` remains `publish = false` with no product dependency. **Retain, do not delete, until RFC-015 lands** — the filter corpus (18 tests) and the C7 font-metrics harness are directly reusable as reference implementations, and deleting them before the real shell exists would discard working evidence. Remove or archive at RFC-015 closeout.

## D7. What M8 inherits

- **Proven:** terminal rendering strategy, the RFC-009 interposition point, trusted-UI separability, font metrics for the split policy, startup and idle-memory baselines.
- ~~**Unproven and owed:** latency verification (R1), accessibility posture (R2), and every property in R4-R7 that RFC-017 must re-establish under real product conditions.~~ **Updated 2026-08-01 on closure.** R1 is **discharged** — RFC-015 PR-015-F measured C2/C5 non-degenerately in `0.4.0` and PR-015-E measured C4 in `0.4.1`, all inside budget. R2 is **owner-accepted** as a standing disclosed absence, not an owed item. **R4-R7 remain owed and are RFC-017's**, to re-establish under real product conditions — they are carried scope, not open items in this record.


## When the spike crate is deleted

Recorded on closure, 2026-08-01, because "the RFC is closed" is not the same question as "the spike is finished" and conflating them would delete evidence or keep dead code.

**Nothing compiles against the spike today.** The only reference in product code is a doc comment in `shell.rs`. So this is not a dependency question — it is about what still needs *reading*.

The spike uniquely holds three things, and RFC-017 draws on each in a different slice:

| File | Drawn on by | Superseded when |
| --- | --- | --- |
| `filter.rs`, `filter/tests.rs` | PR-017-B (filter promotion) | PR-017-B lands |
| `terminal_pane.rs` | PR-017-C (grid rendering) | PR-017-C lands |
| `font_metrics.rs` | PR-017-E (split policy) | PR-017-E lands |

**Delete when all four hold:**

1. No product code compiles against it — already true, and mechanically checkable.
2. Every property it proved has a product-code equivalent with its own tests.
3. Its evidence artifacts live outside the crate — already true; the nine screenshots are committed under `../handoffs/014-desktop-gui-substrate-and-terminal-rendering/evidence/`.
4. Nothing still needs to read it as reference — i.e. PR-017-B, C, and E have all landed.

**The forcing event is PR-017-E.** Until then the crate stays.

**One thing to do earlier, at PR-017-B**, and the reason this section exists rather than a one-line note: once the filter is promoted, **the spike holds a second copy of a security-policy implementation.** This project has hit the duplicate-implementation problem twice already — `text_safety` escaping duplicated in `approval::coordinator`, and the string seam scans duplicated between RFC-015 and RFC-016 — and both times the cost was a later consolidation. The spike never ships, so the risk here is not divergent behaviour in production; it is someone reading `filter.rs` and taking it for current. **Mark it superseded in its module doc when PR-017-B lands**, naming the product module that replaced it.

**One consequence to check at deletion**, not before: removing the crate changes the *workspace* dependency tree. `sys-locale` was once reported as costing `+0` because the spike pulled it transitively through `iced` → `cosmic-text` — an error corrected in review 122 by measuring `cargo tree -p tekstide` instead of diffing the workspace lock. Any figure still measured workspace-wide would shift on deletion. The correct measurements are already per-crate, so this should be a no-op; confirm rather than assume.

**Discharged 2026-08-04.** All four conditions held (PR-017-E, the forcing event, landed 2026-08-03) — see `../handoffs/014-desktop-gui-substrate-and-terminal-rendering/spike-crate-deletion.md` for the full deletion record, including the four stranded doc-comment references (reworded to read as history, provenance preserved on the security-critical two) and `tekstide-pty-spike`'s companion deletion decision. `cargo tree -p tekstide` and `-p tekstide-core` confirmed byte-identical before and after — the no-op was verified, not assumed; `Cargo.lock`'s only change is the two spike crates' own package entries, since every dependency either pulled was already a strict subset of `tekstide`/`tekstide-core`'s own.
