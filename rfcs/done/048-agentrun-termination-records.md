# RFC-048: AgentRun Termination Records

Status: **Implemented and closed 2026-09-16.** The trail now states how an agent run ended —
`ProcessExited`, `ProcessTerminated`, or `RuntimeFailure` after a start — and states nothing for a
run Tekstide stopped supervising. Two premises in the handoff pack were wrong and the implementer
corrected both: the producer already existed with only test callers, and production dropped the
launch value that carried the operation id. Accepted by the human owner 2026-09-16. **D1–D6 decided by the architect on acceptance** — see the end. Proposed the same day. Scoped at the owner's word after `0.19.0` shipped. Reserved by
RFC-046 D3, which recorded a launch and deliberately declined to record how a run ends.
Target milestone: **M12**
Date: 2026-09-16

Related RFCs:

- [RFC-046](../done/046-managed-agentrun-audit-trail.md) — records the launch. Its D3 reserved this
  number and named the condition: the terminating paths must be inventoried first.
- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — owns the record schema, which
  already has the slot this RFC fills.
- [RFC-043](../done/043-terminal-process-containment.md) — owns termination itself, including the
  close flow that kills a run's process.
- [RFC-047](../done/047-audit-store-corruption-recovery.md) — D4's rule that a degraded store makes an
  action unrecorded, never refused. Unchanged here.

## Summary

The audit trail answers *was an AI CLI run launched, when, and under which profile*. It cannot answer
*did it end, and how* — the record exists in the schema, and nothing writes it.

## What is true today, measured

Every line below was read in the code, not inferred from an earlier RFC.

- **`ProjectSession::apply_agent_terminal_outcome` maps five runtime outcomes to four statuses** —
  `Exited` to `Completed` or `Failed` by exit status, `TerminatedBySignal` and `KilledAfterTimeout` to
  `Cancelled`, `OrphanedUnknown` to `Detached`, `Failed` to `Failed` — and **calls no audit producer
  at all.**
- **Plain terminals already record their ending.** `record_plain_terminal_terminated` writes
  `Terminated` with `ProcessExited` or `ProcessTerminated`, and **returns `NotRequired` for
  `OrphanedUnknown` and `Failed`** — a decision this RFC inherits rather than re-opens.
- **The schema already permits the record this RFC would write.** `valid_managed_process` accepts
  `AuditOutcome::Terminated` for `ManagedProcessLifecycle`, requiring a terminal id, `Runtime` actor,
  `RuntimeObserver` source and a reason code. **No schema change is needed** — the same shape RFC-049
  D6 found, where RFC-013 had reserved a slot nothing produced.
- **The store already enforces the ordering.** `valid_managed_phase` admits `Terminated` **only when a
  `started` phase exists** for that operation id. A termination that was never started is rejected by
  the store, not by a caller's care.
- **Two production call sites end an agent run**, at `shell.rs:2755` and `shell.rs:4837`. **Both sit
  beside a `record_plain_terminal_terminated` call for the plain case.** The asymmetry is visible in
  four lines of the same file.
- `AuditActionKind` has `ManagedAgentLaunch` and no separate terminate kind, and
  `valid_managed_process` requires that kind for every phase — so a termination is a later phase of
  the launch's own operation, not a new action.

## Decisions required

**D1 — Which endings get a record, and which do not.** Recommended: `Exited` and the two killed
variants record `Terminated`; `OrphanedUnknown` records **nothing**; a post-start runtime `Failed`
records `Terminated` with a reason code rather than a second `Failed` phase, which the store's phase
rule does not admit after `started`.

**D2 — A detached run gets no record, and the trail says nothing rather than something false.**
Recommended, and it is the whole reason this RFC is small. When Tekstide loses supervision it cannot
observe an ending, and `orphaned_runtime_truth_is_not_mislabeled_as_durable_termination` already holds
that line for plain terminals. The cost is real and must be stated where users read it: a detached
run's trail ends at `Started` forever. **Open: whether that silence needs a surface of its own**, or
whether RFC-049 D8′'s existing disclosure (a detached run's transcript is never reclaimed) is where a
user already learns that supervision ended.

**D3 — Reason codes reuse the plain-terminal pair**, `ProcessExited` and `ProcessTerminated`.
Recommended: **no exit status in the record**, matching plain terminals, and matching RFC-013's rule
that a record says what happened and not what it contained.

**D4 — Best-effort, never a precondition.** `append_observation`, as RFC-046 D1 decided for the launch
and RFC-047 D4 for the degraded store. A run must never fail to be terminated because the trail cannot
be written.

**D5 — One place, not a third call site.** Recommended: the producer is called where the outcome is
already applied, so a future path that ends a run cannot forget it. **This RFC's reviewers have now
twice found an unexercised call site** — the adapter launch site at response 390, and the
command-line project open at response 397 — and the fix both times was to remove the choice rather
than to test it. **Open: whether the coordinator gains an `apply_agent_terminal_outcome` that both
applies and records**, the shape `purge_project_transcripts` already has.

## Scope

The producer, its wiring at the two existing call sites, the user-facing sentence in the changelog and
the book, and the `crates/tekstide-core/README.md` claim that currently says the trail does **not**
answer how a run ended.

## Non-goals

