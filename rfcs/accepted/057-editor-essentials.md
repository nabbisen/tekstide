# RFC-057: Editor Essentials

Status: **Accepted by the human owner 2026-09-26.** D1–D7 as written, plus D8–D11 — see *Decided on acceptance*. Proposed 2026-09-26. `0.28.0` in the authorised schedule. Finishes `REQ-EDIT-002` (line
numbers beside the cursor position it already shows), draws a caret, and adds **undo**, which no
requirement names and `NFR-REL-005` assumes.

## Summary

"Multi-document editing" is in the 1.0 minimum list. The editor it would multiply draws **the whole
file as one string, every frame**, has no line numbers, draws no caret, and cannot undo a keystroke.
The viewport field that would bound the drawing has existed since RFC-006 and **nothing reads it**.

This RFC makes the body rows bounded by that viewport, gives it a gutter and a caret that is a
widget rather than a character, and records every edit so it can be taken back. It also measures
`NFR-PERF-003` — typing latency in a 100 000-line file — which no one ever has.

It carries one piece of owner-requested work that is not about the editor: the two-row window header.

## What is true today, measured

| | Measured |
| --- | --- |
| 1 | **The body is one string.** `body_text` is `document.text().to_string()`, handed to a single `text` widget (`surface/editor.rs:111`, `:333`). The whole file is laid out every frame. |
| 2 | **`TextViewport { first_visible_line }` exists and the editor never reads it.** It is in the model (`content/document.rs:27`) with `viewport()`/`set_viewport()`; `first_visible_line` appears nowhere in `surface/editor.rs`. Dead since RFC-006. |
| 3 | **Every keystroke copies the whole document.** `apply_edit_key` returns `EditResult { text: String, cursor }` — a fresh `String` of the entire file per keypress — which `replace_text` then installs. |
| 4 | `DEFAULT_MAX_EDITABLE_BYTES` is **4 MiB**. `NFR-PERF-003` names *typing latency in a 100 000-line file, p95 ≤ 16 ms, p99 ≤ 33 ms*. **Nobody has measured it**, and 1–3 are what it would be measuring. |
| 5 | **No caret and no line numbers exist**: `caret`, `gutter` and `line_number` appear nowhere in the editor surface or the content module — zero hits, production and test. |
| 6 | **`REQ-EDIT-002` is half met.** It asks for *line numbers **and** cursor position*; `cursor_line` already renders the position (`editor.rs:68`). The half that is missing is the half this RFC adds. |
| 7 | **No undo, anywhere**: `undo` and `redo` are zero hits in the crate. `NFR-REL-005` protects against loss "from external file changes, close events, and process termination" — the three the product handles — and says nothing about a keystroke. |
| 8 | `TextDocumentState` is `Clean \| Dirty \| ExternalChanged \| Conflict \| SaveError`, so `REQ-EDIT-004` is met and undo has a state it must keep honest. |
| 9 | **The edit vocabulary is four keys**: a character, `Enter`, `Space`, `Backspace`. No delete-forward, no selection, no paste. Recorded as a finding; not this RFC's scope. |

## Decisions required

**D1 — The body becomes rows, and the viewport bounds them.** A gutter has to align with lines and a
caret has to sit on one; neither can be carried by a single string. What is drawn is the viewport's
window, not the file, and `first_visible_line` finally gets a reader. **Measured against a
100 000-line fixture, not asserted** — measurement 4 is the budget it is measured against.

**D2 — The caret is a widget, never a character.** A caret glyph inserted into the rendered text is
indistinguishable from the same character in the file, which is the class of untruth RFC-053 exists
to stop. The caret's line is composed — the text before it, the caret element, the text after — so
the caret is its own element and the text stays the file's.

**D3 — Undo records operations, not snapshots.** Measurement 9 makes this cheap: four invertible
operations. A snapshot stack would multiply a 4 MiB cap by its depth. The depth is **bounded and
stated**, and the product says when the bound is reached rather than silently forgetting the oldest.

**D4 — Undo ships under `NFR-REL-005`, and the requirements gap is reported, not patched.** No
`REQ-EDIT` names undo. This RFC implements it as data-loss protection and **records that the
requirements are missing it** for the owner to rule on; an RFC must not mint a requirement the spec
does not have.

**D5 — `REQ-EDIT-002` is finished here, and the coverage row says it was half met, not met.** The
plan currently implies otherwise. Correcting a coverage claim is part of this RFC, not a footnote.

**D6 — `NFR-PERF-003` is measured for the first time, and the number is reported whatever it is.**
If today's whole-document copy per keystroke already misses p95 ≤ 16 ms, that is a finding this RFC
publishes — and if the fix is larger than this RFC, it says so and schedules it rather than quietly
widening.

