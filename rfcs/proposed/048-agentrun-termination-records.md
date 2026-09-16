# RFC-048: AgentRun Termination Records

Status: **Proposed 2026-09-16.** Scoped at the owner's word after `0.19.0` shipped. Reserved by
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
