---
title: "RFC-056 — what a record and a report must not do"
rfc: "RFC-056"
rfc_file: "../../done/056-agentrun-report-and-classification.md"
source_rfc_status: "Implemented and closed 2026-09-25 — agent remainder"
target_milestone: "agent remainder"
created: "2026-09-25"
---

# What a record and a report must not do

This slice writes a new file, on disk, holding what a person wrote and what an agent touched. Four
ways that goes wrong. The first is the one that matters.

## 1. It must not survive a purge

Measured: `purge_transcript_at` calls `fs::remove_file(storage_path)` — **the transcript file, and
nothing else**. The run directory stays. So a `run.json` naming the prompt summary, the changed
paths and the user's own notes would sit there after a user purged the run, and the purge dialog
would have told them it was gone.

That is not a bug in this slice. It is this slice **turning an existing privacy feature into a
leak**, and it is the reason D2 exists.

What purge must do:

| Step | Rule |
| --- | --- |
| The transcript file | removed, as today |
| `run.json` | removed, in the same operation |
| The run directory | removed **only if it is then empty** — never a directory found non-empty |
| The counts the dialog shows | include the record's bytes, so the dialog does not promise less than it removes |

The test that proves it is not "purge removes the record". It is: **plant a third file in the run
directory, purge, and assert the directory and that file both survive.** Anything that removes a
directory it did not find empty is the defect, however good the reason looked.

And afterwards: **search the state directory** for the purged run's prompt, paths and notes, and find
nothing. Assert it by looking at the disk, not by reading the code that was supposed to do it.

## 2. It must not say it knows an ending it does not

A run whose process was killed with the app has no ending. The record says so — `Unknown`, and the
restored run carries it. Not the start time, not zero, not the moment the record was read.

This is the inverse of RFC-053's defect: there, `unknown` was printed where zero was known. Here zero
is *not* known and the temptation runs the other way, because a report with a blank where a duration
should be looks unfinished. It is not unfinished; it is honest, and the rendering says which.

## 3. It must not let the notes and the agent's words look alike

A handoff note exists so a reader knows **which sentences a person stands behind**. The report may
quote agent-derived content and it must name project paths, and both go through `quote_untrusted` —
but escaping is not the point here. The point is that the distinction between *the user wrote this*
and *this came out of a run* survives into the **exported file**, not only the on-screen view, where
a reader with no Tekstide open will read it.

A `custom` classification is the user's string too, and it is quoted like any other untrusted text —
including when it contains a bidi control or a newline.

## 4. It must not grow without a bound

The record is references and the user's words, never content: ids, timestamps, the prompt summary,
the classification, the notes. Caps on the notes length and on how many change-set, approval and
audit ids a record carries, **and the record says when it was bounded** rather than silently
truncating — the same rule the explorer's omitted-entry row follows. The transcript stays the only
place run content lives, under its own budget.

## And two smaller ones

**An older Tekstide counts the record as unclaimed bytes.** Measured: the loader matches one exact
filename and skips everything else into `skipped_entries`/`skipped_bytes`, which feed
`unclaimed_bytes` in `scan_transcript_disk_usage`. That is why the record is safe to add — nothing
mistakes it for a transcript — and also why **this** version must count it as the product's own, or
the disk-usage figure will call Tekstide's own file unclaimed.

**A record that cannot be read is moved aside and named**, exactly as `recent-projects.json` does
(`next_corrupt_path`). The run then appears as it does today — a transcript with no run — never as a
run with invented fields. A record carrying an unknown version is that same case, not a crash, and
without a version field from the very first release this rule has nothing to test.
