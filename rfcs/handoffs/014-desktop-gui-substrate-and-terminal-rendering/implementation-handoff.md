---
title: "RFC-014: Desktop GUI Substrate and Terminal Rendering Strategy - Implementation Handoff"
rfc: "RFC-014"
rfc_file: "../../proposed/014-desktop-gui-substrate-and-terminal-rendering.md"
target_milestone: "M8"
source_rfc_status: "Proposed — criteria accepted"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-014 Implementation Handoff — GUI Substrate Spike

This handoff covers PR-014-B through PR-014-E: a disposable, quarantined spike that produces the evidence for the substrate decision. PR-014-F (the decision record) is design work and is not in scope here.

**The spike's job is to falsify, not to demonstrate.** A spike that only shows things working has failed at its purpose. Record what does not work, what could not be evaluated, and what surprised you — those findings are more decision-relevant than a working window.

## 1. Quarantine rules

Follow the `tekstide-pty-spike` precedent from RFC-007 exactly.

- New crate `crates/tekstide-gui-spike` with `publish = false` in its manifest.
- Added to the workspace `members` list so it builds and lints with everything else.
- **No product crate may depend on it**, and it must not be added to `tekstide` or `tekstide-core` dependencies.
- It **may** depend on `tekstide-core` (read-only use of existing models and the Linux terminal runtime). That direction is fine; the reverse is not.
- It must not write to the audit store, transcript store, recent-project state, or any real Tekstide state root. If it needs state, use a temp directory.
- It must not perform network access.

Verification that quarantine holds:

```sh
grep -n "tekstide-gui-spike" crates/tekstide/Cargo.toml crates/tekstide-core/Cargo.toml \
  && echo "QUARANTINE BROKEN" || echo "quarantine intact"
grep -n "publish" crates/tekstide-gui-spike/Cargo.toml   # expect publish = false
```

## 2. Candidate selection (RFC-014 open question 2, resolved)

**Primary candidate: `iced`.** It is named in the maintainer's stated expertise, which is a real C14 maintenance factor on a small team — substrate familiarity reduces long-term cost more than marginal technical differences.

**Second candidate: selected by you, using this rule.** Evaluating one candidate alone produces an unfalsifiable result, so a comparison baseline is required — but a full second shell would double spike cost for little return. Therefore:

- Run a **bounded paper survey** (timebox: half a day) of pure-Rust GUI toolkits. Shortlist to consider: `egui`, `slint`, `gpui`, `xilem`, and GTK bindings such as `relm4`. This list is a starting point, not exhaustive — add anything current you find.
- **Selection rule: choose the candidate with the strongest existing precedent for text-grid or terminal rendering**, because that is the discriminating risk. A toolkit someone has already rendered a terminal in is worth more as a comparison than a toolkit with nicer buttons.
- Screen out on C13 (licence compatible with Apache-2.0 distribution) and C14 (maintenance posture, native-dependency weight) **before** writing any code. A GPL or royalty-encumbered licence is a hard stop; record it and move to the next.
- The second candidate gets a **narrow probe only** — criteria C1 (terminal grid rendering) and C7 (font metrics). Not a full shell, not the dialog work, not the full measurement suite.
- Record your selection and reasoning in PR-014-B. If the survey shows no viable second candidate, that is a legitimate finding — record it and proceed with one, noting the reduced confidence.

Web research is expected here. Crate ecosystems move faster than any document; verify current state rather than trusting this list.

## 3. Terminal rendering — the central investigation

> **Read `pr-014-c-filter-interposition.md` before starting this section or choosing a terminal-emulation crate.** It carries the detailed instructions: the four claims the filter must establish, eight bypass vectors to test, the adversarial-corpus format, what will be probed at review, and one concrete bypass — chunk-boundary sequence splitting — that a naive implementation will hit by accident. This section is the summary; that document is the specification.

This is the highest-value part of the spike. RFC-014 poses three options:

- **Option A** — adopt a terminal-emulation crate as the grid/parser engine, with the RFC-009 policy interposed *before* it so unsupported sequence families never reach the emulator.
- **Option B** — extend the existing reviewed RFC-009 parser into a cell-grid model and render that directly.
- **Option C** — adopt an emulator unfiltered and amend RFC-009.

**Provisional lean is Option A. Your job is to prove or falsify that it is implementable.**

The specific question to answer: **can the RFC-009 accepted-sequence policy be interposed as a non-bypassable filter in front of the chosen emulator?** Concretely —

- Does the emulator crate accept a byte stream through an API you control, so all input passes your filter first?
- Or does it expose internal state mutation paths that bypass the byte stream?
- Can you demonstrate that a blocked family (say OSC 52 clipboard, or title mutation) cannot reach the emulator's state at all?

If the filter is bypassable, or the API does not permit interposition, **Option A is falsified — report that and fall back to Option B.** Do not quietly adopt Option C; it reopens an accepted, evidence-backed security decision and requires maintainer sign-off plus a threat-model amendment.

The existing accepted subset and inert-family list are pinned in `rfcs/done/009-terminal-security-boundary.md`. Read it before choosing an emulator. The relevant implementation is `crates/tekstide-core/src/runtime/terminal/security/`.

**Minimum rendering evidence:**

- The RFC-009 accepted families render correctly (text, colors, cursor movement, erase, scroll region).
- At least three unsupported families render **visibly inert** — pick from OSC 52 clipboard, title mutation, OSC 8 hyperlinks, DCS/PM/APC, mouse reporting.
- **Styled spans** render correctly — multiple colors and attributes within one text block. This is required by the terminal renderer anyway, and it is the one thing the spike must confirm on behalf of the deferred syntax-highlighting work (RFC-014 open question 4).

