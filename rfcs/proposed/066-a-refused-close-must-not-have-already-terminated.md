# RFC-066: A Refused Close Must Not Have Already Terminated

Status: **Proposed 2026-10-08.** `0.32.0`, M13. Repairs a defect live in `0.30.0`. Found at review
485 (the dev team, the refusal) and traced at review 489 (the reviewer, the termination). No
requirement names this; it is a defect in what `REQ-PROJ-004`'s own close path already promises.

## Summary

Confirming a project close while a document has unsaved changes **kills the project's running
terminal sessions and then does not close the project**, with nothing on screen explaining either
half.

The order in `apply_project_close_confirmation` is: terminate every live terminal session, *then*
ask whether the project may close. A dirty document makes that answer no — `assess_close` pushes a
`DirtyFile` reason, so the assessment is not `SafeToClose` and `close_project` returns without
closing. The modal was already dismissed when the user confirmed. The project is still there, the
shells are gone, and nothing said so.

**The work destroyed is unrecoverable.** RFC-027 is giving unsaved buffers somewhere to come back
from; a terminated terminal session has no such record and never will. This is the same shape as
the defect RFC-065's first slice repaired — ordinary use, silent loss, shipping now — and it is
worse in one respect: the user **asked for the thing that destroyed their work**, and was told
nothing happened.

## What is already right, and must stay right

**Refusing to close is correct.** The assessment exists so that unsaved files, pending approvals and
review-ready changes are not closed over, and the code refuses rather than bypassing it — a
deliberate choice its own comment defends. **Nothing in this RFC forces a close.** The defect is not
that the close was refused; it is that the refusal came after the damage and without a word.

## Decisions

**D1 — Nothing live is terminated until the close is known to be going ahead.** The assessment runs
to completion first; termination happens only on the path where the project is actually closing.
*Measurement 1: a confirmed close refused for an unsaved file leaves every running terminal session
alive, proven against real, spawned sessions.*

**D2 — A refused close says so, and says why.** The user confirmed an action that did not happen.
The reasons already exist as structured values — `CloseReasonCode::{DirtyFile, PendingApproval,
ReviewReadyChange, RunningProcess}` — so the refusal can name them without inventing new text.
*Measurement 2: the refusal names the reason that blocked it.*

**D3 — The dialog must not offer what it cannot do.** A close the assessment will refuse should not
present a confirm button that silently fails. Either the modal states the blocking reason and offers
no confirmation, or confirming performs a close that is already known to be permitted. **Decide
which on acceptance; do not build both.**

**D4 — The reason set is read, not re-derived.** `assess_close` already computes every blocking
reason. Any new surface reads that result. A second predicate that decides "can this close" would
be a second source of truth about the user's own data.

**D5 — Reproduce before repairing.** The chain above was read in the code and **has not been
reproduced live**. The first slice reproduces it — a real project, a real spawned terminal, a real
unsaved edit, a real confirm — and that reproduction is the evidence the fix is checked against.
A defect this serious that nobody has seen happen might not be the defect we think it is.

## Non-goals

- **Not a forced close.** The assessment stands.
- **Not recovery for terminal sessions.** They are process state, not buffers; nothing here makes a
  killed shell come back.
- **Not a new `REQ-`.** This is a repair to behaviour the close path already claims.

## Slices

- **PR-066-A — reproduce and repair the ordering.** D5 first, then D1. The reproduction stays as
  the regression test.
- **PR-066-B — say it.** D2, D3, D4, with a live capture of a refused close.

## Open question for the owner, to decide on acceptance

**D3's two shapes.** Either the modal refuses up front — it lists the blocking reasons and offers no
confirm — or it keeps the confirm and the refusal becomes a message. **My recommendation: refuse up
front.** A dialog whose button sometimes does nothing teaches a user not to trust it, and the
assessment is already known before the modal is drawn.
