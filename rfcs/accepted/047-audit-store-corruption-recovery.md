# RFC-047: Audit Store Corruption Recovery

Status: **Accepted by the human owner 2026-08-28.** **D1–D4 decided by the architect on acceptance** — see "Decided on acceptance" at the end, which also records a fourth built-and-unconnected layer found while deciding. Proposed the same day; reserved by RFC-036's own triage (PR-036-C), which reproduced the
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

---

## Decided on acceptance, 2026-08-28

Two facts checked while deciding changed two of the four answers.

### A fourth layer, found while deciding D2

`AuditEventFamily::AuditStoreRecovery` exists, and `recovery_record()` builds a `Completed`
outcome for it. **The way to tell a user that recovery happened is already written**, and — like
`recover`, like `resume`, like `AuditHealth` — nothing calls it.

So the honest version of this feature is not mostly new code. It is four existing pieces, none
connected to anything.

### D1 — resume the safe case, and say so in the record that already exists for it

`AuditStoreErrorReason::RecoveryIncomplete` means the application was already inside a known, safe
recovery when it stopped. **Call `resume()` once per session at that branch.** Its precondition (no
live connection) is satisfied there by construction.

**Not silently.** Write the `AuditStoreRecovery` record into the store that resume just made
usable. The family is frozen and already designed for it; this is the one case in this RFC where
disclosure costs nothing, because the mechanism and the storage are both already there.

### D2 — auto-recover, because `recover()` quarantines rather than deletes

The proposal called this *"discarding a user's audit history"* and treated it as the hard decision.
**Checked: `recover()` is `fs::rename`.** It moves the unreadable database aside and starts a fresh
one. Nothing is destroyed.

That changes the answer. "Discard the user's history without asking" would be indefensible;
**"set the unreadable file aside, start a working store, and say where the old one went" is not the
same act** and does not need a modal to justify it.

So: **recover automatically, and tell the user the path of the quarantined file.** A dialog on
startup, before any project is open, asking a question whose only sensible answer is "yes, keep
working" would be ceremony — and this project has a standing objection to controls that imply a
choice they do not really offer.

**The disclosure is the condition, not the courtesy.** If the quarantined path is not surfaced,
this decision reverts to being indefensible, because a user then has a working store and no way to
know their previous records exist on disk under a name they were never told.

### D3 — `AuditHealth` gets its first reader, and it shows only when degraded

Fourteen constructions, no readers, not stored on `State`. **Store one on `State`** so failures
accumulate across a session, and read it.

- **On screen when, and only when, degraded.** The project board already carries a runtime summary
  ("Calm", "9 blocked automations"); a degraded-audit line belongs there. **Absent when healthy** —
  RFC-034's §4 disclosure-density problem is real, and a permanent "audit: fine" line is how a
  surface becomes unreadable.
- **Plus something a technical user can find** — the durable record from D1/D2 covers the recovery
  cases; a failure that recovery cannot fix must still leave a trace that is not the screen alone.

### D4 — do **not** refuse. Disclose at the point of the action.

The proposal asked whether a working audit store should be a precondition for the actions it
audits, and pointed at RFC-004's Restricted Mode as precedent for refusing.

**The precedent does not transfer, and the distinction is the decision:**

> **Restricted Mode refuses actions whose *danger* it cannot bound. A broken audit store does not
> make an agent run more dangerous — it makes it unrecorded.**

Refusing would trade a real capability for no reduction in risk. Worse, a user cannot repair a
corrupt store from inside the application, so a refusal would be one they could not clear — the
risk this RFC's own text names.

**Instead: the actions that would have been recorded say so at the point of the click.** Launching
an agent run and granting workspace trust name, in their own confirmation, that this action will
not be recorded while the audit store is degraded.

That is this project's consistent answer whenever it cannot guarantee something: the content
preview says "not a diff"; the close confirmation names what ends; a review decision says it does
not survive the session. **Say it before the click, and let the person decide** — the rule RFC-034
D4 established for one-way controls, applied to an unrecorded one.

### What this RFC must not become

Connecting `resume()` and stopping. D1 and D3's cheap half are close to free and will feel like
completion. **D4 is the reason this is an RFC**, and it is the part with no code in `tekstide-core`
waiting to be called.
