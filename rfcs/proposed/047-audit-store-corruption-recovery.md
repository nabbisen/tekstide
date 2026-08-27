# RFC-047: Audit Store Corruption Recovery

Status: **Proposed 2026-08-28.** Reserved by RFC-036's own triage (PR-036-C), which reproduced the
defect against the release binary rather than reasoning about it, and recommended an RFC rather
than a fix.
Target milestone: **M12**
Date: 2026-08-28

Related RFCs:

- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md) — owns the audit store, its
  schema identity, migration harness, corruption diagnostics and recovery. **All of that exists.**
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md) — owns the precedent this RFC's
  central question rests on: refusing a dangerous action when a precondition is not met.
- [RFC-036](../done/036-dormant-capability-closure.md) — found it. See
  [`corrupted-audit-store-defect.md`](../handoffs/036-dormant-capability-closure/corrupted-audit-store-defect.md)
  for the reproduction and screenshots.

## Summary

The audit store can fail to open. When it does, Tekstide starts normally, the project board reports
**"Calm"**, and every audit-writing action for the rest of the session silently does nothing.
Decide what should happen instead.

## What is actually true today, measured

Three layers, each built correctly, none connected.

**1. The recovery machinery has never run.** `AuditRecovery::recover` and `resume` are reviewed and
tested, with **zero production callers** — confirmed by RFC-036 PR-036-A's compiler-enumerated
sweep and by reading `main.rs`, which references `AuditStore`, `AuditRecovery` and `recovery`
nowhere at all.

**2. The product already knows how to describe the failure, and throws it away fourteen times.**
`AuditHealth` carries `status` (with a `Degraded` variant), `failure_count`, and
`last_failure: Option<AuditStoreErrorReason>`. There are **fourteen** `AuditHealth::default()`
constructions in production code and **not one reader**. It is not stored on `State`, so it does
not even accumulate within a session: every audit-writing operation builds a fresh health record,
writes a failure into it, and drops it.

**3. Every distinct failure collapses to the same silence.** `open_audit_store` is three
`.ok()`/`.ok()?` calls in a row. A missing directory, an unresolvable path, a corrupt database and
an interrupted migration all become `None`, indistinguishable.

**Reproduced, not inferred** (RFC-036 PR-036-C, release binary, scratch state root): a genuinely
corrupted `audit.sqlite3` and an interrupted-migration marker produce **screenshots that are
pixel-comparable** — same board, same "Calm", no dialog, no banner, no marker. The corrupted file
is byte-for-byte unchanged after a full session.

## The question that makes this an RFC

The obvious framing — *"call `recover()` when the store won't open"* — is a wiring task, and it is
not the question.

**Is a working audit store a precondition for the actions it audits?**

This product's entire trust model is accountability: Restricted Mode, workspace trust, and command
approval all exist so that what an AI agent does is *recorded*. RFC-004 already establishes that
this project refuses a dangerous action when its precondition is absent — that is what Restricted
Mode *is*.

Today, when the recording mechanism is broken, Tekstide grants trust, approves commands, and
launches agent runs exactly as if it were working, and says nothing. Either that is acceptable and
should be stated, or it is not and something must change. **Answering that is the RFC; the
`recover()` call falls out of it.**

## Decisions required

**D1 — the safe case.** `AuditStoreErrorReason::RecoveryIncomplete` means the application was
already in a known, safe recovery when it stopped. `resume()` exists for exactly this and its
precondition (no live connection) is trivially satisfied at that branch. Deciding to call it is
close to mechanical. **Recommend yes**; the decision is whether it happens silently.

**D2 — the corrupt case.** `recover()` handles this by **quarantining the existing database and
starting fresh** — that is, discarding the user's audit history. Doing that automatically, without
telling them, is a materially different act from resuming an interrupted migration. Auto-recover?
Ask? Refuse and leave the file untouched for a human? **Do not let this be decided by which error
variant a given corruption happens to produce.**

**D3 — how does anyone find out?** Today there is *nothing*: no log line, no file, no indicator.
This is the cheapest part of the whole RFC and probably the most valuable: it converts a silent
failure into a detectable one. Decide the minimum — a diagnostic a technical user can find, an
on-screen indicator, or both — and note that `AuditHealth` already models exactly what would be
reported.

**D4 — what does the product do while degraded?** The real question above, made concrete. Options,
not exclusive: run as now but visibly; refuse to launch agent runs while unaudited; refuse to grant
trust; refuse to start. **Weigh against RFC-004's own precedent**, and against the fact that a
refusal a user cannot understand or clear is its own defect.

## Scope

1. Distinguish the failure reasons at `open_audit_store` rather than collapsing them.
2. D1's resume, D2's decision, D3's disclosure, D4's degraded-mode behaviour.
3. `AuditHealth` gets a reader — whatever D3 and D4 decide, they read it rather than re-deriving.

## Non-goals

- **Changing what the audit store records.** The missing agent-run launch record is RFC-046.
- Redesigning the schema, the migration harness, or the recovery algorithm. RFC-013 built them and
  they work; this RFC connects them.
- Recovering data from a corrupt database. `recover()` quarantines and restarts; salvage is a
  different problem nobody has asked for.

## Risks

- **A refusal nobody can clear.** If D4 says "refuse", a user whose store broke must have a route
  back. A product that stops working and cannot say why is worse than one that works and says
  nothing.
- **Discarding history by default.** D2. `recover()` is destructive to the user's own record, and
  silence about that is the current defect wearing a different hat.
- **Fixing the wiring and calling it done.** Connecting `resume()` closes the *safe* half and
  leaves the loud question — D4 — untouched. That would be this RFC's failure mode.

## Acceptance-time decisions

**D1–D4 are decided by the architect on acceptance and recorded in this file before implementation
begins**, the rule RFC-041 onward have been accepted under.
