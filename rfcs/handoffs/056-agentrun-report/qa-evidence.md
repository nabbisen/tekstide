# RFC-056 — QA evidence

Written as each slice lands. Numbers are from this machine, 2026-09-25.

## PR-056-A — the pin, and two gate steps

### The defect, reproduced (against the registry, not a description)

`cargo install tekstide --version 0.25.0 --root <temporary> --target-dir <temporary>` — a full build:

```
  Installing tekstide v0.25.0
   Compiling tekstide-core v0.26.0
   Compiling tekstide v0.25.0
error[E0004]: non-exhaustive patterns: `FallbackReason::NotABoolean` not covered
error[E0004]: non-exhaustive patterns: `Some(FileGitStatus::Ignored)` not covered
error[E0004]: non-exhaustive patterns: `ExplorerTreeRowKind::IgnoredHidden { .. }` not covered
error: could not compile `tekstide` (bin "tekstide") due to 3 previous errors
```

**`0.25.0` resolved core `0.26.0`.** The same command with `--locked`:

```
  Installing tekstide v0.25.0
   Compiling tekstide-core v0.25.0
   Compiling tekstide v0.25.0
  Installed package `tekstide v0.25.0` (executable `tekstide`)
```

### A correction to the finding this slice was written from

The post-publish finding said the app archives' lockfiles name the **previous** core, so `--locked` does not rescue an old release. **Measured
against the registry, that is not so**: read with `curl` from `static.crates.io` and `tar`, the published `tekstide 0.24.0`, `0.25.0` and `0.26.0`
lockfiles name core `0.24.0`, `0.25.0` and `0.26.0` — the matching one — and `--locked` builds `0.25.0` (above). What named the previous core was the
**local** `target/package` archive, packaged before its core exists on the registry — the very difference `release-checklist.md` already explains
("`Cargo.lock` differs in the `tekstide` package, and the published one is the correct one"). So the defect is cause A alone (`version = "0"`), and
**`--locked` is a working remedy for an older release**; the `0.27.0` changelog says so, and `delivery-plan.md`'s record carries the correction.

### The pin

`[workspace.dependencies] tekstide-core = { version = "0.26.0", path = … }`, with a comment saying why and what fails if it drifts.
**Not `"0.27.0"` as the plan writes it, and that is deliberate:** a path dependency's version must be satisfied by the crate it points at, and
`tekstide-core` is `0.26.0` until the `0.27.0` candidate bumps it — `"0.27.0"` would not resolve. The property the plan wants (Cargo's `0.x` rule protects
every future release without anyone remembering) is held by a test instead:

`the_workspace_pins_tekstide_core_to_its_own_version` (`rfc_docs_invariants`) fails if the pin is `"0"` **or** differs from `[workspace.package] version`,
so the `0.27.0` candidate's version bump is a red test until the pin moves with it. The packaged manifest is checked directly:
`tar xzOf target/package/tekstide-0.26.0.crate tekstide-0.26.0/Cargo.toml` now reads `[dependencies.tekstide-core] version = "0.26.0"` (and the same for the
dev-dependency). `Cargo.lock` is unchanged.

### The post-publish check

