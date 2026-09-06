---
title: "RFC-047 task breakdown and PR plan"
rfc: "RFC-047"
rfc_file: "../../accepted/047-audit-store-corruption-recovery.md"
source_rfc_status: "Accepted 2026-08-28 — M12"
target_milestone: "M12"
created: "2026-08-28"
---

# Three slices

Ordered so the **hard** decision is not left until the plumbing feels finished. PR-047-C is the
reason this is an RFC and it is deliberately not last-and-optional.

## PR-047-A — stop collapsing the failures, and make the degradation observable

**No recovery yet.** This is the seam everything else needs.

1. **`open_audit_store` distinguishes its failure reasons** instead of three `.ok()` calls
   producing one `None`. `AuditStoreErrorReason::RecoveryIncomplete` in particular must be
   separable from every other failure.
2. **`AuditHealth` moves onto `State`** and gets its first reader. Fourteen production call sites
   construct it fresh today — check each rather than assuming they can all share one instance.
3. **Something a technical user can find** records that the store failed to open and why. Today
   there is not even that.

**Evidence:** reproduce RFC-036 PR-036-C's corruption, and show the difference — same corrupt
store, and now a product that has noticed.

**Gate:** the on-screen indicator is not required yet; the observability is.

## PR-047-B — recover, and say what happened

1. **`RecoveryIncomplete` → `resume()` once per session** (D1), then write the
   `AuditStoreRecovery` record into the store resume just made usable.
2. **Any other open failure → `recover()`** (D2), then the same record — **and surface the
   quarantined file's path.** §3 of the risk document: the path is the condition on which D2 rests,
   not a nicety.
3. **The degraded indicator appears on the project board**, and **only when degraded** (D3).

**Required tests:**

- A store with a recovery marker resumes, and the `AuditStoreRecovery` record is really in the
  store afterwards — read back, not inferred from a return value.
- A corrupt store recovers, **the old file still exists at the quarantined path**, and that path is
  what the product reports.
- A healthy store produces **no** indicator and **no** recovery record.
- Recovery that itself fails leaves `AuditHealth` degraded rather than reporting success.

**Ablations:** remove the resume branch → the marker test fails; remove the path from the
disclosure → the quarantine test fails.

## PR-047-C — say it before the click

**The decision with no existing code waiting to be called.**

The agent-launch and trust-grant confirmations state, while the control is still live, that the
action will not be recorded — RFC-034 D4's rule applied to an unrecorded action.

**Wording constraints are in §5 of the risk document and are not negotiable at the keyboard:** it
must not imply the action is unsafe, must not imply the user can fix it from there, and must not
appear when the store is healthy.

**Required tests:** the wording is present when degraded and absent when healthy, each ablated
separately — deleting the healthy-case assertion must fail on its own, or "absent when healthy" is
untested.

**Evidence:** the live walkthrough, showing a launch confirmation naming the unrecorded state,
against a `mktemp -d` fixture with a corrupted store in a scratch state root.

## PR-047-D — `status` is not a latch

**Added 2026-09-02 by response 359, after PR-047-B closed.** Not a regression of A–C: a defect of
the same class found one layer below the one §3.1 fixed. **The full decision is §3.2 of the risk
document — read it before writing code; this section is the work, not the reasoning.**

Measured, not theorised: `record_failure(Busy)` followed by an ordinary successful open leaves
`status=Degraded, failure_count=1` and the board rendering *"Audit: not recording. Recent actions may
be missing from the record."* The store is open and recording. Nothing clears `status` except a
successful recovery, so a transient failure degrades the session permanently.

**The naive fix is wrong and will pass its own test.** `state.audit_health` is written by two
different kinds of failure — the shell seam records *open* failures, `AuditCoordinator::
append_required`/`append_observation` record *write* failures into the same instance. Clearing on a
successful open would erase a real write failure that a successful open does not cure.

**The work, per §3.2's three rules:**

1. Make open failures and write failures distinguishable inside `AuditHealth`, and have each cleared
   by the success that actually cures it. `status()` answers only *can this session record right
   now?*
2. `failure_count` / `last_failure` become session history that no success clears — including
   `clear_degraded()`, which today zeroes them and silently discards the same history on the
   recovery path. That is a second, existing defect this slice closes.
3. The board renders present-tense and history as **independent lines**: the present-tense line only
   while `status()` is `Degraded`, and a history line whenever `failure_count > 0` **even after
   capability returns**.

Rule 3 is what makes rule 1 safe. Without it, clearing `status` on success hides the very fact this
RFC exists to surface. **Capability returning is not the record healing.**

**Required tests, each ablated separately:**

- A transient `record_failure` followed by a successful open leaves **no** present-tense line — and
  that assertion must fail on its own when the clearing is removed.
- The same sequence still renders the **history** line. Ablating rule 3 alone must fail this and not
  the one above.
- A *write* failure is **not** cleared by a subsequent successful open. This is the one that catches
  the naive fix, and it must fail if open- and write-failures are collapsed back into one flag.
- `clear_degraded()` on the recovery path preserves `failure_count`.

**Evidence:** unit-level is sufficient and expected. Per the delivery plan's bounded-evidence rule,
do not spend review rounds on a live capture here — the D3 lines were already captured live in
PR-047-B (`EVIDENCE-1`/`EVIDENCE-2`), and this slice changes when they appear, not what they look
like.

## Not in this plan

- Salvaging records from a quarantined database.
- The missing agent-run launch record (RFC-046).
- Any change to `AuditStore`'s schema, migration harness, or recovery algorithm — RFC-013 built
  them and they work.
- Refusing any action. D4 decided against it, with a reason; reopening that is a written argument,
  not an implementation choice.
