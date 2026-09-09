---
title: "RFC-046: Managed AgentRun Audit Trail — QA evidence"
rfc: "RFC-046"
rfc_file: "../../accepted/046-managed-agentrun-audit-trail.md"
source_rfc_status: "Accepted 2026-09-06 — M12"
target_milestone: "M12"
created: "2026-09-09"
---

# Evidence

## PR-046-A — make the producer safe to call

**No new behaviour visible to a user. The producer still has zero production callers after this
slice** — PR-046-C is what makes the trail exist.

### D1: the launch no longer depends on the audit store

`launch_managed_agent_run`'s `Authorized` phase moved from `append_required` to
`append_observation` (`integration.rs`). The function tracks whether that write actually persisted
(`authorization_persisted: bool`, read from the real `AuditObservationStatus` the write returns, not
assumed) and uses it to decide **only** whether the later `Failed`/`Started` write is attempted —
never whether the launch itself proceeds. `project.launch_prepared_agent_run_with_runtime(...)` runs
unconditionally.

### §3: absent is permitted, inconsistent is not — enforced in the caller, not only the schema

Both the `Failed` write (on a launch that then fails) and the `Started` write (on one that succeeds)
are gated on `authorization_persisted`. `AuditStore`'s own `MissingAuthorization` check
(`store.rs:424`) would reject either write anyway if attempted without a persisted `Authorized`, but
this producer does not rely on that as the mechanism — it skips the attempt, so the schema check is a
backstop that should never actually fire in practice, matching the risk document's own instruction
("the answer is to skip the `Started` record too — not to weaken the store's check").

### D4: renamed, no alias

`launch_managed_agent_run` → `launch_audited_agent_run`. Grepped the whole tree for the old name
after the rename: the only two remaining hits are historical, in a doc comment on a *different*
method (`record_safe_close_authorized`) that cited the old name and one now-stale claim
("`append_required` for its first phase") in the same comment — both fixed, since a stale
cross-reference here is exactly RFC-036's own defect shape one level down. `Plain` rejection is
unchanged (still the first check, before any write is attempted) and its own comment states why:
the unsupervised passthrough, deliberately out of scope, since nothing this function could write
about a `Plain` run's own I/O would be trustworthy when that I/O is itself unenforced.

### §5: the trail's own boundary, in its own doc comment

Added verbatim to `launch_audited_agent_run`'s doc comment: what the trail answers (*"was an agent
run launched in this project, when, and under which adapter profile"*) and what it does not (still
running, how it ended, or a refusal beyond the one Restricted Mode case), with the reason each
boundary exists (D3: three separate terminating owners exist and RFC-048 is reserved, not authored;
D5: `RunLimitExceeded`/validation/plan-transition refusals never reach this function at all).

### Required tests, each ablated

- **`managed_launch_still_creates_a_process_when_authorization_cannot_persist`** — renamed from
  `managed_launch_does_not_create_process_when_authorization_cannot_persist`, whose old name
  described exactly the behaviour D1 removes. Against a real `AuditCoordinator` with a controlled
  write-failure injection (`RecordingWriter::fail_on(1)`, this file's own established technique for
  "what happens when a write fails" — `termination_truth_survives_observational_audit_failure` and
  `orphaned_runtime_truth_is_not_mislabeled_as_durable_termination` already use it the same way):
  the `Authorized` write fails, and the launch **still succeeds** — a real process exists, the
  returned `audit_status` is `Degraded`, exactly one write was attempted (not two: the `Started`
  write was correctly never tried), and `AuditHealth` still records the failure as session history.
  **Ablated exactly as the task breakdown specifies**: reverted `append_observation` back to
  `append_required` on the `Authorized` write — the test failed immediately, `Err(RequiredAuditUnavailable(..))`
  returned before the launch ever ran. Restored: passes.

  *A note on "do not mock":* the task breakdown asks for this test against a real degraded store via
  PR-047-B's fixtures rather than a mock. Those fixtures (`corrupt_and_interrupt_recovery_for_test`,
  the symlinked `recovery` directory) produce a store that **will not open at all** — the right shape
  for testing what happens before a store exists, not for testing a write failing on an
  already-open one, which is what this function's own `Authorized` write needs. Used
  `RecordingWriter` instead — a real `AuditRecordWriter` implementation (the same trait `AuditStore`
  itself implements), not a mock of `AuditCoordinator`'s own logic, and the technique this exact file
  already uses for every other "a write fails mid-launch" scenario. PR-046-C's own required "same
  launch with a degraded store" test (not yet built) is where a genuinely unopenable store belongs,
  since that test exercises the real `tekstide` shell seam end to end, where PR-047-B's fixtures are
  the natural, already-proven tool. Flagged here for the reviewer to confirm or correct.

- **`managed_launch_failure_is_recorded_after_authorization_without_process_attachment`** (existing,
  renamed only) — authorization persists, the launch itself then fails for an unrelated reason
  (missing executable); `Failed` is recorded, tied to the same `operation_id` as `Authorized`.
  Unaffected by D1's own change, since the audit write itself never fails in this scenario.

- **`plain_agent_launch_is_not_relabelled_as_durably_authorized`** (existing, renamed only) — `Plain`
  is rejected before any write is attempted (`writer.attempt_count == 0`), unaffected by D1.

- **`managed_launch_persists_authorized_started_and_terminated_runtime_truth`** (existing, renamed
  only) — the full happy path against a real, healthy `AuditStore`: `Authorized`, `Started`, and
  (via a later call) `Terminated` all persist and read back correctly. Unaffected by D1's own change
  since every write succeeds in this scenario; still the one test proving the whole trail's shape
  end to end against a real store.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **483 + 4 + 743, fully green** every
time -- no flake this pass (no new test count; one test renamed, five renamed only).
