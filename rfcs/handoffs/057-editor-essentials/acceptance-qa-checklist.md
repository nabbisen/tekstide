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

- [x] Line numbers come from the real line index, not the row's position in the window.
- [x] **The caret is an element, not a character**: the fixture contains a file whose text holds the
      caret's own character, and the two are distinguishable. Ablated.
- [x] **One cursor, one reader**: a planted second derivation fails a test.
- [x] A file crossing 10 000 lines does not shift its text column while scrolling. Captured at 5 and
      6 digits.
- [x] `REQ-EDIT-002` met, **and its coverage row corrected** from implying it already was.

### Required at review 442

- [x] **Q2: a horizontal window that follows the cursor**, on the same rule as the vertical one —
      the least movement that keeps it on screen. **Not** soft wrap: exact one-line-one-row
      arithmetic is the property B rests on. Ruled from B's own capture, which showed *Line 1,
      Column 525* while the visible text ended near column 60 — a user typing into text they cannot
      see, and a caret that could not be visible at all on that line.
- [x] A capture of a long line with the cursor visible, and the changelog correcting B's "clipped at
      the right edge with no horizontal scroll" **by name**.

## PR-057-D — undo

- [x] Operations, not snapshots; the depth is bounded and the bound is **stated when reached**.
      *(`EditOperation`'s four variants store no document copy; `UNDO_MAX_DEPTH = 500`;
      `history_bound_line` is the on-screen disclosure, naming the real number.)*
- [x] Typing then undoing restores text **and cursor**.
      *(Held for all four edit kinds, and through real `Ctrl+Z` routing.)*
