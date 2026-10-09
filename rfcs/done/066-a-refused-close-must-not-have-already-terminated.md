# RFC-066: A Refused Close Must Not Have Already Terminated

Status: **Implemented and closed 2026-10-09; `0.32.0` candidate, not yet published.** See the
*Closed* section. **Accepted by the human owner 2026-10-08.** D1–D5 as written; **D3 decided: refuse up front** — see *Decided on acceptance*. Proposed 2026-10-08. `0.32.0`, M13. Repairs a defect live in `0.30.0`. Found at review
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

## Decided on acceptance (2026-10-08)

**D1, D2, D4 and D5 as written. D3 is decided, and deciding it removes work rather than adding it.**

**D3 — the modal refuses up front.** When the assessment already blocks a close, the modal states
the blocking reasons and offers **no confirm button**. It does not present a confirmation that then
fails, and there is therefore no "refused afterwards" message to design, because there is no
afterwards.

The reasoning is the owner's own standing criterion: **a user must not be able to misunderstand the
interface.** A button that sometimes does nothing teaches a user that confirmations in this product
are unreliable, and that lesson is not confined to this dialog. The assessment is already known
before the modal is drawn, so nothing is gained by asking a question whose answer is already no.

**What this removes:** PR-066-B no longer needs a refusal message, a notice surface, or a decision
about where a post-hoc refusal would appear. It needs the modal to render reasons it already has.

**What it does not change:** D1 still stands on its own. Even with the modal refusing up front, the
ordering defect must be fixed — `terminate_project_live_work` must not run ahead of an assessment
that can still refuse. A modal is a surface; the ordering is the product. **A fix that only changed
the modal would leave every other caller of this path able to destroy live work.**

## Amendment 1 (2026-10-09, `0.32.0` planning): the audit says it closed, too

**Found while planning the release, by re-reading the function after RFC-027 landed in it.** The
defect has a third face, and it is the same one: everything downstream of the refusal behaves as
though the close succeeded.

**D10 — A refused close must not be audited as having closed.** In
`apply_project_close_confirmation`, `record_safe_close_decision(..., SafeCloseDecision::Closed {
… })` runs **unconditionally**, outside the `if closed` branch. Only the
`terminal_session_confirmed_empty` field is corrected, by `&& closed`. So a project that is still
open is recorded in the audit store as **`Closed`**.

`SafeCloseDecision::Cancelled` is not the right value either — that is recorded elsewhere, when the
user dismisses the modal, and means something different. **The decision has two states and needs a
third.** Nothing tests the refused case's record today, which is why it has gone unnoticed.

This matters more than a wrong field: the audit store is this product's record of what happened,
and `local-data-and-privacy.md` documents its honesty carefully. **A false "Closed" is worse than
the silence D2 already fixes**, because silence is merely unhelpful while this is wrong on disk.

**D11 — The ordering fix moves the value the audit depends on.**
`terminal_session_confirmed_empty` is `terminate_project_live_work`'s own return value, read
*before* the decision is recorded. Once D1 moves the termination onto the close-succeeding path,
that value no longer exists where the record is written. **Decide where the record goes as part of
D1, rather than discovering it mid-slice** — it is the reason D1 is not a two-line move.

**D12 — RFC-027's interaction, checked rather than assumed.**
`remove_project_recovery_records_best_effort` already sits correctly inside `if closed`, so D1 moves
the termination *into* that region beside it; nothing about recovery records needs to change.
And D3's refuse-up-front means a project holding a dirty document never reaches a confirmation at
all, so RFC-027 D11's own "close" trigger for recovery records stays unreachable for the dirty case
— as the dev team observed at review 485. **Recorded so that nobody later reads that as a bug and
"fixes" it.**

## Closed (2026-10-09)

Both slices and the Whole-RFC checklist are done. A `0.32.0` **candidate, not yet published** —
see `CHANGELOG.md`'s own status line, which does not claim "released" ahead of the actual publish.

**This RFC began as one defect — a dialog offering an action it then does not perform — and ended
up repairing three separate ways the product behaved as though a refused close had succeeded.**

