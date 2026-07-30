# RFC-021: Command Approval Model and Adapter Capability - Developer Handoff Pack

Source RFC: [RFC-021](../../done/021-command-approval-model-and-adapter-capability.md)
Target milestone: **M11** (headless slices start immediately — see below)
Source RFC status: **Proposed**

**Start here.** This file is the entry point. Everything is linked below in reading order.

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
