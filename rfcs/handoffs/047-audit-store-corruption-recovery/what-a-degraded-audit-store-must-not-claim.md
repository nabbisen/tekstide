---
title: "What a degraded audit store must not claim"
rfc: "RFC-047"
rfc_file: "../../accepted/047-audit-store-corruption-recovery.md"
source_rfc_status: "Accepted 2026-08-28 — M12"
target_milestone: "M12"
created: "2026-08-28"
---

# What a degraded audit store must not claim

**Required reading before writing code.** This slice writes an audit trail *about the audit trail*,
and decides what the product says at the moment it does something it cannot record. Every failure
mode here is a false statement about accountability, which is the one thing this product sells.

## §1 The defect is the silence, not the corruption

A corrupt SQLite file is a fact about a disk. It is not, by itself, a defect in Tekstide.

**The defect is that the product cannot tell you it happened**, and goes on granting trust,
approving commands and launching agents exactly as though it were recording them. RFC-036
reproduced it: same board, same "Calm", no dialog, no banner, and the corrupted file byte-for-byte
unchanged after a full session.

So the measure of this slice is not "does recovery work." It is **can a user find out.** A version
that recovers perfectly and says nothing has not fixed the thing that was wrong.

## §2 Do not tell the user something is fine

D3 says the indicator appears **only when degraded**. That is not a space-saving decision.

A permanent "audit: healthy" line is a claim the product makes continuously and cannot fully
support — the store can break between the check and the next write, and the line would still read
healthy. `en.ftl` already carries 28 `change-review-*` strings, and RFC-034's own security document
had to confront that a surface where every line is a caveat is a surface where none is read.

**Say something when something is wrong. Say nothing when nothing is.**

## §3 The quarantined path is load-bearing, not a nicety

D2 permits automatic recovery **only because `recover()` is `fs::rename`** — the unreadable
database is moved aside, not destroyed. That is what makes recovering without asking defensible
rather than high-handed.

**If the user is never told where the old file went, the justification is gone and the decision was
wrong.** They would then have a working store, a vanished history, and no way to know the records
still exist on disk under a name nobody mentioned.

Surface the path. Not "your audit store was recovered" — *the path*.

### §3.1 Degraded and disclosed are independent (added 2026-08-28, response 358)

§3 and §4 collide in one state, and the original text did not say which wins: a recovery that
**succeeds** — the rename happened, a usable store is returned — whose own `AuditStoreRecovery`
record could not be written. §4 says that is degraded. §3 says the path must be surfaced. Both are
right. PR-047-B resolved it by silently keeping only §4, so the board named no path at all: a
working store, a moved history, and no way to find it — the exact state §3 exists to forbid.

**They govern different fields.** §4 governs `status`. §3 governs `last_recovery`. Recording a
failure must never clear or suppress the disclosure: **failing to attest a rename does not
un-rename it.** A recovery whose own record did not survive is degraded *and* still names where the
old file went.

One consequence for the wording: a line shown alongside a returned, working store must not claim
"not recording". Overstating the damage is the same class of false statement as understating it —
the indicator tracks what is true now, in either direction.

## §4 A record about recovery is still a record, and it is written to a store that just failed

D1 and D2 both write an `AuditStoreRecovery` record into a store that was, moments earlier,
unusable. Two things follow:

- **The write can fail too.** Do not assume the fresh store is healthy because it just opened.
  Failing to write the recovery record is itself a degraded state, and `AuditHealth` is where it
  belongs.
- **Do not claim more than the record supports.** `recovery_record()` produces `Completed`. That
  means *this recovery completed* — not that the previous history is intact, not that nothing was
  lost. This project renamed a field for exactly this class of overclaim (`fully_confirmed` →
  `terminal_session_confirmed_empty`, request 328), and then had to make sure the rename did not
  overclaim in the other direction.

## §5 D4 is the slice, and it is the part with no code waiting

Everything else here is calling functions someone already wrote and tested. D4 is not.

The agent-launch and trust-grant confirmations must say, **before the click**, that the action will
not be recorded. That is RFC-034 D4's rule — an irreversible or unrecoverable property is stated
while the control is still live, not afterwards — applied to an unrecorded action rather than an
irreversible one.

**What that wording must not do:**

- **Must not imply the action is unsafe.** It is not. RFC-004's Restricted Mode refuses actions
  whose *danger* it cannot bound; a broken audit store does not make an agent run more dangerous,
  only unrecorded. Wording that reads as a security warning misrepresents what changed and will
  train users to dismiss it.
- **Must not imply the user can fix it from here.** They cannot. If the message suggests an action
  that does not exist, it is worse than silence.
- **Must not appear when the store is healthy.** §2.

## §6 What you may not do

- **Do not make `open_real_audit_store` noisy.** Its fail-silent contract is correct for its own
  scope; the decision goes above it. Changing it is how an observability path starts blocking
  startup.
- **Do not refuse the action.** D4 decided this, with a reason. If you come to believe refusal is
  right, that reopens D4 in writing — it is not an implementation detail to settle at the keyboard.
- **Do not delete anything.** `recover()` renames. Nothing in this slice should call `fs::remove_*`
  on a user's audit data, ever.
- **Do not surface a healthy state to prove the feature works.** Prove it with the degraded case,
  which is the one that was broken.

## §7 If the honest answer is that a user cannot act on this

Possible. A person told "your audit store is degraded and this run will not be recorded" may have
no idea what to do about it.

**That is an argument for saying where the quarantined file is and what the log says — not for
saying nothing.** The alternative on offer is the current behaviour, which is that the product
knows and does not mention it. A user who cannot act is still better served by knowing than by a
product that decides on their behalf that it was not worth raising.
