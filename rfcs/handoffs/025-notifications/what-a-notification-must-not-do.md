---
title: "What a notification must not do"
rfc: "RFC-025"
rfc_file: "../../done/025-notifications.md"
source_rfc_status: "Implemented and closed 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# What a notification must not do

**Required reading before writing code.**

## §1 A migrated notice keeps its lifetime exactly

Each of the four says something true for a bounded time: audit health **while degraded**; the
recent-list reset or recovery **on the start it happened**; retention cleanup **until the next
cleanup**. A migration that lengthens one leaves a stale claim on screen; one that shortens it hides
a real state.

**Their existing tests are the proof, and they must pass unmodified.** If a test needs editing to
accommodate the model, the model is wrong — say so rather than editing the test.

## §2 Absent when false, always

Every one of the four is absent when its fact is false, and that is the only rule all four already
share. RFC-047 D3: a surface that always says something stops being read. **A notification with no
condition is not a notification.**

## §3 No lifetime outside the two

*While the condition holds* and *for the start it happened*. **Not "until acknowledged"** — nothing
acknowledges anything yet, and a lifetime nothing produces is a dormant capability (D1). The type
must make a third impossible, not merely unused.

## §4 A count is not a state

REQ-NOTIFY-003: `2 running`, `1 awaiting approval`, `1 failed` — never a bare number. The counts
exist in `ProjectRuntimeSummary`; **read them, do not recount**, and do not invent a count the
summary does not have.

## §5 Text first, colour never alone

REQ-NOTIFY-005. Every state reads as a word. Colour may reinforce it and may not carry it.

## §6 Nothing leaves the window

No desktop notification, no sound, no file. A notification that leaves the application is a consent
question this RFC has not asked.
