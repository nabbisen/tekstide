---
title: "RFC-046: Managed AgentRun Audit Trail — QA evidence"
rfc: "RFC-046"
rfc_file: "../../done/046-managed-agentrun-audit-trail.md"
source_rfc_status: "Implemented and closed 2026-09-10 — RFC-046 is in rfcs/done/"
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

## PR-046-C — production calls it

**The trail exists.** `attempt_agent_run_launch_with_profile_state_root_and_capture` (`shell.rs`)
now goes through `open_audit_store_recording_failure` -- the one seam every other audit-writing call
site already uses -- and, when a store comes back, calls `AuditCoordinator::launch_audited_agent_run`
instead of `AppState::launch_agent_run_with_runtime`. When the store will not even open at all
(`None`), the call falls back to `ProjectSession::launch_agent_run_with_runtime` directly --
unaudited, but the launch still proceeds, per D1.

### Substitution, not rewrite

Everything downstream of the launch call is unchanged: the pre-launch generated-change baseline
capture, its insertion keyed by `agent_run_id`, the fresh terminal-id read from the real record
rather than a cached value, `register_approval_channel`'s own call, and pane registration. Only the
launch call itself and the tuple it produces changed shape (`AuditedAgentLaunch`'s own
`agent_run_id()`/`runtime_events`/`approval_endpoint` instead of the plain tuple
`AppState::launch_agent_run_with_runtime` returned) -- confirmed by the existing, unmodified
regression tests for the baseline (`agent_run_change_baselines` keying) and for refusals
(`a_real_workspace_discovery_refusal_writes_a_real_restricted_mode_blocked_record`,
`a_restricted_mode_blocked_record_appears_only_for_workspace_discovery_refusals`) all staying green
without any change to their own code.

The one internal-consistency question this substitution raised: `launch_audited_agent_run` can
return `AuditIntegrationError::InvalidTypedContext`/`RequiredAuditUnavailable`, neither of which
`AppState::launch_agent_run_with_runtime`'s own error type could ever produce. Both are structurally
unreachable from this exact call site (the plan and project are always the same validated pair by
construction; the `Authorized` write is `append_observation`, never `append_required`, so its
own "required" failure can't arise) -- handled with a `panic!` naming the invariant, the same
`.expect()`-a-structural-guarantee convention this same function already uses for
`open_real_agent_run_state_root()`, rather than inventing a new refusal for a state that cannot
occur.

### Required tests, each read back from a real store or a real running process

- `attempt_agent_run_launch_with_profile_writes_authorized_then_started_to_a_real_store` -- a real
  launch through production's own real entry point (`attempt_agent_run_launch_with_profile`,
  unchanged), then the store is **reopened and queried** for the launched `agent_run_id`: exactly
  `Authorized` and `Started`, not asserted at the call site. **Ablated**: reverted the launch call
  to the old unaudited path -- failed, zero matching records (the store was never touched at all).
  Restored: passes.
- `attempt_agent_run_launch_with_profile_still_launches_and_registers_with_an_unopenable_store` --
  **the box that proves D1 reached production, not just the API.** EVIDENCE-2's own fixture
  (corrupted `audit.sqlite3`, `recovery` replaced with a symlink, so the store genuinely will not
  open) finally has its real home, per response 369's own note. A `Managed` profile with a
  configured approval channel proves the whole path still works end to end while unaudited: the
  real process launches (`state.terminal_panes.len() == 1`), the approval channel still registers
  (`state.approval_channels.len() == 1`), and `state.audit_health.status() == Degraded` proves the
  store genuinely never opened -- the only way to be sure nothing could have been written, rather
  than merely observing that nothing happened to be. **Ablated**: same revert as above -- failed,
  `audit_health` stayed `Healthy` (the unaudited path never touches it at all). Restored: passes.
- Generated-change baseline keying and refusal recording: covered by existing, unmodified regression
  tests (see above) -- unaffected by this substitution, confirmed rather than assumed.

### Live evidence: documented gap, per the delivery plan's bounded-evidence rule

Not captured. RFC-047 PR-047-C already spent six escalation rounds establishing that synthetic
keyboard input (`wtype`) does not reliably reach this application in this environment, closed by the
reviewer's own explicit bound ("the next request either carries the capture or records option 3 as
decided") in favor of unit-test evidence. The same environment limitation applies here -- launching
an agent run through the real GUI needs the identical `Ctrl+Alt+A` keyboard route already
established as unreliable to drive synthetically. Not re-litigated: the task breakdown's own words
are explicit that "the store read-back is the load-bearing evidence here, not a screenshot," and
that evidence is what the two tests above provide, against a real store, through production's real
entry point.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **485 + 4 + 746, fully green** every
time -- no flake this pass (two new tests).

