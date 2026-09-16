---
title: "RFC-051 — QA evidence"
rfc: "RFC-051"
rfc_file: "../../accepted/051-recovering-the-recent-project-list.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Evidence

## PR-051-A — the store keeps a copy, and says what happened

### One outcome, because the ordering is the defect

`load_or_recover` does the whole of §6 — **quarantine, then recover, then allow saving** — and
returns `RecentProjectLoad { state, outcome }`. There is no sequence for a caller to get wrong,
which is D6′'s point: two prior slices shipped a correct decision with one call site unguarded, and
the way not to have that problem is not to have the call site.

`RecentProjectLoadOutcome` is `Loaded`, `Recovered { moved_to, message }` or
`Reset { moved_to, message }`. A missing file is `Loaded` — a first start has nothing to recover and
nothing to protect.

### The two rules that keep a repair from becoming a second destruction

- **§1, saving is withheld after a failed quarantine.** An unreadable file is renamed aside like an
  unparseable one; if that rename fails the file is still there, `saving_allowed` is cleared for the
  session, and `save` returns `RecentProjectSave::Withheld` **before touching anything**. A session
  runs perfectly well without persisting a list.
- **§2/D2′, the backup is gated on a real load.** `backup_allowed` is set only by a load that
  produced a list (including a first start, where there is no good copy to lose). A session that
  started empty after a failed load writes the live file and **leaves the backup exactly as it was**.

**`RecentProjectSave` exists so a caller can tell "written" from "deliberately not written"** rather
than inferring it from `Ok(())`, and it reports whether the backup was updated.

### §3, whole file or nothing

`read_backup` reads and parses the whole file; missing, unreadable and unparseable are one answer.
A half-parsed list can resurrect a wrong path-to-id mapping, and a wrong id re-attaches somebody's
trust grant and somebody's transcripts to the wrong folder — worse than a missing one.

### Tests

| Test | Proves |
| --- | --- |
| `a_corrupt_live_file_recovers_the_list_from_the_backup_with_ids_intact` | the list comes back **with its ids**, and the outcome names where the bad file went |
| `an_unreadable_live_file_is_quarantined_rather_than_overwritten` | §1's defect: the unreadable case is now moved aside too |
| `a_failed_quarantine_withholds_every_save_for_the_session` | a read-only directory makes the rename fail; the save is `Withheld` and the original bytes are unchanged |
| `a_session_that_started_empty_after_a_failed_load_writes_no_backup` | §2/D2′, asserted on the backup's own bytes |
| `a_corrupt_backup_is_a_reset_and_nothing_is_salvaged_from_it` | §3, with a backup shaped exactly like something a salvaging implementation would half-read |
| `a_readable_live_file_loads_and_reports_nothing_moved` | the common case stays silent |

**One test found D2′ working before I did.** The recovery test originally saved before loading, so no
backup was written and the recovery failed — because a store that has not loaded may not write the
previous-good copy. The test now follows production's order, and says why in a comment.

## PR-051-B — the product uses it, and says which happened

`boot()` calls `load_or_recover` and renders `recent_project_list_repair_from(&outcome)`. It
sequences nothing.

**The board has two forms** (D5/§5), and the recovered one **replaces** the reset one rather than
joining it:

- *"…it was restored from the last saved copy. Your projects and their settings are back."*
- the existing reset sentence, still carrying RFC-050 PR-050-C's transcripts-remain-on-disk line.

Both still say where the unusable file went. **The recovered form says nothing about orphaned
transcripts**, and a test holds that: a recovery orphans nothing, because the ids came back.

### §4 — recovery restores ids and paths, never trust

`a_recovered_project_with_no_grant_in_the_store_is_still_demoted` takes the list back from the
store's **own backup**, reopens the project, and then re-checks it against a real, empty audit store:
Trusted → Restricted. There is no separate code path for a recovered list, which is why this holds by
construction — `restore_recent_projects` and `verify_restored_trust` cannot tell which outcome
produced the list.

### The live walkthrough

Release binary, `mktemp -d` state root and project. Session 1 opened the project and quit; the store
wrote both `recent-projects.json` and `recent-projects.json.bak`, **carrying the same id**. The live
file was then overwritten with `this is not json at all` and the application restarted.

- The list came back with **the same project id** (`24a01595-…`), checked on disk, not inferred.
- The board said: *"The recent-projects list could not be read, so it was restored from the last
  saved copy. Your projects and their settings are back."* and *"The unreadable list was moved to
  /tmp/…/recent-projects.json.corrupt."*
- Nothing was printed to stderr, and the quarantined file is on disk beside the restored one.

`evidence/00-board-recovered-from-the-last-saved-copy.png`. Only `/tmp` paths.

## Ablations, each restored and hash-checked, `--no-fail-fast`

| | Ablation | Fails |
| --- | --- | --- |
| D1 | skip the quarantine for an unreadable file | **three** store tests — the quarantine is load-bearing for recovery, for §1's withheld save, and for the unreadable case itself |
| D2 | write the backup regardless of how the session started | `a_session_that_started_empty_after_a_failed_load_writes_no_backup` **alone** |
| D3 | salvage what parses from a partial backup | the salvage test **and** the no-backup test — `unwrap_or_default` turns a corrupt backup into an empty "recovery", which changes both |
| D4 | save before attempting recovery — **the ordering defect itself** | **three** tests across both crates: recovery, quarantine, and the GUI recovery test |
| D5 | the recovered start uses the reset sentence | both recovered-line tests — one mechanism, two assertions |

Only D2 fails alone; the rest are disclosed compositions, and D1 and D4 failing broadly is the shape
the rules predict — they are the two properties everything else in this RFC stands on.

## Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 9 passed. `mdbook build docs`: clean. **Three consecutive full-workspace runs
with `--no-fail-fast`: 541 + 9 + 829, green every time** (+3 shell, +6 core). No new intermittent.
`git diff --cached --check` after staging: clean.

## PR-051-C (response 401) — the method this slice made dormant

**`RecentProjectStore::load` had only test callers** once `load_or_recover` superseded it — four, in
the store's own test file. That is the dormant-capability shape RFC-036 closed, created fresh by the
change that replaced it, and this project's rule is that such a capability is deleted rather than kept
for a caller that may never come.

- **`load` is gone.** Its four tests moved onto `load_or_recover`, keeping what each proved: a missing
  file is not a failure, a corrupt file is quarantined and reported, the quarantine does not overwrite
  an existing `.corrupt`, and the outcome names where the file went.
- **`RecentProjectStoreError::CorruptState` is gone with it**, because `load` was its only producer.
  Checked rather than assumed: nothing outside that function constructed it, and nothing outside the
  one migrated test matched on it. `Io` and `PathUnavailable` remain — `save` and the path provider
  still produce both.
- **The changelog records the removal as breaking**, in the shape `0.16.0`'s dormant-API removal set:
  what went, what replaces it, and what a dependent does instead.

### Gate

fmt, clippy and `rfc_docs_invariants` clean. **Three consecutive full-workspace runs with
`--no-fail-fast`: 541 + 9 + 829, green every time** — the same counts as PR-051-B, because the four
tests moved rather than multiplied.
