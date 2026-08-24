# RFC-036: Dormant Capability Closure

Status: **Accepted by the human owner 2026-08-18.** Deletion of published API is on the table deliberately, batched into one release.
Target milestone: M12
Date: 2026-08-18

Related baseline documents:

- `tekstide-requirements-v0.md`

Depends on:

- `handoffs/reachability-audit.md` — the audit that produced the list (2026-08-17).
- [RFC-023](../done/023-configuration-system.md) — **closed 2026-08-22 without wiring
  `set_resource_limits`**, which it owned per the audit's priority 3. The decision returns
  here, conditioned; see §What is already assigned.

## Summary

Decide, per dormant capability: wire it, delete it, or record it as deliberately core-only.

## Why this is scheduled

The reachability audit found **104 of 132 candidate capabilities dormant**, of which 30 had no
caller anywhere — a floor, not a count, since 74 further call chains were never traced to
their roots. Two priority items were discharged immediately (terminal `resize`, trust
granting), and both turned out to be real user-facing defects rather than tidy-up: terminals
were permanently 24×80, and no project could ever leave Restricted Mode.

**That is the argument for this RFC.** The two items anyone looked at were both shipped
defects. The remaining ~18 orphans have not been triaged, and the audit deliberately did not
fix anything it found.

The cost of leaving them is not untidiness. It is that dormant state is **actively
corrupting**: RFC-022 found two real shipped defects the moment `open_surface` got its first
reader, because nothing audits a writer until something finally reads it.

## Scope

A decision for each orphan, from exactly three:

- **Wire it** — it represents a capability the product should have, and the absence of a
  caller is the defect.
- **Delete it** — it is dead. **This is a breaking change to a published crate**, which is
  precisely why this is an RFC and not a cleanup task.
- **Keep it, documented** — it is legitimately core-only API for a consumer that is not the
  GUI, and the audit's "dormant" finding is correct but not a defect.

The output is a table with a decision and a one-line reason per row, not a code change for
every row.

## What is already assigned, and should not be re-decided here

- **`set_resource_limits`** → **returned to this RFC 2026-08-22, conditioned.** The audit named
  it and assigned it to RFC-023, which **closed on 2026-08-22 without wiring it** — correctly, and
  for a reason this RFC must inherit rather than re-litigate: RFC-023 v1 is headless, nothing
  constructs a `ConfigStore`, so there is no runtime configuration change for a limit to come
  from. The dormancy is therefore *conditioned*, not a defect: wiring it is blocked on the same
  missing slice as everything else below, and "delete it" would delete a capability whose owner
  is scheduled, not absent. **Decide it as `keep, documented` unless that slice lands first.**
  Every limit this project tuned is
  fixed forever at its default until configuration is *reachable*, which is a later thing than
  configuration existing. **Note added 2026-08-19**: RFC-031's
  discrimination test now calls it to force a `RunLimitExceeded` refusal. That does not change
  its dormancy — the audit only ever counted production call sites — but **"delete it" is no
  longer free**, and this RFC should not treat it as dead weight.
- **`transition_change_set_review_state`** → RFC-034.
- **The `sensitive_config_changed` producer** — `AuditCoordinator::record_sensitive_config_policy_
  increase` / `_reduce` — **added here 2026-08-22, same condition.** RFC-023 built both methods,
  proved them against the real store round-trip, and proved the negative that no configuration
  *value* can reach a record (`no_config_value_can_reach_a_sensitive_config_changed_record`), then
  closed with zero production call sites for either. It is one of the two audit families still
  unwired after RFC-031, and it is now the only one whose owner is a closed RFC. Note the shape:
  this is not a missing producer, it is a **producer with no event to observe** — a sensitive
  configuration setting cannot change at runtime in an application that never loads
  configuration.
