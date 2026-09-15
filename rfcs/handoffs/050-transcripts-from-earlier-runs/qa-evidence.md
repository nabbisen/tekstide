---
title: "RFC-050 — QA evidence"
rfc: "RFC-050"
rfc_file: "../../accepted/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Accepted 2026-09-13 — M12"
target_milestone: "M12"
created: "2026-09-13"
---

# Evidence

## PR-050-A — the lock, the id, the origin

`tekstide-core` only. **Nothing is loaded from disk, and nothing new is deletable**: `FoundOnDisk`
is constructed only in tests (grepped), and nothing walks `transcripts/`.

### The writer's lock, and the order it forces

`BoundedTranscriptWriter::create` now opens **without** truncating, takes an exclusive `try_lock`,
and only then truncates. **The order is the substance**: truncating on open would wipe a file
another writer still holds before discovering the lock — RFC-049 §2's deletion under a live writer,
by a different route. **Any** lock failure refuses the writer, not only `WouldBlock`: a filesystem
that cannot lock would otherwise produce writers every other process reads as dead.

**Truncation applies to regular files only** — exactly what `truncate(true)` on open used to affect.
`O_TRUNC` is ignored for a FIFO or a character device; `ftruncate` refuses them. My first version
truncated unconditionally and **turned the full gate red**: three reader tests capture into
`/dev/full` or a FIFO, and all three failed to launch with `OpenFileFailed`. The targeted run could
not have shown it. A test now pins that an unlocked leftover still starts empty
(`a_writer_on_an_unlocked_existing_file_starts_it_empty`), which the moved truncation otherwise had
no test for.

### A locked file degrades the launch; it does not refuse it

Before this slice any writer-creation failure refused the launch. A **lock** failure now starts the
process without capture and emits `TerminalRuntimeEvent::TranscriptWriterLockUnavailable`; every
other creation failure still refuses, unchanged. The session reads the event, **attaches no
`Transcript` record** — a record would aim this session's purge and retention at a file another
handle holds — and sets `AgentRun.transcript_absence = Some(TranscriptAbsence::WriterLockUnavailable)`.

**The reason is on the run, not a lifecycle state** (D4′ item 4): no record exists to hold one, and
neither `DisabledByOptOut` nor `CaptureFailed` is true of this case. `TranscriptAbsence` has one
variant; RFC-049 PR-049-C adds its budget case. **Rendering it in the run's detail is not here** — A
is core only.

### The origin type — proposed here, as the plan asks

```rust
pub enum TranscriptOrigin {
    LaunchedHere { terminal_id: TerminalId, agent_run_id: Option<AgentRunId> },
    FoundOnDisk { writer_held_lock_at_load: bool },
}
```

`Transcript.terminal_id` and `.agent_run_id` **moved into it** rather than staying beside it:
leaving them as fields would force a found record to carry *some* `TerminalId`, the fabrication D2
forbids. `terminal_id()` / `agent_run_id()` accessors replace the reads and use `if let`, so **the
only exhaustive match on the origin is the liveness predicate** — a new origin fails to compile there
and nowhere else.

`FoundOnDisk` carries only what the loader will measure. A lock held at the probe is live for the
session; a free one is not, because a file an earlier process left has no writer here and a new run
never reopens an old file.

### Findings about the RFC's own premises

- **`AgentRunId` already validated.** `impl_id!` gives every id type `from_persisted`, which strips
  the prefix and parses the rest as a UUID. The RFC measured it as missing — likely a search for
  `impl AgentRunId`, which a macro does not contain. The box needed a test, not code.
- **Run directories are `agent-run-<uuid>`, not bare UUIDs**, because the resolver uses the prefixed
  id. Risk document §1's *"both directory names must parse as UUIDs"* is exact for the project
  directory only; `from_persisted` handles the run directory correctly.
- **`from_persisted` does not enforce "only what this product writes".** A throwaway probe (run,
  then deleted) found both `ProjectId` and `AgentRunId` accept **uppercase, hyphen-less, braced and
  `urn:uuid:`** spellings, inherited from `uuid::Uuid::parse_str`. The product writes only lowercase
  hyphenated ids, so PR-050-B's loader would accept — and make deletable — names this product never
  wrote. Tightening it changes what persisted state parses, so it is not taken here. The test is
  named for what it proves (`agent_run_id_rejects_non_uuid_and_unprefixed_names`) and says so.
