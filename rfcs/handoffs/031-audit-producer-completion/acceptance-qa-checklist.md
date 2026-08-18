---
title: "RFC-031 acceptance and QA checklist"
status: "Open"
rfc_file: "../../proposed/031-audit-producer-completion.md"
target_milestone: "M11"
created: "2026-08-18"
---

# RFC-031 — acceptance and QA checklist

## Producers

```text
[ ] restricted_mode_blocked has an AuditCoordinator producer, shaped like record_paste_blocked.
[ ] project_added has one.
[ ] Neither sets subject_ref. Asserted as None, not merely unset by inspection.
[ ] reason_code RestrictedMode used for the block; its coarseness recorded.
[ ] A failed observation does not break the action being observed.
```

## Reachability

```text
[ ] The restricted-mode record is produced from a real Ctrl+Alt+A key press on an
    untrusted project, through update, not a dispatched AppCommand.
[ ] The project_added record is produced from the real open path.
[ ] Restore-vs-add is settled, stated, and asserted whichever way it went.
```

## Discrimination — the tests that fail if someone "improves" the producer later

```text
[ ] A record appears for WorkspaceDiscoveryBlocked and does NOT appear for
    RunLimitExceeded or ExecutableUnavailable. Both directions asserted.
[ ] subject_ref is None on both record types, asserted directly.
```

## Public statement

```text
[ ] README.md's audit paragraph narrowed to what is still unwired.
[ ] crates/tekstide-core/README.md's equivalent paragraph narrowed.
[ ] Neither implies the store is viewable. It is not.
```

## Claims that must not be made

```text
[ ] Not claimed: that a user can see these events. Nothing renders the store.
[ ] Not claimed: that the store distinguishes which restricted feature was blocked.
[ ] Not claimed: that safe_close_decision is wired. It is out of scope and unreachable.
```

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
