---
title: "RFC-038: First-Run and Project Entry — implementation handoff"
rfc: "RFC-038"
rfc_file: "../../accepted/038-first-run-and-project-entry.md"
source_rfc_status: "Accepted 2026-08-24 — M12, first"
target_milestone: "M12"
created: "2026-08-24"
---

# Give the product a door

Source RFC: [RFC-038](../../accepted/038-first-run-and-project-entry.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-038](../../accepted/038-first-run-and-project-entry.md) | Goals, non-goals, and the five decisions already made — do not re-open them |
| 2 | [`what-a-path-field-must-not-trust.md`](./what-a-path-field-must-not-trust.md) | **Security-critical. Read before writing any code.** A user-typed path is untrusted input that gets echoed back |
| 3 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Five slices, in order, with what each may and may not touch |
| 4 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced before Final Acceptance |
| 5 | [`qa-evidence.md`](./qa-evidence.md) | Where your evidence goes |

## What this is

`tekstide` has no in-app way to open a project. The only production caller of
`add_project_from_path` is `boot()`'s CLI-argument loop in `main.rs`, so a user who runs the
binary with no path sees a board they can do nothing with, and every capability the product
has — terminals, agent runs, trust, transcripts — sits behind a door that only opens from a
shell command line.

`0.12.1` made the empty state stop lying about that and listed the keybindings. **This slice
adds the missing action.**

## The one thing that will surprise you

`crates/tekstide/src/tests.rs` holds
`add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else`. It is an
**occurrence-count** test: exactly one call in `main.rs`, zero in every other file. Your new
call site **will fail it**, and that is the test doing its job, not an obstacle.

Its own doc comment, written during RFC-031, names this exact slice in advance:

> `record_project_added_if_possible` is called from the call site, not from
> `add_project_from_path` itself — `AppState` holds no `AuditCoordinator`, so the operation and
> the record cannot live together the way `grant_project_trust`'s do. That makes auditing a
> thing a future caller must remember: **an interactive "Add Project" flow would compile and
> work with no record and no error.**

So: when you add the second call site, wire its audit record deliberately — reuse
`open_cli_project_path_and_record` or call `record_project_added_if_possible` yourself — and
update the test's allow-list to name both files with their exact counts. **Do not** relax it to
a file-presence check; the guarded property is "every *call* writes a record", and per
`ARCHITECTURE.md`'s enumeration-test unit rule the unit must stay the call.

## What "done" means here

Not "the field renders." A person who has never read anything, given only the built binary,
can put a project on the board. That is the acceptance criterion, it is proven from a **real
key event through production code**, and the first evidence item in the checklist is a **cold
start** — no arguments, fresh `XDG_STATE_HOME` — because this whole RFC exists because nobody
ever did that.

Reference for what a cold-start capture looks like and how to record one:
[`../first-run-correction/evidence/cold-start-empty-board.md`](../first-run-correction/evidence/cold-start-empty-board.md).

## Scope boundaries

**In:** a path entry field on the Project Board, focused when the board is empty; `Ctrl+Alt+O`;
a help surface that does not require the board to be visible; recent-projects reopen; removal
of `ProjectBoardEmptyState`'s two dead fields.

**Out:** a directory picker (would add this project's first XDG desktop portal dependency —
decided against in D1, explicitly not foreclosed later); project *creation*; a second explorer
tree; anything about RFC-020's change-review surface.

**Escalate rather than descope.** If PR-038-A through C run long, PR-038-D (recent projects) is
the droppable one — but dropping it is the human owner's decision, routed through the architect,
not yours. Say so and stop; do not quietly ship four fifths.
