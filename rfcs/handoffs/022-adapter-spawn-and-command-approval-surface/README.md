---
title: "RFC-022: Adapter Spawn and the Command Approval Surface — handoff pack"
rfc: "RFC-022"
rfc_file: "../../done/022-adapter-spawn-and-command-approval-surface.md"
status: "Implemented — closed 2026-08-17. Not reachable by any real user; cooperative, not enforced."
target_milestone: "M11"
created: "2026-08-16"
---

# Start here

**Closed 2026-08-17.** This built the adapter-spawn pathway and the approval surface, proven
end to end against production code — but **it does not make command approval reachable by a
real user, and cannot**: no shipping AI CLI speaks RFC-021's protocol, so `Managed` (and
therefore command approval) can only ever be exercised by the reference adapter, a test
artifact. See RFC-022's own Status field and `qa-evidence.md`'s PR-022-F section for the full
accounting. What it does unblock, precisely: RFC-021's approval model, RFC-024's diff content,
RFC-011 Amendment 1's transcript reader, and RFC-020's two surfaces all become *reachable* —
not the same as *done*.

It was the largest single piece of work in the project's remaining plan at the time it was
authored, and the first that added a new artifact type — a reference adapter.

## Reading order

1. **[`what-the-dialog-must-not-lie-about.md`](./what-the-dialog-must-not-lie-about.md)** —
   required before any code. The approval dialog renders a command **the adapter chose**,
   in trusted chrome, and asks the user to authorise it. That is the highest-consequence
   rendering decision this project has made.
2. RFC-022 itself — the token-channel decision and its reasoning, and why
   `ExplicitAllowlist` is *not* needed.
3. [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) — six slices and their gates.
4. [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) — tick at closeout.
5. [`qa-evidence.md`](./qa-evidence.md) — record as you go.

## The starting state, verified

- **No `AgentRun` can exist in production.** `launch_agent_run_with_runtime` and
  `add_agent_run` have zero production callers.
- **Nothing speaks RFC-021's protocol.** It is this project's own invention; no third-party
  AI CLI implements it, and `validate_compatibility` (`agent/launch.rs:651-658`) already
  rejects a `Managed` profile that cannot prove capability, because none can.
- **`inject_token_into_environment` exists and is tested, with no production caller.**
- **`spawn_shell` launches a plain interactive shell only**, with `.env_clear()` and five
  fixed variables (`runtime/terminal/launch.rs:482-487`).
- `NavigationAction::OpenCurrentAgentRunDetail` and `OpenDiffReview` both map to `None`.

## The reference adapter is a test artifact, not a product feature

Scope item 6, and the thing that makes every other item demonstrable. Without it there is
no adapter to spawn, no token to deliver, and no approval request for a dialog to answer.

**It is deliberately not an integration with any real AI CLI.** Coupling this RFC's
provability to a third party's release cycle would be the wrong dependency. Integrating a
real tool is later, separate work that this makes possible.

**It must never be presented as evidence that real AI CLIs are supported.** They are not.
The closeout says so explicitly, and so does anything user-facing.

## What must not change

- **Approval stays cooperative, not enforced.** Nothing in Tekstide intercepts execution. A
  determined or buggy adapter can run whatever it likes without asking. This is RFC-021's
  own stated limit and this RFC does not lift it.
- **RFC-009's terminal security boundary.** The adapter spawn path sits *inside* it.
- **`TerminalEnvironmentPolicy::ExplicitAllowlist` stays rejected.** The token is *set*,
  not inherited; see RFC-022 §"What this does not require."
- **RFC-018's modal exclusivity.** The approval dialog is a modal and inherits every rule
  the paste dialog established — including that while it is open, terminal input is not
  produced.

## The failure mode this project has hit three times

RFC-021 shipped a model nobody could reach. RFC-024 shipped content access with no surface.
RFC-020's surfaces were scheduled against inputs that did not exist.

**Every slice here should be checkable against one question: can a user get to this, and
by what path?** If the answer is "once a later slice lands," say so in the evidence rather
than letting it read as done.

## Out of scope

- **Enforcement**, in any form.
- **Integrating a real AI CLI.**
- **Environment inheritance.**
- **RFC-020's surfaces** — unblocked by this, not part of it.
- **Audit producer completion** — split to RFC-031.
- **Multiple concurrent agent runs**, unless free.
