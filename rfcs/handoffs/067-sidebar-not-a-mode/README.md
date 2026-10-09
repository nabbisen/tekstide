---
title: "RFC-067 handoff: the sidebar is not a mode"
status: "**Implemented and closed 2026-10-09** — released as `0.33.0` 2026-10-10. All three slices and the Whole-RFC checklist closed."
rfc_file: "../../done/067-the-sidebar-is-not-a-mode.md"
target_milestone: "M13"
created: "2026-10-08"
---

# RFC-067 handoff

## Read first

1. The RFC, especially *Decided on acceptance* — **D8 is a hazard you would otherwise meet late**.
2. [`what-the-sidebar-must-not-do.md`](./what-the-sidebar-must-not-do.md).
3. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).
4. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## The one-paragraph version

`sidebar_view` matches on `ProjectMode`, so the file explorer renders only in Content mode. In
terminal mode the sidebar shows one sentence — *"Files are listed here in Content mode."* — in a
zone `Tab` can still reach. Make the explorer persistent; delete the sentence. Then measure what a
mode switch costs to draw, and let that number decide whether anything further is needed.

## This slice removes interface

That is the test to hold it to. Nothing is displaced, no zone is added, no vocabulary is introduced.
If the change starts growing a layout concept, it has left this RFC — that is a separate design with
its own RFC, per the non-goals.

## Where the pieces are

| What | Where |
| --- | --- |
| The mode match to remove | `crates/tekstide/src/shell.rs`, `sidebar_view` |
| The string to delete | `sidebar-placeholder-title`, `crates/tekstide/locales/en.ftl` |
| The label that composes it | `sidebar_label`, same file as `sidebar_view` |
| The focus zone that already exists in both modes | `FocusZone::Sidebar` |
| The harness for D4 | `crates/tekstide/src/shell/tests/editor_baseline.rs` — paired rounds, control inside the same run |

## The measurement, and what counts as success

D5 says plainly that **a third slice which builds nothing is a success** if the switch turns out to
be below perception. Report the number per switch, with the spread, and the control carried in the
same run — the shape RFC-027 arrived at after four reviews, not the shape it started with. Do not
divide one condition's figure by another's count and call it a rate.