## 4. Measurement methodology

Measurements done badly are worse than no measurements, because they get quoted later. Specify and record method, not just numbers.

**Mandatory conditions:**

- **Release builds only** (`cargo build --release`). Debug-build latency figures are meaningless and must not be recorded.
- Record machine identification: CPU, RAM, GPU/driver, compositor (X11/Wayland), OS version, Rust version.
- Record whether the display is running at a non-standard refresh rate or scaling factor.
- Minimum **1,000 samples** for input-latency criteria; report **p50, p95, and p99**, not p95 alone.
- Discard the first 100 samples as warmup and say so.

**What "latency" means here — be precise and do not overclaim:**

Measure from **input event receipt in the application** to **frame submitted for presentation**. This is *app-internal latency*. It excludes input-stack latency before the app sees the event and compositor/display latency after submission. State that limitation explicitly in the evidence. Do not describe these figures as end-to-end or photon-to-photon; they are not, and the NFR budgets will be misread if you imply otherwise.

**Per-criterion method:**

| Criterion | Method |
| --- | --- |
| C2 typing latency | Editor surface with a large document loaded. Synthetic keystrokes at a realistic rate. Budget: p95 ≤ 16 ms, p99 ≤ 33 ms |
| C3 terminal input latency | Terminal pane with a **background process flooding output** (RFC-007's flood harness is a starting point). Budget: p95 ≤ 16 ms |
| C4 mode switch | Content ↔ Terminal toggle, measured input-to-frame. Budget: p95 ≤ 32 ms, and confirm no animation |
| C5 warm startup | Process start → window painted with content. Warm = second and subsequent runs. Budget: ≤ 800 ms |
| C6 idle memory | RSS after 60 s idle with one project and one terminal open. **Baseline figure, not pass/fail** — record it |
| C7 font metrics | Demonstrate computing per-pane column count from real font metrics at 1x and a fractional scaling factor |
| C10 i18n | Render a non-Latin script (CJK and one RTL script if practical) in both editor and terminal surfaces |

For C6, note that `/usr/bin/time -v` was unavailable during the 0.3.0 release baseline. If it is still missing, use `/proc/self/status` `VmRSS` or an equivalent and say which.

## 5. Missed performance budget — escalation policy (open question 3, resolved)

Requirements §8.1 already states the NFR values are *"initial measurable targets... Exact values may be calibrated by a later performance RFC."* Calibration with evidence is therefore sanctioned. But it must be a recorded decision, not a quiet slide.

| Result | Action |
| --- | --- |
| Budget met | Record and proceed |
| Missed, and cause is spike-harness overhead rather than substrate-inherent | Re-measure in a more representative harness before drawing any conclusion. Do not report the first figure as the substrate's |
| Missed by ≤ 25%, substrate-inherent | Record as a limitation with evidence and a mitigation path. Recommend an NFR amendment in PR-014-F; maintainer decides |
| Missed by > 2x, substrate-inherent | Treat as substrate-disqualifying. Report immediately rather than completing the remaining criteria |
| Between 25% and 2x | Escalate to maintainer before continuing. This is a judgement call above the spike's remit |

Report a missed budget **as soon as it is confirmed**, not at the end. A disqualifying result should stop the spike early — that is the spike working correctly.

## 6. Trusted-UI evidence (PR-014-D)

This closes a deferral RFC-009 made explicitly: *"screenshot-backed spoofing evidence"* was left to the GUI milestone.

Produce a screenshot showing, **simultaneously and in one frame**:

1. A genuine Tekstide modal dialog — approval-shaped is ideal — rendered **outside** the terminal surface.
2. Adversarial terminal output inside the terminal pane that **imitates** a Tekstide dialog: box-drawing characters, a title like `Command Approval Required`, and fake `[Approve] [Deny]` affordances.

The test a reviewer applies: **can someone looking only at this screenshot tell which one is real?** If the answer is not obviously yes, the substrate has failed C8 and that is a major finding.

Generate the adversarial output with a script committed to the spike, so the evidence is reproducible rather than a one-off. Retain both the script and the screenshots in `qa-evidence.md`.

Also demonstrate: keyboard focus is trapped in the real dialog, and terminal output cannot move focus out of it.

## 7. When you cannot evaluate something

Say so explicitly. RFC-014's evidence requirements include *"explicit statement of every criterion the spike could not evaluate, and why."*

Do not:

- infer a capability from documentation without exercising it;
- report a figure from a debug build;
- describe app-internal latency as end-to-end;
- mark a criterion satisfied because it "should" work.

An honest "could not evaluate C11 screen-reader labelling because no accessibility bridge exists for this toolkit on Linux" is a genuinely useful finding. A vague pass is not.

## 8. Non-goals

Do not build:

- production Project Board, editor, or explorer behavior;
- persistence of any kind;
- audit, transcript, or change-detection integration;
- theming, icon sets, or visual polish beyond what a criterion requires;
- syntax highlighting (styled-span rendering only — see §3);
- Windows or macOS builds. Record cross-platform blockers you notice, but do not chase them.

## 9. Gates

Every PR in this series runs the standard workspace gates, since the spike is a workspace member:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
git diff --check
```

The spike is expected to carry few or no unit tests — it is measurement code. That is acceptable; do not pad it with tests to look thorough. The evidence file is the deliverable.