**PR-066-A (reproduce, then repair the ordering)** reproduced the defect first, against a real
spawned terminal and a real unsaved edit, before any fix existed — watched happen, not read from
the code. D1 fixed: `apply_project_close_confirmation` now runs the read-only assessment before
anything live is touched, so a project blocked by anything other than its own running processes
(a dirty file, a pending approval, a review-ready change) leaves every running terminal exactly as
it was. A stale doc comment claiming the opposite ordering — true for the function's first two
releases — was found and corrected in the same slice, not left for later.

**PR-066-B (refuse up front)** built D3's own decided shape: a modal that opened `can_close: false`
offers no confirm button at all, never a disabled one — `ModalFocusNext`/`ModalFocusPrevious` and
`activate_current_modal` structurally cannot land its focus on `Close`, proven by two independently
ablated guards, not one covering for the other. The title, the reasons' own prefix, the sole
`Dismiss` control and its hint all read as a refusal, not a question with a withheld "yes" — the
reasons-line prefix needed a second pass at review 503 after the capture itself showed it still
read as though the close were going ahead.

**D10's own audit fix uncovered a real, independent production bug, not only a missing Rust-level
case.** `SafeCloseDecision::Blocked`, correctly plumbed and accepted by `valid_safe_close`, still
produced zero audit records: the `safe_close_decision` family's own SQL `CHECK` constraint — not
the Rust validator that mirrors it — had no branch admitting `outcome = 'blocked'` at all, so
SQLite itself silently rejected every such write, the exact shape `append_observation`'s
best-effort design would swallow in production too. Found only because the checklist demanded a
test that queried the store back rather than trusting the validator's own agreement. Fixed with a
real schema migration (`AUDIT_SCHEMA_VERSION` `2 -> 3`, a new `MigrationStep`, following RFC-013
Amendment 1's own established shape exactly — SQLite cannot `ALTER` a `CHECK`), not a workaround;
proven both by a test that migrates a real v2 database holding pre-existing rows and by an
ablation that reproduces the original rejection (`AuditStoreError { reason: Io }`).

**Live-captured against a real blocked close**, isolated `XDG_STATE_HOME` under `/dev/shm`, a real
dirty file, a real `Delete` on the project's own tab: `rfcs/handoffs/066-refused-close/evidence/
pr-066-b/`. The real audit write was confirmed independently with `sqlite3` against the live
store, not only through the unit tests.

**Left open, disclosed rather than silently deferred:**

- **The bug class behind D10's own finding, not only the one instance.** 52 `CHECK` constraints in
  `audit/schema.rs`, and nothing anywhere mechanically asserts that a given family/outcome
  combination actually lands in the store — any future producer whose outcome string the DDL does
  not admit fails identically: passes review, passes the Rust-level validator, writes nothing, says
  nothing. Recorded in `future-work.md` as a pre-1.0 item,
  `enumeration_confirms_only_the_closed_list_reads_full_file_content` named as the shape and
  `record.rs`'s 69 family arms as the enumeration.
- **`Ctrl+Alt+N`'s own doc claim, found but not this RFC's own scope.** While live-capturing this
  slice's evidence, `Ctrl+Alt+N` ("switch to the next open project") did not switch anything —
  `NavigationAction::SwitchActiveProject` has no production caller in the crate at all, only the
  tab strip's own click/keyboard-focus route (`SwitchActiveProjectTabPressed`) reaches
  `switch_to_project_tab`. `docs/src/users/working-with-projects.md` still claims the chord works
  ("press `Ctrl+Alt+N` to cycle to the next with wraparound"). Not fixed here — this RFC's own
  commits do not touch tab-switching — but worth naming so it is not rediscovered as new.
- Two full-workspace gate runs during this RFC's own review cycle failed on already-load-sensitive,
  already-registered tests (`test-process-leak.md` row 787's fourth and fifth occurrences, plus one
  new row for `terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark`); the
  dev team's own scheduled disposition pass over that register rides alongside this release.

See `rfcs/handoffs/066-refused-close/qa-evidence.md` for the full review history (reviews
485, 489, 501–504) and `rfcs/handoffs/066-refused-close/acceptance-qa-checklist.md` for the closed
checklist.
