---
title: "RFC-053 — QA evidence"
rfc: "RFC-053"
created: "2026-09-24"
---

# QA evidence

## PR-053-A — what the surfaces say

Commit `8917003` (code and tests); this file, the checklist and the captures follow it.

### What changed, by decision

| | Change | Where |
| --- | --- | --- |
| D1 | Terminal-mode empty state: *"Terminal mode. Nothing is running in this project yet. Start a terminal with the button below."* The two other placeholders that named an RFC or said only "Sidebar" now say what the area is. | `en.ftl` |
| D9 | `no_catalog_string_names_an_internal_identifier` scans every shipped `.ftl` **message line** for `RFC-` / `PR-0`; `#` comments are skipped. A second test plants a string, a selector arm and a comment to prove the scanner is not simply matching nothing. Failure message says which it covers and not to delete a comment. | `i18n/enforcement.rs` |
| D4 | Change Review's empty state: *"No AI CLI run has produced changes in this project yet. Changes you made yourself are shown by the status bar and the file explorer."* | `en.ftl` |
| D5 | The board row counts the collections it holds when the runtime summary has not been refreshed, so a fresh project reads **0 terminals / 0 agent runs**. `ProjectProviderState` and `project/metadata.rs` untouched. | `project_board.rs` |
| D6 | *"N unsaved files"* replaces *"N dirty files"*; the attention state those buffers raise says *"Unsaved edits"*, not *"Dirty"*. | `en.ftl` |
| D7 | A `blocked: …` line names the blocked automations after the count, from `blocked_automation_labels`. | `surface/board.rs`, `en.ftl` |
| D8 | Empty Approval History says it is empty and shows no caveats; the two caveats appear only with entries (`approval_history_leading_keys`). | `shell.rs` |

### Judgment calls, disclosed

- **The attention word.** The board's *attention* line said "Dirty" for the same unsaved-buffers fact D6 renames. Leaving it would have left the two-words-for-one-fact defect one line below the fix, so it is changed too. Not in the pack's list.
- **The sidebar's "Sidebar".** The RFC's table lists it and no decision covers it. It is one catalog string in the same class as G1 (it said nothing true), so it is reworded here: *"Files are listed here in Content mode."*
- **The close-project dialog still says "dirty file".** Same fact, different surface, and it is not adjacent to a Git count. Left alone; if D6's principle is meant to reach it, that is a one-string change.
- **D7 renders the labels through `untrusted`.** The labels are a fixed, trusted set, but `CatalogArgs` has no runtime-string trusted argument (`trusted_symbol` is `&'static str`), so they go through `quote_untrusted`, which adds only invisible isolate marks to plain ASCII. The alternative was concatenating English in Rust.
- **No keyboard chord in the Terminal-mode text.** Naming `Ctrl+Alt+T` there would be a claim that becomes false if the binding is ever changed — the class of defect this RFC is about. The button is what the text points at.

### Ablation — restore one old wording

Run from a committed tree, restoring each file with `git checkout --` afterward (`git status` empty before and after).

| Restored | Failed |
| --- | --- |
| Terminal placeholder (`RFC-017 adds the terminal here`) | `terminal_mode_empty_state_…` **and** the D9 scan `no_catalog_string_names_an_internal_identifier` |
| Sidebar `Sidebar` | `the_sidebar_and_content_placeholders_…` |
| Change Review "No changes have been detected in this project yet" | `change_review_empty_state_…` and the agreement test |
| `dirty files` (plural arm) | `the_editor_buffer_count_says_unsaved_and_never_dirty` and the agreement test |
| Attention `Dirty` | `the_attention_state_raised_by_unsaved_buffers_says_unsaved` |
| `Unknown` for a fresh project's terminals | the two board tests in core **and** the agreement test |
| Blocked names line disabled | `blocked_automations_are_named_not_only_counted` |
| Caveats restored above the empty state | `approval_history_leads_with_its_empty_state_…` |

Every ablation fails its own dedicated test. "Alone" is not literally true for four rows — the agreement test, or the scan, is a second detector — which is the design, not a leak: it is the cross-surface check the RFC exists to add.

