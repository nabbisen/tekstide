---
title: "What an agent-run audit trail must not claim"
rfc: "RFC-046"
rfc_file: "../../accepted/046-managed-agentrun-audit-trail.md"
source_rfc_status: "Accepted 2026-09-06 — M12"
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

**The test must be that a launch succeeds with a degraded store**, driven through a real degraded
store rather than a mocked one — PR-047-B's own fixtures (`corrupt_and_interrupt_recovery_for_test`,
the symlinked `recovery` directory) already produce that state.

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

It is `None` today because the only profile is `Supervised`. **Do not let that make the test
optional.** A test written as "the endpoint survives" is meaningful now; a test written as "a
`Managed` profile's endpoint survives" cannot be written at all yet and will be forgotten.

The failure this prevents is not a crash. It is command approval quietly not existing for the first
`Managed` profile that ships, months from now, with nothing in the diff that introduced it.

## §5 Say what the trail answers, in the trail's own documentation

After this slice the audit trail answers:

> *Was an agent run launched in this project, when, and under which adapter profile.*

It does **not** answer *is it still running*, *how did it end*, or *why was a launch refused* beyond
the one Restricted Mode case that already exists. D3 and D5 decided both boundaries deliberately.

**Write those two sentences into the producer's own doc comment.** RFC-036 exists because a
documented claim diverged from what the code did; the correction is not to document less but to
document the boundary where a reader would otherwise assume more.
