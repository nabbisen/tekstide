---
title: "RFC-047: Audit Store Corruption Recovery — implementation handoff"
rfc: "RFC-047"
rfc_file: "../../accepted/047-audit-store-corruption-recovery.md"
source_rfc_status: "Accepted 2026-08-28 — M12"
target_milestone: "M12"
created: "2026-08-28"
---

# Connect four things that already exist, then answer the one that does not

Source RFC: [RFC-047](../../accepted/047-audit-store-corruption-recovery.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-047](../../accepted/047-audit-store-corruption-recovery.md) | **Read "Decided on acceptance" first.** D1–D4 are settled, and two of them changed on facts checked while deciding |
| 2 | [`what-a-degraded-audit-store-must-not-claim.md`](./what-a-degraded-audit-store-must-not-claim.md) | **Required.** This slice writes to an audit trail about the audit trail |
| 3 | [`corrupted-audit-store-defect.md`](../036-dormant-capability-closure/corrupted-audit-store-defect.md) | The reproduction, with screenshots. Do not re-derive it |
| 4 | [RFC-013](../../done/013-durable-audit-store-and-local-data-policy.md) | Built everything you are about to call |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Three slices |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |

## The one-sentence version

Four correct, tested pieces exist and none is connected to anything; connect them, and then answer
the question none of them answers — what the product says at the moment it does something it
cannot record.

## What already exists, so you write as little as possible

- **`AuditRecovery::recover`** — `fs::rename`s the unreadable database aside and starts fresh.
  **Quarantines, does not delete.** Zero production callers.
- **`AuditRecovery::resume`** — retries the exact recovery a durable marker identifies. Zero
  production callers. Its no-live-connection precondition holds at the branch D1 names.
- **`AuditHealth`** — `status` (with `Degraded`), `failure_count`, `last_failure`. **Fourteen**
  constructions in production, **no readers**, not stored on `State`.
- **`AuditEventFamily::AuditStoreRecovery`** and **`recovery_record()`** — the durable record for
  "recovery happened", already written, already frozen. Uncalled.

**`open_audit_store` (`shell.rs`) is three `.ok()`/`.ok()?` calls in a row.** Every distinct
failure reason becomes the same `None`. That is the seam: it has to stop collapsing them before
anything else here is possible.

## The trap

**D1 and D3's cheap half will feel like the slice.** Resuming an interrupted migration and putting
a line on the project board are close to free, visibly improve things, and are not the reason this
is an RFC.

**D4 is.** It is the only decision with no existing code waiting to be called: the agent-launch and
trust-grant confirmations must say, before the click, that the action will not be recorded. A slice
that lands D1–D3 and leaves D4 has fixed the plumbing and left the promise.

## Traps this codebase has already set

- **`AuditHealth` on `State` changes its lifetime.** Today fourteen call sites construct it fresh
  and drop it. One stored instance means failures accumulate — which is the point — but check every
  one of those fourteen sites rather than assuming they can all share it.
- **Do not surface a healthy state.** RFC-034's §4 disclosure-density problem is live on this
  project: `en.ftl` already carries 28 `change-review-*` strings. A permanent "audit: fine" line is
  how a surface stops being read. Degraded only.
- **The quarantined path is not optional.** D2's justification is that `recover()` sets the old
  database aside rather than destroying it. If the user is never told where it went, that
  justification evaporates and the decision was wrong.
- **`open_real_audit_store`'s doc says it is deliberately fail-silent.** That is correct *for that
  function* — an observability path must not stop the app starting. Do not change its contract;
  put the decision above it.

## Live GUI evidence

Required, and this one has a real reproduction to work from: RFC-036 PR-036-C corrupted a store in
a scratch `mktemp -d` `XDG_STATE_HOME` and photographed the result. **Use the same method** and show
the difference — the same corruption, and now a product that says something.

Against a `mktemp -d` fixture with a fresh state root, per `ARCHITECTURE.md`. State whether a real
mouse click was sent either way.

## Deferrals to state, not to solve

- **Salvaging data from a corrupt database.** `recover()` quarantines; extracting records from the
  quarantined file is a different problem nobody has asked for.
- **The missing agent-run launch record.** RFC-046. This RFC decides what happens when the store is
  broken, not what goes into it when it works.
