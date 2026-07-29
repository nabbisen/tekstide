---
title: "RFC-014: Desktop GUI Substrate and Terminal Rendering Strategy - Acceptance / QA Checklist"
rfc: "RFC-014"
rfc_file: "../../proposed/014-desktop-gui-substrate-and-terminal-rendering.md"
status: "Proposed — criteria accepted, spike pending"
target_milestone: "M8"
source_rfc_status: "Proposed — criteria accepted"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-014 Acceptance / QA Checklist

## Acceptance Status

Criteria accepted 2026-07-28 (PR-014-A). Spike evidence pending.

**A checked box means evidence exists, not that the result was favourable.** A criterion that was evaluated and failed is checked, with the failure recorded in `qa-evidence.md`. A criterion that could not be evaluated stays unchecked with a stated reason. Do not check a box because something "should" work.

## Quarantine Checklist

- [x] `crates/tekstide-gui-spike` exists with `publish = false`.
- [x] No product crate depends on the spike.
- [x] Spike writes no real Tekstide state — audit, transcript, recent-project, or state root.
- [x] Spike performs no network access.
- [x] Standard workspace gates pass with the spike as a member.

## Candidate Selection Checklist

- [x] Bounded survey performed; shortlist recorded (`egui`, `slint`, `gpui`, `xilem`, `relm4`).
- [x] Second candidate selected using the text-grid/terminal-precedent rule (`egui`, via the `egui_term` widget on `alacritty_terminal`).
- [x] Licence screen (C13) applied before implementation; `slint` screened out (GPLv3/proprietary-EULA, incompatible with Apache-2.0 distribution).
- [x] Maintenance posture (C14) recorded for candidates considered: `gpui` screened out despite the strongest terminal precedent (Zed's own terminal panel) on a ~9-month crates.io publish gap (last release 2025-10-22) versus `egui`'s ~1 month; `xilem` deprioritized as explicitly alpha; `relm4`/GTK deprioritized on native-dependency weight without full evaluation. `iced`'s own dependency weight (`Cargo.lock` 50 → 395 packages, +345) is recorded in `qa-evidence.md` as a PR-014-F input, applied asymmetrically relative to `relm4` since `iced` was assigned by the handoff rule rather than screened.
- [x] Selection rationale recorded in `qa-evidence.md` PR-014-B.

## Criteria Checklist (C1-C14)

- [x] **C1** Terminal grid renders the RFC-009 accepted subset correctly. Real PTY-backed shell (`/bin/sh`) with printed output, prompt, and SGR colors, screenshot in `evidence/pr-014-c/`.
- [x] **C1** At least three unsupported families render visibly inert. OSC 52 clipboard, OSC title (window title independently verified unchanged via `niri`/`xdotool`), and OSC 8 hyperlink all confirmed inert; see qa-evidence.md PR-014-C.
- [x] **C1** Styled spans render — multiple colors/attributes in one text block. "red"/"green"/"bold-blue" render in distinct correct colors within one line, resolved via `Term::renderable_content()`.
- [x] **C2** Typing latency measured; p50/p95/p99 recorded against ≤16 ms p95, ≤33 ms p99. 1,012 post-warmup samples, all 0ms. **Budget not verified** — the only available frame-observation hook (`iced::window::frames()`) forces continuous redraw once subscribed, so input-to-frame measures frame availability rather than rendering cost. Numbers recorded in qa-evidence.md; substrate responsiveness against this budget remains unverified and carries to M8.
- [x] **C3** Terminal input latency measured under background output flood; p95 against ≤16 ms. 1,015 post-warmup samples, all 0ms. **Budget not verified**, same instrumentation limitation as C2 (see above).
- [x] **C4** Mode switch measured against ≤32 ms p95; absence of animation confirmed. 1,000 post-warmup samples, all 0ms. **Budget not verified**, same instrumentation limitation as C2 (see above). No-animation *is* independently confirmed by code inspection (no interpolation/tween/`iced::animation` in `shell.rs`'s view branching) — that sub-requirement is not affected by the frame-observation limitation.
- [x] **C5** Warm startup measured against ≤800 ms. 14 warm runs (1 cold discarded): median 227.9ms, max 255.5ms — met comfortably, and unaffected by the C2-C4 instrumentation caveat (only the first frame after a cold start is timed).
- [x] **C6** Idle RSS recorded as a baseline figure. 178,124 kB after 60s idle with one project + one terminal open (176,176 kB immediately after opening the terminal).
- [x] **C7** Per-pane column count computed from real font metrics at 1x and fractional scaling. Measured headlessly via `iced::advanced::graphics::text::Paragraph` (the real `cosmic-text`-backed layout primitive, not a guess): 7.8 logical px/glyph at 13px monospace. Applied to the real, running desktop's actual fractional scale (1.2x, recorded in machine identification) via the i18n screenshots' correct non-garbled rendering — the invariance is exercised, not just asserted. Limitation: the terminal's PTY grid remains hard-coded 80×24 and does not consume this computation (recorded, not hidden).
- [x] **C8** Trusted UI structurally separable; screenshot evidence produced. Genuine dialog rendered via `iced::widget::stack`/`opaque` (a real GUI layer, not terminal-grid characters) alongside an adversarial box-drawing imitation printed inside the terminal pane; both appear in one frame in `evidence/pr-014-d/genuine-and-adversarial-dialog-one-frame.png`, closing the RFC-009 deferral.
- [x] **C9** All primary spike workflows reachable by keyboard; focus trapping in dialog. Static-shell focus (PR-014-B) plus dialog focus trapping (PR-014-D, real `Tab`/`Enter` input, screenshots in `evidence/pr-014-d/`) — both halves now exist.
- [x] **C10** Non-Latin script renders in editor and terminal surfaces. CJK (Simplified Chinese, Japanese) and Arabic (RTL) all render in both surfaces (`evidence/pr-014-e/i18n-{editor,terminal}-surface.png`). Two disclosed rendering-fidelity gaps in the terminal surface specifically: no bidi reordering (Arabic shows in raw cell order, not visually RTL-reordered) and no wide-cell CJK (single-width cells, not double) — both are properties of the terminal-grid rendering path, not spike-introduced bugs, and neither affects the editor surface.
- [x] **C11** Accessibility affordances assessed; focus indicators visible; screen-reader path identified or its absence recorded. Focus indicators visible (PR-014-B/D). Screen-reader path: absence recorded, not assessed as present — `iced` 0.14 has no accessibility bridge at all (grepped for `accesskit`/`accessibility`/`a11y`, zero matches in `iced`/`iced_winit` source or manifest).
- [x] **C12** No known blocker to Windows/macOS identified, or blockers recorded. Not attempted (non-goal, §8). Noticed without chasing: the spike's dependency on `tekstide_core::runtime::terminal::LinuxTerminalRuntime` is a concrete Linux-only blocker at the terminal-runtime layer specifically; `iced` and `alacritty_terminal`/`vte` are not known to be Linux-only upstream.
- [x] **C13** Licence inventory complete for every crate introduced so far, including transitive dependencies (`iced` in PR-014-B; `alacritty_terminal`, `vte`, `tokio`, and 8 further transitives in PR-014-C). No bundled native C code introduced by this slice, unlike RFC-013's `rusqlite`.
- [x] **C14** Maintenance posture assessment recorded for the chosen substrate (`iced`) and the screened alternatives (`gpui`, `xilem`, `relm4`) in PR-014-B.

## Terminal Strategy Checklist

- [x] Option A tested: RFC-009 policy interposed in front of the emulator.
- [x] Interposition proven **non-bypassable** for the corpus tested (18 tests, including exhaustive chunk-boundary splitting for the mandatory minimum families plus response-106-supplied V5/V6/V7 probes), with two documented limitations (see qa-evidence.md: `linefeed()` also admits VT/FF — constraint-forced; `clear_screen` forwards mode-blind including scrollback erasure — a choice, raised as an RFC-017 question; and 63 of 71 `Handler` methods are blocked by omission rather than individually classified).
- [x] Demonstration that a blocked family cannot reach emulator state. Proven three ways: unit-test grid-content assertions, the filter's own `blocked` log, and an independent real-window check (window title never changed per both `niri msg windows` and `xdotool getwindowname` after sending an OSC-0 title-set sequence).
- [ ] If Option A falsified, Option B evaluated as fallback with reasons recorded. **N/A — Option A was not falsified**, so this item has no applicable evidence to check; left unchecked rather than checked-as-satisfied to avoid implying Option B work happened.
- [x] Option C not adopted without maintainer sign-off and a threat-model amendment. (Trivially true: Option C was never adopted or considered further once Option A held.)

## Trusted-UI Evidence Checklist

- [x] Genuine dialog rendered outside the terminal surface. `iced::widget::stack`/`opaque` overlay, not terminal-grid content.
- [x] Adversarial imitation generator committed and reproducible. `crates/tekstide-gui-spike/adversarial-dialog.sh`, included via `include_str!` so the committed file and the bytes that run cannot drift; includes an 8-bit C1 case per response 106 Q3.
- [x] Screenshot shows both in one frame. `evidence/pr-014-d/genuine-and-adversarial-dialog-one-frame.png`.
- [x] A reviewer can distinguish real from imitation using the screenshot alone. Genuine dialog: opaque GUI layer, sharp border, highlighted focused button. Imitation: dimmer terminal text scrolling with ordinary shell output.
- [x] Terminal output cannot move keyboard focus out of the real dialog. Structural, not just observed: `DialogButton` focus is a separate state field the terminal pane has no message path to reach; demonstrated with real `Tab` input in `evidence/pr-014-d/focus-trap-tab-cycles-approve-deny.png`.

## Measurement Integrity Checklist

- [x] All figures from release builds; no debug-build numbers recorded. All PR-014-E figures (C2-C7) from `cargo build --release`.
- [x] ≥1,000 samples for latency criteria; first 100 discarded as warmup. C2: 1,012 post-warmup. C3: 1,015. C4: 1,000.
- [x] p50, p95, and p99 all reported. All three reported for C2/C3/C4 (all 0ms; see qa-evidence.md's continuous-redraw finding for why that number is degenerate rather than a clean pass).
- [x] Machine identification recorded: CPU, RAM, GPU/driver, compositor, OS, Rust version. Recorded in qa-evidence.md PR-014-E, including non-standard display mode (2560x1440@59.951Hz, non-native) and non-integer scale (1.2x).
- [x] Latency described as **app-internal**, not end-to-end. Stated explicitly, plus the further instrumentation-limitation caveat that makes the C2-C4 numbers even narrower than plain app-internal.
- [x] Missed budgets handled per the escalation policy; any >2x miss reported when confirmed, not deferred to closeout. C2/C3/C4 are **not verified** rather than met or missed (see above) — the escalation policy governs missed budgets and was not triggered, since no budget was actually measured against. The instrumentation-invalidity finding (a different category from a budget miss) was still surfaced prominently and immediately, matching the policy's spirit of not deferring a significant finding to closeout.

## Honesty Checklist

- [x] Every criterion that could not be evaluated is listed with a reason. C11 (screen-reader half) and C12 (beyond noticed-in-passing) in qa-evidence.md's Criteria Not Evaluated.
- [x] No capability inferred from documentation without exercising it. The `window::frames()` continuous-redraw side effect was discovered by direct CPU-tick measurement, not inferred from its doc comment; the C7 font-metrics figure is a real measured value from iced's own layout primitive, not a guessed constant; the C11 accessibility-bridge absence was confirmed by grepping actual source, not assumed.
- [x] Findings that falsify the provisional plan are recorded prominently, not buried. The continuous-redraw/degenerate-latency finding is stated at the top of the PR-014-E section, before any table, with an explicit "read this before the tables" framing — not left to a trailing limitations bullet.

## Evidence Required

- [ ] Commit/PR list. (To be added once this slice's commit lands.)
- [x] Gate command output. Recorded for PR-014-B, PR-014-C, and PR-014-E in qa-evidence.md.
- [x] Latency measurement tables with method. C2/C3/C4 tables plus C5 startup table, all with method described, in qa-evidence.md PR-014-E.
- [x] Trusted-UI screenshots and generator script. Recorded in PR-014-D (unchanged this slice).
- [x] Licence and dependency-weight inventory. Recorded for PR-014-B (`iced`, +345 packages) and PR-014-C (`alacritty_terminal`/`vte`/`tokio`, +11 packages) in qa-evidence.md. PR-014-E added the `advanced` feature flag to the existing `iced` dependency, not a new crate.
- [x] Criteria-not-evaluated list. Consolidated list now in qa-evidence.md (C11 screen-reader half, C12 beyond noticed-in-passing).
- [x] Known limitations. Consolidated list now in qa-evidence.md, including the PR-014-E-specific instrumentation-limitation and delivery-loss findings.

## Final Acceptance Decision

- [ ] Accepted — decision record may proceed.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Substrate disqualified — return to candidate selection.

Reviewer notes:

```text
Pending spike evidence.
```
