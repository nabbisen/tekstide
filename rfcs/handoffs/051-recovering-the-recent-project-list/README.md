---
title: "RFC-051: Recovering the Recent-Project List — implementation handoff"
rfc: "RFC-051"
rfc_file: "../../accepted/051-recovering-the-recent-project-list.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# The reset that takes a user's trust decisions with it

Source RFC: [RFC-051](../../accepted/051-recovering-the-recent-project-list.md)

## What this is

A `recent-projects.json` that cannot be read is replaced by an empty list **saved over it**. The user
loses their projects' ids, and an id is what a trust grant is matched by — so trust cannot be
re-verified — and what a transcript directory is named after, so RFC-050 D6′'s orphaned transcripts
are this same event seen from the other side.

**Nothing is kept to recover from today.** This slice keeps one copy and uses it.

## Read these first, in this order

1. [`what-recovering-must-not-do.md`](./what-recovering-must-not-do.md) — **required before writing
   code.** Five rules; §1 is the defect itself.
2. The RFC's **"Decided on acceptance"**, especially **D2′** (when a backup may be written) and
   **D6′** (the store returns one typed outcome).
3. RFC-047's store recovery for the precedent: quarantine by rename, recover, disclose only when
   degraded.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A then B. The checklist is
[`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md); evidence goes in `qa-evidence.md`.

## Not in this slice

- **Deriving `ProjectId` from the canonical path** (D6). It would survive any reset and change what
  every existing record means.
- **Salvaging a partially parseable file** (D3).
- **Repairing transcripts a past reset already orphaned.** RFC-050 shows them and says no purge
  reaches them.
- **A backup history.** One previous-good copy.