### Live capture — release binary, `mktemp -d` fixture, throwaway `XDG_STATE_HOME`

A git repository with one modified and one untracked file, no agent run.

- `evidence/01-board-and-status-bar-agree.png` — board: *0 terminals, 0 agent runs, 0 pending approvals, 0 reviews, 0 unsaved files*, then *9 blocked automations* and the nine names. Bottom bar: *Git: master 2 changed*.
- `evidence/02-change-review-empty-state-beside-two-changed-files.png` — Change Review: *"No AI CLI run has produced changes in this project yet. Changes you made yourself are shown by the status bar and the file explorer."* In the **same frame**: the explorer's `[modified]` and `[untracked]` badges and the bar's *2 changed*.
- `evidence/03-terminal-mode-empty-state.png` — Terminal mode's empty state and the *+ New Terminal* button it points at; the sidebar reads *Files are listed here in Content mode.*

**The checklist's "one frame" cannot be literal.** The board is a route; Change Review is a surface inside the project workspace. They are never on screen together, so the box stays unticked with that named. Images 01 and 02 are one session and one fixture; 02 is the frame in which Change Review, the explorer and the bar agree.

### Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and three consecutive `cargo test --workspace --all-targets --no-fail-fast` runs, output to files: **581 + 9 + 876**, green all three (`+11` in `tekstide` over `0.22.0`: 2 scan tests, 5 shell tests, 4 board tests; `tekstide-core` unchanged — two existing tests were re-pointed at the new answer, none added). The colour-alone and i18n completeness scans are part of that run and pass. No new intermittent.


## PR-053-B — the layout, last and alone

Commits `9445666` (modal footers, measured pane region) and `d89d366` (row height, status-bar wrap,
scrollbar). Review 418's carried items went first as their own commits: `5ffce5e` (C1, "unsaved" in
the close confirmation and the text harness) and `02b088f` (C2, the bidi-isolate note in
`ARCHITECTURE.md`), so nothing but B is in B's commits.

### What was found first — the defect was larger than the RFC's table

Before writing code I ran the `0.22.0` release binary, opened a real terminal and typed `seq 1 200`.
**The output ended at 177.** The last 23 lines were below the visible area
(`evidence/04-terminal-before-seq-200-ends-at-177.png`). The RFC named the status bar; the bar is one of
**four** independent causes of a pane being sized for space it did not have:

1. `top_bar_height` was `2 × 8 + one heading line`. The top bar has been **three rows** since
   RFC-039/040 (title, tab strip, actions). About 85px unmodelled.
