---
title: "RFC-057 — task breakdown and PR plan"
rfc: "RFC-057"
rfc_file: "../../accepted/057-editor-essentials.md"
source_rfc_status: "Accepted 2026-09-26 — M10 remainder"
target_milestone: "M10 remainder"
created: "2026-09-26"
---

# Task breakdown and PR plan

Five slices. **A changes no product code** and comes first because everything after it is judged
against the number it produces.

## PR-057-A — measure `NFR-PERF-003` against today's editor

A 100 000-line fixture and a typing-latency measurement, run against `main` as it stands: the whole
file in one `text` widget, a whole-document `String` per keystroke.

- p95 and p99, from input arriving to the edit applied, reported as **numbers** against 16 ms / 33 ms.
- The fixture is committed and the measurement is repeatable by the reviewer, not a figure in a file.
- **Whatever the result, it is published.** If today already misses the budget, that is this
  release's finding, and whether the fix fits in B is then a decision with evidence behind it.
- No product change in this slice. It is the baseline, and a baseline taken after the rewrite is not
  one.

## PR-057-B — rows, bounded by the viewport

`surface/editor.rs`, and the viewport's first reader.

- The body becomes one drawn line per visible line, and what is visible is
  `TextViewport::first_visible_line` plus the height — **the window, not the file** (D1). This field
  has existed since RFC-006 and nothing has ever read it.
- **The viewport follows the cursor** (D9). No scrollbar, no wheel handling; navigation is still the
  four arrow keys, and adding more is not this RFC's.
- **One drawn line is one assertable string** (D8), testable without `iced`, the way `node_line` is.
- Re-measure A's number and report both.

## PR-057-C — the gutter and the caret

- Line numbers beside each drawn line, from the real line index, not the row's position in the
  window.
- **The caret is an element, never a character** (D2). The fixture holds a file whose text contains
  the caret's own character, and a test asserts the two are distinguishable.
- **One cursor, one reader** (risk §2). The ablation plants a second, independently derived position;
  a test fails.
- The gutter's width is data-dependent: a file crossing 10 000 lines must not shift its text column
  while scrolling. Captured at 5 and 6 digits.
- `REQ-EDIT-002` is complete at the end of this slice, and **its coverage row is corrected** from
  implying it already was (D5).

## PR-057-D — undo

- **Operations, not snapshots** (D3). The vocabulary is four invertible edits — a character, `Enter`,
  `Space`, `Backspace`. A snapshot stack would multiply a 4 MiB cap by its depth.
- Bounded depth, **stated**, and the product says when the bound is reached rather than silently
  dropping the oldest.
- The four rows of the risk document's §3 table are four tests: undo to `Clean`; the bound announced;
  undo refusing to cross an external-change reload; redo-then-save writing what the screen showed.
- Undo restores the **cursor** as well as the text.
- Recorded under `NFR-REL-005`, and the requirements gap — no `REQ-EDIT` names undo — written up for
  the owner (D4). Do not mint a requirement.

## PR-057-E — the two-row header

Owner-requested, relayed at review 438. **Not editor work**; it is here because it is the only
scheduled work touching the frame.

- `shell.rs:7520–7522` stacks `window_title`, `project_tab_strip`, `top_bar_actions_row`. Make the
  actions row carry the title and put it **above** the tab strip (D10).
- Captured, and **every superseded capture named** — every image in the evidence packs showing the
  top of the window.

## Order, and why

A first because a baseline taken afterwards is not a baseline. B before C because a gutter and a
caret need rows to sit on. C before D because undo has to restore a cursor that something draws. E
last and alone, so the capture churn lands once, at the end, on a product that is otherwise finished.
