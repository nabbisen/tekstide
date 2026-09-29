# RFC-057 — QA evidence

## PR-057-A — the baseline

**No product code changed.** The slice adds `crates/tekstide/src/shell/tests/editor_baseline.rs` and one `mod` line in `shell/tests.rs`; `git diff` against the previous commit shows no other file
under `crates/`. Everything the harness measures is the real `update`, the real `view` and iced's own text-layout engine.

### The number

**Typing in a 100,000-line file, on `main` as it stands, is at the budget and has no headroom. It misses at the start of the file.** Release build, three runs, `p95 / p99` in ms, the sum of the
three stages below against the budget of **16 / 33**:

| Scenario (30 keystrokes each) | Run 1 | Run 2 | Run 3 |
| --- | --- | --- | --- |
| typing a character **at the start** of the file | **16.32** / 16.58 | 15.44 / 15.76 | **16.04** / 16.41 |
| typing a character at the end of the file | 14.63 / 14.89 | 14.04 / 14.04 | 14.69 / 14.72 |
| `Enter` in the middle of the file | 14.81 / 15.00 | 13.98 / 13.99 | 14.66 / 14.94 |
| an arrow key (the cursor moves, the text does not) | 1.77 / 1.79 | 1.81 / 1.94 | 1.79 / 1.81 |

**p99 is inside its 33 ms budget with room. p95 is not inside 16 ms with room: two of three runs put the start-of-file case over it, by 0.04 and 0.32 ms, and every edit case is within 2 ms of it.**
And **this is a lower bound**: painting the laid-out text and presenting the frame are outside any headless measurement (below), so the true figure is higher than every number in this table.

The first stage-by-stage split (run 1, typing at the start):

| Stage | p50 | p95 | p99 | What it is |
| --- | --- | --- | --- | --- |
| `update` | 4.75 | 4.82 | 4.82 | `handle_editor_key` copies the document, `apply_edit_key` builds a `Vec<String>` of **every line** (100,000 allocations) and joins it, `replace_text` installs it |
| `view` | 0.12 | 0.14 | 0.17 | building the element tree, which copies the whole document once more (`body_text`) |
| `layout` | 9.69 | 11.39 | 11.65 | the `text` widget re-laying out the changed body at the height the widget is given. It scales linearly with the number of lines (below), which is consistent with cosmic-text splitting the whole 3.3 MB string into lines on every change; I did not profile inside it |

### It scales with the length of the file, not with the edit

Typing at the end, p95: **1,000 lines 0.44 ms; 10,000 lines 1.7 ms; 100,000 lines 15.8 ms.** The cost is linear in the file: three whole-document copies and a 100,000-way split per keystroke, for a
one-character change. The arrow key, which changes no text, costs 1.8 ms at 100,000 lines — the `Vec<&str>` of every line that `navigate_cursor` builds, plus the copy.

### How this was measured, and what it does not include

`cargo test --release -p tekstide editor_typing_latency_baseline -- --ignored --nocapture`, with `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true` because the suite's tests use iced's `()` renderer, which
exists only with debug assertions on: **opt-level 3, assertions enabled**, so it is a shade slower than the shipped binary and the figures err in the safe direction. `environment.txt` records the commit,
`rustc`, the CPU (AMD Ryzen 9 9950X) and the load before and after each run (1.0–1.6; the machine was quiet).

- **Not measured: painting.** Rasterising or drawing the laid-out text and presenting the frame are outside a headless harness — the limit `measurement.rs` already discloses and for the same reason.
- **`layout` is iced's own engine, not the widget.** It calls `Paragraph::with_text` on the same string, font, size, line height and width as the `text` widget, and I read `iced_core`'s `Paragraph::update` to
  confirm that a changed content string takes exactly that path (`if self.content != text.content { self.raw = P::with_text(text) }`).
- **The height the widget is given matters enormously, and my first harness got it wrong.** It laid the text out with no height bound and measured **~750 ms** a keystroke. The live app, which I ran next as a
  sanity check, answered in under 100 ms — so the harness was wrong, not the app. cosmic-text shapes only what the height shows, and the editor's text widget is inside a container of the window's height.
  **The 750 ms figure is kept in the output as a labelled reference** (`REFERENCE, not the shipped editor`): it is what any design that lets the widget see the whole file would pay — a scrollable body,
  for instance — and PR-057-B must not introduce it.
- **The coarse live check** (`live-coarse-latency.txt`): the release binary with the same fixture open, one keystroke by `wtype`, screenshots until one differs. Idle, a screenshot costs ~41–48 ms; a
  keystroke took **85–101 ms** to show. That is the true latency plus up to one screenshot interval, and includes the compositor and the paint the headless number omits: consistent with a keystroke that costs
  about 15 ms in the code we can measure plus a frame or two. It is a sanity check, not a measurement.

### The fixture

