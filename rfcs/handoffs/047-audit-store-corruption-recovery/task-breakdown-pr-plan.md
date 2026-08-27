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

## Not in this plan

- Salvaging records from a quarantined database.
- The missing agent-run launch record (RFC-046).
- Any change to `AuditStore`'s schema, migration harness, or recovery algorithm — RFC-013 built
  them and they work.
- Refusing any action. D4 decided against it, with a reason; reopening that is a written argument,
  not an implementation choice.
