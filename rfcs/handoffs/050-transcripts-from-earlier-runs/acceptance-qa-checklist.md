---
title: "RFC-050 — acceptance and QA checklist"
rfc: "RFC-050"
rfc_file: "../../done/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Implemented and closed 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-13"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named. That is the reviewer's error to fix, not the
implementer's to paper over.

## PR-050-A — the lock, the id, the origin

- [x] The writer holds an exclusive lock for its lifetime. A second handle gets `WouldBlock` while
      the writer lives and succeeds after it drops. **Ablation:** remove the lock; that assertion fails
      alone.
      *L1 fails the lock test's held assertion — and three more tests, because every test about the
      lock needs the lock. "Alone" cannot hold for this ablation; disclosed as a composition.
      Release is asserted within 5 s: a concurrent fork briefly holds a dropped writer's descriptor.*
- [x] **A writer that cannot lock does not write.** The launch proceeds without capture, and the
      run's reason is distinct from opt-out.
      *Refused-writer and untouched-bytes tests; the launch-degrade test asserts the process started,
      no `Transcript` record, held bytes untouched, and `TranscriptAbsence::WriterLockUnavailable` —
      a reason opt-out never sets. Rendering it in the run detail is not in this core-only slice.*
- [x] `AgentRunId` rejects a non-UUID string.
      *The constructor already existed (`impl_id!`). It still accepts uppercase, hyphen-less, braced
      and `urn:` spellings — a finding for PR-050-B, not fixed here; see `qa-evidence.md`.*
- [x] `Transcript`'s origin is an exhaustive type. A found record names no terminal or run the
      session lacks. **Grep:** no id is fabricated to satisfy an ownership check.
      *`terminal_id`/`agent_run_id` moved into `TranscriptOrigin::LaunchedHere`; grepped, no id is
      minted to satisfy an ownership check.*
- [x] The liveness predicate matches on origin first. **Ablation:** add an origin variant; the compile
      error appears only in the predicate.
      *L7: one compile error, `project/session.rs:1029`, and nowhere else.*
- [x] Nothing is loaded from disk yet.
      *`FoundOnDisk` is built only in tests; no directory walk exists.*

## PR-050-A follow-up (response 388)

- [x] **`RequiredLocalBounded` refuses on a lock failure**, at both launch sites: no process starts,
      and the held bytes are untouched. **Ablation:** degrade regardless of mode; the test fails alone.
      *(Found at review: PR-050-A degraded every mode, and no test launched that mode against a locked
      file.)*
      *`a_required_local_bounded_launch_is_refused_when_its_transcript_file_is_locked`: refused for the
      writer, no process, no run or transcript, held bytes untouched. F1 fails it alone.*
      *Unticked by the reviewer at response 390: the test launches `/bin/sh` without an adapter, so it
      reaches `launch_project_shell` only. Removing the mode check from `launch_project_adapter` alone
      left all 794 tests green. The adapter site is correct, but nothing holds it.*
      *Re-ticked after the second follow-up: both sites call `prepare_transcript_writer`, so the
      required-mode test now holds both. H1 (degrade every mode inside the helper) fails it alone.*
- [x] **One helper prepares the transcript writer for both launch sites** (response 390). Neither
      `launch_project_shell` nor `launch_project_adapter` calls `BoundedTranscriptWriter::create`
      directly (**grep**), so the required-mode test holds both. **Ablation:** degrade every mode inside
      the helper; the required-mode test fails alone. Then tick the box above.
      *`prepare_transcript_writer`; grepped, neither launch function calls `create` directly. H1 fails the
      required-mode test alone.*
- [x] **Regular files only are locked**, decided with the truncate by one `fstat` on the opened
      handle. Two writers on one FIFO both create; a second writer on a locked regular file is refused.
      **Ablation:** lock every file type; the FIFO test fails alone.
      *One `fstat` on the handle; an unreadable type is refused. FIFO and second-writer tests added. F2
      fails the FIFO test **and** the required-mode `/dev/full` test — "alone" cannot hold, and the
      second failure is the very interference this ruling removes.*
- [x] The `/dev/full` test mutex is removed, and the register entry that called it the fix says why.
      *Removed; the register row and dated entry say it hid the problem. The two `/dev/full` tests ran
      together 20 times without it: 20 passed.*

## PR-050-B — load, count, purge