- **Minimum toolchain.** Neither crate declares `rust-version`; `File::try_lock` is stable from 1.89,
  so building either published crate now needs 1.89 or later.

### The lock made two fixture tests interact — and I first blamed load

**My new lock test's release assertion was racy.** It failed 1 run in 40 alongside the two
PTY-spawning launch tests, and 0 in 40 alone. Inferred mechanism: a concurrent fork copies the
writer's descriptor until exec, and `flock` lives until the last copy closes. The test now asserts
release within 5 s; a writer that never releases still fails it (ablation L11).

**The two `/dev/full` reader tests failed because of this change, and I first recorded it as load.**
`local_bounded_…_genuinely_unwritable` timed out while my own measurement loop ran, passed 5 of 5 in
isolation, and I wrote in the register that the load was mine and the change was innocent. **Both
halves were wrong.** The next ablation run showed both `/dev/full` tests failing inside ablations
that do not touch them, and under L4 (lock failure refuses the launch) the error named the cause:
`LockUnavailable` on a path symlinked to `/dev/full`.

Both tests symlink their transcript to **the same device inode**, and `RealProcessLimiter` admits six
real-process tests at once. Under the lock, the second of two overlapping tests correctly launches
without capture (D3) and then waits for a `CaptureFailed` that cannot come. **Fixed in the fixture**:
both hold a shared mutex for their whole duration. The overlap exists only because the fixture reuses
one device; a real run writes its own freshly named regular file, so the rule is not weakened for it.
**The register entry is corrected in place**, stating that the first attribution was wrong.

The lesson: passing in isolation shows a test is not broken on its own. It does not show the change
is innocent of how concurrent tests interact.

**An alternative for you, not taken:** lock only regular files, by the same reasoning as truncating
only regular files. That would avoid the interaction without the mutex, but it bends D3's *"a writer
never writes unlocked"*, which is a decided rule, so it is yours.

### An interrupted run, and how the tree was recovered

This table took three attempts. The first was stopped mid-ablation when the session ended, and the
machine rebooted, wiping the scratchpad backups. The second was stopped by me on purpose once it
exposed the `/dev/full` interaction above, because its results carried failures from that and not from
the ablations. Ablations restore and hash-check one at a time, so
only the last applied could remain: **L11's appended `Drop` impl was still in `writer.rs`**. The
other markers were checked absent. Removed by exact match against the text L11 appends — the edit
refuses if the tail differs.

**Stopping the second run killed my own restore script**: `pkill -f "050a-run2/run.sh"` matched its
own command line, which contained that string. The files were checked against the recorded baseline
hashes afterwards and already matched, and no ablation markers remained. The table below is the third
run, from a fresh baseline, with the mutex in place.

### Sequencing questions

- **The request-387 disclosure does not exist.** The README places RFC-050 after "the purge-defect
  disclosure commit and PR-DOC-C", and D8 removes the disclosure. Neither commit is in the log, and
  grepping `README.md`, the book and `CHANGELOG.md` finds no such text.
- **D8 and the plan put the removal in different slices.** D8: removed *"in the same commit that
  makes purge true: not before, not after."* Purge becomes true in **B**; the plan removes it in
  **C**.

### Ablations — the third run, from a fresh baseline, each file restored and hash-checked

