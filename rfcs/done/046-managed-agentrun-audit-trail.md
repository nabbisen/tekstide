# RFC-046: Managed AgentRun Audit Trail

Status: **Implemented and closed 2026-09-10.** Three slices (PR-046-A through C), reviewed and
accepted across responses 367–372. Production now calls the audited launch producer this RFC's own
D1–D5 shaped: `attempt_agent_run_launch_with_profile_state_root_and_capture` opens the audit store
through the seam RFC-047 established, and calls `AuditCoordinator::launch_audited_agent_run` when a
store comes back — falling back to the pre-existing unaudited launch when it does not, or when the
plan itself is out of audit scope (a `Plain` profile, or a profile id the store's own bounded
charset cannot represent). **Found during review, response 371**: the first cut of that fallback
crashed the application on exactly those two inputs, via a `panic!` whose own comment named a
different invariant (the plan/project pair, always consistent by construction) than the one that
actually failed — `tekstide_core::audit::plan_is_auditable` now exposes the producer's own gate so
the caller can route around it before the plan is consumed, rather than discovering the mismatch
from the error the producer returns. Original:
**Accepted by the human owner 2026-09-06.** **D1–D5 decided by the architect on acceptance** — see
"Decided on acceptance" at the end, which settles D4's actual name and adds D5, a boundary found
while deciding. Proposed the same day. Reserved 2026-08-28 by RFC-036's triage, which found the
defect by counting production callers rather than reading documentation. Authored after RFC-047
closed its last decision, because RFC-047 changed one of this RFC's answers before it was written —
see D1.
Target milestone: **M12**
Date: 2026-09-06

Related RFCs:

- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — owns the audit store, the
  `ManagedProcessLifecycle` family, and the two-phase `operation_id` discipline. **All of it
  exists.**
- [RFC-036](../done/036-dormant-capability-closure.md) — found this: `launch_managed_agent_run` and
  its siblings write real records, are proven against the real store, and have **zero production
  callers**. The crate README claimed otherwise until 2026-08-27.
- [RFC-047](../done/047-audit-store-corruption-recovery.md) — decided what the product does when the audit
  store cannot record. **D1 below is a direct consequence, and it overturns what the existing code
  does.**
- [RFC-021/022](../done/022-adapter-spawn-and-command-approval-surface.md) — own the approval
  endpoint that D2 exists to protect.
- [RFC-048](../future-work.md) — reserved, not authored. D3 below declines to record how a run
  *ends*; this is where that would go, once the three existing terminating paths are inventoried.

## Why

Launching an AI CLI agent is the action that workspace trust and command approval exist to control.
**It leaves no durable audit record.** Not "an incomplete one" — none.

The machinery to record it is written, tested against the real store, and reachable by nothing.
`AuditCoordinator::launch_managed_agent_run` (`audit/integration.rs:412`) implements a careful
two-phase shape: an `Authorized` record before the launch, then `Started` or `Failed` after, tied by
one `operation_id`, with `AuditStore` rejecting any second phase whose first phase was never
persisted (`MissingAuthorization`).

Production does not call it. `Message::LaunchAgentRunButtonPressed` →
`launch_agent_run_in_active_project` → `attempt_agent_run_launch_with_profile_state_root_and_capture`
→ `state.app_shell.state_mut().launch_agent_run_with_runtime(plan, &mut runtime)`. That path is the
whole of it, and it writes nothing to the audit store.

**This is not a wiring task, which is why it is an RFC.** Reading the two paths against each other
turns up three questions the existing code has already answered — and, in one case, answered in a
way this project has since decided against.

## What is already built

- The `ManagedProcessLifecycle` family, its validation (`record.rs:236`), and its schema-level
  two-phase enforcement (`store.rs:424`).
- `launch_managed_agent_run`, which orchestrates `prepare_agent_run_launch` →
  `Authorized` → `launch_prepared_agent_run_with_runtime` → `Started`/`Failed`.
