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
- [ ] **C2** Typing latency measured; p50/p95/p99 recorded against ≤16 ms p95, ≤33 ms p99.
- [ ] **C3** Terminal input latency measured under background output flood; p95 against ≤16 ms.
- [ ] **C4** Mode switch measured against ≤32 ms p95; absence of animation confirmed.
- [ ] **C5** Warm startup measured against ≤800 ms.
- [ ] **C6** Idle RSS recorded as a baseline figure.
- [ ] **C7** Per-pane column count computed from real font metrics at 1x and fractional scaling.
- [ ] **C8** Trusted UI structurally separable; screenshot evidence produced.
- [ ] **C9** All primary spike workflows reachable by keyboard; focus trapping in dialog. (Partial evidence exists for the static shell from PR-014-B; dialog focus trapping is PR-014-D scope, so this stays unchecked until that half exists too.)
- [ ] **C10** Non-Latin script renders in editor and terminal surfaces.
- [ ] **C11** Accessibility affordances assessed; focus indicators visible; screen-reader path identified or its absence recorded.
- [ ] **C12** No known blocker to Windows/macOS identified, or blockers recorded.
- [x] **C13** Licence inventory complete for every crate introduced so far, including transitive dependencies (`iced` in PR-014-B; `alacritty_terminal`, `vte`, `tokio`, and 8 further transitives in PR-014-C). No bundled native C code introduced by this slice, unlike RFC-013's `rusqlite`.
- [x] **C14** Maintenance posture assessment recorded for the chosen substrate (`iced`) and the screened alternatives (`gpui`, `xilem`, `relm4`) in PR-014-B.

## Terminal Strategy Checklist

- [x] Option A tested: RFC-009 policy interposed in front of the emulator.
- [x] Interposition proven **non-bypassable** for the corpus tested (14 tests, including exhaustive chunk-boundary splitting for the mandatory minimum families), with two documented limitations (see qa-evidence.md: `linefeed()` also admits VT/FF, and ~100 of ~119 `Handler` methods are blocked by omission rather than individually classified).
- [x] Demonstration that a blocked family cannot reach emulator state. Proven three ways: unit-test grid-content assertions, the filter's own `blocked` log, and an independent real-window check (window title never changed per both `niri msg windows` and `xdotool getwindowname` after sending an OSC-0 title-set sequence).
- [ ] If Option A falsified, Option B evaluated as fallback with reasons recorded. **N/A — Option A was not falsified**, so this item has no applicable evidence to check; left unchecked rather than checked-as-satisfied to avoid implying Option B work happened.
- [x] Option C not adopted without maintainer sign-off and a threat-model amendment. (Trivially true: Option C was never adopted or considered further once Option A held.)

## Trusted-UI Evidence Checklist

- [ ] Genuine dialog rendered outside the terminal surface.
- [ ] Adversarial imitation generator committed and reproducible.
- [ ] Screenshot shows both in one frame.
- [ ] A reviewer can distinguish real from imitation using the screenshot alone.
- [ ] Terminal output cannot move keyboard focus out of the real dialog.

## Measurement Integrity Checklist

- [ ] All figures from release builds; no debug-build numbers recorded.
- [ ] ≥1,000 samples for latency criteria; first 100 discarded as warmup.
- [ ] p50, p95, and p99 all reported.
- [ ] Machine identification recorded: CPU, RAM, GPU/driver, compositor, OS, Rust version.
- [ ] Latency described as **app-internal**, not end-to-end.
- [ ] Missed budgets handled per the escalation policy; any >2x miss reported when confirmed, not deferred to closeout.

## Honesty Checklist

- [ ] Every criterion that could not be evaluated is listed with a reason.
- [ ] No capability inferred from documentation without exercising it.
- [ ] Findings that falsify the provisional plan are recorded prominently, not buried.

## Evidence Required

- [ ] Commit/PR list.
- [x] Gate command output. Recorded for PR-014-B and PR-014-C in qa-evidence.md.
- [ ] Latency measurement tables with method.
- [ ] Trusted-UI screenshots and generator script.
- [x] Licence and dependency-weight inventory. Recorded for PR-014-B (`iced`, +345 packages) and PR-014-C (`alacritty_terminal`/`vte`/`tokio`, +11 packages) in qa-evidence.md.
- [ ] Criteria-not-evaluated list. (Partial limitations recorded per-slice; the consolidated closeout list is PR-014-F scope, after PR-014-E lands.)
- [ ] Known limitations. (Recorded per-slice so far; consolidated at closeout.)

## Final Acceptance Decision

- [ ] Accepted — decision record may proceed.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Substrate disqualified — return to candidate selection.

Reviewer notes:

```text
Pending spike evidence.
```
