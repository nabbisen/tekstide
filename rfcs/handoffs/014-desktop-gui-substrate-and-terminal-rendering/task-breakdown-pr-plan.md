---
title: "RFC-014: Desktop GUI Substrate and Terminal Rendering Strategy - Task Breakdown and PR Plan"
rfc: "RFC-014"
rfc_file: "../../proposed/014-desktop-gui-substrate-and-terminal-rendering.md"
target_milestone: "M8"
source_rfc_status: "Proposed — criteria accepted"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-014 Task Breakdown and PR Plan

Six slices. PR-014-A is complete. PR-014-B through PR-014-E are spike implementation and measurement. PR-014-F is the decision record and is design work, not developer work.

## PR-014-A — RFC and criteria acceptance ✔ complete

Accepted 2026-07-28. Criteria C1-C14, the candidate set, and the TUI rejection are settled. RFC-014's four open questions are resolved in the handoff pack README.

## PR-014-B — Spike harness and candidate selection

Scope:

- Create `crates/tekstide-gui-spike` with `publish = false`; add to workspace members.
- Bounded paper survey (half-day timebox) selecting the second candidate per the selection rule in `implementation-handoff.md` §2. Screen on licence and maintenance posture **before** writing code.
- Window rendering a static Content Mode shell: sidebar, main area, status bar, at the accepted layout proportions from the external design.
- Keyboard focus model: focus moves between sidebar and main area without mouse.

Review gate:

- Quarantine verified — no product crate depends on the spike.
- Second-candidate selection and rationale recorded, including anything screened out on licence (C13) or maintenance (C14).
- Standard workspace gates pass.

Reviewer focus: is the candidate selection reasoned rather than assumed, and is the quarantine boundary real?

## PR-014-C — Terminal surface and the Option A/B question

The highest-value slice. May be split if the interposition investigation proves large.

Scope:

- PTY-backed terminal pane driven by the existing `LinuxTerminalRuntime`.
- Resolve the Option A/B question: can the RFC-009 accepted-sequence policy be interposed as a **non-bypassable** filter in front of the chosen emulator?
- Render the RFC-009 accepted families correctly.
- Render at least three unsupported families visibly inert.
- Render styled spans — multiple colors and attributes in one text block.
- Narrow C1/C7 probe on the second candidate for comparison.

Review gate:

- Option A proven implementable **or** falsified with specifics. Falsification is a successful outcome; fall back to Option B and say why.
- Demonstration that a blocked family cannot reach emulator state.
- Option C is **not** available without maintainer sign-off and a threat-model amendment.
- Licence inventory for every crate introduced, per the RFC-013 `NOTICE` precedent.

Reviewer focus: is the filter genuinely non-bypassable, or only untested? I will probe this empirically, as with RFC-013's SQL constraints.

## PR-014-D — Trusted-UI evidence

Scope:

- Modal dialog rendered outside the terminal surface, with keyboard focus trapped.
- Committed script generating adversarial terminal output that imitates a Tekstide dialog.
- Screenshots showing genuine dialog and adversarial imitation **in one frame**.
- Demonstration that terminal output cannot move focus out of the real dialog.

Review gate:

- A reviewer looking only at the screenshot can tell which dialog is real. If not, C8 has failed — a major finding, report it as such.
- Adversarial generator is committed and reproducible, not a one-off.

This slice closes the `screenshot-backed spoofing evidence` deferral from RFC-009.

## PR-014-E — Measurement

Scope:

- C2 typing latency, C3 terminal input latency under output flood, C4 mode switch, C5 warm startup, C6 idle RSS, C7 font metrics, C10 i18n rendering.
- All figures from **release builds**, ≥1,000 samples for latency criteria, p50/p95/p99 reported, first 100 samples discarded as warmup.
- Machine identification and method recorded.
- Explicit list of criteria that could **not** be evaluated, with reasons.

Review gate:

- Method recorded, not just numbers.
- App-internal latency described as such — not as end-to-end.
- Missed budgets handled per the escalation policy in `implementation-handoff.md` §5. A >2x miss should have stopped the spike early and been reported at the time, not saved for this gate.

## PR-014-F — Decision record

Design work, authored by the architect after PR-014-E evidence lands.

Scope:

- Amend RFC-014 with the chosen substrate and terminal-rendering strategy.
- Name rejected alternatives with reasons.
- Record the maintenance and licence assessment.
- Record any NFR amendment recommended under the escalation policy.
- Specify what happens to the spike crate — archived or deleted.

Review gate:

- **Maintainer sign-off required before M8 implementation begins.** Substrate is among the most expensive decisions to reverse.
- No product code depends on the chosen substrate before this record is accepted.

## Sequencing

PR-014-B blocks everything. C, D, and E are sequential in practice — D needs a rendered dialog and a terminal surface from C; E needs both. F needs E.

If PR-014-C falsifies both Option A and Option B, stop and escalate rather than proceeding to D and E. The terminal strategy is load-bearing for the whole substrate choice, and there is no value in measuring a substrate that cannot render terminals safely.
