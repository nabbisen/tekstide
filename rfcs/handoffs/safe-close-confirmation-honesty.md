---
title: "safe_close_decision must not claim more than it knows — implementation handoff"
rfc: "none — a correctness fix in a durable security record"
status: "Scheduled 2026-08-25, second of three for 0.14.0"
target_milestone: "M12"
created: "2026-08-25"
---

# `fully_confirmed: true` can be written while the process is still running

**No RFC, deliberately.** This is a correctness fix to a claim, not a design question — the same
treatment `test-process-leak.md` and `minimal-user-documentation` have. Scheduled **second** of
three, ahead of RFC-041, because a small independent item scheduled last is a small item that
slips; the user documentation took three weeks proving that.

## The defect

RFC-039 PR-039-C computes `fully_confirmed` from each terminal's termination outcome
(`shell.rs:3740`):

```rust
let confirmed = matches!(
    outcome,
    TerminationOutcome::Exited { .. }
        | TerminationOutcome::TerminatedBySignal { .. }
        | TerminationOutcome::KilledAfterTimeout { .. }
);
fully_confirmed &= confirmed;
```

`KilledAfterTimeout` counts as confirmation. Request 319 demonstrated, against the real
production path, that `request_terminate` returns exactly
`Terminated { outcome: KilledAfterTimeout { .. } }` — reporting success — **while a job the user
backgrounded inside that shell is still alive**, because bash puts a background job in its own
process group when the shell has a controlling terminal, and both `request_terminate` and
`RunningTerminal`'s `Drop` signal one group.

So `SafeCloseDecision::Closed { fully_confirmed: true }` can reach the **durable audit store**
while a process that terminal launched keeps running.

## Why this matters more than its blast radius suggests

RFC-013 anticipated half of it and does not cover this half:

> A safe-close `applied` outcome means Tekstide issued the selected terminate/abandon action. It
> does not mean the process exited.

That makes the *outcome kind* honest. `fully_confirmed` is a **separate, stronger field added
later** by RFC-039 PR-039-C, and its name and meaning assert that termination *was* confirmed.
That assertion can be false.

This is the class this project treats most seriously: a durable record claiming more than it
knows. The transcript privacy claim, restricted mode's blocked-feature count, and the affordance
audit's own heading were all this shape. It is bounded — it needs a backgrounded job — and it is
not urgent. It is also not cosmetic, and an audit trail is the last place to leave a
comfortable-but-unsupported claim.

## The question to answer

**Is `KilledAfterTimeout` confirmation at all?**

It means the escalation ran and observation was given up on — a weaker statement than `Exited` or
`TerminatedBySignal`, which are observations of the process actually ending. Three answers, with
different costs:

1. **Narrow `confirmed`** to exclude `KilledAfterTimeout`. Cheapest and most honest about what is
   known; means a close that needed the full escalation is recorded as not fully confirmed, which
   may be common enough to be noise.
2. **Re-check group emptiness after the kill** and derive `fully_confirmed` from that observation
   rather than from the outcome kind. Strongest, and it is the only option that would also catch
   the backgrounded-job case, since that job's group is a *different* group and would still be
   missed — say so if you take this one, because it sounds like it fixes more than it does.
3. **Rename the field to what it can support.** Cheapest of all and changes no behaviour, but a
   rename in a persisted audit record is a schema-visible change with its own migration question.

**Decide it and say why.** Do not implement more than one.

## What this is not

Not the product decision. Whether closing a terminal *should* kill a job the user deliberately
backgrounded — or leave it, the way `nohup` exists to let someone ask for — is
`test-process-leak.md`'s third cause and stays unscheduled. **This slice makes the record honest
about what happened; it does not change what happens.**

If you find yourself changing termination behaviour, stop: that is the other question.

## Evidence

- The existing test that a confirmed close writes two phases must still pass, and must still use a
  **scoped** `AuditQuery` — request 313 fixed five `latest(50)` windows over a shared store, and
  nine more of that shape remain in `shell/tests.rs`, named in RFC-039's affordance audit.
- A test that reproduces the actual case: a terminal with a backgrounded job, closed, and the
  recorded `fully_confirmed` asserted against whichever answer you chose.
- Ablate by restoring the old predicate and confirming that test fails.
