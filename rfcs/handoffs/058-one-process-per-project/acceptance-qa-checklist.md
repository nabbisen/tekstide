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

- [x] Process-visible, under the state directory, project-scoped. **Not in-memory** (§row 4).
      New module `tekstide_core::project_lock`: `<state_root>/project-locks/<project_id>.lock`,
      a real OS `flock` (`std::fs::File::try_lock`, the same primitive RFC-050's transcript
      writer already uses) — proven process-visible by `an_acquired_lock_is_visible_to_a_second_independent_handle`
      (a second, independent `File::open` sees it held, the identical proof technique the
      transcript's own lock test uses) and project-scoped by `different_projects_or_different_state_roots_never_collide`.
- [x] **Released when the holder dies** (§row 3), proven against a real killed process — and
      `a_live_writer_holds_an_exclusive_lock_until_it_is_dropped` read first.
      Read first, and its own "release is asserted within a bound, not at the instant of drop"
      lesson applied twice: `dropping_the_lock_releases_it_for_a_later_acquire` (plain drop,
      polled with the same 5s/10ms bound after hitting the identical fork-pressure flake once
      in dev) and `a_real_sigkilled_holder_releases_the_lock` (a real child process holding the
      lock via `project_process_probe`, confirmed held from an independent handle, **SIGKILL**ed
      and reaped, lock confirmed free again within the same bound).
- [x] **Cannot-decide opens the project** (§row 1), tested deliberately on: a lock naming a dead
      process, an unreadable state directory, and a lock this build does not understand.
      `a_lock_file_naming_a_pid_but_held_by_nobody_does_not_block_opening`,
      `an_unreadable_locks_directory_cannot_decide`, `unparseable_lock_content_does_not_block_opening`
      — all three in `project_lock/tests.rs`. The design makes the first and third trivially
      correct rather than merely tested: the exclusivity decision is `try_lock`'s alone, and the
      lock file's own content (a pid, for naming the holder later) plays no part in it, so stale
      or unparseable content can never make the mechanism refuse. Found and fixed while writing
      the second case: the directory's permissions were being unconditionally reset after
      creation (mirroring `recovery::write_recovery_record`'s own defensive reset), which
      silently repaired the simulated-unreadable directory before the open attempt — exactly
      the "quietly fixed instead of reported" failure row 1 warns against, caught by the test
      itself failing, not by re-reading the code.
- [x] The reproduction from PR-058-A now fails to clobber — the regression test.
      Renamed and rewritten: `two_real_processes_opening_the_same_root_share_an_id_but_the_second_is_blocked_from_writing`.
      Same two overlapping real processes PR-058-A built; the second now finds the project
      locked (`BLOCKED <pid>`) and never calls `write_recovery_record` at all — the surviving
      record is the first process's own text, unclobbered.
- [x] The audit store is untouched and still works with two live processes (§row 5).
      `audit::tests::two_live_writers::two_concurrent_writers_against_the_same_store_both_succeed`:
      two independent `AuditStore` handles, barrier-synchronized onto the same `append` instant,
      both succeed, both records readable back. Not two real OS processes — two independent
      connections, the same substitution the transcript lock test already relies on for `flock`,
      since SQLite's own file locking is enforced against the open descriptor, not a process
      identity. `audit/store.rs` itself untouched.

      **A real, narrower hazard found and deliberately left alone**: the first version of this
      test raced the *first-ever creation* of the database file between the two handles (no
      synchronization before `AuditStore::open` itself, only before `append`) and hit a genuine
      `AuditStoreError { reason: Io }`. **Corrected at review 515**: the cause is a TOCTOU in
      `AuditStore::open_internal`, not a `busy_timeout` gap — both connections read
      `storage_path.database_file().exists()` before either opens a connection, both see `false`,
      both reach `create_current_schema`, whose `CREATE_SCHEMA_V3` has no `IF NOT EXISTS`
      anywhere; SQLite correctly rejects the second `CREATE TABLE`. Fixed the test by establishing
      the schema once before the concurrent pair starts, which is what D1's own claim is actually
      about (writers against an *established* store). Not fixed in `audit/store.rs`: still outside
      this RFC's own scope on the merits review 515 confirmed (the audit store is app-wide, not
      project-scoped, so `acquire_project_lock` could not guard it regardless of the exact cause,
      and a general multi-instance coordination fix is this RFC's own non-goal) — but resting on
      the real cause this time, not the wrong one. Disclosed, not silently absorbed into
      "confirmed"; recorded in `rfcs/future-work.md` beside review 503's finding.

### Required at review 515 — the audit race is misdiagnosed, and D1 is too broad

The mechanism is right and I verified the property that matters most: **only `WouldBlock` blocks.**
`TryLockError::Error(_)` is `CannotDecide`, and `read_holder_pid` is explicitly outside the
exclusivity decision — so stale or unparseable content cannot refuse an open **by construction**,
which is what row 1 asked for. D4 holds against a real `SIGKILL` (5 runs on my machine). The
`DirBuilder::mode()` fix is the right distinction, and catching it because a test *failed by
passing* is the find of the slice. Gate: `758 + 19 + 1128`, 0 fixture entries.

- [x] **The audit-store race is not what you diagnosed, and the real cause changes the disposition.**
  You attributed it to two connections racing "before either has set `busy_timeout`". But
  `busy_timeout` is set immediately after `Connection::open_with_flags` and **before** any schema
  work. It is not the cause, and it could not have been: a busy timeout makes a connection *wait
  for a lock*; it does not make `CREATE TABLE` idempotent.

  The real cause is a **TOCTOU**, three statements earlier:

  ```rust
  let existed = storage_path.database_file().exists();   // both processes: false
  …
  if existed { prepare_existing_store(…) } else { create_current_schema(…) }
  ```

  Both processes test `exists()` **before opening any connection**, both take the `else` branch, and
  both run `create_current_schema` — which has **no `IF NOT EXISTS`** anywhere (checked: zero
  occurrences in `audit/schema.rs`). The second one is correctly rejected by SQLite.

  **Why this matters beyond accuracy:** you concluded it was out of scope because a general
  multi-instance coordination fix is a non-goal. The project-lock half of that is right — the audit
  store is app-wide and `acquire_project_lock` could not guard it. But this is **not** a coordination
  problem; it is a contained TOCTOU in one function, and the remedies are ordinary ones. *"Out of
  scope"* may still be the right answer, but it has to rest on the real cause.

- [x] **Amend D1 and the RFC's own table.** They say the audit store is *"Protected today: Yes,
  already"*, full stop. Your own test showed that is true for an **established** store and false for
  its first creation. **That is D1's premise, and confirming it was D1's whole job** — the
  confirmation found the claim too broad, which is the outcome "confirm it, do not duplicate it" was
  there to produce. Narrow the claim to what SQLite's locking actually delivers.

- [x] **Record it where it belongs: beside review 503's audit finding, not here.** An audit store
  that fails to open means audit writes that silently do not happen — the same family as the 52
  `CHECK` constraints with nothing asserting a family and outcome lands. Both are pre-1.0 items about
  the one store this product promises honesty about. They should be read together.

**Disclosing it at all was right**, and fixing the test by establishing the schema first — rather
than quietly widening the test until it passed — is what let the real cause be found at review
rather than in production.

**Done at review 515's own request.** The real cause (a TOCTOU in `AuditStore::open_internal`:
`database_file().exists()` read before either connection opens, `create_current_schema`'s
`CREATE_SCHEMA_V3` has no `IF NOT EXISTS`) is now stated correctly in three places that previously
repeated the wrong one: this checklist (above), the test's own doc comment
(`audit/tests/two_live_writers.rs`), and the RFC's own "What is actually at risk" table, whose
audit-store row is narrowed from "Yes, already" (full stop) to "Yes, for an established store,"
naming the first-creation race explicitly. A new `rfcs/future-work.md` entry sits directly beside
review 503's finding, cross-referencing it, with the same "not scheduled, raised as pre-1.0" shape.
`audit/store.rs` itself remains untouched — the disposition review 515 confirmed (out of this RFC's
own scope, on the real cause this time) stands.

### PR-058-B closed at review 516

The correction landed in all three places, and the table row is now precise: *"Yes, for an
established store"*, the TOCTOU named, and — the part that matters — **the right out-of-scope
reason** (app-wide, not project-scoped, so `acquire_project_lock` could never have guarded it)
rather than the coordination framing that was doing the work before.

The recorded entry names **the shape of a real fix** (`CREATE TABLE IF NOT EXISTS`, or a lock held
across the `exists()` check) rather than leaving it as "coordination". That is what stops a future
reader scoping it as a large problem it is not.

*"It is the diagnosis that had to change, not the conclusion"* is the right summary, and worth
saying back: **a correct conclusion resting on a wrong reason is still worth fixing, because the
next person inherits the reason.** Gate: `758 + 19 + 1128`, 0 fixture entries.

## PR-058-C — saying it

- [x] A second attempt **names the holder and opens no duplicate** (D8).
      `AppState::add_project_from_path_protected` undoes the add (`remove_active_project_session`)
      before returning `Blocked` -- no duplicate session ever reaches `self.projects`. Wired into
      all three real GUI open call sites (`reopen_recent_project`,
      `attempt_open_project_from_path_field`, `choose_current_browsed_directory`) and the CLI path.
      Proven at the GUI level:
      `a_second_attempt_at_a_locked_project_opens_no_duplicate_and_says_nothing_false` holds a real
      lock, drives the real `Message::ReopenRecentProjectRowPressed`, and asserts
      `state.app_shell.state().projects()` is empty. **Ablated**: reverting the
      `remove_active_project_session` call in the `HeldByAnother` arm fails that test with a
      duplicate session left in place; reverted.
- [x] **No wording claims the window was raised, switched to, or focused** (§row 2). Check the
      rendered string, not the intent behind it.
      `project-board-open-blocked` = "This project is already open in another Tekstide window. It
      has been asked for your attention." The same GUI test asserts the rendered string contains
      none of "raised"/"switched"/"focused"/"activated" (case-insensitive). **Ablated**: changing
      the `.ftl` string to "...and has been switched to" fails that assertion by name; reverted.
- [x] The attention request is sent, and the message is true whether or not the compositor shows
      anything.
      `Message::ProjectAttentionRequested`'s own handler calls `iced::window::request_user_attention`
      best-effort against `state.window_id` (learned from `iced::window::open_events()`); `None` is
      a silent no-op, never an error. The sentence itself only claims this process's own action (the
      knock sent), never the compositor's response -- true regardless of what, if anything, the
      compositor renders.
