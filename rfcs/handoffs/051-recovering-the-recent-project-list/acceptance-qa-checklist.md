---
title: "RFC-051 — acceptance and QA checklist"
rfc: "RFC-051"
rfc_file: "../../done/051-recovering-the-recent-project-list.md"
source_rfc_status: "Implemented and closed 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Acceptance and QA checklist

Every box is a property. A box whose plan assigns it elsewhere, or that cannot be satisfied as
written, stays unticked with the contradiction named — the reviewer's error to fix, not the
implementer's to paper over.

## PR-051-A — the store

- [x] A corrupt live file with a good backup **recovers the list**, and every project keeps its id.
      **Ablation:** save before recovering; the test fails alone.
      *`a_corrupt_live_file_recovers_the_list_from_the_backup_with_ids_intact`. D4 (save before
      recovering — the ordering defect) fails **three** tests across both crates, not one: it is the
      property everything else stands on. Disclosed.*
- [x] **An unreadable live file is quarantined, not overwritten**, and when the quarantine rename
      fails, **nothing is saved**. **Ablation:** skip the quarantine; the test fails alone.
      *Two tests: the quarantine itself, and a read-only directory making the rename fail — where the
      save is `Withheld` and the original bytes are asserted unchanged. D1 fails **three** tests;
      disclosed for the same reason as D4.*
- [x] **No backup is written from a session that started empty after a failed load** (§2, D2′).
      **Ablation:** write it regardless; the test fails alone.
      *`a_session_that_started_empty_after_a_failed_load_writes_no_backup`, asserted on the backup's
      own bytes. D2 fails it **alone**.*
- [x] A corrupt or unreadable **backup** yields the reset outcome. **Nothing is salvaged from a
      partial file** (§3), asserted.
      *`a_corrupt_backup_is_a_reset_and_nothing_is_salvaged_from_it`, with a backup shaped exactly
      like what a salvaging implementation would half-read. D3 fails it, plus one more; disclosed.*
- [x] The outcome is **one typed value** naming where a bad file went. **Grep:** the shell does not
      sequence load/recover/save itself.
      *`RecentProjectLoadOutcome`, with `moved_to()`. `boot()` calls `load_or_recover` once and renders
      the result; grepped — no backup path, no rename and no recovery logic outside the store.*

## PR-051-B — the product

- [x] The board says **recovered from the last saved copy**, or **reset with nothing to recover**, and
      nothing on a normal start. Each case ablated alone.
      *Four tests: the recovered line, the reset line, the normal start, and that the recovered form
      says nothing about orphaned transcripts. D5 fails both recovered-line tests — one mechanism.*
- [x] The reset form still says earlier transcripts remain on disk and where (RFC-050 PR-050-C).
      *Unchanged, including the boot-figure snapshot from response 394; its tests still pass, migrated
      to the new outcome type.*
- [x] **A recovered project the audit store has no grant for is demoted** (§4). **Ablation:** skip
      `verify_restored_trust` for recovered lists; the test fails alone.
      *`a_recovered_project_with_no_grant_in_the_store_is_still_demoted`, through the store's real
      backup. **The ablation cannot be written**: there is no separate path for a recovered list —
      `restore_recent_projects` and `verify_restored_trust` cannot tell which outcome produced it. The
      property holds by construction, which is stronger than by test.*
      *Reviewer, response 401: agreed, and **the box's ablation clause was mine and unwritable**. A
      property held by construction is the better outcome; the substituted test is the right
      evidence. The clause stays only as the record of what was asked for.*
- [x] The book's "a reset loses that project's trust decisions" sentence is corrected **in the commit
      that makes it false**, and says what happens now.
      *The book's recent-projects section gains the backup, the restore-with-ids, and that restoring
      never restores trust; the changelog's `0.19.0` limitation line points at the fix. Both in
      PR-051-B's commit, which is the commit that makes them false.*
- [x] Live walkthrough: real project, corrupted state file, restart, list back with the same project,
      board saying so. Throwaway paths only.
      *Release binary; the same id (`24a01595-…`) checked on disk, and the board saying it was restored
      from the last saved copy. `evidence/00-board-recovered-from-the-last-saved-copy.png`.*

## PR-051-C — follow-up (response 401)

- [x] **The superseded `load()` is gone.** `load_or_recover` replaced it, and `load()` now has only
      test callers — four, in the store's own test file. That is the dormant-capability shape RFC-036
      exists to close, created fresh by this slice. Move those four tests onto `load_or_recover` and
      delete the method. **Grep:** no caller of `load()` remains, including in tests.

## Whole-RFC

- [x] `cargo fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --cached --check` after
      staging, `rfc_docs_invariants`, and **three consecutive full-workspace runs with
      `--no-fail-fast`**, output redirected to files.
      *All clean; 541 + 9 + 829, green every time.*
- [x] Every new intermittent failure has a dated row in `test-process-leak.md`.
      *None this slice.*
- [x] `future-work.md`'s reset row is updated: what this RFC fixed, and what remains.
      *What it fixed, and three things that remain: a pre-RFC-051 loss is unrepairable, one copy is not
      a history, and ids are still not derivable from the path (D6).*
- [x] Commits are pushed once the gate is green.

## Final Acceptance Decision

- [x] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Accepted 2026-09-16 (responses 401 and 402). Four reviewer ablations, each restored and
hash-checked: no quarantine before recovery, a backup written whatever happened at load,
a failed quarantine still allowing saves, and no-backup reported as a recovery. Y1 and Y4
fail three and four tests, which is what the rules predict and what the implementer
disclosed. Gate: 541 + 9 + 829, three runs, twice — before and after PR-051-C.
```
