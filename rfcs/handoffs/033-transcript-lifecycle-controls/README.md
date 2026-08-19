---
title: "RFC-033: Transcript Lifecycle Controls — Developer Handoff Pack"
status: "Complete 2026-08-19 — RFC-033 closed, moved to rfcs/done/"
rfc_file: "../../done/033-transcript-lifecycle-controls.md"
target_milestone: "M11"
created: "2026-08-19"
---

# RFC-033: Transcript Lifecycle Controls — Developer Handoff Pack

**This closes a limitation the product published.** `0.11.1` shipped this sentence onto the
crates.io page, as part of correcting a privacy claim that had been wrong for two releases:

> There is no in-app way to turn capture off or to purge it. To remove transcripts today,
> delete the `transcripts/` directory.

That is accurate and it is not an acceptable resting state. Every AI CLI run writes that
session's terminal output to disk — including whatever the CLI quoted from the user's files —
and the only remedy on offer is a filesystem operation performed outside the application.

## Read in this order

1. **[`what-purge-must-remove.md`](./what-purge-must-remove.md)** — required first. Short, and
   the only document here describing an irreversible operation.
2. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md)
3. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md)
4. [`qa-evidence.md`](./qa-evidence.md) — fill as you go.

## Four things that are binding

1. **A prerequisite lands first, and it is not optional.** `without_transcript_capture()` sets
   `transcript_state_root = None`, and a `Managed` profile's approval channel **falls back to
   that same field**. So opting out of capture would break approval-channel binding for
   `Managed` runs. The GUI never sets `approval_state_root`; it must, before the opt-out
   exists. See the task breakdown's PR-033-A.
2. **Almost everything is already built.** `without_transcript_capture`,
   `purge_project_transcripts`, `purge_transcript`, and `transcript_local_data_summary` all
   exist, are tested, and are on the reachability audit's orphan list. This is routes and
   decisions, not a model.
3. **Capture stays on by default.** The owner decided 2026-08-18 that capture is intended.
   This RFC gives a user control over it; it does not revisit the default. A *configurable*
   default is RFC-023's.
4. **The `transcript_purge` audit family has no producer**, and this RFC is the one that gives
   it one. RFC-013 froze the schema; if the family does not fit, that is a finding to report,
   not a field to add.

## What "done" looks like

A user can decline capture for a project before running an agent, can delete transcripts from
inside the application, and can see how much is retained — and `README.md`'s *Local Data and
Privacy* section loses the sentence quoted at the top of this file.