- `AuditedAgentLaunch`, its return value.
- Tests proving all of the above against a real store.

Nothing in this RFC needs new record types or a schema change.

## Decisions

### D1 — The audit record must **not** be a precondition for launching. The existing code says it is.

`launch_managed_agent_run` writes its `Authorized` phase with **`append_required`**, which returns
`Err(RequiredAuditUnavailable)` when the store will not accept the write. The launch then never
happens. Wiring this API as written would mean: **audit store degraded → agent runs refused.**

RFC-047 D4 decided precisely against that, and the reasoning transfers without modification:

> Restricted Mode refuses actions whose *danger* it cannot bound. A broken audit store does not make
> an agent run more dangerous — it makes it unrecorded.

Worse, it would make a sentence we shipped three days ago false. PR-047-C renders, above a live
launch button, *"This run will not be recorded while the audit store is degraded."* That promises the
run proceeds unrecorded. Wiring `append_required` would refuse the run instead — the notice would be
describing an outcome the product no longer produces.

**Decided: the `Authorized` phase becomes best-effort (`append_observation`), and a launch never
depends on the audit store.** The degradation is disclosed (RFC-047 D3 and D4 already do this) and
the run proceeds.

**This is a deliberate weakening of a property the existing code holds, so state what is lost.** With
`append_required`, an `Authorized` record exists for every launch that happened. With
`append_observation`, a launch can occur with no record at all. What survives is the two-phase
*integrity* guarantee — the store still rejects a `Started` whose `Authorized` was never persisted,
so the trail can be **absent** but never **inconsistent**. A reader can distinguish "no record of
this run" from "a run that claims to have been authorised and was not."

`record_safe_close_authorized` already made this exact call for the same reason and documents it.
This is that precedent, not a new one.

### D2 — The approval endpoint must survive. Adopting the API verbatim would drop it.

The two paths return different shapes:

- Production's `launch_agent_run_with_runtime` returns `(agent_run_id, events, approval_endpoint)`.
- `launch_prepared_agent_run_with_runtime`, which the audited path calls, returns
  `(agent_run_id, events)`. `AuditedAgentLaunch` (`integration.rs:207`) has **no endpoint field**.

So swapping production onto `launch_managed_agent_run` as it stands would silently discard the
endpoint. Production registers it via `register_approval_channel`, and its own comment records that
this endpoint *"used to be silently dropped one layer down"* — response 227's found defect.

It is `None` today, because `claude_code_linux_default()` is `Supervised` and Supervised never binds
one. **That is exactly what makes this dangerous**: the regression would be invisible at the moment
it is introduced and would surface only when the first `Managed` profile ships, as a security
control that silently does not exist.

**Decided: `AuditedAgentLaunch` carries the approval endpoint, and the audited path returns
everything the current path returns.** No caller loses a value by becoming audited. A test must fail
if the endpoint is dropped — and it must be written so that it fails *today*, with `None`, rather
than waiting for a `Managed` profile to give it something to hold.

### D3 — Record the launch, not the run's whole life. `Started` is not `Finished`.

The family is named `ManagedProcessLifecycle`, and the built API records three points: `Authorized`,
`Started`, `Failed`. **A run that starts and later exits produces no terminating record.**

Two ways to read that gap. Either the family is misnamed for what it records, or it is missing its
final phase. **Decided: this RFC records the launch and does not add a termination record.**

