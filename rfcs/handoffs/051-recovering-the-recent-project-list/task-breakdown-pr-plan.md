---
title: "RFC-051 — task breakdown and PR plan"
rfc: "RFC-051"
rfc_file: "../../done/051-recovering-the-recent-project-list.md"
source_rfc_status: "Implemented and closed 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# Task breakdown and PR plan

**A then B.** A is the store and changes no surface; B wires it and says it.

## PR-051-A — the store keeps a copy, and says what happened

- **One typed outcome** from `RecentProjectStore::load` (D6′): loaded, recovered from backup, or
  reset with nothing to recover — carrying where an unreadable or unparseable file was moved.
- **Quarantine before anything else**, for an unreadable file as well as an unparseable one (§1). If
  the rename fails, the outcome says so and **nothing is saved**.
- **`recent-projects.json.bak`**, written on each successful save, **only when the live file loaded**
  (§2, D2′).
- **Recovery reads the backup whole** (§3), and a backup that is itself unreadable or unparseable is
  a reset, not a partial anything.

**Required tests:**

- A corrupt live file with a good backup **recovers the list, ids intact**.
- An unreadable live file is **quarantined and never overwritten**; when the rename fails, **no save
  happens at all**.
- A session that started empty after a failed load **writes no backup** on its next save.
- A corrupt backup yields the reset outcome, and nothing is salvaged from it.
- The outcome names where the bad file went.

**Ablations:** save before recovering; write the backup regardless of load state; salvage a partial
file; skip the quarantine. Each fails its own test.

## PR-051-B — the product uses it, and says which happened

- `boot()` renders the store's outcome. **No sequencing in the shell** (D6′).
- **The board line has two forms** (§5, D5): recovered from the last saved copy, or reset with nothing
  to recover — the second keeping RFC-050 PR-050-C's existing sentence about transcripts remaining on
  disk. Absent on a normal start.
- **`verify_restored_trust` runs over a recovered list** exactly as over a loaded one (§4).
- The book's sentence that a reset loses trust decisions is corrected to say what now happens, in the
  commit that makes it false.

**Required tests:** each line present when true and absent otherwise, ablated separately; **a
recovered project the audit store has no grant for is demoted**.

**Evidence:** a live walkthrough against a `mktemp -d` state root — a real project opened, the state
file corrupted, the app restarted, the list back with the same project, and the board saying so.
Throwaway paths only.


## PR-051-C — follow-up (response 401), before the release candidate

- **Delete `RecentProjectStore::load`.** `load_or_recover` supersedes it, and after PR-051-A its only
  callers are four tests in `recent/tests.rs` — a dormant capability created by this slice, which is
  what RFC-036 closed and what this pack has now produced in a third shape.
- **Move those four tests onto `load_or_recover`**, keeping what they prove: a missing file is not a
  failure, a corrupt file is quarantined and reported, the quarantine does not overwrite an existing
  `.corrupt`, and the reported path is where the file went.
- **Grep:** no caller of `load()` remains anywhere, including tests.
- It is a `pub fn` on a published crate, so removing it is a breaking change in the API sense. This
  project's own rule (RFC-036) is that a capability nothing calls is deleted rather than kept for a
  caller that may never come, and `0.x` is where that is paid for cheaply.