2. The mode-toggle row and the `+ New Terminal` row were **not in the formula at all**.
3. The status bar wraps at a narrow window (the RFC's G3).
4. Independent of all of the above: rows were **counted** with `LineHeight::Relative(1.0)`
   (`font_metrics::line_height_px`) but **drawn** with `rich_text`'s default `Relative(1.3)`. Every
   pane got ~30% more rows than its height held.

Fixing only the bar would have left `seq 1 200` ending near 190 and the box "a terminal is sized to the
space that exists" unsatisfiable. After measuring the region (1–3) the output still ended at **188**;
that residue is what exposed (4).

### The deviation from D3 as written — this is the thing to review

D3 says `content_area_height` derives from the **rendered status-bar height**. I did not keep a formula
with one measured input, because the formula's other inputs were wrong for reasons above. Instead:

- `surface/frame.rs::MeasureSize` wraps the region the panes occupy and publishes the size the layout
  engine gave it (`Message::PanesRegionMeasured`), on the redraw after a layout and **only when it
  changes**.
- `terminal_workspace_content_size` **reads** that size. It no longer computes anything. The seven
  chrome constants, `window_size`, `Message::WindowResized`, `Message::WindowOpened` and the two window
  subscriptions existed only to feed the formula and are **deleted**, not left dormant.
- The window is `column![top_bar, content, status_bar]` with the content `Fill`
  (`assemble_window_layout`), so a taller bar shrinks the content by exactly its extra height because
  the layout engine says so.

This reverses response 242's "a computed size needs no measurement". That principle was sound while
the computation was right; the number it protected was wrong. Hidden panes are unaffected — every pane
is sized from the same region.

### D2 — modal actions stay in the window

`surface/frame.rs::PinnedFooter` lays the **footer out first**, then gives the body what remains.
`iced`'s `Column` lays children in order, so `column![scrollable(body), footer]` hands the scrollable the
whole height and the footer none; `a_plain_column_clips_the_footer_which_is_why_the_widget_exists` pins
that so the widget is not "simplified" away. `modal_dialog_box` now takes `(body, footer)`; the eleven
modal builders each say which trailing lines are actions. The 16px outer margin keeps a full-height
dialog off the window edge, and the scrollbar is embedded with spacing — an overlaid bar covered the last
characters of the longest line in the first capture.

### Ablation — the constant back, and five more

Each from a committed tree, restored with `git checkout --`, `git status` clean after. One of my first
runs did not compile (I had deleted a variable the ablation needed) and is **not** counted; it was redone.

| Restored | Failed |
| --- | --- |
| PTY size from a fixed size, ignoring the measured region — **the RFC's "put the constant back"** | `pane_geometry_follows_the_measured_region_and_nothing_else`, plus the two region tests that share its mechanism |
| Status bar fixed at 20px inside `assemble_window_layout` | `a_taller_status_bar_shrinks_the_content_area_by_exactly_the_difference` **alone** |
| `PinnedFooter` laying the body out first | `the_footer_stays_inside_the_window_however_long_the_body_is` **alone** |
| Row height measured at `Relative(1.0)` | `the_measured_line_height_is_the_drawn_line_height_not_font_size` **alone** |
| `MeasureSize` publishing on every redraw | `measure_size_publishes_the_laid_out_size_once_and_again_only_on_change` **alone** |
| Status bar as a plain `row!` | `the_status_bar_wraps_whole_fields_rather_than_squeezing_them` **alone** |

The first row fails three tests, not one: they all assert the same mechanism (a measured region sizes the
pane). I have not called that "alone".

### Live captures — release binary, `mktemp -d` fixture, throwaway `XDG_STATE_HOME`

- `04-…` before (`0.22.0`): `seq 1 200` ends at **177**.
- `05-…` after: ends at **200** with the `tekstide$` prompt visible and a small margin.
- `06-…` at **520×400**: the status bar wraps its fields whole onto two lines, every field intact; the
  pane holds the four rows that exist (198, 199, 200, prompt).
- `07-…` and `08-…`: the keyboard reference at **760×560** and **520×400** — the body scrolls, `Close`
  and "Escape closes this." are inside the window.
- **Not committed:** a capture of the folder browser at 520×400. It confirmed its `Open this folder`
  button and hint are pinned, but the browser opens at `$HOME` and the image shows the home path.
  Committed screenshots may show throwaway state only.

### Limits, stated

- **Live-verified modals: the keyboard reference (both sizes) and the folder browser (520×400, not
  committed).** The other nine are covered structurally: `no_modal_is_built_without_a_pinned_footer`
  scans that none uses the old undivided call and that the eight `Vec`-built ones each name a footer
  length, and I read each builder's tail to check its count (buttons, then the hint). The scan cannot
  tell *which* lines a builder put in its footer.
- **Null-renderer tests prove the algorithm, not font metrics.** Text measures as zero height there, so
  stand-in lines have stated heights. Real metrics are what the captures are for.
- **`niri msg action set-window-width/height` resized the window exactly** (`Window size: 520 x 400`).

### Gate (whole RFC, after B)

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, three consecutive
`cargo test --workspace --all-targets --no-fail-fast` runs, output to files: **593 + 9 + 877**, green all
three (`tekstide` +6 over A: four `surface::frame` tests + two `font_metrics` + the D2/D3/wrap shell tests,
net of the geometry tests renamed rather than added; `tekstide-core` +1, C1's test). The colour-alone and
i18n completeness scans are inside that run and pass (§7). No new intermittent; nothing added to
`test-process-leak.md`.