- [x] **A real restart** (a fresh `AppState` restored from saved recent-project state): the earlier
      transcript is counted, and purge removes **the file on disk**. **Ablation:** skip loading; the
      test fails alone.
      *Core half in part 1: `a_transcript_from_an_earlier_run_is_counted_and_purged_after_a_real_restart`.
      Left unticked until part 2 wires loading into the product's open path; K1 fails seven tests at
      this layer, so "alone" is for the GUI-level ablation.*
      *Part 2: `reopening_a_project_loads_an_earlier_runs_transcript_and_purge_deletes_it`, through a real
      `Enter` on the remembered row. M1 (no loading on reopen) fails it at its loading assertion.*
- [x] A file whose lock another handle holds loads as live, survives purge, and is reported.
      **Ablation:** remove the probe; the test fails alone.
      *Part 1: held-lock test; K2 (no probe) and K6 (purge ignores it) each fail it alone. The dialog's
      naming of such files is PR-050-C.*
- [x] The probe drops its lock at once. Assert that a later `try_lock` succeeds.
      *`the_load_probe_releases_the_lock_it_took`; K3 (leak the handle) fails it alone.*
- [x] Skipped **and still present**: a symlinked run directory (pointing outside the state root), a
      non-UUID name, a stray file, and a `transcript.log` at the wrong depth.
      *`every_unrecognised_entry_is_skipped_and_still_present_after_purge`; K5 (follow symlinks) fails it
      alone.*
- [x] Loaded records are purged through `purge_transcript_at` only. **Grep:** no other
      `remove_file` reaches `transcripts/`.
      *Grepped: the only `remove_file` reaching `transcripts/` is `remove_transcript_file`.*
- [x] `last_write_at` comes from mtime, read at load. **Grep:** no filesystem read inside retention
      selection (RFC-049 D7).
      *Grepped: no filesystem read in `retention.rs` or in retention candidate selection.*
- [x] The app-wide figure includes a closed project. Unclaimed bytes are computed separately.
      *Core: `scan_transcript_disk_usage`; K9 fails the closed-project test alone. The GUI figure
      switches to it in part 2.*
- [x] A directory whose id is not in the recent list is not loaded, not deleted, and counted as
      unclaimed.
      *`an_unclaimed_project_directory_is_not_loaded_or_deleted_and_is_counted`.*
- [x] The dialog's count is of retained transcripts, not tombstones. Verify the reading first; if it
      was wrong, say so here.
      *Reading verified: `transcripts().len()` includes tombstones. `purgeable_transcript_count` is added
      in part 1; the dialog switches in part 2, so this stays unticked until then.*
      *Part 2: the dialog captures `purgeable_transcript_count`, and Trust Settings excludes tombstones.
      M2 and M3 each fail the acceptance test at their own assertion.*