`fixture_text()` generates it from the line number alone: **100,000 lines, 3,307,639 bytes**, ordinary source-shaped lines of varying length. **A test pins its line count, its size against
`DEFAULT_MAX_EDITABLE_BYTES`, and its FNV-1a checksum**, so it cannot drift silently; a reviewer runs the same function and gets the same bytes. **It is committed as a generator, not as a 3.3 MB blob** — that is
a deviation from "the fixture is committed", said plainly. `write_the_baseline_fixture` (ignored) writes the file to `$TEKSTIDE_BASELINE_FIXTURE_OUT` so a capture opens the same bytes.

**A finding the fixture produced.** My first fixture averaged 51 bytes a line — ordinary for source — and the editor **refused to open it**: 5,120,111 bytes against the 4 MiB cap. **A 100,000-line file
can only be opened if its lines average under 42 bytes**, so `NFR-PERF-003`'s "100,000-line file" and `DEFAULT_MAX_EDITABLE_BYTES` are two numbers that disagree for most real files. The fixture was
shortened to fit (33 bytes a line). Whether the cap or the requirement should move is not this RFC's to decide, and it is recorded here for the owner.

### The baseline picture

`live-01-…png`: the 100,000-line fixture open in the release binary. **No gutter, no caret, no line numbers; the header says *Line 1, Column 1*; about 48 lines fit and the rest of the file is clipped with
no scrollbar** — the cursor at line 60,000 would be somewhere no one can see. Every claim RFC-057 makes about today's editor is visible in this one image.

### Order-of-work note

This slice ran before any rendering change, as D11 requires, and the numbers above are the baseline PR-057-B is judged against. **What they say about B:** the budget is already at its limit at 100,000 lines
with nothing drawn but the top of the file, and the cost is O(file) in `update` (three copies) and in `layout` (the split). Bounding the *drawn* rows, as D1 proposes, addresses neither by itself: the
`update` copies and the widget's whole-string `set_text` would remain unless the body is no longer handed to one text widget as one string. That is a decision for B with this evidence behind it.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short
`TMPDIR`: `681 + 16 + 1037` = 1,734 passed, 0 failed, 0 entries left after each** (loads 3.7, 3.6, 5.2). The two new ordinary tests are the fixture pin and the harness smoke test; the measurement itself is
`#[ignore]`d and asserts nothing about speed, so it cannot make the gate depend on the machine. No intermittent this time. The type-alias change clippy asked for landed after the three measurement runs and
touches no measured code.


## PR-057-B — rows, bounded by the viewport

### What was built

