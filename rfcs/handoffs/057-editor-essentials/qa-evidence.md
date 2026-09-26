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
