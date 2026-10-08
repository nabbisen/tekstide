# What recovery must not do

The risk document for RFC-027. **Rows 1–4 are data-loss or privacy rows: a defect in any of them is
worse than not shipping the feature.** Each row names the failure, not the fix, so that a reviewer
can check the failure is impossible rather than check that some particular code was written.

## §1 Data loss and false recovery

| # | It must not | Why this one |
| --- | --- | --- |
| 1 | **Write to any path inside the project.** | This is not autosave. The moment a recovery feature writes the user's own file, a bug in it destroys the thing it exists to protect. There is no path through this RFC that opens a file in the project for writing. |
| 2 | **Restore a buffer over a file that changed since the crash, without saying so.** | D5. The user's edits were made against a file that no longer exists in that form. Silently presenting them as current invites a save that overwrites whatever changed. The existing `ExternalChanged`/conflict path already handles this; use it. |
| 3 | **Restore anything without the user choosing.** | D4. Unsaved text may have been deliberately abandoned. Resurrecting it silently makes the editor disagree with the disk for a reason the user never caused and cannot remember. |
| 4 | **Claim protection it is not providing.** | D9. A buffer too large to persist, a write that failed, persistence switched off — in every case the user must be able to find out *at the time*. The worst outcome of this feature is a user who stops saving because they believe something is catching it. |

## §2 Privacy

| # | It must not | Why this one |
| --- | --- | --- |
| 5 | **Put buffer content in the audit store.** | D13. `local-data-and-privacy.md` promises that store records what happened and never content — a refused paste without the pasted bytes. One blob of buffer text in `audit_events` breaks the promise in the document that makes it. |
| 6 | **Leave a record readable by other users.** | `0600`, and the directory `0700`. This is the user's unsaved work sitting outside the directory they put it in. |
| 7 | **Outlive its reason.** | D11. Saved, closed or recovered means the record is gone. A recovery record that survives its buffer is not a cache; it is the user's text persisting after they believe they are finished with it. |
| 8 | **Escape the purge or the retained figure.** | D10, D14. Content Trust Settings does not count is content the user cannot find. Extend the existing per-project purge; do not add a second control. |
| 9 | **Ship the writing before the purge.** | D10. No slice may create user content before the thing that removes it exists. This is an ordering constraint on the work, not only on the design. |

## §3 Honesty about what was recovered

| # | It must not | Why this one |
| --- | --- | --- |
| 10 | **Imply the undo history came back.** | D3. It does not. A user who presses `Ctrl+Z` on a recovered buffer expecting their pre-crash stack must have been told. |
| 11 | **Report a crash it did not detect.** | D1, D12. Recovery data with no marker is a bug in our own cleanup. Report it as one; do not quietly treat it as a crash, which would make the bug invisible exactly when it matters. |
| 12 | **Count a clean document as persisted.** | D2. The direct repeat of RFC-065 PR-065-D, where `save_all` was described as writing N documents when it writes only the dirty ones. The count that is true here is the dirty count. |

## §4 The measurements

| # | It must not | Why this one |
| --- | --- | --- |
| 13 | **Choose the debounce window by taste.** | D7. The harness exists and has been used twice. A cadence nobody measured is a cost nobody knows. |
| 14 | **Report a cost without the document count attached.** | D8. The §4.1 pattern — a number describing something adjacent to what was measured — is the most frequent finding in this project's review history. |
| 15 | **Prove durability with a simulated crash.** | D6. A `simulate_crash()` helper proves our abstraction round-trips. It says nothing about data still sitting in the page cache. Kill a real process. |
