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
