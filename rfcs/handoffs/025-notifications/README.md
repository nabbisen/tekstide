---
title: "RFC-025: Notifications — implementation handoff"
rfc: "RFC-025"
rfc_file: "../../accepted/025-notifications.md"
source_rfc_status: "Accepted 2026-09-22 — M12"
target_milestone: "M12"
created: "2026-09-22"
---

# One model, before a fifth mechanism arrives

Source RFC: [RFC-025](../../accepted/025-notifications.md)

## What this is

The requirements define a `Notification`. None exists. **Four RFCs each added their own notice to
the project board** — audit health, configuration diagnostics, the recent-list reset or recovery, and
retention cleanup — and each decided locally how long its notice lives. This slice gives them one
model and adds what REQ-NOTIFY-002 and 003 ask of the status bar.

**It is a refactor with a surface on top.** The risk is not what it adds; it is what a migration can
silently change.

## Read these first

1. [`what-a-notification-must-not-do.md`](./what-a-notification-must-not-do.md) — **required before
   writing code.**
2. The RFC's **"Decided on acceptance"**, especially **D1** (two lifetimes, and why not three).
3. The four existing producers in `shell.rs`, and the tests that hold them absent when their fact is
   false. Those tests are the acceptance evidence for the migration.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A the model and the migration, B the
status bar. Checklist: [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## Not in this slice

- **Acknowledgement**, and the third lifetime that would need it (D1).
- **Producing Git state** — RFC-030. The status bar says "not available" until it lands.
- **Desktop notifications, sound, or history.**