| | |
| --- | --- |
| `surface/editor.rs` | the body is **one fixed-height row per visible line**, each one string. `window_rows(text, first, capacity)` is the window; `drawn_rows(document, capacity)` is what `view` builds **and** what every test measures — one value; `rows_that_fit` is arithmetic on a **measured** height; `viewport_following` is the D9 rule (it reuses the explorer's `window_for`, the same "move as little as possible" rule) |
| `shell.rs` | `editor_viewport` measured by `MeasureSize` like the explorer's; `Message::EditorViewportMeasured`; `settle_editor_viewport` after every edit, every cursor move and every re-measure |
| core | `set_active_viewport` through the same four layers the cursor's write path takes; **`TextViewport::first_visible_line` has a writer and a reader for the first time since RFC-006** |
| gone | `body_text`, which turned the whole document into one string |

### Q1 (review 441): the whole file never reaches one widget

Three things hold it, none of them a clock:

1. **`the_document_text_reaches_a_widget_only_through_window_rows`** scans `surface/editor.rs`: the document's text is read in exactly one place, it goes straight into `window_rows`, `body_text` does not exist,
   and `text().to_string()` appears nowhere. **Ablation B4** — the view reading the whole document a second way — fails it alone.
2. **`the_editor_draws_a_window_of_a_100_000_line_file_and_never_the_file`**, with A's reference number in its message (*~745 ms a keystroke*): 54 rows reach the widgets, and `drawn * 500 < document bytes`.
   **B5** (a window that ignores its capacity) fails it.
3. **The measurement's labelled reference** — the whole file in one widget at unbounded height — is still in the output and read **707–895 ms** at p50 in these runs, next to a shipped figure of 5–6 ms.

**What none of the three does:** the 100,000-line test asserts on `drawn_rows`, the value `view` builds from, and cannot see a `view` that bypasses it — only the scan (1) can. I say so rather than call the pair redundant.

### The re-measurement, and the 8 ms rule

Release build, the same fixture and harness as A, `layout` now laying out each drawn row whose string changed since last frame (what a `text` widget does). **Two batches of three runs**; the first batch's run 1 was taken at machine load 11 by other
projects and is kept (`batch-1/`).

| p95 / p99 ms, typing at 100,000 lines | A (baseline) | B, batch 1 run 2–3 | B, batch 2 (three runs) |
| --- | --- | --- | --- |
| at the start of the file | 14.0–16.3 / 14.0–16.6 (start-of-file worst) | 5.6 / 5.6 | **5.3–5.5** / 5.3–5.6 |
| at the end of the file | 14.0–14.7 | 6.0–6.6 | **5.9–6.3** |
| `Enter` in the middle | 14.0–14.8 | 6.3–6.8 | **5.9–6.4** |
| an arrow key | 1.8 | 2.8 | **2.6–2.8** |

**Every edit scenario is under the 8 ms line the review set, at 5.3–6.8 ms p95, in five of six runs. Batch 1 run 1 was not — 13.5, 19.3, 14.0 ms p95 — and it was taken at load 11 while three other projects' suites ran; its `update` stage alone reads 13–15 ms where every other run reads
4.7–5.3.** I did not tune around it, and I did not discard it: the 8 ms rule is about the *design*, and the design's cost is the five quiet runs; but a reviewer who wants the rule read strictly can see one run that missed it, and why.

Stage split, a quiet run: **`update` ~4.7 ms, `view` 0.1–0.8 ms, `layout` 0.02–0.4 ms** (the layout was 9.7 ms). So **the per-keystroke document copy is now ~80 % of what is left** — ruled out of B, and not needed for the budget: 5–6 ms against 16 with threefold headroom, as review 441 predicted.
Scaling at the end of the file, p95: 1,000 lines **0.08 ms**; 10,000 **0.59 ms**; 100,000 **6.1 ms** (A: 0.44, 1.7, 15.8).

**A regression, disclosed:** an arrow key costs 2.8 ms, not 1.8. It now also counts the file's lines to keep the window on the cursor, and `view` skips to the first visible line. Still inside the budget by a wide margin.

### Live

`live-00-…` the window at the start of the 100,000-line file: 47 rows fit, the last one wholly visible. `live-01-…`: **100 presses of Down later the header says *Line 101* and the window has followed — the last row is `// line 100`**; the window did not
scroll while the cursor was inside it (`the_window_follows_the_cursor_through_real_arrow_keys` asserts that at rows 0–9, 10, 40 and back). `live-coarse-latency.txt`: keystroke-to-changed-screenshot **50–53 ms** typical against screenshots that cost 45–50 ms idle — that is, the frame is on screen inside the first screenshot
interval, where A's was 85–101 ms. One sample read 114 ms; the machine was busy. A sanity check, not a measurement.

`live-02-…` **shows a limitation, and was an accident.** My key helper typed the words `-k Down -k Down …` into the document instead of sending the keys, and the result is the evidence: **a long line is clipped at the right edge**, not wrapped, and *Line 1, Column 525* is a cursor position no one can see.
The fixture is throwaway and was not saved.

### Tests

`surface/editor/tests.rs` (6 new: the window, the first-line reader, rows-that-fit, the follow rule, and the two scans), `shell/tests/editor_rows.rs` (5, through the **real router and real `update`**: follow by arrow keys, the cursor's line is always drawn, `Enter` at the window's bottom scrolls, a re-measure that shrinks the window, a measurement with no document), the 100,000-line test above, and two core tests for the viewport's write path.

### Ablations (`ablate.sh`, clean tree each)

| # | Ablation | Failed |
| --- | --- | --- |
| B1 | the window ignores the viewport's first line | three tests |
| B2 | the viewport never follows the cursor | five tests |
| B3 | an arrow key does not settle the viewport | the arrow-key test and the cursor-is-drawn test |
| B4 | the view reads the whole document a second way | the scan, alone |
| B5 | the window ignores its capacity | four tests, including the 100,000-line one |
| B6 | a re-measure does not settle | the re-measure test, alone |
| B7 | an edit does not settle | the `Enter` test, alone |

### Deviations and things to know

1. **Rows do not wrap.** One line is one row, so the window's arithmetic is exact. Before this slice a long line wrapped. Now it is clipped at the right edge with no horizontal scroll, so the end of a very long line is not visible. Soft wrap is a non-goal of the RFC; I chose exact rows over wrapping and it is a visible change, in the changelog and the book.
2. **`window_for` is the explorer's**, reused: the same rule, already ablated. It lives in `surface/explorer.rs`; if you would rather it move to a neutral module, that is a rename.
3. **The window is not persisted across a reload.** After an external-change reload the viewport may be past the new end until the next key settles it; `window_rows` draws nothing rather than panic, and the next cursor move brings it back.
4. **No caret and no gutter yet** — that is C. The header still says *Line N, Column M*.
5. The harness's module documentation still says "as it stands"; its measured code is now B's, and PR-057-A's numbers live in this file above.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `692 + 16 + 1039` = 1,747 passed, 0 failed, 0 entries left after each**
(loads at the end 16.1, 16.6, 17.6 — other projects' suites; none of this slice's tests read a clock). No intermittent.

## PR-057-C — the gutter, the caret, and the horizontal window (Q2)

### What was built

| | |
| --- | --- |
| `surface/editor.rs` | `gutter_digits`/`gutter_lines` (real line indices, sized to the whole file); `columns_that_fit`/`column_pitch` (the horizontal twin of `rows_that_fit`/`row_pitch`, the same disclosed approximation); `windowed_line` (per-row horizontal clip, char-indexed); `viewport_following` now follows **both** axes, on the cursor's own line; `caret_row_position` (where the caret is inside the drawn window, or `None`); `caret_split`/`RowPlan`/`row_plan` (the caret's placement, factored into a pure, testable decision — D8); `caret_element` (the caret itself: a coloured, fixed-size container, never a character); `EditorWindow` (rows+columns together, so `view`'s parameter count stays under clippy's own limit and a caller cannot swap them unnoticed) |
| core | `TextViewport` gains `first_visible_column` |
| `shell.rs` | `editor_column_capacity`, sized from the **active document's own** total line count, so the gutter reservation is correct before any row is drawn |

