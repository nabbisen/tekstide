# RFC-058 acceptance and QA checklist

Tick a box only when the thing it names has been **run**.

## PR-058-A — reproduce, and nothing else

- [x] Two **real** processes, one real project root, the same file edited differently in each.
      A new test-only `[[bin]]`, `project_process_probe` (`tekstide-core/src/bin/`, the same
      shape as `reference_adapter`), runs the exact production sequence a real boot already
      runs — `RecentProjectStore::load_or_recover`, `ApplicationShell::add_project_from_path`,
      `RecentProjectStore::save`, `tekstide_core::recovery::write_recovery_record` — never a
      reimplementation of any of them. Spawned twice as real OS processes
      (`app::tests::two_real_processes_opening_the_same_root_get_the_same_project_id_and_clobber_the_record`),
      sharing one real state root and one real project root, writing `"instance A's edit"` then
      `"instance B's edit"` for the same `doc.txt`.
- [x] The two instances are shown to hold the **same project id**, and therefore the same record
      path — the mechanism, not just the symptom. Asserted directly (`assert_eq!(first_id,
      second_id, ...)`), and confirmed the mechanism is the cause, not a coincidence, by running
      the identical probe against two *different* project roots by hand first and observing two
      *different* ids (`9d779765-...` vs. `76ad7a16-...`) — the harness can tell the two cases
      apart, so "same id" below is not a tautology.
- [x] **The clobber is observed**: one buffer in the record, the other gone, nothing said.
      `records_dir(&state_root, &first_id)` has exactly one file; its content is `"instance B's
      edit"`; `"instance A's edit"` is not present anywhere in it, and nothing on either process's
      stdout/stderr or in the record itself says a first edit ever existed.
- [ ] **If it did not reproduce, that is reported instead of a fix** (§row 6). *(Not applicable —
      it reproduced on the first attempt. Left unticked rather than ticked for an event that did
      not happen: this box names what to do if reproduction fails, and nothing was done because
      nothing failed.)*
- [x] No product change in this slice. `git status` for this slice: one new test file
      (`crates/tekstide-core/src/app/tests.rs`, additions only) and one new test-only binary
      (`crates/tekstide-core/src/bin/project_process_probe.rs`). No file under `src/` outside
      `tests.rs`/`bin/` touched.

### Required at review 513 — the two processes never overlap

**The root mechanism is proven and that is the valuable half**: two real processes opening the same
canonical root get the **same project id**, therefore the same `records_dir`/`record_file_name`, and
one record survives. The probe calls production functions rather than reimplementing them
(`load_or_recover`, `add_project_from_path`, `save`, `write_recovery_record`) — I checked. So is the
negative control: different roots give different ids, so the assertion is not a tautology. No
product code touched. Gate reproduces `758 + 19 + 1119`, 0 fixture entries.

- [x] **Make the two probes concurrent.** `run_project_process_probe` uses `Command::…output()`,
  which **waits for the process to exit**. The first probe is gone before the second starts, so what
  was reproduced is two *sequential* sessions, not two instances holding one project. That matters
  twice:

  1. **It is not the hazard.** Sequentially, the product itself mediates: the second instance's own
     recovery offer fires on open, because records exist, and the user is shown the first session's
     unsaved text. The probe never reads existing records (checked: zero references to
     `read_project_recovery_records`), so the clobber it demonstrates is one the real application
     would have surfaced rather than silently taken.
  2. **It cannot be PR-058-B's regression test.** D4 requires the lock to be released when its
     holder dies. The first probe has exited, so a **correct** implementation releases the lock, lets
     the second open, and the record is overwritten again — the test still sees a clobber and
     PR-058-B looks like it failed. A regression test a correct fix cannot pass is worse than none.

  The first probe must still hold the project when the second starts — spawned rather than waited
  on, held open until the test releases it.

  **Done.** `project_process_probe` takes a new `--hold` flag: after its own write lands, it prints
  its project id, flushes, then blocks reading stdin until EOF. A new `HeldProbe` (`app/tests.rs`)
  spawns it with piped stdin/stdout, reads that id line back (which cannot happen before the write
  does, since the print follows it in `main`), and only then does the test start the second,
  ordinary probe — with an explicit `assert_still_holding()` on either side of that call, via the
  real `libc::kill(.., 0)` liveness check (`test_support::process_is_alive`), not an assumption that
  spawning without waiting implies staying alive. `release()` closes the held probe's stdin and
  waits for a clean exit.

  **A second, unrelated gate failure found while fixing this**: the first version blocked with
  `std::io::stdin().read_to_end(&mut discard)`, which tripped
  `project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content` — a scan
  for raw full-file-content reads outside a reviewed allowlist, matching on the literal text
  `read_to_end(` with no way to tell a stdin drain from a file read. Draining stdin is not what that
  scan is asking about, so the fix was not to add this file to the allowlist (which would misstate
  what it does) but to stop writing the pattern the scan matches on: a `read_line` loop until `Ok(0)`,
  discarded each time, which blocks on EOF exactly the same way without going anywhere near the
  question that check exists to answer.

**This is partly my fault and I should have caught it when I wrote the pack.** The task breakdown
says *"the reproduction becomes the regression test, and it is what PR-058-B is checked against"*
without ever saying the two processes must overlap. D2 asked for a reproduction; it did not say
concurrent, and the word was doing all the work.

### PR-058-A closed at review 514

The overlap is proven, not assumed, and **better than I asked for**: I said the first probe must
still hold the project when the second starts; `assert_still_holding()` runs on **both** sides of
the second probe, so the first is shown alive across the whole window — each a real
`process_is_alive` → `libc::kill(pid, 0)`, not an inference from "spawned without waiting". Reading
the id line back before starting the second is an ordering guarantee rather than a sleep. Five
consecutive runs clean on my machine, ten on theirs. Gate: `758 + 19 + 1119`, 0 fixture entries.

**The allowlist judgement is the part worth keeping.** A first attempt at the blocking read tripped
`enumeration_confirms_only_the_closed_list_reads_full_file_content`, and the easy fix was to add
this file to `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT`. They refused: the file reads neither a
project file nor a record, so the entry would have *misstated what the code does* in order to
satisfy a scan whose question never applied to it. Changing the drain to a `read_line` loop — with a
comment saying why, so nobody "simplifies" it back — answers it honestly instead. **An allowlist
that accumulates entries which do not belong stops being a statement about anything**, and that is
the one this project relies on to know where full-content reads live.

The `bind_recovers_from_a_stale_socket_file` occurrence was correctly not re-registered — **Accept**
is its standing verdict — and noting it only so it is not mistaken for something this slice
introduced is the right amount.

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
