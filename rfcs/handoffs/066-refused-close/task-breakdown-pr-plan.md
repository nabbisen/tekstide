# RFC-066 task breakdown and PR plan

## PR-066-A — reproduce, then repair the ordering

1. **Reproduce first** (D5): a real project, a real spawned terminal session, a real unsaved edit,
   a real confirmed close. Show the terminal dying and the project staying open. **If it does not
   reproduce, stop and report that** — it outranks the fix.
2. **Repair the ordering** (D1): the assessment runs to completion first; `terminate_project_live_work`
   runs only on the path where the project is actually closing.
3. The reproduction stays as the regression test.

**Proves:** a confirmed close refused for an unsaved file leaves every running terminal alive.

## PR-066-B — the modal refuses up front

- When the assessment already blocks, the modal states the blocking reasons and offers **no confirm
  button** (D3). There is no post-hoc refusal message to design, because there is no afterwards.
- The reasons are read from `assess_close`'s own result (D4), rendered from the existing
  `CloseReasonCode` values rather than new text.
- Live capture of a close that is refused up front, showing the reason named.

**Proves:** the dialog never offers an action the assessment will refuse.

## Not in scope

Forcing a close; recovering terminated sessions; a new `REQ-`.
