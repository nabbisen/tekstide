---
title: "RFC-053 — acceptance and QA checklist"
rfc: "RFC-053"
rfc_file: "../../accepted/053-what-the-window-says-is-true.md"
source_rfc_status: "Accepted 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-053-A — what the surfaces say

- [x] **No catalog string contains an internal identifier**, held by a scan (D1/D9). The scan **fails
      on a planted string** and **passes on a doc comment**, and its message says which it covers.
      — `no_catalog_string_names_an_internal_identifier` (real `.ftl` files) and
      `the_internal_identifier_scan_fails_on_a_planted_string_and_passes_on_a_comment`; the failure
      message states that `#` comments and Rust doc comments are deliberately legal.
- [x] Terminal mode's empty state says what the mode is and how to start a terminal — no `RFC-`
      anywhere near it. — `terminal_mode_empty_state_says_what_it_is_and_how_to_start_a_terminal`.
- [x] Change Review's empty state names **agent runs**, and points at where repository changes show
      (D4). — `change_review_empty_state_names_agent_runs_and_points_at_where_git_changes_show`.
- [x] A freshly opened project reports **zero** terminals and zero agent runs (D5), and
      `ProjectProviderState` is untouched. — the change is one place, `active_project_row` in
      `project_board.rs`, which counts the collections it holds when the summary has not been
      refreshed yet; `git diff` shows no edit to `project/metadata.rs`. Two pre-existing tests
      that asserted the old `Unknown` were updated to the new answer, their comments saying why.
- [x] The board says **unsaved** for editor buffers; *changed* is left to mean Git (D6).
      — `the_editor_buffer_count_says_unsaved_and_never_dirty`, and (same fact, other word) the
      attention state raised by those buffers now reads "Unsaved edits", not "Dirty" —
      `the_attention_state_raised_by_unsaved_buffers_says_unsaved`. Not in the pack's list; disclosed.
- [x] Blocked automations carry their labels, not a bare count (D7).
      — `blocked_automations_are_named_not_only_counted`.
- [x] Approval History's caveats follow its content (D8).
      — `approval_history_leads_with_its_empty_state_and_shows_caveats_only_with_entries`, over
      the new `approval_history_leading_keys`.
- [x] **Ablation:** restore one old wording; its own test fails alone. — run for **eight**
      wordings/behaviours from a committed tree; each fails its own dedicated test. Three also trip
      the agreement test as a second detector, and D1's terminal string trips the scan as well as
      its own test — reported plainly in `qa-evidence.md` rather than called "alone".
- [x] **Captures**: Change Review's empty state, the explorer's badges and the bar's `2 changed` in
      **one frame** — that is where the contradiction lived — plus the board with the bar. Throwaway
      state only.
      *Corrected at review 418: the box asked for the board, the bar and Change Review in one frame.
      The board is a route and Change Review is a surface inside the workspace, so no such frame can
      exist. The reviewer's box was unsatisfiable; the two captures are better evidence than it asked
      for.*
      — **left unticked; the box cannot be satisfied as written.** The Project Board is a *route*
      and Change Review is a surface *inside* the project workspace, so they are never on screen
      together. What exists instead: `evidence/01-…` (board: 0 terminals, 0 agent runs, 0 unsaved
      files, bar "2 changed") and `evidence/02-…` (Change Review's agent-run empty state, the
      explorer's two badges and the bar's "2 changed" **in one frame**), from one session against
      one fixture. Reviewer to decide whether that discharges it.

## PR-053-B — the layout

- [x] At **760×560**, every modal's `Close` control and dismiss hint are reachable; content scrolls.
      — `PinnedFooter` (`surface/frame.rs`), proven headlessly for any body length at 560 and 400px;
      live at 760×560 **and** 520×400 for the keyboard reference (`evidence/07-`, `08-`). **Named
      limit:** the other nine modals are covered structurally, not by a capture each — see
      `qa-evidence.md`.
- [x] At **520×400**, `content_area_height` subtracts the **rendered** bar height: the content area
      shrinks by exactly the bar's extra height. **Ablation:** restore the constant; that test fails
      alone. — **satisfied by a different mechanism than the box names, and that is the point to
      review.** There is no `content_area_height` any more: the formula was wrong for reasons beyond the
      bar (see `qa-evidence.md`), so the PTY is sized from the region the layout gave it.
      `a_taller_status_bar_shrinks_the_content_area_by_exactly_the_difference` (fails alone when the
      bar is given a constant height) and `pane_geometry_follows_the_measured_region_and_nothing_else`
      (fails when a constant size replaces the measurement) are the two halves.
- [x] A terminal open at the narrow size is sized to the space that exists — captured, not reasoned.
      — `evidence/06-`; and the same at normal size, before and after (`04-`, `05-`): 177 → 200.
- [x] No other change rides along in this commit.
      *Reviewed 419: the risky change — the measured region and `PinnedFooter` — is alone in
      `9445666`. The three riders the capture box required (row height, status-bar wrap, embedded
      scrollbar) are in `d89d366`, each with its own test. Commit granularity is what this box
      protects, and it holds; disclosing them rather than ticking quietly was the right instinct.*
      — **left unticked; three changes rode along, each required by the capture box above.** (1) Rows
      were counted at line height 1.0 and drawn at 1.3 (`grid_line_height`); without it the pane
      still ends 12 lines short (`seq 1 200` ended at 188). (2) The status bar wraps whole fields
      (`Row::wrap`) — at 520px a plain row cut `Ctrl+Alt+P` to `Ctrl+Alt+`. (3) The modal scrollbar is
      embedded, not overlaid — the first capture showed it covering the end of the longest line. Also
      deleted, as dead once the formula went: `window_size`, `WindowResized`, `WindowOpened`, the two
      window subscriptions and seven chrome constants. Carried items C1/C2 are **separate commits**
      (`5ffce5e`, `02b088f`).
- [ ] **C1 (carried from review 418):** the close-project dialog says **unsaved**, like the board.
      After PR-053-A the product calls one fact "unsaved" in one place and "dirty" in another, and
      that inconsistency is one this RFC introduced. One string.
- [ ] **C2 (carried from review 418):** the bidi-isolate lesson is in `ARCHITECTURE.md` — numbers
      render inside invisible isolate marks, so `0 agent runs` is not a substring of the rendered
      line. A comment in one test file is not where the next person will look.

## Whole-RFC

- [x] The colour-alone scan and the i18n completeness scan still pass (§7). — both inside every
      full-workspace run below.
- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files. — 593 + 9 + 877, all three.
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`. — none occurred.
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
