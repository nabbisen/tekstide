---
title: "RFC-046: Managed AgentRun Audit Trail — implementation handoff"
rfc: "RFC-046"
rfc_file: "../../accepted/046-managed-agentrun-audit-trail.md"
source_rfc_status: "Accepted 2026-09-06 — M12"
target_milestone: "M12"
created: "2026-09-06"
---

# The launch that trust exists to control is unrecorded

Source RFC: [RFC-046](../../accepted/046-managed-agentrun-audit-trail.md)

## What this is

Launching an AI CLI agent writes **no durable audit record**. Workspace trust and command approval
exist to control that action; the audit store has never heard of it.

The producer is already built. `AuditCoordinator::launch_managed_agent_run`
(`crates/tekstide-core/src/audit/integration.rs:412`) writes a real two-phase
`ManagedProcessLifecycle` trail and is proven against the real store. It has **zero production
callers**. Production runs `launch_agent_run_with_runtime` instead.

## Read these first, in this order

1. [`what-an-unrecorded-launch-means.md`](./what-an-unrecorded-launch-means.md) — **required before
   writing code.** Four rules about what this trail may and may not claim.
2. The RFC's D1–D5. Every one of them changes what the existing code does, so none is optional
   context.

## The trap this slice sets

**This looks like a wiring task and is not.** The built API encodes three decisions that are wrong
for this product as it now stands, and all three are invisible if you simply call it:

- Its `Authorized` phase is `append_required`, so **wiring it as written refuses agent runs whenever
  the audit store is degraded** — which RFC-047 D4 decided against, and which would make the notice
  PR-047-C already ships above the launch button false.
- Its return value has **no approval-endpoint field**, so adopting it verbatim silently drops what
  production registers today. `None` right now, and therefore a regression nothing would catch.
- It is named for a compatibility level it does not check.

**A slice that lands "agent launches are now audited" and gets any of those wrong has made things
worse than leaving it unrecorded**, because the trail would then exist and be trusted.

## Sequencing

**After RFC-047 PR-047-D.** PR-047-D redefines what `AuditHealth::status()` means; this slice writes
new code against that definition. Landing them in the other order means writing against a definition
about to change.

## What is not in this pack

- **Termination records.** D3 declines them and RFC-048 is reserved. A run that starts and later
  ends still produces no terminating record after this slice, deliberately.
- **New refusal records.** D5 settles the boundary: Restricted Mode's existing record stays, and
  nothing new is added.
- **Any schema or record-type change.** Everything needed exists.