The reason is that termination is not one fact here. A run can end by the agent exiting, by the user
closing the project (RFC-039's safe-close path, which already writes its own records per terminal),
or by the process being killed during RFC-043's session-scoped termination. Those are three
different events with three existing owners, and inventing a fourth record that tries to cover all
of them would produce a trail that is wrong in a way nobody notices — the failure mode RFC-047 §1
names.

**What this RFC owes instead is honesty about the boundary**: the audit trail answers *"was an agent
run launched in this project, when, under which adapter profile"* — and does not answer *"is it
still running"* or *"how did it end."* That belongs in its own RFC, with the three existing
terminating paths inventoried first. Reserve it; do not smuggle it in here.

### D4 — Name the API for what it audits, not for a compatibility level it does not check.

`launch_managed_agent_run` rejects only `AgentCompatibilityLevel::Plain`. It accepts `Supervised` and
`Managed` alike — and production's only profile today is **`Supervised`**. So the name says
`managed` while the behaviour is "anything but Plain," and the first consumer will be a Supervised
run.

That mismatch is how a future reader concludes Supervised runs are unaudited and writes a second
producer. **Decided: rename to `launch_audited_agent_run`** — settled on acceptance, not left as "or equivalent"; an implementer must not inherit a naming decision. Keep
the `Plain` rejection explicit and commented — Plain runs are the unsupervised passthrough and
deliberately out of scope.

Naming is not cosmetic here: RFC-036 exists because a documented claim diverged from what the code
did, and this is the same divergence one layer down.

## What this RFC must not become

- **A termination-recording RFC.** D3 draws that line deliberately. Three existing owners must be
  inventoried before anything writes a fourth record.
- **A refusal mechanism.** D1 settles it. If implementation finds a reason to refuse a launch on
  audit grounds, that reopens RFC-047 D4 and is an architect decision, not an implementation one.
- **A schema change.** Everything needed exists.

## Risks

- **Weakening `append_required` to `append_observation` is a real reduction**, taken deliberately and
  named in D1. The mitigation is that inconsistency remains impossible even when absence is possible.
- **D2's regression is invisible by construction.** A test that only exercises today's `None` proves
  little unless it asserts the endpoint is *carried*, not merely that the launch succeeds.
- **This RFC gives the audit store its first mandatory-feeling producer on the hottest path.** RFC-047
  PR-047-D is landing changes to `AuditHealth` on that same path; sequence this after it, or the two
  slices will collide in `open_audit_store_recording_failure`'s callers.

## Decided on acceptance (2026-09-06)

### D5 — Record what the launch did. Do **not** add refusal records here.

Found while deciding, by reading the production refusal path rather than the happy path.
`attempt_agent_run_launch` can refuse for four reasons — `Validation`, `RunLimitExceeded`,
`PlanTransition`, `Runtime` — and exactly **one** shape is audited today:
`record_restricted_mode_blocked_if_applicable` records `WorkspaceDiscoveryBlocked`, the Restricted
Mode case, and returns early for everything else.

That asymmetry is correct and stays. **The distinction is whether a control refused, or a condition
failed.**

- **Restricted Mode blocking a launch is a security control doing its job**, and the record is the
  evidence it did. Already recorded; unchanged by this RFC.
- **`RunLimitExceeded` is a limit the user configured for themselves.** There is no third-party
  accountability question in a user's own limit refusing the user's own action.
- **`Validation` and `PlanTransition` are internal-state conditions**, not decisions about the user.
  Recording them would fill the trail with events that carry no accountability meaning — RFC-034's
  disclosure-density problem, moved into the audit store where it is harder to see.

**So this RFC's boundary is: a launch that reaches the runtime gets `Authorized` → `Started` or
`Failed`. A launch refused before that point adds no new record.** Named here rather than left for
an implementer to infer from which branch happens to have a `record_*` call in it.

If run-limit refusal should be recorded, that is a decision about RFC-023's resource limits and
belongs with them, not smuggled in behind an audit-trail slice.

### Reserved: RFC-048, AgentRun Termination Records

D3 declines to record how a run *ends* and says the successor needs the three existing terminating
paths inventoried first. **Number reserved now** — RFC-024 was authored just-in-time into a number
the delivery plan had already reserved, and the reserved-numbers table exists because of it. A
reservation is not a commitment to build.

### Sequencing, restated

**After RFC-047 PR-047-D.** Both change behaviour around `AuditHealth` on the same path, and
PR-047-D redefines what `status()` means. Landing this first would mean writing the audited launch
path against a definition about to change under it.