- **`to_ai_cli_profile` and `ConfigStore` itself** — **new orphans as of 2026-08-22**, created by
  RFC-023's own closure and recorded here so the count stays honest. Both are correct, both are
  tested against hostile input, neither has a production caller. They are the *reason* the three
  items above are conditioned rather than actionable, so they are not separately decidable: the
  slice that constructs a live `ConfigStore` and lets a configuration-defined profile reach
  `attempt_agent_run_launch` discharges all of them at once, and also carries RFC-023's OQ3
  first-use confirmation gate, which was deferred to exactly that slice.

  **This is the successor RFC-023's closure implies and does not schedule.** It has no number.
  Recommending one is a scheduling decision for the human owner, recorded here so that closing
  RFC-023 does not make it invisible.
- **`record_terminal_transcript_write_summary` / `record_transcript_write_summary` — one fewer
  reason to wire, as of 2026-08-19.** These set `Transcript.byte_count`, and RFC-033 PR-033-C
  needed a real retained-bytes figure for its purge confirmation. It did **not** wire them,
  and the reasoning generalises: **a tracked counter is only ever correct prospectively.**
  Wiring it would have left every transcript written before the wiring at `0`, so the dialog
  would still have lied about existing data. Reading `fs::metadata` is correct retrospectively
  too, and it is the same source `remove_transcript_file` already uses at delete time — so the
  displayed figure and the deleted figure now agree by construction. **This RFC's wire /
  delete / document decision should know that the obvious consumer went elsewhere on purpose**,
  and that "wire it" would not have served that consumer even if taken.
- **`purge_project_transcripts`** → RFC-033.
- **`close_project`** → **RFC-039, added 2026-08-24.** Missed by the reachability audit
  entirely, and the miss is instructive: the audit searched for functions with no callers, and
  found seven, but nothing searched for **actions a user cannot take**. `close_project` is
  reviewed, tested core API that no GUI code has ever called, so a user cannot close a project
  at all. Found by the owner asking why there was no button for it, not by any sweep this
  project runs.
- **`add_detected_generated_change_set`**, **`capture_agent_run_filesystem_baseline`**,
  **`apply_agent_terminal_outcome`** → discharged by `change-detection-wiring` (`0.11.0`).

The remainder — `purge_all_records`, `recover`, `set_viewport`, `set_git_summary`,
`set_warning_state`, `decide_with_edited_argv`, `shutdown`, `add_agent_run`, `add_transcript`,
`add_audit_event`, `add_approval`, `save_project_text_document`,
`apply_managed_agent_terminal_outcome`, `record_terminal_transcript_write_summary`,
`request_terminate`, and the rest — is this RFC's.

## Non-goals

- **Tracing the 74 untraced chains.** That is a second audit pass and its own unit of work.
  This RFC triages what the first pass actually confirmed.
- Deleting anything before the decision is reviewed. The audit's own instruction — *do not fix
  anything you find* — applies until this RFC replaces it.

## Decisions required

**D1 — is deletion on the table at all?** `tekstide-core` is published. Removing public items
is a breaking change and forces a minor bump under pre-1.0 SemVer. The alternative is that
the crate accumulates API nothing uses, which is its own cost — a published surface implies a
contract. **Recommend yes, deliberately**, with removals batched into one release rather than
trickled.

**D2 — what counts as "legitimately core-only"?** `tekstide-core` is described as the headless
core the GUI consumes. If nothing consumes an item and no design names a future consumer,
"core-only" is indistinguishable from "dead." Recommend requiring a *named* consumer — an RFC,
not an intention — before a row is classified this way.

**D3 — some rows are worse than dormant.** `recover` (audit-store recovery) and
`purge_all_records` are not features nobody reached; they are **recovery and data-deletion
paths that have never been exercised from the application.** If the audit store is corrupted
in production today, nothing calls the recovery this project built and tested. Recommend these
are separated from the triage and treated as their own finding.

## Risks

- **Triage becomes a rewrite.** Mitigated by the output being a decision table; anything
  requiring real design gets its own RFC rather than being absorbed.
- **Deleting something a future RFC needed.** Mitigated by D2's named-consumer rule, and by
  the git history — deletion is recoverable, whereas a wrong "keep" is invisible forever.