| Ablation | Tests that fail |
| --- | --- |
| **L1** remove the writer's lock | the lock test's *held* assertion, the refused-writer test, the untouched-bytes test, the launch-degrade test |
| **L2** try the lock but ignore failure | refused-writer, untouched-bytes, launch-degrade |
| **L3** truncate on open | untouched-bytes, launch-degrade — the held file is wiped before the lock is tried |
| **L4** refuse the launch on a lock failure | launch-degrade **alone** |
| **L5** attach a record despite the lock failure | launch-degrade **alone** |
| **L6** treat every found-on-disk file as not live | found-and-locked test **alone** |
| **L7** add a `TranscriptOrigin` variant | **one compile error, `project/session.rs:1029`** — the liveness predicate, nowhere else |
| **L8** `from_persisted` stops parsing the UUID | the ids test (plus register row 1's socket flake, unrelated — see below) |
| **L9** skip the regular-file truncate | truncation test **alone** |
| **L10** truncate every file type | the FIFO test and both `/dev/full` tests — `ftruncate` refuses them, exactly the gate failure this slice first shipped |
| **L11** a writer whose lock outlives it | the lock test's *release* assertion, and the required-mode `/dev/full` test |

**L1 cannot fail "alone", and the checklist box says it should.** Every test that proves something
*about* the lock needs the lock to exist, so removing it fails all four. The lock test's own failure
is its first assertion, which is what the box asks for; the other three are a disclosed composition
rather than an entanglement, because each asserts a different consequence of the same fact.

**L11 hung, and that was a flaw in the ablation, not the product.** It leaks the writer's whole
descriptor on drop, because a lock cannot be leaked without the descriptor that holds it. That also
holds the FIFO test's write end open, so its reader waited more than ten minutes for an end-of-file
that could not come; the test binary was killed by PID. This hang is also the likeliest reason two
earlier background runs lost their wall-clock time before their sessions ended.

**L8's second failure is not L8.** `approval::tests::channel::bind_recovers_from_a_stale_socket_file`
is register row 1, a stale-socket bind that uses no persisted id. Another project's release build,
wasm build and test run were loading the machine throughout. Recorded as a dated recurrence.

### Greps the checklist asks for

```
exhaustive matches on TranscriptOrigin:   session.rs (the predicate) — accessors use `if let`
FoundOnDisk constructed:                  project/tests/retention.rs only
ids minted beside a transcript record:    none outside TerminalSession::new / AgentRun::draft
directory walks under transcripts/:       none
reads of the removed fields:              none
```

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean. `git diff --check`:
clean after removing a trailing blank line my register edit left. **Three consecutive full-workspace
runs, output redirected to files: 507 + 6 + 791, green every time** (+8 core tests). Load rose from
6.3 to 9.2 across them, with another project's builds running.

## PR-050-A follow-up (response 388)

Two rulings from the PR-050-A review, both about the lock. `tekstide-core` only.

### `RequiredLocalBounded` refuses on a lock failure

PR-050-A degraded **every** mode: both launch sites, `launch_project_shell` and
`launch_project_adapter`, matched on the error reason (`LockUnavailable`) and never on the mode. The
guard now also requires `!mode.rejects_launch_when_unavailable()`, so only `LocalBounded` starts
without capture, and `RequiredLocalBounded` refuses as RFC-011 says it must.

`a_required_local_bounded_launch_is_refused_when_its_transcript_file_is_locked` asserts the refusal is
for the transcript writer, **no process starts**, no run or transcript is recorded, and the held bytes
are untouched. It reaches the mode through the builder and says so: no production launch requests it.

### Only regular files are locked, decided by one `fstat`

`create` opens without truncating, reads the handle's type once, and then:

- **a regular file** is locked, and truncated only once the lock is held;
- **anything else** (a FIFO, a device) is neither locked nor truncated;
- **a handle whose type cannot be read is refused**, so a regular file is never written unlocked.

This replaces two separate decisions the first version made, which were a lock for every file type and
a truncate for regular files only. The `/dev/full` test mutex is **removed**, and the register entry
that called it the fix now says it hid the problem rather than solving it.

`two_writers_on_one_fifo_both_create` and `a_second_writer_on_a_regular_file_is_refused` pin both
sides: narrowing the lock must not loosen it for regular files.

### Ablations, each restored and hash-checked

| | Ablation | Fails |
| --- | --- | --- |
| F1 | degrade regardless of mode | the `RequiredLocalBounded` refusal test **alone** |
| F2 | lock every file type again | the FIFO test, **and** the required-mode `/dev/full` test |

**F2 cannot fail "alone", and the second failure is the evidence for the ruling.** Locking a device
makes the two `/dev/full` fixture tests contend for one inode's lock, which is the exact interference
the narrowing removes. The checklist asks for the FIFO test alone; the property cannot give that.

### The interference is gone without the mutex

The two `/dev/full` reader tests were run **together, twenty times, with no mutex: 20 passed, 0
failed.**

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean. **Three consecutive
full-workspace runs, output redirected to files: 507 + 6 + 794, green every time** (+3 core tests).
`git diff --cached --check` after staging, per response 388: clean.

## PR-050-A second follow-up (response 390)

**Response 390 found my required-mode test held only one of the two launch sites.** It launches
`/bin/sh` without an adapter, so it reaches `launch_project_shell` only. Removing the mode check from
`launch_project_adapter` left all 794 tests green. The code there was correct, but nothing proved it.

### One helper, not a second test

The block that creates the writer and decides between degrading and refusing was duplicated in both
functions, and had already needed the same fix twice. It is now **`prepare_transcript_writer`**,
called by both sites. Nothing else moved: the rest of each launch function stays duplicated, for
RFC-022's reason. The existing required-mode test therefore holds both sites, and no new test was
added.

```
BoundedTranscriptWriter::create in launch_project_shell:    none
BoundedTranscriptWriter::create in launch_project_adapter:  none
prepare_transcript_writer called at:                        both sites
BoundedTranscriptWriter::create called at:                  inside prepare_transcript_writer only
```

### Ablation, restored and hash-checked

| | Ablation | Fails |
| --- | --- | --- |
| H1 | degrade every mode inside the helper | `a_required_local_bounded_launch_is_refused_when_its_transcript_file_is_locked` **alone** |

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean. **Three consecutive
full-workspace runs, output redirected to files: 507 + 6 + 794, green every time** — no new tests, as
the ruling asked. `git diff --cached --check` after staging: clean.

## PR-050-B — first commit: two carried fixes

Two items assigned to this slice's first commit by other reviews. Neither touches the loader.

### The wake-notifier test's first-wake race (response 391)

`runtime::terminal::reader::tests::the_wake_notifier_wakes_when_real_pty_output_arrives` wrote
`printf …; exit` to the shell **before** it started the thread that waits for the first wake, and it
failed once in the reviewer's gate.

**The proposed fix was to start the waiting thread first. That narrows the window but does not close
it**, because of how `block_until_woken` reports:

```rust
pub fn block_until_woken(&self) -> bool {
    if !block_on_eventfd(self.file.as_raw_fd()) { return false; }
    let mut buffer = [0_u8; 8];
    let _ = (&self.file).read(&mut buffer);
    self.reader_alive.load(Ordering::Acquire)
}
```

It answers whether the reader is alive **when it reads**, not when it was woken. A waiter that is
already blocked can still be woken by the output, lose the scheduler, and read `reader_alive` after
the shell has run `exit`. **The fix used instead holds `exit` back**: start the waiter, write the
`printf` alone, observe the first wake, then write `exit`. The shell is still waiting for input when
the first wake is read, so the reader cannot have stopped. The test's doc comment says why.

**Measured, not inferred.** A throwaway probe put a 500 ms sleep in the waiting thread before its
read, deliberately widening the window. It was run, then removed, and the file's hash was checked
afterwards.

| Order | Runs with the 500 ms delay | Failed, with *"…not that the reader has already stopped"* |
| --- | --- | --- |
| **old**: `printf; exit` written before the waiter starts | 10 | **9** |
| **new**: waiter, `printf`, first wake, then `exit` | 10 | **0** |

So the reviewer's reading of the mechanism is right, and the new order holds under the same stress
that breaks the old one nine times in ten. The one old-order pass shows that even a 500 ms head start
does not always let the reader stop first. That is further reason the fix is ordering rather than
timing.

### PR-DOC-C's link-test follow-up (response 392)

Recorded where that slice's evidence lives: `rfcs/handoffs/documentation-readme-and-book-qa-evidence.md`,
*PR-DOC-C follow-up*. In short, `the_readme_has_no_relative_links` now checks markdown, HTML
`src`/`href` and reference-style definitions, each ablated alone. Its messages say crates.io rewrites
relative links against `crates/tekstide/`, and `CONTRIBUTING.md`'s gate example writes one log per
run.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 9 passed. **Three consecutive full-workspace runs, output redirected to files:
507 + 9 + 794, green every time.** `git diff --cached --check` after staging: clean.

## PR-050-B, part 1 — the loader, in the core, with no caller

**Nothing new is deletable in the product yet.** No code outside tests calls the loader. PR-050-B's
second commit wires it into the GUI's project-open paths, switches the figures, and removes the
request-387 disclosure: that is the commit that makes purge true, as D8 requires. Splitting it keeps
that commit's meaning exact.

### What was added

- **`transcript::loading`**, which reads and never deletes:
  - `scan_project_transcripts(state_root, project_id)` enumerates `<state>/transcripts/<project_id>/`;
  - `scan_transcript_disk_usage(state_root, claimed_project_ids)` returns all bytes under
    `transcripts/`, and those no claimed project owns;
  - `is_product_run_directory_name(name)` is the run-directory spelling rule.
- **`ProjectSession::load_transcripts_from_disk(state_root)`** adds a `FoundOnDisk` record per
  accepted file, skipping any path the session already has a record for.
- **`purge_transcript_at` skips a found file whose lock was held at load** and reports it in the new
  `ProjectTranscriptPurgeSummary::skipped_still_being_written`. This is the single deletion path, so
  retention inherits it.
- **`purgeable_transcript_count` / `purgeable_transcript_bytes`** count what purge would delete:
  neither tombstones nor still-written found files. `transcripts_still_being_written_count` is for the
  dialog's notice (PR-050-C).
- `Transcript::found_on_disk` and `DomainTimestamp::from_unix_seconds`.

### The rules, from the risk document's §1, as the code enforces them

- **No symlink is followed at any level**: `real_directory` uses `symlink_metadata`, and the byte
  count never follows one either.
- Only a **regular file named `transcript.log`, two levels under `transcripts/`**, is loaded.
  Anything else is skipped, counted, and never deleted.
- **Run directories must be `agent-run-` plus a lowercase hyphenated UUID, checked as a round trip.**
  `from_persisted` is not used and not changed (response 388).
- **The lock is probed once, then released**: the probe drops its handle before returning. A file that
  cannot be opened or locked for any reason reads as **held**, because unknown liveness is live.
- **Age comes from the mtime, read at load.** `created_at` is the epoch, no later than the true
  creation time. If the filesystem gives no mtime, `created_at` names no instant, so the transcript
  has no computable age and is never expired — rather than reading as fifty-six years old.
- The state root is **canonicalised once**, as the launch's resolver does, so a found path compares
  equal to the path a launch in this session recorded, and nothing is loaded twice.

### Tests

| Test | Proves |
| --- | --- |
| `a_transcript_from_an_earlier_run_is_counted_and_purged_after_a_real_restart` | **the acceptance criterion, in the core**: a real launch writes a real transcript; `AppState` is dropped and a fresh one restored from its saved recent-project state; the reopened project knows nothing, then loads it, counts it, and purge deletes **the file on disk** |
| `a_found_transcript_whose_lock_is_held_loads_as_live_and_survives_purge` | a held lock loads as live, survives purge, is reported, and is not counted as purgeable |
| `the_load_probe_releases_the_lock_it_took` | a later `try_lock` succeeds |
| `a_found_transcript_takes_its_age_from_its_mtime` | `last_write_at` is the mtime and `created_at` is the epoch |
| `every_unrecognised_entry_is_skipped_and_still_present_after_purge` | a symlinked run directory pointing outside the state root, a non-UUID name, a stray file, and a `transcript.log` a level too deep all survive purge |
| `run_directories_in_other_uuid_spellings_are_skipped_and_still_present` | uppercase, hyphen-less, braced and `urn:uuid:` survive purge; the product's own spelling is loaded and purged |
| `an_unclaimed_project_directory_is_not_loaded_or_deleted_and_is_counted` | D6 |
| `the_app_wide_figure_includes_a_closed_project` | D5; a stray at the root counts as unclaimed |
| `a_transcript_this_session_already_has_is_not_loaded_twice` | a launched record is not duplicated |

### Ablations, each restored and hash-checked

| | Ablation | Fails |
| --- | --- | --- |
| K1 | load nothing | **seven tests**: every one that needs a loaded record (disclosed composition). The skipped-entries and disk-usage tests pass, because neither depends on loading |
| K2 | the probe never reports a held lock | the held-lock test **alone** |
| K3 | the probe leaks its handle | the probe-releases test **alone** |
| K4 | accept every spelling `from_persisted` accepts | the spellings test **alone** |
| K5 | follow symlinks when classifying directories | the unrecognised-entries test **alone** — the file outside the state root is deleted |
| K6 | purge ignores a held lock | the held-lock test **alone** |
| K7 | the purgeable count includes still-written files | the held-lock test **alone** |
| K8 | no dedupe against known records | the already-known test **alone** |
| K9 | disk usage treats claimed directories as unclaimed | the closed-project test **alone** |

**K1 cannot fail "alone" at this layer.** The checklist's *"skip loading; the test fails alone"* is
about the product's open path. PR-050-B's second commit wires loading into the GUI, where an ablation
that removes that call can fail the GUI-level restart test alone.

### Greps

```
remove_file reaching transcripts/:                session.rs remove_transcript_file only (the single deletion path)
fs reads in retention.rs:                         none
fs reads in retention candidate selection:        none
new tests naming the real state root:             none (linux_default, .local/state, XDG_STATE_HOME)
```

### The dialog-count reading (risk document §4), verified

`transcript_local_data_summary_for` passes `project.transcripts().len()`, and a purge keeps a
tombstone record, so **the dialog's count includes tombstones**: after a purge, reopening the dialog
counts the purged transcripts again. The reading holds. The core now has `purgeable_transcript_count`,
and the dialog switches to it in PR-050-B's second commit.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 9 passed. **Three consecutive full-workspace runs, output redirected to files:
507 + 9 + 803, green every time** (+9 core tests). `git diff --cached --check` after staging: clean.

## PR-050-B, part 2 — the product loads, and purge is true

**This is the commit that makes purge true, so it also removes the request-387 disclosure (D8).**

### Wiring

- **Every project-open path loads.** The three GUI paths — reopening a remembered project, the path
  field, and the folder browser — call `load_earlier_transcripts_for_opened_project` in their `Added`
  arm, beside `verify_restored_trust` and `apply_configured_resource_limits`. The command-line open in
  `main.rs`, which runs before `State` exists, calls `shell::load_earlier_transcripts`. Grepped: all
  four call it.
- **The state root comes from `resolve_agent_run_state_dir`**, the split the launch already uses. A
  test build gets its own temporary directory and a guard that panics on the real one (§5). The root is
  resolved, not created, so opening a project before any run has ever happened creates nothing.
- **The app-wide figure is cached, not scanned per frame.** `trust_settings_view` renders every frame.
  `State.transcript_disk_usage` is filled by `scan_transcript_disk_usage` at boot, after each load, and
  after each purge, with the open and recent project ids as the claimed set.

### Figures

- **Trust Settings' retained count excludes tombstones.** Its app-wide bytes are the cached whole-
  directory total, closed projects included (D5).
- **The purge dialog captures `purgeable_transcript_count` and `purgeable_transcript_bytes`**, so it
  counts only what purge will delete: no tombstones, and no found file still being written.

### The disclosure leaves, and the defect entry arrives, in this commit

- **`README.md`** and **the book's privacy page**: the known-defect paragraphs are gone. They now say
  purge covers transcripts from earlier runs, name the two kinds of file it leaves in place — one still
  being written when the project opened, and one whose project is no longer in the recent list — and
  that deleting `transcripts/` removes everything.
- **`CHANGELOG.md` Unreleased**: the known-defect entry is replaced by the defect entry D8 asks for.
  It states `0.12.0` through `0.18.0`; what a user saw (a count of zero after a restart, and a purge
  that left earlier transcripts); that deleting `transcripts/` was the only complete removal; that
  earlier purges may have left files behind; and what purge still leaves in place.

### The acceptance test, through the product's open path

`reopening_a_project_loads_an_earlier_runs_transcript_and_purge_deletes_it` puts an earlier session's
transcript at the product's path under the test's own state root, plus a transcript for a project that
is never opened. It then:

1. reopens the remembered project with a real `Enter`, and checks that the project counts the
   transcript;
2. opens the purge dialog through the real key, checks it counts one, and clicks **Purge**;
3. checks the file on disk is gone;
4. reopens the dialog and checks it counts **zero**, because the tombstone is not a transcript;
5. checks that Trust Settings also counts zero, and that the app-wide figure equals exactly the other
   project's bytes, refreshed after the purge;
6. checks that the other project's transcript is untouched.

### Ablations, each restored and hash-checked

| | Ablation | Fails |
| --- | --- | --- |
| M1 | remove loading from the reopen path only | the acceptance test, at *"the reopened project knows the transcript its earlier run left"* |
| M2 | the dialog counts every record again | the acceptance test, at *"the tombstone … is not counted"* |
| M3 | Trust Settings counts tombstones again | the acceptance test, at *"Trust Settings does not count…"* |
| M4 | the app-wide figure from open sessions only | the acceptance test, at *"the app-wide figure covers a project that is not open"* |

**Each fails only this test, but they all fail the same test**, so it is a composition, and its doc
comment says so and names which assertion each ablation reaches (response 367's lesson). Each
property is also held on its own by a core test from part 1. **M1 is the checklist's *"skip loading;
the test fails alone"***, which K1 could not satisfy at the core layer.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 9 passed. `mdbook build docs`: clean. **Three consecutive full-workspace runs,
output redirected to files: 508 + 9 + 803, green every time** (+1 shell test).
`git diff --cached --check` after staging: clean.
