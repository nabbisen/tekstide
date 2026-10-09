# RFC-058 acceptance and QA checklist

Tick a box only when the thing it names has been **run**.

## PR-058-A — reproduce, and nothing else

- [ ] Two **real** processes, one real project root, the same file edited differently in each.
- [ ] The two instances are shown to hold the **same project id**, and therefore the same record
      path — the mechanism, not just the symptom.
- [ ] **The clobber is observed**: one buffer in the record, the other gone, nothing said.
- [ ] **If it did not reproduce, that is reported instead of a fix** (§row 6).
- [ ] No product change in this slice.

## PR-058-B — the mechanism

- [ ] Process-visible, under the state directory, project-scoped. **Not in-memory** (§row 4).
- [ ] **Released when the holder dies** (§row 3), proven against a real killed process — and
      `a_live_writer_holds_an_exclusive_lock_until_it_is_dropped` read first.
- [ ] **Cannot-decide opens the project** (§row 1), tested deliberately on: a lock naming a dead
      process, an unreadable state directory, and a lock this build does not understand.
- [ ] The reproduction from PR-058-A now fails to clobber — the regression test.
- [ ] The audit store is untouched and still works with two live processes (§row 5).

## PR-058-C — saying it

- [ ] A second attempt **names the holder and opens no duplicate** (D8).
- [ ] **No wording claims the window was raised, switched to, or focused** (§row 2). Check the
      rendered string, not the intent behind it.
- [ ] The attention request is sent, and the message is true whether or not the compositor shows
      anything.
- [ ] The IPC reaching the holder works, and its socket path stays inside the limit this project has
      already hit twice.
- [ ] **Live capture**: two real instances, the second's message on screen, under an isolated
      `XDG_STATE_HOME` with a throwaway project.
- [ ] `REQ-PROJ-009` returns to the Project-lifecycle coverage row, naming **what the mechanism is**.

## Whole-RFC

- [ ] The colour-alone, i18n completeness and internal-identifier scans still pass.
- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs**, 0 fixture entries in a fresh short fixed `TMPDIR`.
- [ ] Every new intermittent has a dated row in `test-process-leak.md`.
- [ ] The changelog is written **incrementally as each slice closes**, re-read against the finished
      set at the candidate.
- [ ] The book is read against the changelog **in both directions**, and **`locales/en.ftl` is
      grepped for the words whose meaning this RFC changes** — the step that has caught the same
      drift three times.
- [ ] `what-works-today.md` says what happens when a project is already open elsewhere.
- [ ] **`local-data-and-privacy.md` says a lock file exists, where, and what is in it** — it is new
      state in the state directory, and that page documents everything else there.
- [ ] The core pin bumps with the version. *(Release-cut item.)*
- [ ] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
