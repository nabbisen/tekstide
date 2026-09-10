---
title: "What an agent-run audit trail must not claim"
rfc: "RFC-046"
rfc_file: "../../done/046-managed-agentrun-audit-trail.md"
source_rfc_status: "Implemented and closed 2026-09-10 — RFC-046 is in rfcs/done/"
target_milestone: "M12"
created: "2026-09-06"
---

# What an agent-run audit trail must not claim

**Required reading before writing code.** This slice creates the record of the single most
consequential action the product takes. Every failure mode below is a false statement about
accountability.

## §1 An absent record and a wrong record are not the same failure

Today the trail is **absent**: no record, and `RFC-036` corrected a README that said otherwise. That
is a real defect, and it is an *honest* one — nobody reading the audit store concludes a launch did
not happen, because the store says nothing about launches at all.

After this slice the store speaks. From that moment a reader will draw conclusions from what it
says, including from what it does not say. **A trail that is present and wrong is worse than one
that is absent**, because absence is visible and error is not.

This governs every decision below. When in doubt, record less and state the boundary, rather than
recording more and hoping the reader interprets it correctly.

## §2 A launch must never depend on the audit store — and D1 is where that is easy to undo

RFC-047 D4: *Restricted Mode refuses actions whose danger it cannot bound. A broken audit store does
not make an agent run more dangerous — it makes it unrecorded.*

The built producer disagrees. Its `Authorized` phase is `append_required`, which fails the whole
call when the store will not write. **Changing it to `append_observation` is D1's entire mechanical
content, and it is one word.** That is exactly why it needs a test: a one-word change is a one-word
revert, and nothing else in the codebase would notice.

**The test must be that a launch succeeds when the authorization write does not persist.**

*Corrected 2026-09-09, response 367.* This originally said "driven through a real degraded store…
PR-047-B's own fixtures", and **that instruction could not be followed.** `AuditCoordinator::new`
takes an already-open `&mut AuditStore`; PR-047-B's fixtures produce a store that will not open at
all, so no coordinator can be built around one and the fixture cannot reach this code. Drive it
through `with_writer`, the module's own `pub(crate)` injection seam, with a writer that fails the
attempt — that is the real trait `AuditStore` implements, not a mock of the logic under test.

**The unopenable-store fixture belongs at PR-046-C**, where the failure genuinely is the *open*, the
path runs through `open_audit_store_recording_failure`, and "not a mock" is about production wiring.

And it must fail if `append_required` comes back. A test that only asserts "a record was written on
the happy path" passes unchanged when the store is degraded, because on the happy path it is not.

## §3 Absent is permitted. Inconsistent is not.

D1 trades a guarantee away, and the trade is only defensible because of what survives it.

- **Gone:** every launch that happened has an `Authorized` record.
- **Kept:** no `Started` record can exist whose `Authorized` was never persisted. `AuditStore`
  enforces this at the schema level (`store.rs:424`, `MissingAuthorization`).

So a reader can distinguish *"no record of this run"* from *"a run claiming an authorisation that
never happened."* **That second state must remain unreachable.** If an implementation finds itself
writing a `Started` record on a path where the `Authorized` write was skipped or failed, it has
broken the one property D1 kept, and the answer is to skip the `Started` record too — not to
weaken the store's check.

## §4 A security control that is `None` today is still a security control

`AuditedAgentLaunch` has no approval-endpoint field. Production's current path returns one and
registers it through `register_approval_channel`. Its own comment records that this endpoint *"used
to be silently dropped one layer down"* — a defect found at response 227 and fixed.

It is `None` in *production* today, because the only shipped profile is `Supervised`.

*Corrected 2026-09-10, response 369.* This said a `Managed`-profile test "cannot be written at all
yet". **That is false — I asserted a limitation instead of checking it, and the weaker test that
resulted cannot fail for the failure mode this section exists to prevent.** A Supervised launch
returns `None` whether the endpoint is carried, hardcoded `None`, or dropped in the plumbing; only
*deleting the field* breaks such a test, and deletion is not what happened at response 227.

**A `Managed` launch binding a real endpoint is writable today**, verified by running it:

1. build the plan at `AgentCompatibilityLevel::Managed` (`launch_plan_for_level` already
   parameterises this);
