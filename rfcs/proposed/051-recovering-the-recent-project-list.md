# RFC-051: Recovering the Recent-Project List

Status: **Proposed 2026-09-16.** Scoped at the owner's word, to ship with RFC-048 in `0.20.0`. Found
by the architect at RFC-050's acceptance (D6′) and carried in `future-work.md` since, with the trigger
*"any slice that touches `RecentProjectStore`"* — this is that slice.
Target milestone: **M12**
Date: 2026-09-16

Related RFCs:

- [RFC-032](../done/032-workspace-trust-persistence.md) — persists the trust decision this loses.
- [RFC-047](../done/047-audit-store-corruption-recovery.md) — the precedent: quarantine by rename,
  recover, and disclose at the point of the action.
- [RFC-050](../done/050-transcripts-from-earlier-runs.md) — **D6′** measured the consequence: a reset
  gives every project a new id, so its transcripts belong to nothing.
- [RFC-031](../done/031-audit-producer-completion.md) — `project_added` records an id and, by rule,
  **no path**. That is why the audit store cannot rebuild this list.

## Summary

When `recent-projects.json` cannot be read, Tekstide starts with an empty list **and then saves that
empty list over the file**. The user loses their projects' identities, and with them their trust
decisions and the ownership of their transcripts. Nothing is kept to recover from.

## What is true today, measured

- **An unparseable file is quarantined; an unreadable one is not.** `RecentProjectStore::load`
  renames a file that fails to parse to `recent-projects.json.corrupt` and reports where it went. A
  file that fails to *read* — a permission error, an I/O error, a directory in its place — is **not
  renamed**.
- **`boot()` then saves the empty list over it.** The save is a temp-file write followed by
  `fs::rename`, so in the unreadable case **the only copy is destroyed**. In the parse case the
  quarantine survives, and nothing reads it afterwards.
- **No previous-good copy exists anywhere.** One live file, no backup, no history.
- **What the file holds, and so what is lost**: each project's `project_id`, display name, root path,
  canonical root path, its timestamps, the display-only trust hint, and the per-project
  transcript-capture opt-out.
- **Trust is keyed by project id.** `AuditStore::has_applied_trust_grant(project_id)` is what
  `verify_restored_trust` checks. A project that reopens under a new id **cannot** match its own
  earlier grant, however intact the audit store is.
- **The audit store cannot rebuild the list.** `project_added_record` carries the project id and no
  path — deliberately, and that rule is not up for revision here. Ids without paths cannot restore a
  mapping.
- **The consequence is already disclosed, not fixed.** RFC-050 PR-050-C's board notice says the list
  was reset, where the unreadable file went, and that earlier transcripts remain on disk.

## Decisions required

**D1 — Never overwrite a state file we could not read.** Recommended: quarantine it by rename first,
exactly as the parse failure does, and if the rename fails, **do not save at all**. Keeping the
user's unreadable file is worth more than keeping our empty list; a save that destroys the only copy
is the defect, not the symptom.

**D2 — Keep one previous-good copy, and recover from it.** Recommended: write
`recent-projects.json.bak` on each successful save, and when the live file is unusable, restore from
the backup rather than starting empty. **This is what turns a reset into a repair**, and it is the
whole of the user-visible value.

**D3 — Recovery is whole-file, never salvage.** Recommended: no partial JSON surgery on a corrupt
file. A half-parsed list can resurrect a **wrong** path-to-id mapping, and a wrong id is worse than a
missing one: it re-attaches somebody's trust grant and somebody's transcripts to the wrong folder.
Whole or nothing.

**D4 — A recovered list is verified, never trusted.** `verify_restored_trust` already re-checks each
restored trusted project against the audit store and demotes what it cannot find. Recovery restores
ids and paths; **it must not restore trust by itself**, and the existing check is what enforces that.

**D5 — The board says which of the two happened.** Extend RFC-050 PR-050-C's notice: recovered from
the previous copy, or reset with nothing to recover — and in the second case, what that means for
transcripts. **Open:** whether a recovery that succeeds needs to say anything at all, or whether a
silent repair is the better outcome once trust is re-verified.

**D6 — Deriving `ProjectId` from the canonical path is out of scope.** It would make ids survive any
reset, and it changes what a project id *means* for every record already written. If D2 proves
insufficient, that is its own RFC with a migration.

## Scope

`RecentProjectStore`'s load, save and recovery; `boot()`'s ordering; the board notice's second case;
and the documentation sentence in the book that currently says a reset loses trust decisions.

## Non-goals

- **Recovering anything from the audit store.** It has ids and no paths, by RFC-031's rule.
- **Changing project identity** (D6), or the trust model.
- **A general backup history.** One previous-good copy, not N.
- **Repairing the transcripts a past reset already orphaned.** RFC-050 shows those bytes and says no
  purge reaches them; this RFC stops the next reset from creating more.

## Risks

- **A backup can be written from a state we should not have saved** — for example after a reset that
  this RFC failed to prevent, overwriting a good backup with an empty list. **The backup must be
  written from a load that succeeded**, never from a session that started empty.
- **Two files double the failure surface.** The recovery path must itself be tested against an
  unreadable backup.
- **A recovered project whose folder is gone** must behave exactly as a stale recent entry does today,
  which the availability check already handles.

## Acceptance criteria

- A corrupt live file with a good backup **recovers the list**, and the projects keep their ids.
- **An unreadable live file is never overwritten**, and when it cannot be quarantined, nothing is
  saved.
- A recovered project that the audit store has no grant for is **demoted**, proving recovery did not
  restore trust by itself.
- **Nothing is recovered from a partially parseable file** (D3), asserted.
- The board says which case happened, and says nothing when the list loaded normally.
- A backup is never written from a session that started with an empty list after a failed load.
