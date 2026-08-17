---
title: "RFC-032: Workspace Trust Granting — handoff pack"
rfc: "RFC-032"
rfc_file: "../../proposed/032-workspace-trust-granting.md"
status: "Ready for implementation — accepted by the owner 2026-08-17, both open questions answered"
target_milestone: "M11"
created: "2026-08-17"
---

# Start here

**No project can currently leave `Restricted`.** `grant_project_trust` and `revoke_trust` both
exist, are correct, and are fully audited — and neither has a production caller. RFC-004's
Restricted Mode is not a mode; it is the only state.

This is the reachability audit's highest-consequence finding, and the single blocker on the
entire agent-run chain RFC-022 built.

## Reading order

1. **[`what-the-trust-dialog-must-say.md`](./what-the-trust-dialog-must-say.md)** — required
   before any code. This is the largest grant in the application and its dialog is almost
   entirely a rendered path, which is the live attack surface.
2. **[`docs/src/contributors/security-decisions.md`](../../../docs/src/contributors/security-decisions.md)**
   — the **canonical** statement of both decisions and their reasoning. The RFC points here;
   so does this pack. If you find yourself restating it, stop and link it instead.
3. RFC-032 itself — scope, non-goals, what the grant authorises.
4. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) — five slices and their gates.
5. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) — tick at closeout.
6. [`qa-evidence.md`](./qa-evidence.md) — record as you go.

## The two decisions, in one line each

Both were the owner's, both answered 2026-08-17, both reasoned in the decisions page:

- **Trust persists across sessions.** Not because it is the cautious option — because asking
  on every launch trains people to click through, and a trust prompt users dismiss unread is
  worse than none.
- **Trust binds to the canonical path**, not the path as opened. A redirected symlink would
  otherwise inherit an existing grant while the literal path stayed identical.

## Three requirements, not intentions

Persistence is acceptable *because* of these. If one is dropped, the decision it supports
stops holding:

1. **Revoking is always available.** Trust is never one-way.
2. **Trust state is visible on the project board.** You can see what you granted without
   remembering it.
3. **The dialog says the folder's contents, present and future** — not "this project."

## What must not change

- **Opening a folder never implies trust** (RFC-004 §2). Persistence remembers an explicit
  decision; it does not infer one.
- **RFC-004's nine restricted features.** This RFC makes the existing gate passable, it does
  not change what the gate covers.
- **The audit records.** `grant_project_trust` already writes `TrustGrant` authorization and
  application correctly. This gives it a producer, not a rewrite.

## Out of scope

- **Per-feature trust.** One grant, all nine.
- **Any automatic or heuristic trust.** No "trust everything under `~/src`."
- **Trust expiry** — whether trust should lapse on its own has been open since RFC-004 and
  stays open.