`rfcs/handoffs/post-publish-check.sh <version> [--no-install]` — three checks against the registry: the app's `tekstide-core` requirement is the
release's own version (not `"0"`); the app archive's `Cargo.lock` names the matching core; and `cargo install tekstide --version <v>` into a temporary root
builds. `release-checklist.md` gains it as a post-publish step **for the release just published and the previous one**, with the reason (only an old app
shows a new core's break), and a scope line for the pin. Run against the published releases, structural checks only:

| Release | Requirement | Lockfile core | Result |
| --- | --- | --- | --- |
| `0.26.0` | `"0"` — **fails** | `0.26.0` — ok | FAIL, as it must: frozen metadata |
| `0.25.0` | `"0"` — **fails** | `0.25.0` — ok | FAIL, as it must |

The install step reproduces the `0.25.0` failure above.

## PR-056-B — the record

### What was built, and the four decisions the plan left to me

| | |
| --- | --- |
| `transcript/run_record.rs` | the file layer: a versioned `RunRecord` (`version: 1`), an atomic write (temporary file in the run directory, `create_new`, `sync_all`, `rename`, directory sync), and a read that checks the **version before the shape**, so a newer Tekstide's record is recognised as newer and not as corruption |
| `project/session/run_records.rs` | the session side: persistence, the two setters, restoration. A child module of `session`, so it reaches the collections without widening any of them |
| `domain/agent.rs` | `AgentRun` gains `ending`, `classification`, `notes`, `record_bounds`, `origin`; the caps are constants beside it |

1. **A restored run is not in `agent_runs`.** It lives in `restored_agent_runs`. Reading the code before writing any showed why: `agent_runs` is read
   by the **launch limit** (`agent_run_limit`, so every restart would have used up a slot), by **change attribution**
   (`agent_run_status_blocks_strong_association` and `other_run_temporally_overlaps_baseline`, where a restored `Detached` run, or a restored
   `Completed` run with no `ended_at`, would turn every later change ambiguous), by the **running/failed counts** and the close prompt. A flag on
   the run would have needed every one of those to remember to test it. A separate collection needs none of them to. The board's *agent runs*
   count includes both, because the count is the thing that read `0` after a restart and was wrong.
2. **`ended_at` is left alone; the ending is a new field.** `ended_at` is never set anywhere, and `other_run_temporally_overlaps_baseline` reads
   `None` as "no end recorded, so conservatively overlapping". Setting it at a terminal transition would change which changes are attributed
   strongly — a behaviour change to a feature this slice is not about. `RunEnding::{NotEnded, Ended(at), Unknown}` is set by `transition_to`
   (`Ended` on completed/failed/cancelled, `Unknown` on detached). `started_at`, which nothing reads either, is set on entering `Running`.
3. **The record is derived from the run every time and compared to what was last written**, rather than written from each mutation site. "On every
   change" then holds whichever path changed the run, and an unchanged run costs a comparison. The shell calls it from a one-second tick offered
   only while an open project has a launched run; **the user's own annotations are written by their setter at once**, not by the tick.
4. **A record never creates a run directory.** A run with transcript capture off, or none for any other reason, has no record: there is no
   directory, and the record does not make one. `RunRecordWrite::NoRunDirectory` says so to the caller, and the annotation stays in memory.

### What is on disk, from the running app

`04-record-written-by-the-live-app-before-the-kill.json`: the record the release binary wrote for a run **no setter had touched**, so it is the
tick's write — `"status": "running"`, `"ending": {"kind": "not_ended"}`, the profile, the prompt summary, `null` classification and notes, empty id
lists, and the `bounds` block all zero. It contains none of the transcript's text.

### Tests

`project/tests/run_records.rs`, 26 tests, and 3 in the shell. Every fixture is a fresh temporary state root; nothing reads the real one. "Killed" is a
session dropped with no closing step, because the product has none to call.

| Checklist box | Test(s) |
| --- | --- |
| beside the transcript, versioned, references only | `a_launched_run_gets_a_record_beside_its_transcript_holding_references_only` (asserts the transcript's text is absent) |
| written when it changes, not only at the end | `a_classification_set_mid_run_survives_a_kill`, `a_run_that_ended_keeps_the_ending_it_was_seen_to_have` (the status path), `a_pass_over_an_unchanged_run_writes_nothing` |
| atomic; nothing written through a planted name | `a_write_replaces_a_temporary_file_an_earlier_kill_left`, `a_record_is_never_written_through_a_planted_symlink` |
| restored run carries its fields and its transcript | `a_restored_run_carries_its_prompt_profile_ids_and_its_transcript`, `a_record_beside_no_transcript_still_restores_its_run` |
| ending unknown | `a_run_killed_with_the_app_says_its_ending_is_unknown`, `a_finished_run_whose_record_holds_no_ending_says_unknown_not_the_read_time`, and in the shell `a_restored_run_says_it_does_not_know_its_ending_and_is_never_called_finished` |
| corrupt / unknown version / not this run's: moved aside and named | `a_corrupt_record_is_moved_aside_and_the_run_is_a_transcript_with_no_run`, `a_record_of_an_unknown_version_is_set_aside_and_named_as_such`, `records_that_are_not_this_runs_are_set_aside_and_never_guessed_at` (six wrong shapes), `a_record_over_the_size_cap_is_set_aside_unread`, `a_record_that_cannot_be_moved_is_named_as_left_in_place_not_as_set_aside`; the board in `the_board_names_a_run_record_that_was_set_aside_and_is_silent_otherwise` |
| caps hold and the record says so | `the_caps_hold_and_the_record_says_it_was_bounded` (250 ids of each kind, a 4,100-character note; survives a restore **and a later rewrite**), `a_hand_edited_record_beyond_the_caps_is_bounded_on_the_way_in` |
| disk-usage counts it as own | `the_disk_usage_figure_counts_the_record_as_the_products_own` |
| a restored run is a record | `a_restored_run_is_never_running_never_failed_and_never_counts_against_a_limit`, `a_run_this_session_launched_is_not_restored_over_itself` |
| a purged run is not written back | `a_purged_run_is_not_brought_back_by_a_late_annotation`, `a_purged_restored_run_is_not_written_back_by_a_late_annotation` |
| the shell asks for it | `a_real_agent_run_launch_gets_its_record_from_the_shells_tick` (a real launch of the marker script; asserts **no record exists before the tick**, so it cannot pass by accident) |

### Ablations (`rfcs/handoffs/ablate.sh`, clean tree each time)

| # | Ablation | Failed |
| --- | --- | --- |
| 1 | the classification setter does not persist | `a_classification_set_mid_run_survives_a_kill` (also two more that depend on the setter's write) |
| 2 | a killed run's ending read as the time it was read | `a_run_killed_with_the_app_says_its_ending_is_unknown`, `a_restored_run_can_be_annotated_and_the_record_follows` |
| 3 | a finished run whose record holds no ending gets the read time | `a_finished_run_whose_record_holds_no_ending_says_unknown_not_the_read_time` **alone** (I had no test for this arm until the first ablation text did not match a test and I wrote one) |
| 4 | a corrupt record is not moved | the corrupt test and the unknown-version test |
| 5 | an unknown version is accepted | the unknown-version test **alone** |
| 6 | the id cap not applied | `the_caps_hold_and_the_record_says_it_was_bounded` **alone** |
| 7 | the omitted count not recorded | the same test, alone |
| 8 | the note not bounded | the caps test and the hand-edited-record test |
| 9 | the record's bytes counted as unclaimed **and nothing else changed** | `the_disk_usage_figure_counts_the_record_as_the_products_own` **alone** (my first version of this ablation stopped the loader seeing the record at all, and 16 tests failed for that reason; it proved the wrong thing, so I wrote the isolated one) |
| 10 | a restored run pushed into `agent_runs` | 9 tests, including the isolation test's *"the launched collection stays empty"* |
| 11 | the temporary file opened with `create` instead of `create_new` | `a_record_is_never_written_through_a_planted_symlink` **alone** |
| 12 | a purged restored run's directory still resolved | `a_purged_restored_run_is_not_written_back_by_a_late_annotation` **alone** |
| 13 | the board's list omits the record notice | `the_board_names_a_run_record_that_was_set_aside_and_is_silent_otherwise` **alone** |
| 14 | a restored run gets no ending line | `a_restored_run_says_it_does_not_know_its_ending_and_is_never_called_finished` |
| 15 | a restored run keeps the "has finished" status line | the same test |
| 16 | the set-aside count not accumulated on the session | the board test and the corrupt test |
| 17 | the shell's tick does nothing | `a_real_agent_run_launch_gets_its_record_from_the_shells_tick` **alone** |

**Not ablated, and proven only live:** that the tick is *offered* (the `subscription()` condition — an open project has a launched run). Test 17
proves the message does the work; the live walk below proves the message arrives, because the record it captured was written by it.

### A guard I tripped and did not weaken

`enumeration_confirms_only_the_closed_list_reads_full_file_content` failed on the new file, correctly: `run_record.rs` reads a whole file. The read is
`Take`n at the size cap + 1 and a file that fills it is set aside unread, so I added the file to the list with the reason, in the same doc comment
every other entry has, rather than widening the pattern.

### Live walk

Against `target/release/tekstide` at `ce11208`, with `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, the project and the configured AI CLI each in a `mktemp -d`
(under `/dev/shm`, see the note below). The CLI is a script that prints one marker line and then sleeps, so the run is **still running when the app
is closed**. Input was `wtype`, every send preceded by a check that the niri-focused window was mine; screenshots by window id, no window floated or
resized.

| Step | What happened | Evidence |
| --- | --- | --- |
| 1 | Session 1: granted trust, launched the configured CLI through the confirmation. Within three seconds `agent-run-<id>/` held `transcript.log` **and `run.json`** | `04-…json` |
| 2 | Closed the window. The process exited; the run's script was gone; `run.json` was still there | — |
| 3 | Session 2, same state: `Ctrl+Alt+R`. The AgentRun Report shows the run, **says it does not know when it ended**, and shows its transcript (50 bytes, the marker line) | `01-…png` |
| 4 | The record on disk was **byte-identical** before the kill and after this session (`cmp`): restoring a run does not rewrite its record | — |
| 5 | The Project Board reads *1 agent run*, **0 terminals, Calm** — a restored run is not counted as running or failed | `02-…png` |
| 6 | Closed. `run.json` overwritten with half a record. Session 3: the board says *1 agent run record could not be read and was set aside under a .corrupt name beside its transcript … Nothing was deleted*, *0 agent runs*, and the directory holds `run.json.corrupt` (36 bytes, the half record, intact) and `transcript.log` | `03-…png` |

**Not shown live: a run that was classified.** There is no control to classify a run until PR-056-D, and the environment-variable route D10 forbids
is not one I will invent for evidence. The classification round trip, and the classification set mid-run surviving a kill, are the core tests above,
ablated (1); the checklist box that says *captured live* for the classification is therefore **split**, and its live half is D's capture.

**A note on the machine.** `/tmp` was 100% full during this slice — 23 GB in another project's session scratchpad, which is not mine and which I left
alone — so the harness lost some command output and I ran every test, the gate and the fixture in `/dev/shm`. Nothing in the results depends on it,
but it is why the fixture path in the images reads `/dev/shm/tek056-live.…`.

### Things that are true now and that C must close

- **Purge does not remove `run.json` yet.** Between this slice and C, a purged run's record — its prompt summary and ids — stays on disk.
  Nothing is released in between; C is next and is the one that matters.
- **C must decide what it does about the files beside the record.** `run.json.tmp` (a write a kill interrupted) and `run.json.corrupt[-N]` (a record
  set aside) are counted as the product's own by name and are **not** deleted by anything. A set-aside record can hold exactly what D2 says a purge must
  not leave behind, so I would take them under the same purge, by exact name, as regular files only, with the directory removed only when empty. That
  is C's decision and the architect's; I have not made it.
- The loader now recognises three more names in a run directory, so the third-file test C plants must use a name that is **none** of them.

### Deviations and judgment calls, in one place

1. Restored runs are a separate collection (above, decision 1).
2. `ended_at` untouched; `RunEnding` is new (decision 2). `started_at` is now set on entering `Running` — nothing read it.
3. `prompt_summary` is capped at 1,000 characters, with a flag. The plan caps notes and id counts; the summary is Tekstide's own fixed string today,
   but a record read from disk is not trusted to have kept to any cap.
4. A record that could not be read **and could not be renamed** is a third case, `left_in_place`, and the board says *"could not be renamed either"*
   rather than *"set aside"*, which would be false. Tested by making the directory read-only.
5. `NotificationKind::RunRecordSetAside` is new, ordered after the transcript-retention kind.
6. The board's *agent runs* count includes restored runs; the AgentRun Report shows the latest restored run when none was launched this session, with
   its own ending line, and **drops the "has finished / still active" status line for a restored run**, because that line says something a restored run
   cannot know.
7. The set-aside counts are the session's own and never reset — a fact about this state directory, not about the last load.
8. A change to a run's status reaches its record within a second, not instantly. A run killed inside that second loses that status change only.

### Gate

`cargo fmt --all --check` and `clippy --workspace --all-targets -D warnings`: clean. `mdbook build docs`: clean (the changelog is included into
the book). `git diff --cached --check` after staging, before every commit: clean. `rfc_docs_invariants`: 16 passed.

**Three consecutive full-workspace runs, `--no-fail-fast`, output to files, a fresh short `TMPDIR` (`/dev/shm/tek056g`): `671 + 16 + 1014` = 1,701
passed, 0 failed, 0 entries left in `TMPDIR`, after each** (loads at the end of each run 1.54, 2.05, 4.18). The counts are the previous slice's
`668 + 16 + 988` plus 3 shell and 26 core tests.

**An earlier attempt at this gate was not clean and was not counted:** in run 1, `closing_a_project_with_a_backgrounded_descendant_kills_it_through_a_real_close`
(the registered row 8, PTY read timing) failed with its captured message, at load 2.35; runs 2 and 3 were green. It passed in the redone gate above.
It has a dated row in `test-process-leak.md`. It is not this slice's: nothing here touches terminal termination.

## PR-056-C — purge takes it too

### Order of work, as the review asked

1. **Q1** (`8b4bed4`): `is_run_record_file_name` is exact — `run.json`, `run.json.tmp`, `run.json.corrupt`, or `run.json.corrupt-` followed by one or more ASCII digits.
   Its test was written first and **failed against the prefix matcher** (`"run.json.corruption-notes" is a name this product never writes`); a second test
   shows the same bug where it cost something in B, a lookalike file counted as *the product's own* bytes. Ablated back to `starts_with`: both fail.
2. **The third-file test** (`fff6260`), committed **before any code that deletes a record or a directory existed**. It plants `run.json.corruption-notes` and
   an unrelated `notes-from-a-human.txt`, purges, and asserts a positive control (the transcript is gone), that both files survive, and that the directory survives.
3. **The deleting code** (`64d3a45`).

### What purge does now

`purge_transcript_at` — the one function user purge, per-run purge **and both retention passes** go through — now: (1) removes the run's record files, (2) removes
the transcript as before, (3) `fs::remove_dir`s the run directory, which the operating system refuses unless it is empty, (4) drops a restored run from the
session. **Only a directory in exactly the layout the product writes is looked at**: `product_run_directory_of` requires a file named `transcript.log` in a real
directory (no symlink) named `agent-run-<lowercase hyphenated uuid>`. A transcript at any other path keeps the old behaviour — its file and nothing beside it.
The record files are matched by the exact names above, **regular files only**: a symlink or a directory carrying one of those names is left alone.
`remove_dir_all` is not used anywhere; a directory is never removed by anything but `remove_dir`.

**The record files go first, then the transcript**, so a failed record removal leaves the transcript beside it and a retry finds both
(`a_purge_that_cannot_remove_the_record_leaves_the_transcript_and_can_be_retried`). *That order is a decision I could not test in isolation*: making the
directory read-only fails the transcript's removal too, so reversing the order (ablation C9) is caught only by the byte accounting, not by the retry test.
I say so rather than claim the order is guarded.

### Tests (10 new; the two B tests about a purged run's late annotation were rewritten)

| Checklist box | Test |
| --- | --- |
| directory only if empty; the third file | `purge_never_removes_a_directory_it_did_not_find_empty_or_a_file_it_did_not_write` |
| the record and every set-aside name go, then the empty directory | `purge_removes_the_transcript_every_record_file_and_then_the_empty_directory` (`.tmp`, `.corrupt`, `.corrupt-1`, `.corrupt-42`) |
| regular files, exact names; a symlink or directory with a record's name survives | `purge_deletes_only_regular_files_by_exact_name_and_leaves_the_rest` (the symlink's target is checked too) |
| only the product's layout is touched | `a_transcript_outside_the_products_layout_is_purged_alone` |
| dialog counts and bytes removed include the records | the first test asserts `purgeable_transcript_bytes() == on disk` and `bytes_removed == on disk` |
| **a search of the state directory** finds nothing | `after_a_purge_nothing_on_disk_names_the_prompt_or_the_notes` — unique needles in the prompt summary and the notes, found before the purge (positive control) and **not in any file or file name** after, nor the run id |
| a restored run leaves with its record | `a_purged_restored_run_leaves_the_session_and_the_board_count`, `a_purged_restored_run_cannot_be_annotated_back_into_existence`, `a_purged_run_is_not_brought_back_by_a_late_annotation` |
| retention takes it too | `a_transcript_expired_by_retention_takes_its_records_with_it` (a real expiry, positive control `expired.purged_transcripts == 1`) |
| failure leaves both | the retry test above |

### Ablations

| # | Ablation | Failed |
| --- | --- | --- |
| C1 | the record files are not removed | 7 tests, including the search test and the directory test |
| **C2** | **delete by prefix (`run.json*`)** | **`purge_never_removes_a_directory_it_did_not_find_empty_or_a_file_it_did_not_write` alone**, *"a file this product never wrote is not deleted"* |
| C3 | `remove_dir_all` in place of `remove_dir` | the third-file test and the exact-name test |
| C4 | symlinks removed too | the exact-name test alone |
| C5 | the layout guard removed | `a_transcript_outside_the_products_layout_is_purged_alone` alone |
| C6 | the dialog omits the record bytes | the first purge test alone |
| C7 | `bytes_removed` omits the record | the same test alone |
| C8 | a restored run is not forgotten | the two restored-run tests |
| C9 | transcript removed before the record | the byte-accounting assertion only (see above) |
| C10 | record removal disabled, retention path | the retention test alone |

The sharp one is C2: the plan's danger, made real, is caught by exactly the file the review named.

### Live walk

Release binary at `64d3a45`, fixtures in `mktemp -d` under `/dev/shm`, focus-verified `wtype`, by window id. Two runs launched in two sessions, each **killed with the app**;
before the third session I planted `run.json.corruption-notes` (36 bytes) in run 1's folder and a `run.json.corrupt` (26 bytes) in run 2's.

| Step | Result | Evidence |
| --- | --- | --- |
| Trust Settings | *Retained locally: 2 transcripts (100 bytes)*, and **36 bytes** "belong to no project … or are files Tekstide does not recognise" — **the lookalike only**; the two `run.json` and the `.corrupt` are counted as the product's own | `01-` |
| Purge dialog | *permanently deletes 2 transcripts **(1,706 bytes)*** = 100 + 2 × 790 (the records) + 26 (the set-aside file); the lookalike's 36 is not in it | `02-` |
| After the purge | Trust Settings reads *0 transcripts (0 bytes)*, the 36 bytes still listed. **On disk: run 2's folder is gone; run 1's folder holds exactly `run.json.corruption-notes`** | `03-`, `04-` |

### Two things I am flagging, not deciding

1. **The purge dialog's sentence now says *"2 transcripts (1,706 bytes)"* while Trust Settings says *"2 transcripts (100 bytes)"*.** The dialog's figure is what will be
   removed (D2 asks for that), so it is the honest one; but the word *transcripts* now covers run records, and the two figures on two screens disagree. I did not
   reword either: the wording is yours, and the catalog line is `transcript-purge-*`. A one-clause change to the dialog (*"…and their run records"*) would close it.
2. **Retention also takes the record.** D2 speaks of purge; the retention passes share the function, so an expired transcript takes its run's record — including
   the user's notes, once D exists. The alternative (retention keeps the record) has a hole: `purge_transcript_at` returns early for a tombstone, so a record left
   by retention could then never be purged. I took the consistent reading and said so in the book and the changelog; it is your call whether notes should outlive
   a transcript's expiry.

### Not handled

A **record with no transcript** (the user deleted `transcript.log` by hand) is not reached by purge, which walks transcripts. It cannot arise through the product
(every product deletion goes through the function above). Named, not built.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `mdbook build docs` (the book and the changelog changed), `rfc_docs_invariants` 16: clean.
**Three consecutive full-workspace runs, `--no-fail-fast`, fresh `TMPDIR` (`/dev/shm/tek056g`): `671 + 16 + 1024` = 1,711 passed, 0 failed, 0 entries left after each**
(loads at the end 7.55, 9.54, 11.37). No intermittent this time. `git diff --cached --check` clean before each commit.