2. set `profile.adapter_capabilities.structured_action_approval = true`, or validation returns
   `ManagedCapabilityMissing`;
3. pass a **short** approval state root — the socket path has a ~108-byte `sun_path` limit and
   `TestAuditDirs`' own base overflows it, failing as `Bind(SocketPathTooLong)`, which reads like a
   bug in the code under test rather than in the fixture.

**Assert `Some`.** That fails on all three losses. Keep a Supervised `None` test alongside it —
Supervised must not bind a channel, and both branches are reachable.

The failure this prevents is not a crash. It is command approval quietly not existing for the first
`Managed` profile that ships, months from now, with nothing in the diff that introduced it.

## §6 "This cannot happen" is a claim, and a `panic!` is how it gets tested in front of a user

Added 2026-09-10, response 371, after PR-046-C put a `panic!` on the agent-launch path.

`launch_audited_agent_run` returns `InvalidTypedContext` for **three** different reasons, and only
one of them is a plan/project mismatch. The other two are ordinary inputs:

- **`AgentCompatibilityLevel::Plain`** — not a malformed state. It is the unsupervised passthrough
  this RFC deliberately excludes from auditing (D4). The producer is saying *"I decline to audit
  this"*, not *"you have handed me something impossible."*
- **A profile id outside `[A-Za-z0-9-_.:]`** — `AuditReference::new` rejects it. Profile ids are
  `AiCliProfile` data, and RFC-023's configuration system exists to supply that data from a user.

Both were proven to crash the application through the real production entry point, not argued about.

**The rule this leaves:**

1. **A producer declining to audit a plan is not an error the caller cannot handle.** It is the same
   situation as the store failing to open, and it has the same answer, which D1 already decided:
   **launch, unaudited.** Never refuse, and never crash.
2. **A `panic!` justified by an invariant must be justified by the invariant that actually fails.**
   Both crashes above asserted "the plan and project are always mutually consistent by construction"
   — a claim not at stake in either case, which would have sent a debugger to the wrong file.
3. **Precedent is not a justification.** This one cited `open_real_agent_run_state_root()`'s
   `.expect()`. That one is genuinely structural and its comment proves it. Citing a sound
   `.expect()` nearby does not make a new one sound.

This is §4.1 at its most expensive: not a sentence on a board overstating damage, but a fatal error
naming the wrong cause.

### §6.1 The residual, recorded rather than fixed (2026-09-10, response 372)

R1 removed both *input*-reachable panics. What remains is narrowed to genuine
"the code is wrong" conditions, which is a defensible panic and matches this project's `.expect()`
convention. **Two things about it are still true and should not be rediscovered.**

`launch_audited_agent_run` can return `Err` from two places that sit **after**
`launch_prepared_agent_run_with_runtime` has returned `Ok`: the `launched_agent_run_id != agent_run_id`
comparison, and the terminal-id lookup's own `.ok_or(InvalidTypedContext)`. The call site's comment
names only the *pre-launch* id check as "the one way left in."

**By then the process is running and `plan` has been moved.** The caller has no recovery: falling
back would double-launch (and will not compile), and refusing would be a lie, because the run exists.
A panic there kills the application and orphans a spawned process — the outcome RFC-043 exists to
prevent.

**The fix, if it is ever wanted, is producer-side and is a change to the return contract, not a
slice of this RFC:** `launch_audited_agent_run` must not return `Err` once a process exists. It
should return the launch with a degraded audit status, the same way every other best-effort producer
here reports a write it could not make.

Left deliberately: both conditions are internal-consistency violations rather than ordinary inputs,
which is exactly what separates them from `Plain` and an invalid profile id — those were *valid
data*, and valid data must never reach a `panic!`.

## §5 Say what the trail answers, in the trail's own documentation

After this slice the audit trail answers:

> *Was an agent run launched in this project, when, and under which adapter profile.*

It does **not** answer *is it still running*, *how did it end*, or *why was a launch refused* beyond
the one Restricted Mode case that already exists. D3 and D5 decided both boundaries deliberately.

**Write those two sentences into the producer's own doc comment.** RFC-036 exists because a
documented claim diverged from what the code did; the correction is not to document less but to
document the boundary where a reader would otherwise assume more.
