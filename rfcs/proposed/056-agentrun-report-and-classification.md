# RFC-056: AgentRun Report And Classification

Status: **Proposed 2026-09-25.** `0.27.0` in the authorised schedule. Closes `REQ-AGENT-011` (a final
report or handoff note) and `REQ-AGENT-015` (classify a run) — both of which the 2026-09-23 audit
found **recorded as implemented and never built**.

## Summary

The product's own pitch is *what did this run do, and what do I hand on*. It cannot answer either.
There is no classification field, no notes field, and no report — and underneath that, **no AgentRun
survives the process at all**: after a restart a project holds transcript files with no prompt, no
profile, no change sets and no ending. The one thing that does survive is the run's **identity**, in
the directory name `agent-run-<uuid>`.

This RFC makes a run leave a small record in the directory it already owns, so the run exists again
tomorrow; gives it the seven classifications the requirement names and a place for the user's own
notes; and assembles a report from those, on demand, for the user to keep where they choose.

## What is true today, measured

| | Measured |
| --- | --- |
| 1 | `AgentRun` (`domain/agent.rs:30`) already carries every input `REQ-AGENT-011` names but one: `profile_id`, `prompt_summary`, `full_prompt_ref`, `started_at`/`ended_at`, `transcript_ref`, `change_set_ids`, `approval_ids`, `audit_event_ids`, `artifact_refs`, `status`. **There is no classification field and no notes field** — grepped for both; neither exists anywhere in the crate. |
| 2 | **No `AgentRun` survives the process.** `load_transcripts_from_disk` (`project/session.rs:350`) reconstructs `Transcript::found_on_disk` and nothing else: a path, a modified time, a lock flag. After a restart a project has transcripts and **zero runs**. |
| 3 | **The run's identity does persist.** The directory is `agent-run-<uuid>` (`RUN_DIRECTORY_PREFIX`, `transcript/loading.rs:27`), built from the `AgentRunId` the path type carries (`transcript/path.rs:14`). `is_product_run_directory_name` parses the UUID back out. The id on disk is the only surviving fact about a run. |
| 4 | **Purge removes the transcript *file*, not the directory**: `fs::remove_file(storage_path)` (`project/session.rs:2301`), leaving a tombstone. **A second file in that directory would outlive a purge.** |
| 5 | **The loader matches one exact filename** (`run_entry.file_name() == TRANSCRIPT_FILE_NAME`) and skips everything else into `skipped_entries`/`skipped_bytes` — which feed `unclaimed_bytes` in `scan_transcript_disk_usage`. So a new file beside the transcript is never mistaken for one, **and today would be reported as unclaimed bytes**. |
| 6 | `recent-projects.json` is the precedent for a small persisted record: one file under `$XDG_STATE_HOME/tekstide`, written atomically, and a file that cannot be parsed **moved aside** rather than guessed at (`next_corrupt_path`, `project/recent/store.rs:310`). |
| 7 | `REQ-AGENT-015` names exactly seven values: coding, review, documentation, testing, refactoring, release, **custom**. |
| 8 | `REQ-AGENT-011` names four inputs: captured metadata, transcript references, changed files, **and user notes**. The fourth has never existed. |
| 9 | The audit family `transcript_purge` records a purge and its scope, **never a path or a byte count** (RFC-033) — the standing rule for what a record may hold. |

## Decisions required

**D1 — A run leaves a record on disk, in the directory it already owns.** `agent-run-<id>/run.json`,
beside the transcript. Measurements 3 and 5 are why this is the small answer rather than a new store:
the id is already the directory name, and the loader already ignores anything that is not the
transcript. No new directory, no new naming scheme, no second place to look.

**D2 — Purge takes the record with the transcript, or the purge becomes a lie.** Measured: purge
removes only the transcript file. A record naming the prompt, the changed paths and the user's own
notes, left behind after a user purged the run, is worse than not having written it. Purge removes
the record and the transcript, then the run directory **if it is empty** — never a directory it did
not find empty — and the purge dialog's counts say what will go.

**D3 — Classification is the seven values the requirement names, and nothing more.** `custom` carries
a user string, rendered through `quote_untrusted` like any other untrusted text. Not free-text tags,
not multiple classifications, not a taxonomy anyone has to maintain.

**D4 — Notes are the user's text, and nothing but the user writes them.** No agent output, no
summary Tekstide generated, ever lands in the notes field. A report may *quote* agent-derived
content, and when it does the rendering keeps it distinguishable from what a human wrote — because
the one thing a handoff note is for is telling a reader which sentences a person stands behind.