### Q2 (review 442) — the horizontal window

`viewport_following` now takes a `column_capacity` and follows the cursor's column on its own line, by the same D9 rule (least movement) the vertical axis already used, reusing `window_for` a second time. **One global horizontal position, applied to every drawn row** — the same way `first_visible_line` is one value for the whole view, not one per line.

**Held two ways.** `a_cursor_anywhere_on_a_long_line_ends_up_inside_the_horizontal_window` drives every column of a 300-character line through `viewport_following` and asserts `caret_row_position` finds it, every time — this is Q2's own acceptance criterion, held in code, not only in a screenshot. Live: `04-…` and `05-…` show the cursor at column 151 and column 301 of a 400-character line, the caret visible near the window's right edge both times, the text scrolled to match. **`03-…` shows the pre-Q2 shape for comparison** — column 1, the line clipped at the window's edge — the same limitation the changelog now corrects by name.

### The gutter — the acceptance criterion

`gutter_digits` sizes to `line_count`, the whole file, never to what is drawn. Live: `02-…`, a 10,010-line file scrolled from line 9964 to line 10010 — **the text column does not move** as the line numbers grow from 4 digits to 5. (The scroll from the top, `git-exclude`-free capture sequence, also passed through 999→1000 without a visible shift, not kept as a separate image since the 9999→10000 capture is the stronger case the acceptance criterion names.)

### The caret — D2

**Structural guarantee, not only a runtime one.** `RowPlan::WithCaret { before: &str, after: &str }` holds **borrowed** slices of the row's own text; there is no `String` field to splice a synthetic character into without an allocation the type does not offer. The caret itself (`caret_element`) is a coloured `iced` container with no text content at all — it is not merely *escaped* like `chrome_line`'s path, it **contains no character**, so there is no string for an escape discipline to fail on.

**Live**, `01-…`: `caret.txt` open, cursor at its default `(0,0)`. The real caret (a thin coloured bar at the very start of line 1) and the file's own look-alike glyph (a full block, mid-line 2) are both on screen at once and are visibly different marks — the acceptance criterion, met with the fixture containing the caret's own plausible character, exactly as required.

**One cursor, one reader.** `caret_row_position` and `caret_split` both work in **characters**, the same unit every other position in this module uses (`clamp_cursor`, `insert_at`). `the_caret_position_is_the_real_cursor_and_nothing_independently_derived` constructs a row with a two-byte character before the caret's column and asserts the char-indexed answer; ablation C4 (below) turns `caret_split` byte-indexed and both this test and the bidi-glyph test fail — a byte-indexed derivation is exactly R2's risk (RFC-053 D3 in a new place), made concrete.

### Tests (18 new, 1 widened) and the source-scan property

`gutter_digits_is_sized_to_the_whole_file_not_to_what_is_on_screen`, `the_gutter_does_not_shift_the_text_column_while_scrolling_across_a_power_of_ten`, `gutter_lines_are_real_line_indices_not_window_positions`, `columns_that_fit_reserves_room_for_the_gutter_it_is_told_about`, `windowed_line_clips_by_character_not_byte_and_never_panics_past_the_end`, `the_horizontal_window_follows_the_cursor_by_the_least_it_must`, `a_cursor_anywhere_on_a_long_line_ends_up_inside_the_horizontal_window`, `the_caret_split_contributes_no_characters_and_cannot_be_confused_with_a_real_glyph`, `the_caret_position_is_the_real_cursor_and_nothing_independently_derived`, `a_caret_outside_the_drawn_window_is_none_not_a_wrong_position`, `row_plan_puts_the_caret_only_on_its_own_row_and_never_as_a_character`.

