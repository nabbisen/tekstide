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

- [ ] A 100 000-line fixture is committed, and the measurement is repeatable by the reviewer.
- [ ] Typing latency **p95 and p99 as numbers**, against 16 ms / 33 ms, measured on `main` before any
      rendering change.
- [ ] No product code changed in this slice.
- [ ] The number is in the changelog whatever it says.

## PR-057-B — rows, bounded by the viewport

- [ ] What is drawn is the viewport's window, not the file — proved by what is **built per frame**,
      not by a screenshot that looks fast.
- [ ] `TextViewport::first_visible_line` has a reader for the first time since RFC-006.
- [ ] The viewport follows the cursor; no scrollbar, no wheel handling, navigation still four keys.
- [ ] **One drawn line is one assertable string**, testable without `iced`.
- [ ] A's measurement re-run, both numbers reported.

## PR-057-C — the gutter and the caret

- [ ] Line numbers come from the real line index, not the row's position in the window.
- [ ] **The caret is an element, not a character**: the fixture contains a file whose text holds the
      caret's own character, and the two are distinguishable. Ablated.
- [ ] **One cursor, one reader**: a planted second derivation fails a test.
- [ ] A file crossing 10 000 lines does not shift its text column while scrolling. Captured at 5 and
      6 digits.
- [ ] `REQ-EDIT-002` met, **and its coverage row corrected** from implying it already was.

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
