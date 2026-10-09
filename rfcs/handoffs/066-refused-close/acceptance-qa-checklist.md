# RFC-066 acceptance and QA checklist

Tick a box only when the thing it names has been **run**.

## PR-066-A — reproduce, then repair

- [ ] **Reproduced first, against real things** (D5): a real project, a real spawned terminal, a
      real unsaved edit, a real confirmed close — the terminal dies, the project stays open.
      **If it did not reproduce, that is reported instead of a fix.**
- [ ] The assessment completes before anything live is terminated (D1, §row 1).
- [ ] A confirmed close refused for an unsaved file **leaves every running terminal alive**, proven
      against real spawned sessions, not a synthetic list.
- [ ] A close that *is* permitted still terminates and still closes — the repair must not cost the
      working path.
- [ ] The reproduction is kept as the regression test.
- [ ] **Where the audit record is written is decided, not inherited** (D11):
      `terminal_session_confirmed_empty` comes from the termination, which D1 moves.
- [ ] RFC-027's `remove_project_recovery_records_best_effort` still runs only on the closing path,
      and the termination now sits beside it (D12).

## PR-066-B — refuse up front

- [ ] A close the assessment blocks shows the reasons and **offers no confirm button** (D3).
- [ ] The reasons are read from `assess_close`'s own result, not re-derived (D4, §row 4).
- [ ] **No forced close exists anywhere in the change** (§row 3).
- [ ] Live capture of a refused close, with the blocking reason named on screen.
- [ ] **A refused close is not recorded as `Closed` in the audit store** (D10), and not as
      `Cancelled` either — that already means the user dismissed the modal. **Proved by a test;
      nothing pins the refused case's record today**, which is why it went unnoticed.

## Whole-RFC

- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs with `--no-fail-fast`**, **0 fixture entries left** in a fresh short fixed
      `TMPDIR` — a literal, not `mktemp`.
- [ ] Every new intermittent has a dated row in `test-process-leak.md`.
- [ ] The changelog is written **incrementally as each slice closes**, and re-read against the
      finished set at the candidate — not against the last slice's diff.
- [ ] The book is read against the changelog **in both directions**, including any user-visible word
      this RFC's commits touch.
- [ ] `what-works-today.md` says what a blocked close now does, since the behaviour a user meets
      changes.
- [ ] The core pin bumps with the version. *(Release-cut item.)*
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
