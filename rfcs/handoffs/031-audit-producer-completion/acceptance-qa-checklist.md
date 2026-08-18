---
title: "RFC-031 acceptance and QA checklist"
status: "Complete 2026-08-19 — RFC-031 closed"
rfc_file: "../../done/031-audit-producer-completion.md"
target_milestone: "M11"
created: "2026-08-19"
---

# RFC-031 — acceptance and QA checklist

## Producers

```text
[x] restricted_mode_blocked has an AuditCoordinator producer, shaped like record_paste_blocked.
    (record_restricted_mode_blocked / restricted_mode_blocked_record, audit/integration.rs)
[x] project_added has one.
    (record_project_added / project_added_record, audit/integration.rs)
[x] Neither sets subject_ref. Asserted as None, not merely unset by inspection.
    (restricted_mode_blocked_persists_a_valid_record_conforming_to_the_frozen_family,
    project_added_persists_a_valid_record_conforming_to_the_frozen_family, both core)
[x] reason_code RestrictedMode used for the block; its coarseness recorded.
    (restricted_mode_blocked_record; doc comment on
    record_restricted_mode_blocked_if_applicable, shell.rs, states the one-code coarseness)
[x] A failed observation does not break the action being observed.
    (both call sites use `if let Some(store) = audit_store.as_mut() { let _ = ...; }` --
    a missing store, or a failed append, is silently swallowed; matches record_paste_blocked)
```

## Reachability

```text
[x] The restricted-mode record is produced from a real Ctrl+Alt+A key press on an
    untrusted project, through update, not a dispatched AppCommand.
    (a_real_workspace_discovery_refusal_writes_a_real_restricted_mode_blocked_record, shell/tests.rs)
[x] The project_added record is produced from the real open path.
    (opening_a_real_new_project_from_the_cli_path_writes_exactly_one_real_project_added_record,
    tests.rs -- against open_cli_project_path_and_record, the function boot()'s real
    CLI-argument loop calls, not a direct AppState call)
[x] Restore-vs-add is settled, stated, and asserted whichever way it went.
    See "Correction" below -- restoring produces no record, and never reaches the add
    path at all. Asserted directly:
    restoring_recent_projects_on_boot_writes_no_project_added_record (tests.rs).
```

**Correction to this pack's own task breakdown.** `task-breakdown-pr-plan.md`'s PR-031-B
section states the trigger is "reached when a project is opened from the CLI or restored
from recent projects" -- this is wrong on the second half. Direct reading of
`AppState::restore_recent_projects` (`app.rs:41`) shows it only builds the passive
`recent_projects: Vec<RestoredRecentProject>` list; it never calls `add_project_session`
or constructs a live `ProjectSession`. The only production caller of `add_project_from_path`
in the shipped GUI is `boot()`'s CLI-argument loop (`main.rs`) -- there is no interactive
"Add Project" GUI flow yet. So restore-vs-add was not a design choice between two reachable
call sites; only one call site reaches `add_project_session` at all, and the producer is
called from that one, gated on `AddProjectOutcome::Added` (not `FocusedExisting`, which
means an already-open project was merely re-focused). Left uncorrected in the task
breakdown itself, per this project's evidence-correction convention -- flagged here rather
than silently rewritten there.

## Discrimination — the tests that fail if someone "improves" the producer later

```text
[x] A record appears for WorkspaceDiscoveryBlocked and does NOT appear for
    RunLimitExceeded or ExecutableUnavailable. Both directions asserted.
    (a_restricted_mode_blocked_record_appears_only_for_workspace_discovery_refusals, shell/tests.rs)
[x] subject_ref is None on both record types, asserted directly.
    (restricted_mode_blocked_persists_a_valid_record_conforming_to_the_frozen_family,
    project_added_persists_a_valid_record_conforming_to_the_frozen_family, both core;
    also re-asserted surface-side in
    opening_a_real_new_project_from_the_cli_path_writes_exactly_one_real_project_added_record
    and a_real_workspace_discovery_refusal_writes_a_real_restricted_mode_blocked_record)
```

## Public statement

```text
[x] README.md's audit paragraph narrowed to what is still unwired.
    (README.md, "Durable audit currently records..." paragraph)
[x] crates/tekstide-core/README.md's equivalent paragraph narrowed.
[x] Neither implies the store is viewable. It is not.
```

## Claims that must not be made

```text
[x] Not claimed: that a user can see these events. Nothing renders the store.
[x] Not claimed: that the store distinguishes which restricted feature was blocked.
    (README.md and crates/tekstide-core/README.md both describe the addition as
    "restricted-feature refusals," not naming a specific feature; reason_code stays
    the single RestrictedMode value)
[x] Not claimed: that safe_close_decision is wired. It is out of scope and unreachable.
```

## Final Acceptance Decision

- [x] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes: Accepted 2026-08-19 (requests 263/264/265, responses in
`.git-exclude/reviewed/`). Suite re-run by the reviewer: **922 passed, 0 failed**.

Both producers landed with their discrimination asserted in both directions, and the
implementer added schema-boundary ablations nobody asked for — proving the chosen `outcome` is
the only one RFC-013's validation accepts, which turns a judgement call into a constrained one.

Two corrections went the other way, from the implementer to me:

1. **This pack's task breakdown claimed restoring recent projects reaches
   `add_project_session`.** It does not. The restore-vs-add "decision" I posed did not exist.
   Corrected in `task-breakdown-pr-plan.md` at closeout, with the original left visible.
2. **The enumeration test I required was the wrong shape**, and I pointed at a precedent
   without saying which unit the property needed. A file-level allow-list is right for
   "only the reviewed reader reads"; an occurrence count is required for "every call site must
   also audit." Corrected across two rounds, and the rule is now in `ARCHITECTURE.md` so the
   next one does not need this exchange.

Known limitation, stated and not claimed away: one `RestrictedMode` reason code cannot
distinguish which of RFC-004's nine restricted features blocked a launch, and finer
granularity would be a frozen-schema change. Nothing renders the audit store; recording an
event does not make it visible.
