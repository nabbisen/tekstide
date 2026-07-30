# RFC-021: Command Approval Model and Adapter Capability - Developer Handoff Pack

Source RFC: [RFC-021](../../done/021-command-approval-model-and-adapter-capability.md)
Target milestone: **M11** (headless slices start immediately — see below)
Source RFC status: **Implemented headless with documented limitations** — closed 2026-07-30 by PR-021-F

**Start here.** This file is the entry point. Everything is linked below in reading order.

## Active work: one item outstanding (2026-07-30)

RFC-021 is **closed** (PR-021-F, accepted with one required follow-up). Everything below is historical except this:

**Add a regression test for "no timeout approves."**

RFC-021's fail-closed matrix says: *no response → pending indefinitely; **no timeout approves***. The property holds today, verified structurally at closeout — `approval/coordinator.rs` contains no `Duration`, `Instant`, `elapsed`, `SystemTime`, or `now()`, records `created_at` only via `ApprovalRequest::pending`, and reads no timestamp anywhere; the module's single timeout (`channel.rs`'s `PROPOSAL_READ_TIMEOUT`) only drops a connection and cannot produce a decision.

It is held by the **absence of code**, which does not survive a future edit visibly. Someone will one day add a timeout for a good-sounding reason — a stuck-proposal cleanup, a UI hint — and nothing will object. This is the same class as a fixture corpus that cannot fail (review response 110).

What the test must assert:

1. A proposal received and left undecided is still `Pending` after elapsed time, with `find()` returning it unchanged.
2. No decision was sent over the connection — read from the peer half of the `UnixStream::pair()` with a bounded read timeout and assert nothing arrives.
3. `decide` on it afterwards still succeeds normally, i.e. the request was not silently abandoned either.

Do not sleep for a meaningful duration — the point is not to wait out a real timeout but to prove no time-based path exists. A short elapsed interval plus the structural assertion is enough.

Then check the corresponding line in [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) (Fail-Closed Checklist, "No response → pending indefinitely") and record it in [`qa-evidence.md`](./qa-evidence.md) under the PR-021-F section's "Required follow-up", replacing that heading with the evidence.

**This is the last thing owed on RFC-021.** After it lands the RFC needs nothing further until the adapter-spawn slice makes any of it reachable.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-021](../../done/021-command-approval-model-and-adapter-capability.md) | The model, the enforcement boundary, and the adapter contract. **Read the "Enforcement Boundary" section first — it constrains everything else.** |
| 2 | This file | Orientation and what is binding. |
| 3 | [`implementation-handoff.md`](./implementation-handoff.md) | Protocol shape, risk classifier rules, fail-closed matrix, audit correlation. |
| 4 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 5 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 6 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Also read before starting, because this RFC conforms to them rather than amending them:

- [RFC-009](../../done/009-terminal-security-boundary.md) — why the approval channel must not touch the PTY stream.
- [RFC-010](../../done/010-agentrun-launch-model-and-ai-cli-profiles.md) — `Managed` capability rules that still apply.
- [RFC-013](../../done/013-durable-audit-store-and-local-data-policy.md) — the frozen `command_approval` audit family.

## Where to start work

**Begin at PR-021-B.** PR-021-A is design acceptance.

PR-021-B through PR-021-E are **fully headless** — no GUI, no substrate dependency. They can proceed in parallel with the RFC-014 GUI spike. The rendered approval dialog is RFC-022's job, not yours.

## Five things that are binding

1. **Approval is cooperative, never enforced.** Tekstide cannot intercept what a spawned process executes. Nothing you build may imply otherwise, and no code path may present approval as a guarantee.
2. **The approval channel is out-of-band.** Never over the PTY stream. An in-band protocol would let untrusted terminal output forge requests or decisions — exactly threat T-026, and a direct violation of the RFC-009 boundary.
3. **Every adapter-submitted proposal requires an explicit decision.** No risk threshold executes silently. No timeout approves.
4. **Everything fails closed.** Disconnect, malformed message, bad token, audit failure — all deny. See the matrix in `implementation-handoff.md`.
5. **No command text in durable audit.** The dialog needs the exact command; the audit record must not have it. Two retention policies for the same data, deliberately.

## Scope boundary

`Plain` and `Supervised` AgentRuns gain **nothing** from this RFC — not a degraded approval, none at all. If your implementation gives them any approval path, that is a defect.
