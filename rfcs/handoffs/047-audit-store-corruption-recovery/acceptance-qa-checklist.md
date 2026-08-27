---
title: "RFC-047 acceptance and QA checklist"
rfc: "RFC-047"
rfc_file: "../../accepted/047-audit-store-corruption-recovery.md"
source_rfc_status: "Accepted 2026-08-28 — M12"
target_milestone: "M12"
created: "2026-08-28"
---

# Acceptance and QA checklist

## The claim this RFC exists to be able to make

- [ ] **A user can find out that the audit store is broken.** Proven against a real corrupted
      store in a scratch state root — the same reproduction RFC-036 PR-036-C used, which produced
      a screen indistinguishable from a healthy one.

## PR-047-A — the seam

- [ ] `open_audit_store` distinguishes its failure reasons; `RecoveryIncomplete` is separable.
- [ ] `AuditHealth` is stored on `State` and **read**. All fourteen former construction sites
      accounted for, each checked rather than assumed.
- [ ] A failure to open leaves a trace a technical user can find.

## D1 / D2 — recovery

- [ ] `RecoveryIncomplete` resumes once per session.
- [ ] Any other open failure recovers.
- [ ] **In both cases the `AuditStoreRecovery` record is read back out of the store**, not inferred
      from a return value.
- [ ] **The quarantined file still exists**, and its **path is what the product reports**. This is
      the condition D2 rests on — without it the decision was wrong.
- [ ] Recovery that itself fails leaves `AuditHealth` degraded, not reporting success.
- [ ] Nothing in this slice calls `fs::remove_*` on a user's audit data.

## D3 — the indicator

- [ ] Present when degraded.
- [ ] **Absent when healthy**, with its own test, ablated separately — deleting that assertion must
      fail on its own.

## D4 — say it before the click

- [ ] The agent-launch and trust-grant confirmations state the action will not be recorded, **while
      the control is live**.
- [ ] The wording does not imply the action is unsafe, does not imply the user can fix it from
      there, and does not appear when healthy.
- [ ] Present-when-degraded and absent-when-healthy are **separately** ablated.

## Live GUI evidence

- [ ] Against a **`mktemp -d` fixture with a fresh `XDG_STATE_HOME`**, using RFC-036 PR-036-C's own
      corruption method.
- [ ] Shows the degraded indicator and a launch confirmation naming the unrecorded state.
- [ ] Whether a real mouse click was sent is stated either way.

## Gates

- [ ] `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`.
- [ ] Full workspace suite, **three consecutive runs**, each logged to a file; any flake given a
      **row** in the register, not a mention.

## The outcome this slice must not reach

- [ ] **PR-047-C is done.** D1–D3 are calls to functions that already exist and will feel like
      completion. A slice that lands them and leaves the confirmations unchanged has fixed the
      plumbing and left the promise.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