- **Any schema change.** If a decision here needs one, it is the wrong decision.
- **Recording detachment as an event.** `ManagedProcessLifecycle` permits four outcomes, and an
  observation of "supervision ended" is not among them. That would be a schema change (see above).
- **A termination surface in the GUI.** The AgentRun Report may later say how a run ended; this RFC
  writes the durable record, which is what nothing does today.
- **Changing what termination itself does.** RFC-043 owns that.

## Risks

- **A record that claims an ending Tekstide did not observe** is worse than no record. D2 is the
  mitigation, and the existing plain-terminal decision is the precedent.
- **A third call site added later, unrecorded.** D5 is the mitigation, and the two prior instances are
  the evidence that the risk is real rather than theoretical.
- **Small RFCs invite scope creep toward the report surface.** The non-goals are the fence.

## Acceptance criteria

- A real run that exits, and one that is killed through the close flow, each leave a `Terminated`
  record with its reason code, **read back from a real store**, after a `Started` record for the same
  operation id.
- **A detached run leaves no termination record**, asserted, with the reason named in the test.
- A degraded store makes the ending unrecorded and never blocks termination.
- No production path applies a terminal outcome without the producer — held by construction, not by
  grep.
- `crates/tekstide-core/README.md`'s "does not answer how it ended" sentence is corrected in the same
  change that makes it false.

## Decided on acceptance (2026-09-16)

**D1 — the mapping, and every code it uses exists today.**

| Runtime outcome | Record |
| --- | --- |
| `Exited` | `Terminated`, reason `ProcessExited` |
| `TerminatedBySignal`, `KilledAfterTimeout` | `Terminated`, reason `ProcessTerminated` |
| `Failed` after the run started | `Terminated`, reason `RuntimeFailure` |
| `OrphanedUnknown` | **no record** (D2) |

A zero and a non-zero exit are the same record. **Whether the run succeeded is `AgentRunStatus`'s
answer, not the trail's** — the trail says the process ended and how it was ended.

**D2 — a detached run gets no termination record, and the silence is disclosed rather than left to be
discovered.** Tekstide cannot observe an ending after it loses supervision, and a record claiming one
would be the worst thing this RFC could ship. `record_plain_terminal_terminated` already returns
`NotRequired` for the same case.

**The open question, answered: the disclosure belongs in the changelog and the book's audit
description, not in a new surface.** RFC-049 D8′'s existing sentence is about a detached run's
*transcript*; this is about its *record*, and a reader of "what the audit trail answers" must find
the limit there. The AgentRun Report stays out of scope.

**D3 — nothing from the outcome's payload reaches the record.** No exit status, no signal number, and
**no `BoundedRuntimeSummary` text**: `Failed` and `OrphanedUnknown` carry summaries that can contain
paths, and RFC-013's rule is that a record says what happened, never what it contained. The reason
code is the whole of the detail.

**D4 — best-effort, never a precondition.** `append_observation`. A run must never fail to be
terminated because the trail could not be written (RFC-046 D1, RFC-047 D4).

**D5 — one path, so a later one cannot forget.** The coordinator both applies the outcome and records
it, the shape `purge_project_transcripts` already has, and **production stops calling
`ProjectSession::apply_agent_terminal_outcome` directly.** This project has twice shipped a correct
decision with one call site unguarded — the adapter launch at response 390, the command-line open at
response 397 — and both times the fix was to remove the choice. The checklist asks for that by
construction, not by grep.

**D6 — the ordering is the store's job, not the caller's.** `valid_managed_phase` admits `Terminated`
only after a `started` phase for that operation id. The producer does not re-check it, and a test
asserts the store refuses a termination for a run that never started.

## Corrected during implementation (2026-09-16, request 400)

- **The producer was not missing.** `apply_managed_agent_terminal_outcome` existed, mapped the reason
  codes, and was called only from its own tests — RFC-036's shape, and the third instance this pack
  names. **The architect asserted it was absent without grepping for it**, which is the same
  shape-not-callers error that produced D8 in RFC-049.
- **"Move the two call sites to it" could not have compiled.** Production destructured
  `AuditedAgentLaunch` at the launch, moving `runtime_events` and `approval_endpoint` out and dropping
  the `operation_id` with it — and D6's ordering is keyed on that id. `AuditedAgentRunIdentity` is the
  implementer's answer: the four ids an ending needs, carrying no exit status, signal or summary, so
  §2 is a property of the type rather than a discipline in the producer.
- **The `Failed` arm was grouped with `OrphanedUnknown`**, so an *observed* runtime failure recorded
  nothing — saying of something seen what §1 says only of something unseen. D1 separated them.
- **An unaudited launch keeps its branch.** A launch with no store, or a plan that is not auditable,
  has no `Started` phase, so it can have no `Terminated` one. Removing the branch would mean either
  refusing such launches — which contradicts RFC-047 D4, where a degraded store makes an action
  unrecorded rather than refused — or writing a record the store must reject under D6. The absence is
  typed (`Option<AuditedAgentRunIdentity>`), which is the right shape for it.
- **The pack contradicted itself about one box**, and the implementer split it correctly: the
  `crates/tekstide-core/README.md` sentence belongs in the commit that makes it false (A), the rest of
  the documentation in B. The checklist said one thing and the plan another; that was the architect's
  error, named in both places.
