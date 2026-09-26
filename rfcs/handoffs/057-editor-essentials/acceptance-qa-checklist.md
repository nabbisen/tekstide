---
title: "RFC-057 — acceptance and QA checklist"
rfc: "RFC-057"
rfc_file: "../../accepted/057-editor-essentials.md"
source_rfc_status: "Accepted 2026-09-26 — M10 remainder"
target_milestone: "M10 remainder"
created: "2026-09-26"
---

# Acceptance and QA checklist

Tick a box when the evidence is in `qa-evidence.md`, not when the code looks right. A box that says
*measured* with no number in the evidence is not ticked.

## PR-057-A — the baseline

- [x] A 100 000-line fixture is committed, and the measurement is repeatable by the reviewer.
      *(Committed as a **generator** with a pinned size, line count and checksum, not as a 3.3 MB blob — a stated deviation. Command in `qa-evidence.md`.)*
- [x] Typing latency **p95 and p99 as numbers**, against 16 ms / 33 ms, measured on `main` before any
      rendering change.
      *(p95 14.0–16.3 ms, p99 14.0–16.6 ms over three runs; a lower bound, painting excluded.)*
- [x] No product code changed in this slice.
      *(Two files, both test code; the diff is in `qa-evidence.md`.)*
- [x] The number is in the changelog whatever it says.
      *(`Unreleased`, *Measured*.)*

## PR-057-B — rows, bounded by the viewport

- [x] What is drawn is the viewport's window, not the file — proved by what is **built per frame**,
      not by a screenshot that looks fast.
- [x] `TextViewport::first_visible_line` has a reader for the first time since RFC-006.
- [x] The viewport follows the cursor; no scrollbar, no wheel handling, navigation still four keys.
- [x] **One drawn line is one assertable string**, testable without `iced`.
- [x] A's measurement re-run, both numbers reported.
      *(p95 14.0–16.3 ms → 5.3–6.8 ms; one run under load 11 missed the 8 ms line and is kept in `batch-1/`.)*

### Required at review 441

- [x] **Q1: the body never hands the whole file to one widget.** Measured in A: an unbounded layout
      costs **~745 ms a keystroke**. A scrollable body, or anything that lets the widget see past the
      visible height, pays it. A test holds the property, with A's reference number beside it.
- [x] **The 8 ms rule (ruled at 441):** B re-measures. Bounding the rows should leave roughly 4.4 ms
      `update` plus a fraction — about 5 ms. If p95 does not land **under 8 ms**, the per-keystroke
      document copy comes into scope in this release, said at review rather than tuned around.

## PR-057-C — the gutter and the caret

- [ ] Line numbers come from the real line index, not the row's position in the window.
- [ ] **The caret is an element, not a character**: the fixture contains a file whose text holds the
      caret's own character, and the two are distinguishable. Ablated.
- [ ] **One cursor, one reader**: a planted second derivation fails a test.
- [ ] A file crossing 10 000 lines does not shift its text column while scrolling. Captured at 5 and
      6 digits.
- [ ] `REQ-EDIT-002` met, **and its coverage row corrected** from implying it already was.

### Required at review 442

- [ ] **Q2: a horizontal window that follows the cursor**, on the same rule as the vertical one —
      the least movement that keeps it on screen. **Not** soft wrap: exact one-line-one-row
      arithmetic is the property B rests on. Ruled from B's own capture, which showed *Line 1,
      Column 525* while the visible text ended near column 60 — a user typing into text they cannot
      see, and a caret that could not be visible at all on that line.
- [ ] A capture of a long line with the cursor visible, and the changelog correcting B's "clipped at
      the right edge with no horizontal scroll" **by name**.

## PR-057-D — undo

- [ ] Operations, not snapshots; the depth is bounded and the bound is **stated when reached**.
- [ ] Typing then undoing restores text **and cursor**.
- [ ] Undoing to the opened text returns the document to `Clean`.
- [ ] Undo does not cross an external-change reload and write over a file that moved underneath.
- [ ] Redo then save writes what the screen showed.
- [ ] The requirements gap (no `REQ-EDIT` names undo) is written up for the owner; no requirement is
      minted.

## PR-057-E — the two-row header

- [ ] The actions row carries the title and sits above the tab strip.
- [ ] Captured, and **every superseded capture named** across the evidence packs.

## Whole-RFC

- [ ] `REQ-EDIT-002` moves to met with evidence a user can see; `NFR-REL-005` records undo;
      `NFR-PERF-003` has a measured number for the first time.
- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh short `TMPDIR`.
- [ ] Every new intermittent failure has a dated row in `test-process-leak.md`.
- [ ] The core pin bumps with the version (`the_workspace_pins_tekstide_core_to_its_own_version`).
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
