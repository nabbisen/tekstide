---
title: "RFC-053 — task breakdown and PR plan"
rfc: "RFC-053"
rfc_file: "../../accepted/053-what-the-window-says-is-true.md"
source_rfc_status: "Accepted 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Task breakdown and PR plan

**A is the words. B is the layout, and it goes last and alone.**

## PR-053-A — what the surfaces say

- **D1**: the Terminal-mode placeholder becomes an empty state that says what the mode is and how to
  start a terminal. **D9**: a scan refuses an internal identifier in any catalog string, and its
  failure message states that doc comments are not covered.
- **D4**: Change Review's empty state says no **agent run** has produced changes, and points at where
  repository changes are shown.
- **D5**: a project whose collections have never been mutated reports **zero** terminals and zero
  agent runs. The change is at the board row that has the counts, **not** in `ProjectProviderState`.
- **D6**: the board's *dirty files* says *unsaved* — it counts open editor buffers — leaving
  *changed* to mean what Git means.
- **D7**: the board's blocked-automation count carries the labels the model already holds
  (`REQ-NOTIFY-003`, applied where it was not).
- **D8**: Approval History's caveats follow its content; an empty surface says it is empty first.

**Required tests:** the scan fails on a planted `RFC-0` catalog string and passes on a doc comment;
a fresh project reports zero, not unknown; each reworded string is asserted by its own test rather
than by eye. **Ablation:** restore one old wording; its own test fails alone.

**Evidence:** one live capture of a project with two changed files and no agent run, in which the
board, the status bar and Change Review **agree**.

## PR-053-B — the layout, last and alone

- **D2**: modal content scrolls; `Close` and the dismiss hint stay reachable at any size the
  application can be given.
- **D3**: `content_area_height` derives from the **rendered** status-bar height, not a constant.

**Required tests:** at **760×560** every modal's `Close` is reachable; at **520×400** the content area
shrinks by exactly the bar's extra height. **Ablation:** put the constant back; the height test fails
alone — that failure is the whole point of the slice, so run it and report it.

**Evidence:** captures at both sizes, and a terminal open at the narrow size showing that its pane is
sized to the space that actually exists.

## Why B is separate

Every PTY's row/column count comes from this number. A wrong measurement resizes every terminal a
user has open, silently. Separate commit, separate review, no other change riding along.
