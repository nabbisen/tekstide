---
title: "RFC-039: Interaction Model and Visible Affordances — implementation handoff"
rfc: "RFC-039"
rfc_file: "../../done/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-039 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-24"
---

# Give the product workflows

Source RFC: [RFC-039](../../done/039-interaction-model-and-visible-affordances.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-039](../../done/039-interaction-model-and-visible-affordances.md) | The seven workflows, three principles, and three decisions already made |
| 2 | [`what-closing-a-project-must-not-lose.md`](./what-closing-a-project-must-not-lose.md) | **Required reading before any close code.** The only destructive action in this RFC |
| 3 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Four slices and their order |
| 4 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |
| 5 | [`qa-evidence.md`](./qa-evidence.md) | Where evidence goes |

## What this is

Everything shipped so far is reachable by a keystroke someone had to learn. Five buttons exist
in the whole application, all inside Trust Settings and Approval History; the Project Board — the
surface a user arrives at — has none. Its rows are text. There is no visible way back to it, and
**no way at all to close a project**: `close_project` is reviewed, tested core API with no
production caller.

This RFC makes the product's work visible: a tab strip that shows which projects are open, lets
you move between them, take you home, and close one.

## The principle that governs every review of this RFC

> **A capability with no visible affordance is not shipped.**

The existing reachability doctrine — *name the path a user takes to reach it* — has been
satisfied nine times by naming a keystroke. That is no longer sufficient. When a slice here
claims a workflow is served, the answer must name **the control a user sees**, not the key that
also works.

## What "done" means

A person who has never read anything opens two projects, moves between them, closes one, and
returns to the board — using only what the window shows them. Proven from real events through
production code, with a cold-start capture, exactly as RFC-038's slices were.

## Scope boundaries

**In:** the tab strip (open projects, switch, close, home, `+` to add); wiring `close_project`
and `safe_close_decision`; the close confirmation; the affordance audit.

**Out:** themes, icons, visual restyling. Mouse-only anything — every control added must be
keyboard-operable, RFC-015's focus model and RFC-018's trusted-chrome rules unchanged. RFC-020's
change-review surface. The Help surface and folder browser, which are RFC-038's.

**The affordance audit is not optional and not a formality.** Every `NavigationAction`, and
every capability a user is expected to perform, listed against the visible control that invokes
it. Anything with none is a finding, reported — not quietly given a keybinding.