**D5 — The report is generated, never stored a second time.** It is assembled on demand from the
record plus whatever transcript and change sets still exist, and written where the user asks. Tekstide
keeps exactly one copy of the facts — the record — so purge has exactly one thing to remove. The
export is the user's file, outside the product's reach, and **the product says so at the moment of
export** rather than implying it can take it back.

**D6 — A run restored from a record is a record, not a running thing.** It is complete by definition:
it carries its ending, or it says it does not know one (a run whose process was killed with the app
has no ending, and inventing one would be the `unknown`-versus-zero defect RFC-053 fixed). No
lifecycle resumes, nothing is re-attached to a process. This also repairs what RFC-050 left: a
restored transcript stops being a nameless file and gets its prompt and profile back.

**D7 — A record that cannot be read is moved aside and named.** `recent-projects.json`'s precedent
exactly (measurement 6). A run whose record is corrupt appears as it does today — a transcript with
no run — never as a run with invented fields, and the Project Board says one was set aside.

**D8 — The record holds references and the user's own words, not content.** Ids, timestamps, the
prompt summary, the classification, the notes. **Bounded**: a cap on the notes, and on how many
change-set, approval and audit ids a record carries, so a run directory cannot grow without limit and
a hostile or runaway run cannot turn the state directory into a heap. The transcript remains the only
place run *content* lives, under its own existing budget.

## Non-goals

Persisting the rest of the domain. Resuming a run, or re-attaching to its process. A report format
anyone else must parse — this is a document for a person. Editing a finished run's metadata beyond
its classification and notes. Search across runs. Anything the adapter would have to produce:
`REQ-AGENT-011` says *generated from captured metadata*, and every input is already captured.

## Risks

**R1 — a new file where a loader is already looking.** Measured as safe today (the loader matches one
exact name), but the same measurement says an **older Tekstide counts the record as unclaimed bytes**.
This version must count it as its own, and the disk-usage figure must not call the product's own
record unclaimed.

**R2 — D2 widens a delete.** Removing a directory is the most dangerous thing in this RFC. It happens
only when the directory is empty after both known files are gone, and the test for it is the one that
plants an extra file and asserts the directory survives.

**R3 — the report mixes three kinds of text.** The user's notes, the project's paths, and
agent-derived content. Every one of the last two goes through `quote_untrusted`, and D4's distinction
has to survive into the exported file, not only the on-screen view.

**R4 — version skew in the record.** A record written by a newer Tekstide and read by an older one,
and the reverse. It needs a version field from the first release and a rule for an unknown one, or
D7's "moved aside" will fire on every future upgrade.

**R5 — a partial write.** The app can be killed while the record is being written. Atomic write, as
`recent-projects.json` already does; a half-written record is D7's case, not a crash.

**R6 — exporting moves project paths outside the project.** A report names changed files. The export
is the user's decision and the product states what the file will contain before writing it.

## Acceptance criteria

- A run classified and annotated, then the app closed and reopened: **the run is there**, with its
  prompt, profile, classification, notes and ending, and its transcript attached to it rather than
  orphaned. Captured live.
- A restored run whose process was killed with the app **says it does not know its ending** — not a
  guessed one, and not zero.
- Purge removes the record and the transcript, and leaves the directory only when it is empty. **The
  ablation plants a third file in the run directory and asserts the directory survives.**
- After purge, nothing on disk names the prompt, the changed paths or the notes of the purged run —
  asserted by searching the state directory, not by reading the code.
- The disk-usage figure counts the record as the product's own, not as unclaimed bytes (measurement 5).
- A corrupt record is moved aside, named on the Project Board, and the run appears as a transcript
  with no run — never as a run with invented fields. Ablated.
- A record with an unknown version is D7's case and does not crash, and a record this version wrote
  is readable by the rule this version states (R4).
- The seven classifications, and `custom` carrying a user string that is quoted on render and in the
  exported file — including a string containing a bidi control and a newline.
- The exported report distinguishes the user's notes from agent-derived content, in the file, not
  only on screen.
- The record's caps hold: a run with more change sets, approvals and audit events than the cap, and an
  over-long note, produce a bounded record and say they were bounded.
- `REQ-AGENT-011` and `REQ-AGENT-015` move to implemented **with evidence a user can reach both**, not
  merely that the fields exist.
- Gate green three times with `--no-fail-fast`, and **0 fixture entries left in a fresh short
  `TMPDIR`**.
- **Carried from the `0.26.0` post-publish finding, and first in the slice order:**
  `tekstide-core = { version = "0.27.0", path = "crates/tekstide-core" }` in `[workspace.dependencies]`,
  plus two post-publish gate steps — install this release into a temporary root, and assert the app
  archive's lockfile names the matching core.