- [x] Undoing to the opened text returns the document to `Clean`.
      *(`replace_text`'s one-way latch fixed, guarded to not override `ExternalChanged`/`Conflict`/`SaveError`.)*
- [x] Undo does not cross an external-change reload and write over a file that moved underneath.
      *(Free by construction — a reload replaces the whole `TextDocument`, never patches it in place —
      proved end to end with a real conflict, a real Reload, then a real `Ctrl+Z`.)*
- [x] Redo then save writes what the screen showed.
      *(Bytes read back from disk after redo, compared against the screen's own text.)*
- [x] The requirements gap (no `REQ-EDIT` names undo) is written up for the owner; no requirement is
      minted.
      *(`delivery-plan.md`, "Requirements gap: no `REQ-EDIT` names undo," 2026-09-29.)*

### Required at review 443

- [x] **Q3: the width approximation gets a boundary, not a disclaimer.** `CHAR_WIDTH_FACTOR = 0.6`
      approximates a face that capture `01-` shows is **proportional by default**. Drive the
      every-column test with a line of wide characters (`W`) and one of narrow (`i`) and report
      whether the cursor stays inside the window — and if it leaves, by how many columns. A number.
      If it can leave, the book says so where `ui_font` is documented.
      *(`W` overflows by ~51 columns, measured against real `iced` layout and captured live — the
      caret vanishes at column 81 while the header still reports it. `i` never overflows. The book's
      `configuration.md` discloses it beside the family setting. Test `#[ignore]`d, reason stated:
      a real, disclosed, structural finding pending the owner's decision, not a load flake.)*
- [x] **Owner decision, ruled 2026-09-29 — and neither option I offered.** *The editor body stays
      user-configurable; its default becomes fixed-width.* Better than both: a monospace face has one
      advance width, so `CHAR_WIDTH_FACTOR = 0.6` is right **by construction** for anyone who does not
      change it, and review 444's ~51-column caret loss stops being shipped by default. A user who
      chooses a proportional family still gets it, and inherits an approximation the book already
      discloses with its measured number.
- [x] **Implemented 2026-09-29, accepted at review 448.** the editor body defaults to a fixed-width face
      **Reviewer, at 448:** `Theme::editor_font()` is `family.map_or(Font::MONOSPACE, Font::with_name)`
      — the same resolution with one different fallback. `Font::MONOSPACE` measures **8.400 px/char
      against 8.400 approximated, 0.0 % off**, asserted rather than `#[ignore]`d because it holds;
      Q3's proportional finding stays `#[ignore]`d because it is a published defect, not a gate.
      **The boundary rule:** anything that takes part in the body's column arithmetic shares the
      body's face — which is why the gutter follows it and `chrome_line`, `cursor_line`, Save and the
      empty state do not. Original wording of this box:
      while still honouring a configured `family`; `configuration.md`'s paragraph and the changelog
      say so; review 444's disclosure stays, reframed as the consequence of a choice rather than the
      default anyone gets. The original wording of this box: the book says the family
      applies *"including the editor's"* while the terminal and tree keep a fixed-width face
      *"because a column of code has to line up"*. The editor now has a gutter and column arithmetic,
      so it meets that reason. Either it joins the fixed-width side (recommended), or the horizontal
      window measures real text. The changelog must describe whichever is chosen.
      *(`Theme::editor_font()` added, same `family` resolution as `font()`, `Font::MONOSPACE`
      fallback. Held: `the_default_editor_font_is_monospace_and_the_width_approximation_holds_for_it`
      — `8.400` px/char measured vs `8.400` approximated, `0.0%` off, zero overflow. Captured live at
      column 81, the exact column review 444's `07-` lost the caret at — the caret is visible now.
      `configuration.md` and the changelog both corrected.)*

## PR-057-E — the two-row header

- [x] The actions row carries the title and sits above the tab strip.
      *(`top_bar_actions_row` renamed `top_bar_title_and_actions_row`, now builds the title in as its
      first element; `top_bar` stacks it and `project_tab_strip` alone, two rows not three.)*
- [x] Captured, and **every superseded capture named** across the evidence packs.
      *(`evidence/pr-057-e/01-…`/`02-…`, with-project and no-project. All eleven prior images named
      as superseded on the header only, not on what each was actually evidence for — see
      `qa-evidence.md` § PR-057-E.)*

### Required at review 445

- [x] **`crates/tekstide/typing-measurement-sample.rs` is deleted or relocated.** 1,472 tracked
      lines, moved there at the `0.4.0` candidate and untouched for twenty-three releases; not
      compiled (crate root, not `src/`); invisible to every mechanical scan (they walk `src/`);
      seven `use crate::…` imports naming *core* modules from inside the *app* crate; **and it ships
      in every published archive, including `0.27.0`.** Not RFC-057's doing — found tracing
      `replace_text`'s callers at review 445.
      *(Relocated, not deleted — the file is real, load-bearing content
      (`crates/tekstide/src/shell.rs`'s `TYPING_MEASUREMENT_DOCUMENT`, RFC-015 PR-015-F's
      typing-measurement surface, `include_str!`-ed) and deleting it would have broken that surface.
      Now `crates/tekstide/tests/fixtures/typing-measurement-sample.rs` — a subdirectory of `tests/`,
      so cargo still never auto-builds it as an integration test binary, and it reads correctly in a
      directory listing. See the response to review 445.)*
      **Reviewer, at 446: the finding above was wrong on its central claim.** Not compiled,
      not scanned and shipping are all true and do not add up to dead — the file is `include_str!`-ed,
      so its shipping is *required* and its uncompiled state *deliberate*. `src/shell.rs` has said so
      since `0.4.0`; I read six lines of the file and the archive listing and never grepped for its
      name. **The test for deadness is whether anything references it**, and `include_str!` is a
      reference. Only the location was wrong.
- [x] **A package check**: the archive carries no `.rs` outside `src/`, `tests/`, `examples/`,
      `benches/`. The reviewer's entry-list diff compares releases and is blind to a file that has
      always been wrong.
      **Reviewer, at 446:** read this as a **placement convention, not a deadness check** — the
      property worth holding is what cargo compiles, and a file included by `include_str!` is data
      with a `.rs` extension. It still closes a real gap: an entry-list diff compares releases and is
      blind to a placement that has always been wrong.
      *(Added to `release-checklist.md`'s Package Smoke section, with the exact `tar tzf` command;
      verified once by hand against a real `cargo package -p tekstide --locked --no-verify` tarball —
      every `.rs` path now starts with `src/` or `tests/`.)*
- [x] **Before PR-057-E starts**: the capture path works. E is a purely visual change with no state
      to assert, so an unresolved `niri` screenshot gap blocks it rather than inconveniencing it.
      *(Resolved: `screenshot-window` always copies to the clipboard regardless of niri's own
      `screenshot-path` config (`null` on this machine, which silently drops the on-disk write even
      with `-d true`) — `wl-paste --type image/png > file.png` right after the action produces the
      file. Verified against a real, focused Tekstide window. Recorded in memory for future sessions.)*

## Whole-RFC

- [x] `REQ-EDIT-002` moves to met with evidence a user can see; `NFR-REL-005` records undo;
      `NFR-PERF-003` has a measured number for the first time.
      *(`REQ-EDIT-002`: PR-057-C, `02-…`. `NFR-REL-005`: PR-057-D's whole evidence section.
      `NFR-PERF-003`: PR-057-A/B's measured p95/p99 figures, in the changelog.)*
- [x] The colour-alone, i18n completeness and internal-identifier scans still pass.
      *(Part of every full-workspace run this RFC's slices gated on; last confirmed in PR-057-E's own
      three clean runs.)*
- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, **three consecutive full-workspace runs with `--no-fail-fast`**
      to files, **0 fixture entries left** in a fresh short `TMPDIR`.
      *(PR-057-E's own gate: clean, `712 + 16 + 1045` ×3, 0 left, no intermittent.)*
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`.
      *(2026-09-29 entries: the sharper `SocketPathTooLong` reading, and row 1's recurrence.)*
- [ ] The core pin bumps with the version (`the_workspace_pins_tekstide_core_to_its_own_version`).
      *(Release-time step, not yet due — `0.28.0` has not been cut. `[workspace.dependencies]` still
      pins `0.27.0`, correctly, until the version bump that release-checklist.md's own process owns.)*
- [x] Commits are pushed once the gate is green. — **Reviewer, at 447:** this is a per-slice
      step, not a release-time one, and every slice A–E was pushed after its own gate. The pin
      bump above *is* release-time and is correctly left unticked.
      *(Every implementer commit was pushed once its own gate went green, including this RFC's
      tail: `1f7003d` (D), `d517276` (the review-445 fix), `ec9bb0b` (E).)*

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