- [x] **Run-directory names are the product's own spelling.** Uppercase, hyphen-less, braced and
      `urn:uuid:` spellings are each skipped and still present. `from_persisted` is unchanged.
      *Spellings test; K4 (accept `from_persisted`'s spellings) fails it alone.*
- [x] **The dialog counts only what purge will delete**: a live found file is never counted.
      *`purgeable_transcript_count` excludes still-written files (K7 fails the held-lock test alone);
      unticked until the dialog uses it in part 2.*
      *Part 2: the dialog captures `purgeable_transcript_count` and `purgeable_transcript_bytes`.*
- [x] **In the commit that makes purge true:** the request-387 disclosure is removed from the book,
      `README.md` and the changelog, and the changelog defect entry names `0.12.0` through `0.18.0`,
      what a user saw, and that deleting `transcripts/` was the only complete removal. *(Moved from C
      at response 388.)*
      *Part 2: the README and book known-defect paragraphs are removed, and the changelog's Unreleased
      entry is now the defect entry for `0.12.0`–`0.18.0`.*
- [x] **No test enumerates the real state root.** Every fixture is `mktemp -d`, reached through the
      `cfg(test)` split.
      *Grepped the new tests; every fixture is a fresh temporary directory.*

## PR-050-C — say it, and stop saying the old thing

- [x] Trust Settings shows unclaimed bytes and where they are when there are some, and nothing when
      there are none. Each case is ablated alone. The wording fits the reset case (D6′): it does not
      imply the user did something, or that anything is dangerous.
      *P5 and P6 each fail their own test alone. Wording states the fact without blame or warning.*
      *Unticked at response 394 (F2): the line says "on this computer", but the figure is one state
      directory's; and `unclaimed_bytes` also counts unrecognised files inside claimed project
      directories, which do belong to a project. The wording must describe what is measured.*
      *Re-ticked: the line now names the directory it measured and says the bytes "belong to no
      project in the recent list, or are files Tekstide does not recognise". No claim about the
      computer, and both halves of the figure are named.*
- [x] The purge dialog names files still being written; absent when none. Each case is ablated alone.
      *P1 and P2 each fail their own test alone.*
- [x] **A run still in progress** (D3′, response 393; wording settled at response 394, F3): when a
      purgeable transcript may belong to a run still in progress, the dialog says so, that the
      transcript is deleted either way, and that deleting it does not stop the run; absent otherwise.
      Each case ablated alone.
      *P3 and P4 each fail their own test alone; the count's status check is held by P11.*
      *Unticked at response 394 (F3): the count uses the conservative liveness predicate, which includes
      `ReviewReady` and `Detached`, so "The run keeps running" can be false. Say what is known: the run
      may still be in progress.*
      *Re-ticked: the notice says the transcripts "may belong" to a run still in progress, that the
      transcript is deleted either way, and that deleting it does not stop the run. The test asserts
      "may belong" and that "keeps running" is absent; R3 (restore the old wording) fails it alone.
      The test is renamed `the_purge_dialog_says_a_run_may_still_be_in_progress`.*
- [x] **Unclaimed bytes exclude closed recent projects** (response 393): a recent project that is not
      open has its transcript bytes counted as claimed, not unclaimed. *(Dropping recent projects
      from the claimed set failed nothing at review.)*
      *`a_closed_recent_projects_transcripts_count_as_claimed`; P9 fails it alone.*
- [x] **Byte counts never follow a symlink** (response 393): a symlink under `transcripts/` pointing
      at a file outside the state root adds nothing to the total or the unclaimed figure. *(Counting
      through symlinks failed nothing at review.)*
      *`a_symlink_under_transcripts_adds_no_bytes_to_either_figure`; P10 fails it alone.*
- [x] **The reset notice (owner's decision, 2026-09-13).** On a start where `recent-projects.json`
      could not be read, the board says the list was reset, that earlier transcripts remain on disk
      (bytes and where), and where the unreadable file went. It appears on that start only; a later
      start with a readable file shows nothing. **Ablation:** drop the notice; its test fails alone.
      *P7 (drop it) and P8 (show it on a readable start) each fail their own test alone.*
      *Unticked at response 394 (F1): the notice reads the live disk figure on every render, so once new
      transcripts are written and the figure refreshes, "Transcripts from before" includes bytes that
      are not from before. Snapshot the figure at boot, and say nothing about transcripts when there
      were none.*
      *Re-ticked: `RecentProjectsReset.transcript_bytes_at_boot` is frozen in
      `with_recent_projects_reset`, and the transcripts sentence is its own line, absent at zero.
      `the_reset_notice_keeps_the_boot_figure_after_the_live_one_changes` (R1 fails it alone) and
      `the_reset_notice_says_nothing_about_transcripts_when_there_were_none` (R2 fails it alone).*
- [x] `remove_recent_project`'s doc comment states that any removal control must say the project's
      transcripts stay on disk. No caller or API is added for it.
      *Stated; no caller and no API added.*
- *(The disclosure's removal and the changelog defect entry are PR-050-B boxes now; see response
  388.)*
- [x] Live walkthrough of launch, quit, restart, the count shown, purge, and the file gone, against
      throwaway state only.
      *Release binary, `mktemp -d` state; seven images in `evidence/pr-050-c/`. Also the reset notice
      and the unclaimed line, using a hand-placed stand-in file (disclosed).*

## Whole-RFC

- [ ] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`,
      `rfc_docs_invariants`, and three consecutive full-workspace runs with output redirected to a
      file.
- [ ] Every new intermittent failure has a row in `test-process-leak.md`.
- [ ] Commits are pushed once the gate is green.
- [ ] **`rust-version` is declared only after both crates build on that toolchain.** 1.89.0 is when
      `File::try_lock` was stabilised, not a measured minimum.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
