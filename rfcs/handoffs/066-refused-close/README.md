---
title: "RFC-066 handoff: a refused close must not have already terminated"
status: "**Implemented and closed 2026-10-09** — `0.32.0` candidate, not yet published. Both slices and the Whole-RFC checklist closed."
rfc_file: "../../done/066-a-refused-close-must-not-have-already-terminated.md"
target_milestone: "M13"
created: "2026-10-08"
---

# RFC-066 handoff

## Read first

1. The RFC, especially *Decided on acceptance* — D3 removes a surface you might otherwise build.
2. [`what-the-close-path-must-not-do.md`](./what-the-close-path-must-not-do.md) — four rows, two of
   them work-loss rows.
3. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).
4. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## The one-paragraph version

Confirming a project close while a document is dirty terminates every running terminal in that
project and then refuses to close it, saying nothing. The termination runs before the assessment is
consulted. Fix the ordering; then make the modal refuse up front so the confirmation never existed.

## Start by reproducing it

**Nobody has watched this happen.** It was read in the code at review 489, not seen. The first thing
PR-066-A does is reproduce it — real project, real spawned terminal, real unsaved edit, real
confirm — and that reproduction becomes the regression test. If it does not reproduce, **that is the
finding**, and it outranks the fix: say so instead of repairing something that was never broken.

## Where the pieces are

| What | Where |
| --- | --- |
| The confirmation path, and the ordering defect | `crates/tekstide/src/shell.rs`, `apply_project_close_confirmation` |
| What it terminates first | the same file, `terminate_project_live_work` |
| The assessment that then refuses | `crates/tekstide-core/src/app.rs`, `assess_project_close` → `close.rs`, `assess_close` |
| The reasons, already structured | `CloseReasonCode::{DirtyFile, PendingApproval, ReviewReadyChange, RunningProcess}` |
| The code's own admission of half of it | the comment at the end of `apply_project_close_confirmation` |

## The trap in this one

**A fix that only changes the modal looks complete and is not.** D3 makes the dialog refuse up
front, so the bad sequence stops being reachable *through that dialog* — and the ordering defect
survives for every other caller of the path. **D1 is the fix; D3 is the interface.** Do both, in
that order, and do not let the second make the first look unnecessary.
