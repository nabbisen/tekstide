---
title: "RFC-050: Transcripts From Earlier Runs — implementation handoff"
rfc: "RFC-050"
rfc_file: "../../accepted/050-transcripts-from-earlier-runs.md"
source_rfc_status: "Accepted 2026-09-13 — M12"
target_milestone: "M12"
created: "2026-09-13"
---

# Load what is on disk, and delete nothing the loader did not recognise

Source RFC: [RFC-050](../../accepted/050-transcripts-from-earlier-runs.md)

## What this is

A `ProjectSession` knows only the transcripts launched since the project was opened in the current
process. So RFC-033's purge and the retained-size figures have covered nothing from earlier runs
since `0.12.0`, while the purge dialog said they did. RFC-049's retention would inherit the same
blindness. This slice loads records from disk at project open, so the existing purge, figures and
retention act on what exists.

It **widens what purge and retention can reach.** Everything it loads becomes deletable, so the
rules for what may be loaded are the security surface.

## Read these first, in this order

1. [`what-loading-a-transcript-must-not-do.md`](./what-loading-a-transcript-must-not-do.md) —
   **required before writing code.**
2. The RFC, especially **"Decided on acceptance"**. D6′ corrects the proposal's premise, and the
   details section settles lock failure, the probe, and the state-root seam.
3. RFC-049's [`what-deleting-a-transcript-must-not-do.md`](../049-transcript-retention-enforcement/what-deleting-a-transcript-must-not-do.md)
   §2–§3, and RFC-033's [`what-purge-must-remove.md`](../033-transcript-lifecycle-controls/what-purge-must-remove.md).
   This slice extends both.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A → B → C. **Nothing new is deletable
until B.** The checklist is [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md). Evidence
goes in `qa-evidence.md`, written by the implementer.

## Sequencing

- **Ahead of RFC-049 PR-049-C's paused parts**, which resume on loaded records.
- **After the purge-defect disclosure commit and PR-DOC-C**, per the documentation handoff. D8 then
  removes that disclosure from the trimmed README, in the commit that makes purge true.

## Not in this slice

- A persisted transcript index; restoring agent runs or terminals; in-app deletion of unclaimed
  directories; coordination between instances beyond the writer's lock; platforms other than Linux.
- **Repairing a corrupt `recent-projects.json`**, or restoring the trust decisions it loses. Reserved
  in `future-work.md`. This slice makes the reset **visible**: it shows the unclaimed bytes and, on the
  start it happens, a board notice (the owner's decision, 2026-09-13).