### Documentation

- `launch_audited_agent_run`'s own doc comment (PR-046-A) states the trail's boundary; the new call
  site in `shell.rs` now points there explicitly, rather than leaving a reader of production code to
  find it only in this handoff pack.
- `crates/tekstide-core/README.md`'s own claim -- corrected once already by RFC-036, for this exact
  producer -- rewritten again: launching an AI CLI agent is recorded as of `0.16.0`, best-effort,
  never a precondition for the launch; the trail answers *launched, when, under which profile* and
  not *still running*, *how it ended*, or *why a non-Restricted-Mode refusal happened*.
- `rfcs/README.md`/`rfcs/delivery-plan.md`'s own RFC-046 rows are **not** touched here, per response
  367's explicit instruction: fold the update in when RFC-046 closes, not slice by slice.

## PR-046-C — response 371 required follow-up (R1)

**The `panic!` above was wrong, and it was the most serious finding of the RFC.** Its own comment
claimed every `InvalidTypedContext` at this call site reduces to "the plan and project it validated
against are always the same pair, by construction." That is true of only one of the three checks
inside `launch_audited_agent_run` that produce `InvalidTypedContext`. The other two have nothing to
do with plan/project consistency, and both are real, production-reachable inputs:

- **A `Plain` profile.** `Plain` is not malformed -- it is the unsupervised passthrough RFC-046
  deliberately places out of audit scope. The producer declining to audit it is not a state that
  "cannot occur"; it is the expected answer for every `Plain` launch there will ever be.
- **A profile id outside `AuditReference`'s bounded charset** (`[A-Za-z0-9-_.:]`). Unreachable today
  only because production hardcodes `claude_code_linux_default()` -- RFC-045 is reserved to make a
  configuration-supplied profile id reach exactly this path.

Both crashed the application before this fix. Proved, not reasoned about: two probe tests through
the real production entry point panicked at the exact line, both naming the plan/project-consistency
invariant that was never at stake in either case -- exactly the shape §4.1 warns about, in its most
expensive form: a fatal error naming the wrong invariant.

**The fix**: `AuditCoordinator::launch_audited_agent_run`'s own first two checks (`Plain` rejection,
`AuditReference::new(profile_id)`) are now exposed as a standalone predicate,
`tekstide_core::audit::plan_is_auditable`, so a caller can decide *before* moving `plan` into that
function which branch to take. `shell.rs`'s launch-call block now checks `plan_is_auditable(&plan)`
up front and routes a `Some(store)` with an unauditable plan to the same unaudited fallback
(`ProjectSession::launch_agent_run_with_runtime`) that a `None` store already used -- "the producer
will not audit this plan" and "the audit store will not open" are the same situation from the
caller's side. `launch_audited_agent_run` itself now calls the same predicate rather than repeating
the two checks inline, so the two cannot drift apart silently. The remaining `panic!` arm is
narrowed to the one case it was ever actually true for: a project/agent-run-id mismatch, which really
is guaranteed by construction at this call site (this same `project_id` built the request that
produced `plan`).

### Required tests, each driven through the real production entry point

- `attempt_agent_run_launch_with_profile_launches_a_plain_profile_unaudited_instead_of_panicking` --
  a `Plain` profile against a real, pinned, healthy store. Asserts the process launches
  (`state.terminal_panes.len() == 1`) and that the store, reopened and queried, has **zero** records
  for the launched `agent_run_id` -- proving the unaudited fallback was taken, not that a producer
  call merely failed to crash by accident. **Ablated**: reverted the `Some(store) if
  plan_is_auditable` guard to unconditional `Some(store)` -- panics at the exact line response 371
  named. Restored: passes.
- `attempt_agent_run_launch_with_profile_launches_a_profile_with_an_invalid_id_unaudited_instead_of_panicking`
  -- a `Supervised` profile (independent of the `Plain` check above) with an id containing a space,
  against the same kind of real, pinned, healthy store. Same assertions: process launches, zero
  matching records. **Ablated**: same revert -- panics at the same line. Restored: passes.

Both use a real, healthy, pinned store deliberately (not a degraded one), so a pass cannot be
confused with D1's own "still launches when the store won't open" property: this is a store that
opens fine and correctly declines to audit this particular plan.

### Gate

`fmt`, `clippy --workspace --all-targets -D warnings`, `git diff --check`, `rfc_docs_invariants`
(4 tests): clean. Three consecutive full-workspace runs: **487 + 4 + 746, fully green** every
time -- no flake this pass (two new tests, replacing the prior slice's 485 baseline).