**D7 — The two-row window header, owner-requested, as its own slice.** Menus and the title logo
above, project tabs below (relayed at review 438). It is **not editor work** and is not folded into
the editor slices: it is here because it changes the frame, this is the only scheduled work that
touches the frame, and it invalidates every capture showing the top of the window — paying that once
costs less than paying it twice. It is recorded in the delivery plan as owner-requested.

## Non-goals

Syntax highlighting (`REQ-EDIT-003`, and the owner's open 1.0 decision). Selection, clipboard, or
delete-forward — measurement 9 is a finding, not an invitation. Soft wrap. Multi-document (RFC-026).
Autosave (`REQ-EDIT-008` already permits explicit save only).

## Risks

**R1 — this rewrites the rendering of the one surface holding unsaved work.** Everything else in the
product can be redrawn; a buffer cannot be re-typed.

**R2 — the caret and the cursor model must not become two sources of truth.** That is the
terminal-sizing defect (RFC-053 D3) in a new place: a drawn position derived independently of the
model's position will disagree eventually.

**R3 — undo across a reload.** Undoing past an external-change reload must not resurrect text over a
file that changed underneath. `ExternalChanged` and `Conflict` already exist; undo has to respect
them.

**R4 — undo and the dirty flag.** Undoing back to the opened state should make the document `Clean`
again, or `REQ-EDIT-004` starts lying in the direction that makes a user save what they did not mean
to keep.

**R5 — the gutter's width is data-dependent.** Line 9 999 and line 10 000 are different widths; a
layout that shifts while scrolling is a defect, and the fixture that shows it is a file that crosses
a power of ten.

**R6 — D7 invalidates captures.** Every image in the evidence packs that shows the top of the window
is superseded when the header changes.

## Acceptance criteria

- **`NFR-PERF-003` measured** on a 100 000-line file: typing latency p95 and p99, reported as
  numbers, against 16 ms / 33 ms. Whatever the result, it goes in the changelog.
- The viewport bounds what is drawn: the 100 000-line file draws a window, not a file, and the proof
  is a measurement of what is built per frame, not a screenshot that looks fast.
- **The caret is an element, not a glyph**: a file whose text contains the caret's own character is
  indistinguishable from nothing — asserted, with the file containing that character in the fixture.
- The caret's drawn position and `document.cursor()` are **one value with one reader** — ablated: a
  second, independently derived position fails a test.
- The gutter aligns at 5 and 6 digits: a file that crosses 10 000 lines does not shift its text
  column while scrolling. Captured.
- **Undo**: typing then undoing restores the text *and* the cursor; undoing to the opened state
  returns the document to `Clean`; the bound is stated and the product says when it is reached; undo
  does not cross an external-change reload.
- `REQ-EDIT-002` moves to met **and its coverage row is corrected** from implying it already was.
- The requirements gap — no `REQ-EDIT` names undo — is recorded for the owner (D4).
- **D7**: the two-row header, captured, with the superseded captures named.
- The colour-alone, i18n completeness and internal-identifier scans still pass.
- Gate green three times with `--no-fail-fast`, **0 fixture entries left** in a fresh short `TMPDIR`.
- The core pin bumps with the version; `the_workspace_pins_tekstide_core_to_its_own_version` is red
  until it does.

## Decided on acceptance (2026-09-26)

**D1–D7 as written.** Four additions, one of which changes the order of the work.

**D8 — a drawn line stays one assertable string.** RFC-052 named *losing testability* as a risk and
answered it by keeping `node_line`: one row, one string, testable without `iced`. Rows here must do
the same — each line is one string plus the caret element, not a scatter of widgets. If the gutter
and the caret can only be verified by looking at a picture, the surface has become untestable and
that is a D1 failure, not a style choice.

**D9 — the viewport follows the cursor; this RFC does not invent scrolling.** Measured: navigation is
four arrow keys and there is no scroll input at all. The window moves because the cursor moved.
Adding a scrollbar or wheel handling is an input-vocabulary change, and measurement 9 already records
that gap for someone else to schedule.

**D10 — D7 is three lines of composition, and here is where.** `shell.rs:7520–7522` stacks
`window_title`, `project_tab_strip`, `top_bar_actions_row` in that order. D7 makes the actions row
carry the title and puts it **above** the tab strip. Naming it precisely keeps the slice small and
makes the superseded capture set knowable in advance.

**D11 — measure `NFR-PERF-003` before changing anything, not after.** This reverses the obvious order
and it is the important decision. The baseline must be taken against **today's** code — the whole
file in one `text` widget, a whole-document `String` per keystroke — or there is nothing to compare
against and "it got faster" is unfalsifiable. Slice A measures and changes no product code. If the
baseline already misses p95 ≤ 16 ms, that number is published whatever it implies for the rest of
the work.

**Ships as `0.28.0`.**