`the_document_text_reaches_a_widget_only_through_window_rows` (B's Q1 guard) is **widened, not weakened**: a second legitimate reader joined the first — `gutter_digits(line_count(document.text()))`, which counts newlines and never builds a string of the file — and the test now names both allowed call sites by function, failing on any third.

### Ablations (`ablate.sh`, clean tree each)

| # | Ablation | Failed |
| --- | --- | --- |
| C1 | gutter width sized to what is on screen, not the whole file | the digits test and the crossing test |
| C2 | gutter shows the window position, not the real line index | the real-indices test and the crossing test |
| C3 | `row_plan` never plants the caret | `row_plan_puts_the_caret_only_on_its_own_row_and_never_as_a_character` alone |
| C4 | `caret_split` byte-indexed, not char-indexed | the multi-byte position test and the glyph-distinguishability test |
| C5 | the horizontal window never moves | the "every column" test and the "least movement" test |
| C6 | `windowed_line` clips by byte, not char | the character-clip test alone |
| C7 | `caret_row_position` ignores the row capacity | the outside-window test |
| C8 | `caret_row_position` ignores the column capacity | the outside-window test |
| C9 | the column capacity ignores the gutter digits | `columns_that_fit_reserves_room_for_the_gutter_it_is_told_about` alone |

**C3's first version was wrong, and I caught it before running it for the record.** My first attempt replaced the whole caret-composition branch *inside `view`* — a function no test in this crate calls directly, since RFC-057 (like every other GUI surface here) tests the logic feeding a view, not the `iced::Element` tree itself. It failed nothing. I factored the decision into `row_plan`, a pure function, added a test for it, and re-ran the same shape of ablation against the new location — it now fails alone. The lesson is the same one B's own Q1 guard already taught: a property is only as testable as the function it lives in.

### Live walk

Release binary, fixtures in `mktemp -d` under `/dev/shm`, focus-verified `wtype` (checked against niri's own focused-window id before every send), no window floated or resized, real navigation through the real router — no test-only shortcut. `caret.txt` (a lookalike glyph), `gutter.txt` (10,010 lines), `longline.txt` (one 400-character line) — three fixtures, three properties.

### Not shown live

A mouse click on any control (every step was a key, matching this project's established practice for this surface). The approximate column-width factor's own accuracy was not measured against `cosmic-text`'s real shaping — it is disclosed as an approximation, the same as row height already is, not verified against the renderer.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `703 + 16 + 1039` = 1,758 passed, 0 failed, 0 entries left after each** (loads at the end 1.50, 2.09, 3.62 — a quiet machine). No intermittent.

## PR-057-C review 443, Q3 — the width approximation, falsified

### The number

`cargo test -p tekstide editor_column_width -- --ignored --nocapture`, real `iced` text layout (the same technique as `editor_baseline`'s harness), default face, 880px region, 14px font:

```
wide (W): measured 12.927 px/char vs approximated 8.400 px/char (+53.9%); max caret overflow 427.6 px (50.91 columns)
narrow (i): measured 3.534 px/char vs approximated 8.400 px/char (-57.9%); max caret overflow 0.0 px (0.00 columns)
```

**Not a rounding error.** A line of `W` can leave the caret **~51 columns outside the region that is actually drawn**, walked at every column of a 300-character line — the worst case, not a sampled one. A line of `i` never overflows: the approximation is safe in that direction, wasteful (draws fewer characters than would fit) rather than wrong.

### Live, both sides of the finding

`06-…`: `wide.txt` (200 `W`s), cursor at column 16 — inside the window that has not needed to scroll yet, and the caret is visible. `07-…`: the same file, cursor at column 81 — **the header still reads *Line 1, Column 81*, and there is no caret anywhere in the row.** The horizontal window's own char-index arithmetic considers the cursor "inside" (`caret_row_position` returns `Some`), but the real, rendered row is wider than the region, so the caret's real pixel position is off the right edge of what is drawn. This is the defect the numbers above measure, seen.

### Why the test is `#[ignore]`d, not asserted

The overflow is real, structural and disclosed — not a machine-load flake like `NFR-PERF-003`'s own reason for being `#[ignore]`d, but the same shape of answer: a number that must be published, not a threshold the ordinary gate should pass or fail on while the underlying question (join the fixed-width side, or measure real text) is the owner's to decide, not this slice's. A tolerance loose enough to pass today would either be meaningless or would read as tolerating the defect; the honest tolerance fails every gate until the owner's decision is implemented. The test is real, runnable, produces the same numbers reported here, and stays in the suite for whenever that decision lands.

### The book

`docs/src/users/configuration.md`, beside the paragraph review 443 named (`ui_font`'s own family setting): a new paragraph states the approximation, the measured direction (`i` safe, `W` not), the ~50-column figure, and that it is a known, unfixed limitation.

### Left for the owner (not blocking D)

Recorded in `delivery-plan.md`'s coverage table for `REQ-EDIT-002`/`003` area: whether the editor's body joins the terminal and file tree on the fixed-width side (my recommendation, per review 443, because it costs nothing to implement here and the book's stated reason already applies), or the horizontal window is changed to measure real text. Either changes the changelog; neither is decided by this slice.

### Gate (Q3 follow-up)

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `703 + 16 + 1039` = 1,758 passed, 0 failed, 0 entries left after each** (loads 2.5, 4.1, 5.4). `editor_column_width`'s own two tests: one ordinary (sanity only, runs every gate), one `#[ignore]`d (the measurement, run by hand for this evidence). No intermittent.

## PR-057-D — undo

### What was built

| | |
| --- | --- |
| core, `content/undo.rs` (new) | `EditOperation` (`Insert`/`Enter`/`RemoveChar`/`JoinLines`, each storing exactly what its own inverse needs, never a copy of the document) and `UNDO_MAX_DEPTH = 500` |
| core, `content/document.rs` | `TextDocument` gains `undo_stack`/`redo_stack`/`undo_bound_reached`; `record_edit_operation`/`undo_operation`/`redo_operation`/`can_undo`/`can_redo`/`undo_depth_bound_reached`; `replace_text`'s one-way `Dirty` latch fixed — text back to exactly the opened content now returns to `Clean`, guarded to only apply from `Clean`/`Dirty` (an `ExternalChanged`/`Conflict`/`SaveError` state is never silently cleared by a coincidence of content) |
| core, `content/snapshot.rs` | `FileSnapshot::matches_content` — an in-process hash comparison, the Clean-detection signal, reusing the existing `content_hash` rather than storing a second copy of the opened text |
| `surface/editor.rs` | `remove_at`/`join_lines_at` (the two new pure helpers `apply_undo` needs; `join_lines_at` is also `backspace_at_cursor`'s own start-of-line case, factored out and reused rather than duplicated); `apply_undo`/`apply_redo` (redo is literally `insert_at`/`split_line_at_cursor`/`backspace_at_cursor` called again at the operation's own `at` — no third implementation); `apply_edit_key` now returns `EditOutcome { result, operation }` instead of bare `EditResult`, so every caller gets the operation to record for free; `history_bound_line` (the D3 disclosure, `None` until the bound is actually reached) |
| core, `navigation.rs` | `UndoActiveDocument`/`RedoActiveDocument`, bound to `Ctrl+Z`/`Ctrl+Shift+Z`, the same "global, needs real document-level write, no `AppCommand`" shape `SaveActiveDocument` already uses |
| `shell.rs` | `handle_editor_key` records the operation after every ordinary edit; `attempt_undo_active_document`/`attempt_redo_active_document`, the same two-step `replace_active_project_text`/`set_active_project_cursor` shape an ordinary edit already uses, just fed `apply_undo`/`apply_redo`'s output instead of `apply_edit_key`'s |
| `keyboard_help.rs` | both actions catalogued; `ControlCoverage::KeyboardOnly` (no toolbar exists to put a button on, and the task breakdown names none) |

### Operations, not snapshots; the bound, stated

`EditOperation`'s four variants store only what their own inverse needs (a cursor and, at most, a string, a `char`, or a `usize`) — never a copy of the document, so the stack's own size is bounded by edit count, not by file size times depth. `record_edit_operation` caps the stack at `UNDO_MAX_DEPTH` (500), dropping the oldest entry and latching `undo_bound_reached` past it. **Held in code**: `recording_past_the_bound_drops_the_oldest_edit_and_latches_the_reached_flag` records 501 edits and asserts the 501st undo reaches the *second*-recorded edit, never the first, and that the flag stays true afterward (it does not un-latch once the stack has room again). **Stated, not merely tracked**: `history_bound_line` is the on-screen disclosure (`editor-history-bound-reached`, naming the real `UNDO_MAX_DEPTH` number, not "a lot") — `None` until the bound is hit, `Some` after, held by `history_bound_line_is_none_until_the_bound_is_reached_then_names_the_limit`.

### Typing then undoing restores text and cursor

`EditOperation::at` is, on every variant, the cursor *before* the operation — and every operation's own inverse (`remove_at`, `join_lines_at` at the split point, `insert_at`/`split_line_at_cursor` reapplied at the join point) lands the cursor back at exactly `at`, by construction of what each inverse function already returns, never a manual override. Held for all four kinds of edit at once: `undoing_every_kind_of_edit_restores_the_original_text_and_cursor` drives `apply_edit_key` for a typed character, `Enter`, and both `Backspace` cases, inverts each with `apply_undo`, and asserts both the text and the cursor land back where they started. `redoing_every_kind_of_edit_reproduces_the_original_outcome` is the mirror: undo then redo reproduces exactly the original edit's own text and cursor. Through real keys and the real router, not only the pure functions: `ctrl_z_undoes_a_real_typed_character_and_returns_to_clean` presses a real `!`, then a real `Ctrl+Z` (proved to route to `RoutedInput::Shell`, not fall through to `Surface`), and asserts both.

### Undoing to the opened text returns to Clean

`replacing_text_back_to_the_opened_content_returns_to_clean` (core): `TextDocument::replace_text` away from the opened content (`Dirty`), then back to it exactly, returns to `Clean`. Guarded correctly, not merely permissively: `replacing_text_back_to_the_opened_content_does_not_override_an_external_change_state` puts the document in real `Conflict` (a real external write, then `refresh_external_state`) and proves that replacing the text back to the opened content does **not** silently clear it — a `Conflict` still needs its own resolution. `ctrl_z_undoes_a_real_typed_character_and_returns_to_clean` closes the loop through the real router: type, `Ctrl+Z`, and the document's own `state()` reads `Clean`.

### Undo does not cross an external-change reload

**Free by construction, not by a new guard.** `activate_current_modal`'s Reload arm calls `open_active_project_text_document`, which constructs a brand-new `TextDocument::open(...)` and replaces `self.active_document` wholesale (`ProjectContentWorkspace::open_text_document`) — never patches `text` on the existing instance. A fresh `TextDocument` has empty `undo_stack`/`redo_stack` by construction (`open`'s own field initialization), so a reload's own replacement already clears undo history; there is no separate `clear_undo_history` method to call or forget to call. `undo_does_not_cross_a_real_external_change_reload` proves it end to end: type a real edit, write a real conflicting change to disk, save (refused, opens the conflict modal), Reload (which really re-opens the file, discarding the local edit — the same property `saving_over_a_real_external_change_opens_the_conflict_modal_and_reload_takes_the_disk_content` already proved), then a real `Ctrl+Z` — a no-op, the discarded edit never resurrected.

### Redo then save writes what the screen showed

`redo_then_save_writes_what_the_screen_showed`: type, undo, redo (screen now shows the redone edit), `Ctrl+S`, and the bytes on disk are read back and compared byte-for-byte against what the screen showed — not what was on disk before the edit, and not what undo left behind.

### The requirements gap

Written up in `delivery-plan.md` ("Requirements gap: no `REQ-EDIT` names undo"), the same disclosed-not-minted shape as RFC-056 D11's open question: `NFR-REL-005` is the closest existing text and only assumes undo exists; no `REQ-EDIT` row names it. Not fixed by minting a requirement from inside this slice — that is the owner's call.

### Tests (17 new: 5 core, 6 pure-function, 6 real-routing)

Core (`content/tests/edit.rs`): `replacing_text_back_to_the_opened_content_returns_to_clean`, `replacing_text_back_to_the_opened_content_does_not_override_an_external_change_state`, `recording_an_edit_after_an_undo_clears_the_redo_stack`, `recording_past_the_bound_drops_the_oldest_edit_and_latches_the_reached_flag`.

Pure functions (`surface/editor/tests.rs`): `undoing_every_kind_of_edit_restores_the_original_text_and_cursor`, `redoing_every_kind_of_edit_reproduces_the_original_outcome`, `remove_at_removes_exactly_the_given_span_and_restores_the_cursor`, `join_lines_at_joins_the_given_line_into_the_one_before_it`, `history_bound_line_is_none_until_the_bound_is_reached_then_names_the_limit`. The five existing `apply_edit_key` tests were widened, not just updated for the new return type: each now also asserts the exact `EditOperation` produced.

Real routing (`shell/tests.rs`): `ctrl_z_undoes_a_real_typed_character_and_returns_to_clean`, `undo_does_not_cross_a_real_external_change_reload`, `redo_then_save_writes_what_the_screen_showed`, `ctrl_shift_z_is_a_real_global_keybinding_for_redo`.

Plus the mechanical keybinding tests every new `NavigationAction` gets: `undo_active_document_shortcut_is_a_candidate_that_collides_with_no_other_rule`, `redo_active_document_shortcut_is_a_candidate_that_collides_with_no_other_rule` (core `navigation/tests.rs`).

### Not shown live

**Tooling gap, disclosed rather than skipped silently.** `niri msg action screenshot-window` (and a full-screen `niri msg action screenshot`) both returned exit 0 against the real, focused Tekstide window this session, but produced no file anywhere under the configured screenshot path or `$HOME`/`/dev/shm` — a live capture of undo/redo could not be produced this slice. Every property above is instead proven through the real router (`route_non_modal_input`, asserted to classify `Ctrl+Z`/`Ctrl+Shift+Z` as `RoutedInput::Shell` rather than falling through) and real document state (`TextDocument::state()`, real bytes read back from disk) — the same "no test-only shortcut" standard the live captures exist to hold, met here without a screenshot. The 500-edit bound disclosure (`history_bound_line`) is proven directly against a real `TextDocument`, not through 501 real keystrokes — the pure function is the same one `view()` calls, so a live capture would show the same string this test already asserts, not a different property.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `712 + 16 + 1045` = 1,773 passed, 0 failed, 0 entries left after each.** One intermittent met along the way (row 1, `approval::tests::channel::bind_recovers_from_a_stale_socket_file`, twice back to back in redone attempts of the middle run) — passed in isolation both times, not the slice, the gate redone rather than counted; see `test-process-leak.md`'s 2026-09-29 recurrence entry, which also records a sharper reading of the `SocketPathTooLong` class found along the way (a `mktemp -d /dev/shm/…` `TMPDIR` can still be too long; a short fixed literal is the actual fix).

## PR-057-E — the two-row header

### What was built

Owner-requested, relayed at review 438 (recorded in `delivery-plan.md`): split the single three-row top bar (`window_title`, `project_tab_strip`, `top_bar_actions_row`, each its own stacked row) into two rows. `shell.rs`'s `top_bar` now stacks `top_bar_title_and_actions_row` (the title plus the same two global actions the actions row already carried — "the actions row carries the title," the owner's own wording) and `project_tab_strip` alone below it. `top_bar_actions_row` is renamed to `top_bar_title_and_actions_row`, which now builds its `Vec<Element>` starting with the title text rather than `top_bar` composing the title as a separate `column!` entry. No other chrome changed: same `container` padding/background/border, same `top_bar_offers_trust_settings` gate deciding whether Trust Settings shows, same spacing.

### Captured

Two live states, a fresh `XDG_STATE_HOME` each (`mktemp -d` under `/dev/shm`, no recent-projects list to leak another session's paths):

- `01-two-row-header-with-project.png`: an open project. Row 1: "Tekstide", "Trust Settings", "?". Row 2: "[Projects]", "● project ×". Trust Settings shows because a project is active — the same gate as before, now sharing a row with the title instead of sitting below the tab strip.
- `02-two-row-header-first-run-no-project.png`: the cold-start first screen, no project open. Row 1: "Tekstide", "?" only — Trust Settings correctly absent. Row 2: "[Projects]" alone. Proves the two-row shape holds independent of which actions are showing, not merely a coincidence of the with-project state.

### Every superseded capture, named

**Every existing image in this RFC's evidence packs shows the old three-row header at the top of the frame** — none of them were capturing the header itself, but every one is a full-window screenshot, so all eleven carry the shape this slice replaced:

`evidence/pr-057-a/live-01-baseline-100000-line-file-no-gutter-no-caret-no-scroll.png`; `evidence/pr-057-b/live-00-the-window-at-the-start.png`, `live-01-the-window-follows-the-cursor-to-line-101.png`, `live-02-a-long-line-is-clipped-not-wrapped.png`; `evidence/pr-057-c/01-caret-and-lookalike-glyph-distinguishable.png` through `07-q3-caret-invisible-at-column-81-header-still-says-81.png` (all seven).

None are superseded on the property each was actually taken to show (the gutter, the caret, the horizontal window) — only the header chrome at the top of each frame is stale. Not retaken: retaking eleven images to change a part of the frame none of them were evidence for would be churn for its own sake, and this section's own two new captures already show the new header on its own terms.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `712 + 16 + 1045` = 1,773 passed, 0 failed, 0 entries left after each.** No intermittent this time.

## The owner's font ruling — the editor body defaults to fixed-width

### What was built

`Theme` (`theme.rs`) gains `editor_font: Font`, resolved in `from_settings` from the same `family: Option<&'static str>` input `font` already uses, with a different fallback: `family.map_or(Font::MONOSPACE, Font::with_name)` instead of `Font::DEFAULT`. A configured family reaches the editor body exactly as it reaches the rest of the interface; only the *default*, with nothing configured, changed. `surface/editor.rs`'s four body-text elements (both sides of the caret, the plain-row case, the gutter number) now take `.font(theme.editor_font())`, overriding the `.font(ui_font())` `crate::theme::text` already applied — the same "a widget that wants a specific face sets `.font(..)` after it" pattern `explorer::TREE_FONT` and the terminal's `MONOSPACE` already use, just per-render rather than process-global (the editor body still honours a user's configured family, unlike those two, which never do).

### Held, not merely measured

Q3's own falsification harness (`editor_column_width.rs`) is generalized to take a `font: iced::Font` parameter (previously hard-coded to `crate::theme::ui_font()`, i.e. always `Font::DEFAULT`). The original Q3 test is unchanged in what it measures (still `Font::DEFAULT`, still finds the same ~51-column overflow — that risk is real whenever a user configures a proportional family, and stays disclosed). A new test, **not** `#[ignore]`d, measures `Font::MONOSPACE` — the shipped default — with the same harness: `the_default_editor_font_is_monospace_and_the_width_approximation_holds_for_it` asserts every character gives the identical real advance width (true by definition of monospace) and that overflow is exactly zero. Measured number: **8.400 px/char measured against 8.400 approximated — 0.0% off.** Held as a real regression guard, not printed and forgotten, because (unlike Q3's finding) this is a property that actually holds and is meant to keep holding.

A direct unit test on `Theme::editor_font()` itself (`shell/tests.rs`, extending `a_family_is_used_only_if_the_lookup_finds_it_and_is_named_when_not`): with a configured, installed family, `editor_font()` equals `font()`; with nothing configured, `editor_font()` is `Font::MONOSPACE` while `font()` stays `Font::DEFAULT` — the two are the same only when a family is actually set.

### Captured live, at the exact column the defect was found at

Review 444's own `07-q3-caret-invisible-at-column-81-header-still-says-81.png` showed the caret gone entirely at *Line 1, Column 81* of a 200-`W` line, under the proportional default. `evidence/font-ruling/01-caret-visible-at-column-81-fixed-width-default.png` is the same fixture, the same column, under the shipped fixed-width default — the caret is visible, a thin bar inside the drawn row, and the header's *Line 1, Column 81* is no longer a claim the screen contradicts. Release binary, fresh `XDG_STATE_HOME`, fixture under `/dev/shm` via `mktemp -d`, real navigation through the real router (tab strip, explorer, `ArrowRight` × 80) — no test-only shortcut.

### The book and the changelog, corrected

`docs/src/users/configuration.md`'s paragraph beside the family setting now states the editor body's own fixed-width default explicitly, and reframes Q3's ~50-column finding as a risk that returns specifically when a proportional family is configured — not a property of what ships. The `Unreleased` changelog entry states the same, with the measured number.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs`, `rfc_docs_invariants` 16: clean.
