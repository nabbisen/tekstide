---
title: "RFC-048: AgentRun Termination Records — implementation handoff"
rfc: "RFC-048"
rfc_file: "../../accepted/048-agentrun-termination-records.md"
source_rfc_status: "Accepted 2026-09-16 — M12"
target_milestone: "M12"
created: "2026-09-16"
---

# The ending the trail cannot state

Source RFC: [RFC-048](../../accepted/048-agentrun-termination-records.md)

## What this is

The audit trail says an AI CLI run was launched. It cannot say whether the run ended, or how. The
record for it **already exists in the schema and nothing writes it**: `valid_managed_process` permits
a `Terminated` phase, and the store admits one only after a `started` phase for the same operation
id. This slice writes the producer and gives it one call path.

**It is small, and one decision in it is not.** A **detached** run gets no record, because Tekstide
cannot observe an ending after it loses supervision. Read D2 before you write anything.

## Read these first, in this order

1. [`what-a-termination-record-must-not-claim.md`](./what-a-termination-record-must-not-claim.md) —
   **required before writing code.** Four rules; §1 is the one this RFC exists to hold.
2. The RFC's **"Decided on acceptance"**, D1–D6.
3. `record_plain_terminal_terminated` in `audit/integration.rs` — the precedent for every choice
   here, including the silence on an orphaned process.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A then B. The checklist is
[`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md); evidence goes in `qa-evidence.md`,
written by the implementer.

## Not in this slice

- **Any schema change.** If something here seems to need one, it is the wrong thing.
- **A GUI surface for how a run ended.** The AgentRun Report may say it later; the durable record is
  what nothing writes today.
- **Changing termination itself** — RFC-043 owns that — or recording detachment as an event, which
  the family's four outcomes cannot express.
