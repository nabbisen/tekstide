---
title: "RFC-050 — acceptance and QA checklist"
rfc: "RFC-050"
rfc_file: "../../accepted/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Accepted 2026-09-13 — M12"
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

- [ ] **A real restart** (a fresh `AppState` restored from saved recent-project state): the earlier
      transcript is counted, and purge removes **the file on disk**. **Ablation:** skip loading; the
      test fails alone.
- [ ] A file whose lock another handle holds loads as live, survives purge, and is reported.
      **Ablation:** remove the probe; the test fails alone.
- [ ] The probe drops its lock at once. Assert that a later `try_lock` succeeds.
- [ ] Skipped **and still present**: a symlinked run directory (pointing outside the state root), a
      non-UUID name, a stray file, and a `transcript.log` at the wrong depth.
- [ ] Loaded records are purged through `purge_transcript_at` only. **Grep:** no other
      `remove_file` reaches `transcripts/`.
- [ ] `last_write_at` comes from mtime, read at load. **Grep:** no filesystem read inside retention
      selection (RFC-049 D7).
- [ ] The app-wide figure includes a closed project. Unclaimed bytes are computed separately.
- [ ] A directory whose id is not in the recent list is not loaded, not deleted, and counted as
      unclaimed.
- [ ] The dialog's count is of retained transcripts, not tombstones. Verify the reading first; if it
      was wrong, say so here.
- [ ] **Run-directory names are the product's own spelling.** Uppercase, hyphen-less, braced and
      `urn:uuid:` spellings are each skipped and still present. `from_persisted` is unchanged.
- [ ] **The dialog counts only what purge will delete**: a live found file is never counted.
- [ ] **In the commit that makes purge true:** the request-387 disclosure is removed from the book,
      `README.md` and the changelog, and the changelog defect entry names `0.12.0` through `0.18.0`,
      what a user saw, and that deleting `transcripts/` was the only complete removal. *(Moved from C
      at response 388.)*
- [ ] **No test enumerates the real state root.** Every fixture is `mktemp -d`, reached through the
      `cfg(test)` split.

## PR-050-C — say it, and stop saying the old thing

- [ ] Trust Settings shows unclaimed bytes and where they are when there are some, and nothing when
      there are none. Each case is ablated alone. The wording fits the reset case (D6′): it does not
      imply the user did something, or that anything is dangerous.
- [ ] The purge dialog names files still being written; absent when none. Each case is ablated alone.
- [ ] **The reset notice (owner's decision, 2026-09-13).** On a start where `recent-projects.json`
      could not be read, the board says the list was reset, that earlier transcripts remain on disk
      (bytes and where), and where the unreadable file went. It appears on that start only; a later
      start with a readable file shows nothing. **Ablation:** drop the notice; its test fails alone.
- [ ] `remove_recent_project`'s doc comment states that any removal control must say the project's
      transcripts stay on disk. No caller or API is added for it.
- *(The disclosure's removal and the changelog defect entry are PR-050-B boxes now; see response
  388.)*
- [ ] Live walkthrough of launch, quit, restart, the count shown, purge, and the file gone, against
      throwaway state only.

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
