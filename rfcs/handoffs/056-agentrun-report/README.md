---
title: "RFC-056: AgentRun Report And Classification — implementation handoff"
rfc: "RFC-056"
rfc_file: "../../accepted/056-agentrun-report-and-classification.md"
source_rfc_status: "Accepted 2026-09-25 — agent remainder"
target_milestone: "agent remainder"
created: "2026-09-25"
---

# A run that still exists tomorrow

Source RFC: [RFC-056](../../accepted/056-agentrun-report-and-classification.md)

## What this is

Four slices. **A repairs a live defect in already-published artifacts and goes first and alone.**
B makes a run leave a record in the directory it already owns, C makes purge take that record with
the transcript, and D gives the run its classification, its notes and the report a person can hand on.

| | |
| --- | --- |
| Release | `0.27.0` |
| Depends on | RFC-050 (transcripts from earlier runs), RFC-033 (purge), RFC-048 (termination records) |
| Requirements | `REQ-AGENT-011`, `REQ-AGENT-015` |
| Slices | [A](./task-breakdown-pr-plan.md#pr-056-a), [B](./task-breakdown-pr-plan.md#pr-056-b), [C](./task-breakdown-pr-plan.md#pr-056-c), [D](./task-breakdown-pr-plan.md#pr-056-d) |

## Read these first, in this order

1. [The RFC](../../accepted/056-agentrun-report-and-classification.md) — the nine measurements,
   D1–D8, and *Decided on acceptance* (D9–D12).
2. [What a record and a report must not do](./what-a-record-must-not-do.md) — **the risk document.
   D2 is in it, and D2 is the one that can turn a privacy feature into a leak.**
3. [The PR plan](./task-breakdown-pr-plan.md).
4. [The acceptance checklist](./acceptance-qa-checklist.md) — write `qa-evidence.md` as you go.

## The shape, in one paragraph

A run's transcript already lives at `…/transcripts/agent-run-<uuid>/<transcript file>`, and that
`<uuid>` is the `AgentRunId` — **the only fact about a run that survives the process today**. This
RFC writes `run.json` beside the transcript, holding references and the user's own words: profile,
prompt summary, timestamps, ids, classification, notes. At project open it is read back, so the run
exists again — complete by definition, or saying it does not know its ending. Purge removes the
record and the transcript and then the directory *if it is empty*. The report is assembled from the
record on demand and written where the user asks; Tekstide never keeps a second copy.

## What is not in this slice

Persisting the rest of the domain. Resuming a run or re-attaching to a process. A machine-readable
report format. Search across runs. A thirteenth audit family (D11 — decided against, recorded as an
open question, not dropped).
