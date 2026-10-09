# RFC-066 task breakdown and PR plan

## PR-066-A — reproduce, then repair the ordering

1. **Reproduce first** (D5): a real project, a real spawned terminal session, a real unsaved edit,
   a real confirmed close. Show the terminal dying and the project staying open. **If it does not
   reproduce, stop and report that** — it outranks the fix.
2. **Repair the ordering** (D1): the assessment runs to completion first; `terminate_project_live_work`
   runs only on the path where the project is actually closing.
3. The reproduction stays as the regression test.

**Proves:** a confirmed close refused for an unsaved file leaves every running terminal alive.

**And decide where the audit record goes (D11).** `terminal_session_confirmed_empty` is
`terminate_project_live_work`'s own return value, read before `record_safe_close_decision` writes
it. Moving the termination onto the close-succeeding path moves that value out from under the
record. **This is why D1 is not a two-line move** — settle it here, not mid-slice.

**RFC-027 landed in this function since the RFC was written (D12).**
`remove_project_recovery_records_best_effort` already sits correctly inside `if closed`; the
termination moves *beside* it. Nothing about recovery records changes.

## PR-066-B — the modal refuses up front

- When the assessment already blocks, the modal states the blocking reasons and offers **no confirm
  button** (D3). There is no post-hoc refusal message to design, because there is no afterwards.
- The reasons are read from `assess_close`'s own result (D4), rendered from the existing
  `CloseReasonCode` values rather than new text.
- Live capture of a close that is refused up front, showing the reason named.

- **A refused close is not audited as `Closed` (D10).** `record_safe_close_decision(...,
  SafeCloseDecision::Closed { … })` runs unconditionally today, outside `if closed`, so a project
  still open is recorded as closed. `Cancelled` is not the answer — it already means the user
  dismissed the modal. The decision needs a third state.

**Proves:** the dialog never offers an action the assessment will refuse, **and the audit store
never claims a close that did not happen.**

## Not in scope

Forcing a close; recovering terminated sessions; a new `REQ-`.