- [x] The IPC reaching the holder works, and its socket path stays inside the limit this project has
      already hit twice.
      New `project_lock::attention` module: a real `AF_UNIX` listener at
      `<state_root>/project-locks/<project_id>.attention.sock`, with its own `max_socket_path_len`
      check (a second, independent three-line copy of `approval::channel`'s own computation, not a
      shared dependency -- see the module's own doc for why). Proven end to end between two real
      processes:
      `app::tests::a_blocked_second_process_names_the_real_holder_and_its_knock_is_received` -- the
      second process's own status line names the *real* holder pid (parsed back out of the rendered
      line), and the held probe, released afterward, reports `ATTENTION_RECEIVED` on its own
      listener, not merely that the knocker's `connect()` returned `Ok`.
- [x] **Live capture**: two real instances, the second's message on screen, under an isolated
      `XDG_STATE_HOME` with a throwaway project. **Done.** Taken once the desktop was quiet (checked
      via `niri msg -j windows` first: four terminals, nothing else live). Two real processes under
      `/dev/shm/t58cap`; instance A held the project, instance B's CLI-launched attempt was refused
      (confirmed in `stderr`) but its board showed nothing special, because a CLI open resolves
      inside `boot()` before `State` exists — recorded in `qa-evidence.md`'s own "Live capture"
      section as the reason the capture needed a second, GUI-driven step. Focus on instance B's
      window was checked immediately before `wtype -k Return` (driving the real
      `ReopenRecentProjectRowPressed` path) and again immediately before
      `niri msg action screenshot-window`. Screenshot saved from the Wayland clipboard
      (`screenshot-path null` in this desktop's own config means `-d` has nowhere to write) and
      copied into `evidence/pr-058-c/second-window-asked-for-attention.png`, showing the real
      `project-board-open-blocked` sentence on screen. Both processes `SIGTERM`ed and reaped, throwaway
      `/dev/shm` state removed.
- [x] `REQ-PROJ-009` returns to the Project-lifecycle coverage row, naming **what the mechanism is**.
      `rfcs/delivery-plan.md`'s own "Implemented" table, Project lifecycle row: names the real
      `flock` and the real `AF_UNIX` knock, not "a lock exists now."

### Required at review 517

Strong slice. **The round trip is the part I would keep**: the holder reporting
`ATTENTION_RECEIVED` on its own listener proves both ends, where asserting `connect()` returned
`Ok` would have proved only that a socket existed. Both ablations were run on the committed tree.
The enumeration test catching a literal duplicate call site in `main.rs` is the house pattern
working. D9's wording is right and for the right reason: *"has been asked for your attention"*
claims **this** process's own action and nothing about the other end. Gate: `759 + 19 + 1132`,
0 fixture entries.

- [x] **`holder_pid` is justified as "a diagnostic" and nothing diagnoses with it.** It is written
  into `ProjectOpenBlockedNotice` at three call sites and **never read again** — not rendered, not
  logged, not traced (grepped). A field kept for a purpose it does not serve is the shape review 490
  found in a different guise: a value whose stated reason is not the one holding it up. **Give it a
  consumer or remove it** — writing the blocked pid to `stderr` beside the existing crash-detection
  line would be a real diagnostic and costs one line.

  **Done.** New `log_project_open_blocked(holder_pid: Option<u32>)` (`shell.rs`), the same
  non-fatal `eprintln!`-to-`stderr` shape `boot()`'s own crash-detection line already uses for a
  fact nothing in the product renders. Called from all three real GUI open call sites and the CLI
  path (`main.rs`, replacing its own pid-less `eprintln!`), so every place `holder_pid` is captured
  now does something with it.

- [x] **D8 says "names the instance that holds it" and the product does not — and the wording is
  mine to fix.** Your reading is right and I agree with it: a raw pid means nothing to a user, and
  the knock addressing the correct holder is what matters. But that resolution currently lives in
  two doc comments, while the RFC still says something the UI deliberately does not do. **Amend D8**
  to say the holder is *addressed*, not named to the user, and why. A decision a reader can only
  find by reading the implementation is not recorded.

  **Done.** Both of the RFC's own "names the holder" occurrences (D8's own line and the "D8 is
  implemented as" paragraph below D9) now say "addresses," with a new paragraph explaining why: the
  pid means nothing to a user and is never rendered; the real mechanism is the attention socket's
  own path, derived from the project id, never a pid a person would read.

- [x] **The wording guard reads the catalog, not the view.** `state.catalog.get("project-board-open-blocked")`
  proves the *string* carries none of the forbidden words. The view at `shell.rs:9956` inserts that
  string verbatim, so today the two are equivalent — **but nothing holds that**, and a later edit
  composing it with other text would keep the test green while the screen claimed a switch. This is
  review 492's gap again, in the one place D9 exists to protect. Either assert on whatever the view
  composes, or state in the test that it is a string-level guard and the view placement is covered
  only by the capture.

  **Done, the first option.** `content_area`'s own board-lines composition factored into
  `project_board_lines(state) -> Vec<String>` — the exact function the view now calls, not a
  parallel copy of its logic — and the test reads `project_board_lines(&state).first()` instead of
  the catalog directly. **Proven to close the real gap, not merely rephrased**: an `ablate.sh` run
  changing `content_area`'s own insertion line to append `" It has switched to this window."`
  (composition drift the catalog string itself never shows) fails the same assertion by name;
  reverted. The original catalog-only version of this test would have passed that exact ablation.

- [ ] **The live capture: deferral accepted, required before the candidate.** I checked the desktop
  myself rather than ruling on it abstractly — six windows, the focused one not ours, **including
  the owner's own browser windows**. Declining was correct and is what this project's own
  "verify focus before every interaction" discipline is for; disclosing it rather than rushing or
  claiming it is what made the judgement reviewable. **It is still the only thing that proves the
  view places this message**, per the item above, so it moves to the candidate rather than being
  dropped.

### PR-058-C closed at review 518 — one item carried to the candidate

All three fixes verified, and the third **proven rather than rephrased.** I ran my own ablation:
appending `" It has switched to this window."` inside `project_board_lines` — composition drift the
catalog string itself never shows — now fails the guard by name, quoting the composed line back:

```
found "switched" in "This project is already open in another Tekstide window.
It has been asked for your attention. It has switched to this window."
```

The catalog-only version would have passed that silently. **That closes review 492's gap in the one
place D9 exists to protect**, by reading what the view composes rather than what the catalog holds —
and re-running the original D9 ablation on the new path, to confirm the guard still catches what it
used to, is the check that distinguishes a moved assertion from a kept one.

`holder_pid` now reaches `stderr` through `log_project_open_blocked`, consumed from every call site
that previously stored it unread. D8 reads *"addresses"* rather than *"names"*, with the reasoning
in the RFC rather than only in two doc comments. Gate: `759 + 19 + 1132`, 0 fixture entries.

**Carried to the candidate: the live capture.** Deferred on my own ruling at review 517, for a
desktop I checked myself. It is the last unproven thing in this RFC.

## Whole-RFC

- [x] The colour-alone, i18n completeness and internal-identifier scans still pass. These are
      ordinary tests inside `cargo test --workspace` (`i18n::enforcement::*`,
      `the_highlight_and_the_open_file_are_distinguishable_without_colour`,
      `paste_dialog_accept_reject_distinction_is_a_real_textual_marker_not_colour_only`, etc.), not
      a separate invocation — covered by the three full-workspace runs below. The one new catalog
      key (`project-board-open-blocked`) passed `no_catalog_string_names_an_internal_identifier` and
      `every_source_locale_key_resolves_in_every_shipped_locale` (the latter by its own documented
      fallback-to-`en.ftl` design — `pl.ftl` does not have this key and is not required to; a
      missing translation resolves through the source locale rather than failing).
- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check`
      **after staging**, `rfc_docs_invariants`, `cargo test --doc --workspace`, **three consecutive
      full-workspace runs**, 0 fixture entries in a fresh short fixed `TMPDIR`. All clean. Gate:
      `759 + 19 + 1132`, 0 failed, 0 fixture entries, three consecutive runs (fresh `/dev/shm`
      `TMPDIR` each), `git diff --cached --check` silent on the candidate-cut commit.
- [x] Every new intermittent has a dated row in `test-process-leak.md`. One found and not
      previously registered: `project_lock::tests::dropping_the_lock_releases_it_for_a_later_acquire`
      flaked once in dev during PR-058-B (the same fork-pressure duplicate-fd-past-drop class the
      `transcript` row already documents), fixed immediately with the identical poll-with-deadline
      precedent, and recorded retroactively at the candidate under its own dated heading — fixed at
      first disclosure, so retired the same time it was added.
- [ ] The changelog is written **incrementally as each slice closes**, re-read against the finished
      set at the candidate. **Not done incrementally this time** — `CHANGELOG.md` had no `0.34.0`
      entry anywhere in this RFC's three review cycles; it was written in one pass at the candidate,
      from the closed checklist and `qa-evidence.md` rather than from memory of each slice as it
      closed. Disclosed rather than ticked to match the box's own words: the entry itself is
      complete and was re-read against the finished set, but the *incremental* habit — which is
      what catches a detail before it is forgotten, not after — was not followed this slice.
- [x] The book is read against the changelog **in both directions**, and **`locales/en.ftl` is
      grepped for the words whose meaning this RFC changes** — the step that has caught the same
      drift three times. `en.ftl` grepped for `already open`, `duplicate`, `attention`, and the new
      key's own name; the only other `attention` hit (`project-board-attention`) is an unrelated
      pre-existing status-tag label, not a meaning collision. Reading the book against the changelog
      found a real, pre-existing drift unrelated to this RFC's own diff: `local-data-and-privacy.md`
      still said RFC-027 lived in `rfcs/accepted/` "until the release that ships it moves it" —
      RFC-027 shipped in `0.31.0` and has been in `rfcs/done/` since; corrected in the same edit that
      added RFC-058's own line.
- [x] `what-works-today.md` says what happens when a project is already open elsewhere. New
      paragraph in *Projects, files, and editing*, right after the recovery-offer paragraph it
      protects: states the refusal, the board sentence verbatim, that the first window is
      unaffected, and that the attention request is best-effort and may do nothing visible on
      Wayland — the same disclosure D9 itself makes, not a stronger claim.
- [x] **`local-data-and-privacy.md` says a lock file exists, where, and what is in it** — it is new
      state in the state directory, and that page documents everything else there. New section *The
      project lock and attention socket*, placed directly after *Unsaved-buffer recovery data* (the
      state it protects): the lock file's path, that its content is a pid used only for this
      process's own diagnostic and never shown to the user, that it cannot outlive its holder
      (including a crash), the attention socket's path and that it only ever asks, the fail-open
      behaviour when the mechanism cannot decide, and how to remove it by hand.
- [x] The core pin bumps with the version. *(Release-cut item.)* `[workspace.package] version` and
      the `tekstide-core` dependency pin both `0.33.0` → `0.34.0` in the same commit;
      `the_workspace_pins_tekstide_core_to_its_own_version` passed in all three full-workspace runs.
- [x] Commits are pushed once the gate is green. `c446766`, pushed to `origin/main` after the gate
      above ran clean.

Gate: `759 + 19 + 1132`, 0 failures, 0 fixture entries. One box left honestly unticked (the
changelog was not written incrementally this slice) and one real pre-existing drift found and fixed
along the way (RFC-027's stale `rfcs/accepted/` reference). **`0.34.0` is recommended to the
owner.**

### Required at review 519

The candidate is otherwise verified: version, pin and `Cargo.lock` at `0.34.0`; `cargo package
--workspace` verifying both crates with the packaged archive naming `tekstide-core 0.34.0`; audit
matching the register's three rows; no accesskit; gate `759 + 19 + 1132`, 0 fixture entries. The
capture is clean — the real sentence on a real screen, one throwaway project under `/dev/shm`,
nothing of the owner's in frame. **Fixing the handoff-pack frontmatter because it was stale rather
than because a test would catch it**, and fixing `local-data-and-privacy.md`'s RFC-027 drift found
by the both-directions read, are both the habit working on things nothing was watching.

- [x] **A launcher-started user is told nothing on screen.** You found this and called it "not a
  bug — D6 is still satisfied, the same `stderr` shape every other consumer gets." **It is not the
  same shape.** The three GUI call sites set `project_open_blocked_notice` *and* log; the CLI path
  logs only. So `tekstide /path/to/held/project` from a desktop launcher, a file manager, or a
  terminal the user then stops looking at opens a window with **an empty board and no explanation**
  — the app silently did not do the thing it was asked to do, which is the exact class this RFC and
  RFC-066 both exist to end.

  **"No `State` to set it on" is the current structure, not an obstacle**: `State::new` already
  takes `instance_marker`, threaded out of `boot()` for precisely this reason at RFC-027 PR-027-A.
  Carry the blocked outcome the same way.

  This is also the *likeliest* way a user meets this feature at all, and it is the one path the
  capture could not show — which is worth noticing about the capture as much as about the code.

  **Done.** `open_cli_project_path_and_record` now returns
  `Result<Option<shell::ProjectOpenBlockedNotice>, ...>` instead of `Result<(), ...>`; its `Blocked`
  arm builds the notice (the field made `pub(crate)` so `main.rs` can construct it, the same
  visibility `log_project_open_blocked`'s own parameter already has) alongside the existing `stderr`
  lines, never in place of them. `boot()`'s CLI loop carries the **last** refused argument's own
  notice out to a new `cli_project_open_blocked` parameter on `State::new`, threaded exactly the way
  `instance_marker` already is — same single-notice-at-a-time shape the three GUI call sites already
  have, so a later argument's refusal is what the booted window shows if more than one path is given.
  **Proven through the real CLI entry point, not only inspected**:
  `a_cli_launched_second_attempt_at_a_locked_project_sets_the_real_board_notice` calls
  `open_cli_project_path_and_record` twice — two independent `ApplicationShell`s standing in for two
  processes, the same substitution `project_lock::tests` already established for a real `flock` —
  with the second shell's `recent_project_state` restored from the first's before its own CLI open,
  mirroring the real `RecentProjectStore::load_or_recover()` → CLI-loop order `boot()` itself follows
  so the two shells reuse the same project id rather than minting two different ones for the same
  path. Builds a real `State` from the returned notice and asserts `project_board_lines` leads with
  the real sentence, with the same raised/switched/focused/activated wording guard the GUI-level
  sibling test already has. **Ablated** (clean tree, `ablate.sh`): reverted `project_open_blocked_notice:
  cli_project_open_blocked` to `None` — this test failed by name (`a CLI-launched blocked attempt
  must produce at least one board line ... the exact gap this fix closes`), the GUI-level sibling
  test still passed throughout, confirming only the CLI path's own wiring was ever untested; reverted.
  `759 + 19 + 1132` → `760 + 19 + 1132` (one new test), three consecutive clean full-workspace runs,
  0 fixture entries. Commit `43d1e10`, pushed.

**Observation, not required.** The capture puts the notice at the **bottom edge** of the window
while the row the user activated is at the top, and nothing else on screen changes — the project
does not open, so the board looks identical apart from one line ~1200px away. I did not establish
whether that matches where other board notices render, so I am not calling it inconsistent; I am
saying the capture made it visible and it is worth a judgement before release.

**Accepted as disclosed: the changelog was not written incrementally.** Leaving the box unticked
rather than ticking it to match different words is the right call, and disclosing it is most of what
the box is for. The entry itself is complete and was re-read against the finished set. **I am not
asking for rework** — reconstructing an incremental history after the fact would be a worse lie than
the missed habit.

**Answering your question: the publish is mine.** Tag, `cargo publish --workspace`, the
`release-0.34.0.md` record and `post-publish-check.sh` for `0.34.0` and `0.33.0` are the
architect's, on the owner's authorisation, exactly as for `0.30.0` through `0.33.0`. You were right
not to guess.

**On the bottom-edge observation: left for the owner's own judgement, not acted on unprompted.**
Moving where `project_board_lines`' inserted notice renders relative to the activated row is a
layout decision outside what this review asked for, and the capture
(`evidence/pr-058-c/second-window-asked-for-attention.png`) already shows exactly where it lands
today for whoever makes that call. Noted here rather than guessed at or silently fixed.

Required item done at review 519, commit `43d1e10`. `760 + 19 + 1132`, 0 failures, 0 fixture
entries, three consecutive clean full-workspace runs. **`0.34.0` is recommended to the owner.**

### Required at review 520 — the one line that delivers the fix is unguarded

The fix is right and the threading matches `instance_marker` exactly. **Catching that two brand-new
`ApplicationShell`s mint different ids — so the second never contended for the lock at all — is the
find of this round**: that test would have passed while proving nothing, for the very mechanism
RFC-058's own summary names as what makes the clobber possible. Restoring the recent-project state
to mirror `boot()`'s real order is the right fix, not a workaround. Gate: `760 + 19 + 1132`.

- [x] **`main.rs`'s wiring is not covered by any test.** Your ablation reverted
  `project_open_blocked_notice: cli_project_open_blocked` **inside `State::new`**, which the test
  does exercise. I ablated one line further out — `cli_project_open_blocked = blocked;` in `boot()`'s
  own CLI loop — and **every test still passed.**

  The reason is structural: the test calls `crate::open_cli_project_path_and_record` directly and
  never reaches `boot()`. So it proves the function *returns* the notice and that `State::new`
  *stores* one — but the three lines in `main.rs` that carry the first into the second are
  unguarded. **Delete the assignment and a launcher-started user is silently back to an empty board
  with no explanation**, with a green gate.

  That is the entire delivery mechanism of the fix, and it is the only part of it nothing watches.

  **The remedy is your own, one slice old**: at review 518 you factored `project_board_lines` out of
  `content_area` so a test could assert on what the view actually composes. Do the same here —
  lift the CLI loop's "keep the last refusal" step into something callable, and test that.

  **Done.** The loop's own "keep the last refusal" reduction is now `fold_cli_project_open_blocked(current,
  latest) -> latest.or(current)`, a free function with no `boot()`-only dependency, called from the loop
  as the single line `cli_project_open_blocked = fold_cli_project_open_blocked(cli_project_open_blocked,
  blocked);`. Three direct unit tests in `tests.rs`:
  `a_later_refusal_overwrites_an_earlier_one`, `a_clean_open_never_clears_a_refusal_an_earlier_argument_already_recorded`,
  `no_refusal_at_all_stays_none`. **Ablated at the exact call site you ablated** (clean tree,
  `ablate.sh`, `cargo test --workspace`): replacing the loop's assignment with `let _ =
  fold_cli_project_open_blocked(...)` (discarding the result, as if the assignment itself had been
  deleted) left every test green except one unrelated, already-registered flake
  (`approval::tests::channel::bind_recovers_from_a_stale_socket_file`, `test-process-leak.md` row 1,
  standing verdict Accept) — confirming the residual gap is exactly what you named: `boot()`'s own
  one-line call to the now-tested fold function, the same structural limit
  `open_cli_project_path_and_record`'s own doc comment already accepts for the rest of this file (it
  reads real `std::env::args_os()` and cannot be driven from a test). The logic itself — what the fold
  does — is now fully covered; the one line invoking it in `boot()` is the same irreducible untested
  sliver every other piece of this function's own logic already has, not a gap specific to this fix.
  `760 + 19 + 1132` → `763 + 19 + 1132` (three new tests), three consecutive clean full-workspace
  runs, 0 fixture entries. Commit `68eda97`, pushed.

### Required at review 521 — the test suite writes into the owner's real state directory

**The fold itself is accepted and the residual gap is closed.** Three direct unit tests cover the
logic, and I closed the "does `boot()` call it" sliver the way RFC-027's own marker wiring was
closed at review 481 — **by running it**: two real CLI-launched instances under an isolated
`XDG_STATE_HOME`, and instance B's board carries *"This project is already open in another Tekstide
window. It has been asked for your attention."* on a real screen. That is the defect from review 519
confirmed fixed on the one path a test cannot reach. Evidence:
`evidence/pr-058-c/reviewer-cli-launched-second-instance.png`.

- [x] **`cargo test -p tekstide --bin tekstide` writes 8 entries into
  `~/.local/state/tekstide/project-locks/` — the owner's real state.** Measured, not inferred:

  ```
  full gate run:        before=190  after=198   delta=8
  tekstide-core --lib:  delta=0
  tekstide --bin:       delta=8
  ```

  Four projects' worth of `<uuid>.lock` **and `<uuid>.attention.sock`** per run, with fresh ids each
  time, never removed. **190 had accumulated since PR-058-B landed on 10-10.** The GUI open paths
  resolve their state root through `open_real_recovery_state_root()`, which is
  `AppStatePathProvider::linux_default()` with no test override, so any test driving those messages
  binds a real socket in the real directory.

  **Three reasons this blocks the candidate rather than waiting:**
  1. It is **new with this RFC** and would ship with `0.34.0`.
  2. It runs on **every gate** — three times per slice, many times a day — not once per capture.
  3. **The gate's own cleanliness criterion cannot see it.** "0 fixture entries left in a fresh short
     `TMPDIR`" has been passing truthfully this whole time while this accumulated somewhere else.

  The shape of the fix is already in the codebase twice: `AppStatePathProvider::from_state_dir` for
  the recent store, and `project_lock`'s own tests using `TMPDIR` correctly. The GUI paths need the
  same injectable root.

  **The 198 existing files are yours to clear**, not mine — the owner's ruling at review 495 is that
  leftovers belong to the agent that created them. Please remove them with the fix.

  **Done, with one correction found while fixing it.** The GUI open paths (`reopen_recent_project`,
  `attempt_open_project_from_path_field`, `choose_current_browsed_directory`) were never the
  culprit — they already go through `open_real_recovery_state_root()` → `resolve_agent_run_state_dir()`,
  which **does** have a `#[cfg(test)]` thread-local override (a fresh temp dir per test thread, with
  its own "belt and suspenders" panic if it ever resolves to the real directory). The actual leak was
  `open_cli_project_path_and_record` in `main.rs`: it called `AppStatePathProvider::linux_default()`
  **directly**, its own independent resolution written at PR-058-C time (matching `instance_marker`'s
  own independent-resolution shape, which does not write files the same way) — the one production
  call site this crate's existing state-root isolation split had not yet reached. Every CLI-path test
  in `tests.rs`, including review 519's and 520's own new ones, was binding a real socket and writing
  a real lock file through it.

  Fixed by reuse, not a new mechanism: `open_real_recovery_state_root` made `pub(crate)` and called
  from `open_cli_project_path_and_record` in place of its own direct resolution — the same function
  the GUI paths already use, so the CLI path now gets the identical test isolation for free.
  **Measured the fix the same way the gap was measured**: `cargo test -p tekstide --bin tekstide`
  before and after, real directory count unchanged (`0` → `0`, confirmed empty after cleanup, see
  below) across three consecutive full-workspace runs. `cargo fmt --check`, `clippy -D warnings`,
  `rfc_docs_invariants` (19/19), doctests: all clean. Commit `7a1633c`, pushed.

  **The 206 accumulated files removed**, not 198 — more had landed between your measurement and the
  fix. Checked liveness before deleting anything, rather than assuming every file was stale: read
  each `.lock` file's own recorded pid and confirmed with `kill(pid, 0)` that no real process held
  any of them (103 `.lock` + 103 `.attention.sock` pairs, 0 live). No real Tekstide session was
  running on this machine at the time (confirmed via `pgrep`) and none would have been touched if one
  had been, since the check is per-pid, not a blind wipe of the directory.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
```
