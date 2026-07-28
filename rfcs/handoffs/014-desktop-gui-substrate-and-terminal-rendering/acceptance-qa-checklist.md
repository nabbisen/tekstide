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
- [x] Maintenance posture (C14) recorded for candidates considered: `gpui` screened out despite the strongest terminal precedent (Zed's own terminal panel) because Zed Industries paused community-facing development; `xilem` deprioritized as explicitly alpha; `relm4`/GTK deprioritized on native-dependency weight without full evaluation.
- [x] Selection rationale recorded in `qa-evidence.md` PR-014-B.

## Criteria Checklist (C1-C14)

- [ ] **C1** Terminal grid renders the RFC-009 accepted subset correctly.
- [ ] **C1** At least three unsupported families render visibly inert.
- [ ] **C1** Styled spans render — multiple colors/attributes in one text block.
- [ ] **C2** Typing latency measured; p50/p95/p99 recorded against ≤16 ms p95, ≤33 ms p99.
- [ ] **C3** Terminal input latency measured under background output flood; p95 against ≤16 ms.
- [ ] **C4** Mode switch measured against ≤32 ms p95; absence of animation confirmed.
- [ ] **C5** Warm startup measured against ≤800 ms.
- [ ] **C6** Idle RSS recorded as a baseline figure.
- [ ] **C7** Per-pane column count computed from real font metrics at 1x and fractional scaling.
- [ ] **C8** Trusted UI structurally separable; screenshot evidence produced.
- [ ] **C9** All primary spike workflows reachable by keyboard; focus trapping in dialog.
- [ ] **C10** Non-Latin script renders in editor and terminal surfaces.
- [ ] **C11** Accessibility affordances assessed; focus indicators visible; screen-reader path identified or its absence recorded.
- [ ] **C12** No known blocker to Windows/macOS identified, or blockers recorded.
- [ ] **C13** Licence inventory complete for every crate introduced, including transitive native dependencies.
- [ ] **C14** Maintenance posture assessment recorded for the chosen substrate.

## Terminal Strategy Checklist

- [ ] Option A tested: RFC-009 policy interposed in front of the emulator.
- [ ] Interposition proven **non-bypassable**, or falsified with specifics.
- [ ] Demonstration that a blocked family cannot reach emulator state.
- [ ] If Option A falsified, Option B evaluated as fallback with reasons recorded.
- [ ] Option C not adopted without maintainer sign-off and a threat-model amendment.

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
- [ ] Gate command output.
- [ ] Latency measurement tables with method.
- [ ] Trusted-UI screenshots and generator script.
- [ ] Licence and dependency-weight inventory.
- [ ] Criteria-not-evaluated list.
- [ ] Known limitations.

## Final Acceptance Decision

- [ ] Accepted — decision record may proceed.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.
- [ ] Substrate disqualified — return to candidate selection.

Reviewer notes:

```text
Pending spike evidence.
```
