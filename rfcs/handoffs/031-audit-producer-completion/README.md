---
title: "RFC-031: Audit Producer Completion — Developer Handoff Pack"
status: "Ready for implementation — accepted by the owner 2026-08-18"
rfc_file: "../../proposed/031-audit-producer-completion.md"
target_milestone: "M11"
created: "2026-08-18"
---

# RFC-031: Audit Producer Completion — Developer Handoff Pack

**The last M11 item.** Two audit families that can fire today and record nothing.

## Read in this order

1. **[`what-the-store-may-hold.md`](./what-the-store-may-hold.md)** — required first. It is
   short, and it is the only document here that can cause a durable, un-erasable mistake.
2. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) — two slices, independent.
3. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md)
4. [`qa-evidence.md`](./qa-evidence.md) — fill as you go.

## What this is, in one paragraph

`RestrictedModeBlocked` and `ProjectAdded` are two of RFC-013's twelve frozen audit families.
Both have **live triggers in the shipped product** — pressing `Ctrl+Alt+A` on an untrusted
project refuses, and opening a project adds one — and **neither has a producer**, so neither
has ever written a record. `REQ-SEC-014` is the requirement, and every release since `0.1.0`
has shipped a public statement that these are "defined in the audit schema but not yet
wired."

## Three things that are binding

1. **This is not a wiring slice.** `record_paste_blocked` and its siblings exist as
   `AuditCoordinator` methods; these two have **no equivalent**. Both the producer and its
   call site are the work. Do not go looking for a `record_*` to call.
2. **`safe_close_decision` is out of scope**, though the reservation's title names it. Its
   surface does not exist — `OpenSafeCloseDialog` maps to `None`. Building a producer for an
   event nothing can cause is the zero-reachable-surface failure one layer down.
3. **The schema is frozen.** If a family does not fit what you need to record, that is a
   finding to report, not a field to add.

## What "done" looks like

A user who presses `Ctrl+Alt+A` on an untrusted project, and a user who opens a project, each
cause exactly one durable record — proven from a real key press, not a dispatched command —
and both READMEs' public "not yet wired" statement is narrowed to what is still true.
