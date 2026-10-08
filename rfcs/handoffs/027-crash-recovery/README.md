---
title: "RFC-027 handoff: crash recovery and unsaved buffer persistence"
status: "**Accepted 2026-10-08**, D1–D11 as written plus D12–D15. `0.31.0`, M13. Three slices; A is a prerequisite for B and B for C."
rfc_file: "../../accepted/027-crash-recovery-and-unsaved-buffer-persistence.md"
target_milestone: "M13"
created: "2026-10-08"
---

# RFC-027 handoff

## Read these first, in this order

1. The RFC itself — especially *Decided on acceptance*, which closes three design questions you
   would otherwise hit mid-slice.
2. [`what-recovery-must-not-do.md`](./what-recovery-must-not-do.md) — the risk document. **Read it
   before writing the first line of the store.** Four of its rows are data-loss or privacy rows.
3. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) — the three slices and their order.
4. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) — what each slice must prove.

## The one-paragraph version

The editor can hold twenty documents with unsaved edits, all in memory. If the process dies, all of
it is gone. This RFC writes a side record of each **dirty** buffer so a restart can offer it back.
It never writes the user's own file — that would be autosave, which is a different feature with a
different failure mode.

## What this work actually is

**It is mostly a privacy design.** Unsaved buffer text is the user's content, and it goes outside
their project directory. The only other content Tekstide stores — AgentRun transcripts — has a
capture control, a *Retained locally* figure and a purge, and that is the template. Everything else
under `~/.local/state/tekstide/` is metadata, and `local-data-and-privacy.md` says so in a sentence
this work makes false.

**The editor half is small.** Dirty text, a cursor, a viewport and the `FileSnapshot` the buffer was
based on, written on a debounce; a marker that says the last session did not exit cleanly; and an
offer on restart. The existing external-change machinery already knows how to say "the file changed
underneath you" — D5 reuses it rather than inventing a third conflict word.

## Where the substrate already is

| What you need | Where it already exists |
| --- | --- |
| The state directory, with `XDG_STATE_HOME` and a `HOME` fallback | `project/recent/store.rs` |
| A content-retention precedent: capture, retained figure, purge | AgentRun transcripts; Trust Settings |
| "The file changed under you" states | `TextDocumentState`, `ExternalChanged`, the conflict path |
| The snapshot type to compare against disk | `FileSnapshot`, already stored per document |
| A paired-control latency harness | `shell/tests/editor_baseline.rs` (RFC-026, reused by RFC-065 D7/D13) |
| A real-process liveness check | `test_support` — **test code.** D12 needs this in production; promote it or write it, but do not import test support into the product |

## The two measurements most likely to be got wrong

**The cadence (D7)** is not "pick 500 ms and move on". Use the paired harness and carry the control
in the same run, the way RFC-065 D13 did.

**The per-document cost (D8)** must be reported per unit. A figure for "persistence" without the
dirty-document count attached is the §4.1 pattern, and it is the single most common finding in this
project's review history.
