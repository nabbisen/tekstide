---
title: "What recovering the recent-project list must not do"
rfc: "RFC-051"
rfc_file: "../../accepted/051-recovering-the-recent-project-list.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# What recovering the recent-project list must not do

**Required reading before writing code.** This file is about a user's trust decisions. The failure
that matters is not failing to recover — it is destroying what recovery needed, or handing back
something that looks recovered and is wrong.

## §1 Never overwrite a file we could not read

A file that fails to **parse** is quarantined today. A file that fails to **read** is not — and
`boot()` then saves an empty list over it. **That save is the defect.**

Quarantine first, by rename, as the parse path does. **If the rename fails, save nothing at all**:
the user's unreadable file is worth more than our empty list, and a session can run perfectly well
without persisting one.

## §2 A backup is only ever written from a list that loaded

If a session starts empty because the load failed, **its first save must not become the backup**.
That would overwrite the only good copy with the empty list — the same destruction as §1, one file
further along. The store knows whether the live file loaded; that fact gates the backup write, and a
test holds it.

## §3 Whole file or nothing

**No partial JSON surgery.** A half-parsed list can resurrect a wrong path-to-id mapping, and a wrong
id is worse than a missing one: it re-attaches somebody's trust grant and somebody's transcripts to
the wrong folder. Recovery reads a whole backup or reports that there was none.

## §4 Recovery restores ids and paths; it never restores trust

`verify_restored_trust` re-checks every restored trusted project against the audit store and demotes
what it cannot find. **That check stays exactly where it is**, and a test proves a recovered project
with no grant is demoted. A recovery that handed back trust by itself would make this RFC a
vulnerability rather than a repair.

## §5 The user is told which of the two happened

**Recovered** and **reset with nothing to recover** are different sentences (D5). One line, on that
start only. A user who sees the reset line must not have to guess whether their projects came back.

## §6 Ordering is the whole defect

**Quarantine, then recover, then save.** A save that runs before a recovery attempt destroys what the
recovery needed. The store returns one typed outcome (D6′) so that no caller can sequence these
wrongly — if the shell has three steps to get right, this will regress.
