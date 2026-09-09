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

## PR-046-A — response 367 required follow-up (R1)

Response 367 identified the same defect shape named in §4.1, in test-naming form: the one test
above carried two separate properties (launch succeeds unconditionally; no `Started` write is
attempted once `Authorized` fails to persist), and ablating either one failed the *same* test under
the *other* one's name — a future reader who broke the §3 consistency guarantee would have been told
about launch unconditionality instead.

**Split into two tests**, and it turned out the split was not purely mechanical: `launched.
audit_status` and `health.status()` are themselves entangled with whether the `Started` write gets
attempted. With `RecordingWriter::fail_on(1)`, a wrongly-attempted `Started` write is attempt 2,
which *succeeds* — flipping `audit_status` to `Persisted` and clearing `write_status` back to
`Healthy` via `clear_write_failure()`. Asserting either of those in the "launch still succeeds" test
would have kept the same coupling under a different name. Resolved by moving both assertions to the
§3 test, where they now correctly serve as *additional* symptoms of that specific property breaking,
and leaving the "launch succeeds" test with only genuinely independent signals: the real process
exists, and `failure_count` (monotonic, never cleared by any success) recorded the failure.

- `managed_launch_still_creates_a_process_when_authorization_cannot_persist` — now asserts only:
  `project.agent_runs()`/`terminal_sessions()` non-empty, `health.failure_count() == 1`.
- `no_started_write_is_attempted_when_its_own_authorized_did_not_persist` (new) — asserts
  `writer.attempt_count == 1`, `writer.records.is_empty()`, `launched.audit_status == Degraded`,
  `health.status() == Degraded`.

**Ablated both directions, run by me:**

- Reverting `append_observation` → `append_required` (D1's own revert) fails both tests, as
  expected — they share the same precondition (the launch must succeed at all before either
  property can be checked), so this ablation cannot discriminate between them and was never claimed
  to.
- Forcing the `Started` write unconditionally (`if authorization_persisted` → `if true`, the
  ablation that motivated the split) now fails **only** `no_started_write_is_attempted_when_its_own_
  authorized_did_not_persist` — `managed_launch_still_creates_a_process_when_authorization_cannot_
  persist` stays green. Confirmed the entanglement is fully resolved, not merely hidden.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **483 + 4 + 744, fully green** every
time -- no flake this pass (one new test).

## PR-046-B — nothing is lost by becoming audited

**Still no production caller.** D2 only.

### The endpoint is carried, not silently dropped

`AuditedAgentLaunch` gains `pub approval_endpoint: Option<ApprovalChannelEndpoint>`, and
`launch_audited_agent_run` now captures `prepare_agent_run_launch`'s own return value (previously
discarded outright -- `project.prepare_agent_run_launch(&mut plan).map_err(...)?;`, the exact
silent-drop the pack's own trap warns about) and threads it into the returned value. Matches what
`launch_agent_run_with_runtime` already returns today and what `register_approval_channel`
(`tekstide` crate) already consumes by value.

**`ApprovalChannelEndpoint` holds a live `UnixListener`**, so it implements neither `Clone` nor
`PartialEq`/`Eq`. `AuditedAgentLaunch` drops those three derives (`Debug` only now) -- checked, not
assumed, that nothing outside this producer's own test suite relied on comparing or cloning a value
of this type; grepped the tree for every use first.

### Required test: reading the field is what makes it real

`managed_launch_carries_the_approval_endpoint_field_through`: asserts `launched.value.
approval_endpoint.is_none()` against a real launch through a real `Supervised` plan -- today's real
value, per §4's own instruction not to invent a `Managed`-profile test that "cannot be written at
all yet and will be forgotten."

**The bar the task breakdown set**: "fails if the field is deleted", not merely "passes because the
launch succeeded." Reading `launched.value.approval_endpoint` in the assertion is what gives this
test that property -- it is a **compile-time** dependency, not a runtime one. **Ablated by literally
deleting the field** (removed `approval_endpoint` from the struct and from the constructing struct
literal, restoring the pre-D2 discard): the whole crate failed to build --
`error[E0609]: no field approval_endpoint on type AuditedAgentLaunch` at this test's own assertion,
plus `error[E0560]` at the struct literal that used to set it. A test that only checked the launch
succeeded would have kept compiling and passing with the field entirely gone, which is exactly the
silent-loss failure mode §4 describes. Restored: builds and passes again.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **483 + 4 + 745, fully green** every
time -- no flake this pass (one new test).

## PR-046-B — response 369 required follow-up (R1)

§4 claimed a test asserting a `Managed` launch's endpoint survives "cannot be written at all yet and
will be forgotten." The reviewer checked that claim by writing the test themselves against `fff3712`
and found it false; corrected §4 and the checklist (`71cdcda`, `a4ca959`) with the working recipe.

**Why the `Supervised`-only test wasn't enough**: `None` is both the correct answer for `Supervised`
and the answer a silently-broken path would also produce (hardcoded `None`, or the endpoint dropped
anywhere between `prepare_agent_run_launch` and the returned struct -- response 227's own historical
defect, "silently dropped one layer down"). A test that can only see `None` cannot tell those apart.

### The `Managed` test, following §4's recipe exactly

`managed_launch_carries_a_real_bound_approval_endpoint`: `AgentCompatibilityLevel::Managed`,
`profile.adapter_capabilities.structured_action_approval = true` (otherwise validation returns
`ManagedCapabilityMissing`), `.with_approval_channel(<short path>)`. The approval channel's state
root becomes part of a real Unix domain socket path, bound by the kernel's ~108-byte `sun_path`
limit -- `TestAuditDirs`' own base overflows it (`Bind(SocketPathTooLong)`, a fixture-length failure
that reads like a bug in the code under test); used `$TMPDIR/tk<pid>` instead, noted in the test's
own doc comment for the next person who binds a real channel in a test.

Renamed the sibling test to `supervised_launch_does_not_bind_an_approval_endpoint` -- its old name,
`managed_launch_carries_the_approval_endpoint_field_through`, no longer fit once a genuine `Managed`
sibling existed, and now correctly describes what it alone proves (a narrower, but still real,
property: `Supervised` must never bind a channel).

**Ablated**: hardcoded `approval_endpoint: None` in the returned struct literal (the "silently
dropped in the plumbing" defect this test exists to catch) -- failed only `managed_launch_carries_
a_real_bound_approval_endpoint`; `supervised_launch_does_not_bind_an_approval_endpoint` stayed
green, correctly, since `None` is also its own real answer. Restored: both pass. Field-deletion
ablation (from response 369's own review) still holds: either test reading the field means deleting
it fails the whole crate's build, not just one test.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **483 + 4 + 746, fully green** every
time -- no flake this pass (one new test, one renamed).
