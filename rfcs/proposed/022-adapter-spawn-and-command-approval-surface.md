# RFC-022: Adapter Spawn and the Command Approval Surface

Status: **Accepted by the human owner 2026-08-16.** Open question 1 answered by the architect the same day — see below; it adds a scope item rather than changing the design.
Target milestone: M11
Date: 2026-08-16

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-external-design-v0.md`

Depends on:

- [RFC-021](../done/021-command-approval-model-and-adapter-capability.md) — the approval
  model, the socket protocol, the capability token. Implemented, tested, **unreachable**.
- [RFC-010](../done/010-agentrun-launch-model-and-ai-cli-profiles.md) — AI CLI profiles and
  the AgentRun launch model.
- [RFC-009](../done/009-terminal-security-boundary.md) — the terminal security boundary this
  spawn path sits inside.
- [RFC-011 Amendment 2](../done/011-transcript-retention-and-local-data-policy.md) —
  transcript capture, re-homed. Its named prerequisite, discharged 2026-08-16.

## Summary

Launch an AI CLI through a reviewed spawn path, deliver a per-run capability token, and
render the dialog that answers an adapter's approval requests.

**Corrected 2026-08-16 (response 218).** This section originally read *"Make command
approval reachable."* **It does not, for a real user, and cannot.** Open question 1
established that no shipping AI CLI speaks RFC-021's protocol. `validate_compatibility`
rejects `Managed` unless a profile declares `structured_action_approval`, which only an
adapter speaking our protocol can honestly declare, and only the reference adapter does.

So **`Managed` — and therefore command approval — can only ever be exercised by the
reference adapter**, which is a test artifact. What a real user gets from this RFC is an
AgentRun at `Plain` or `Supervised`: a real AI CLI in a project-owned terminal, with
transcript capture and audit, and no approval protocol involved.

**What this RFC delivers, stated honestly:** the approval pathway exists and is proven end
to end; AgentRuns become reachable; command approval becomes reachable *the day a real
adapter exists*, which is not this RFC's to produce. The architect answered open question 1
and did not trace this consequence until PR-022-D's research forced it.

## Why this is one RFC and not two

The reserved slot for RFC-022 was *"Security Dialogs and Audit Producer Completion."* This
RFC takes the dialog and adds the spawn pathway, because **they are not separable**:

- An adapter that can be launched but whose approval requests nobody can answer is either
  useless (every request times out) or dangerous (something auto-approves).
- A dialog with nothing to display is the zero-reachable-surface failure this project has
  now hit three times.

**Audit producer completion separates out cleanly** — `safe_close_decision`,
`restricted_mode_blocked` and `project_added` are independent wirings with no dependency on
either half. They are not in this RFC.

## What exists, verified rather than assumed

**Exists, reviewed, unreachable:**

- The whole approval protocol: socket channel, `CommandProposal::decode`, risk
  classification, the coordinator, the audit family. RFC-021, closed 2026-07-30.
- `inject_token_into_environment` (`approval/channel.rs:374`) — sets
  `APPROVAL_TOKEN_ENV_VAR` on a `std::process::Command`. **No production caller.**
- AI CLI profiles, AgentRun lifecycle, transcript capture (RFC-011 Amendment 2).

**Does not exist:**

- Any path that launches an AI CLI as an adapter. `spawn_shell` launches a plain interactive
  shell with `.env_clear()` and five fixed variables (`runtime/terminal/launch.rs:482-487`).
- Any production caller of `launch_agent_run_with_runtime` or `add_agent_run` — every call
  site is a test. **No `AgentRun` can exist in production today.**
- Any route from the GUI. `NavigationAction::OpenCurrentAgentRunDetail` maps to `None`.
- Any rendered approval dialog.

## The token delivery channel — decided here, with the reasoning

The obvious question is whether putting a capability token in a child's environment is
acceptable, since it is readable via `/proc/<pid>/environ` by any process of the same user,
and is inherited by everything the adapter spawns.

**Decision: environment delivery, and the reason is that the alternative defends a boundary
this project explicitly does not claim.**

RFC-021's own conclusion is that approval is **cooperative, not enforced**. The token binds
a proposal to a run; it authenticates *which run is asking*, not *that the asker is
trustworthy*. Against a hostile same-user process the token is worthless — but so is the
whole mechanism, because that process can simply run the command itself without asking
anyone. Nothing in Tekstide intercepts execution.

The alternatives do not change this:

- **File descriptor inheritance** hides the token from `/proc/<pid>/environ`, but the fd
  number must be communicated — conventionally through an environment variable — and a
  same-user process can read the adapter's fds anyway.
- **Socket handshake** requires the socket path in the environment, and authenticating the
  connection before issuing a token is the problem the token exists to solve.
- **A file with restrictive permissions** still needs its path communicated, and is readable
  by the same user.

Every option bootstraps through the environment and every option is transparent to a
same-user attacker. Choosing a more elaborate one would buy no security and would imply a
guarantee RFC-021 is careful not to make.

**What this RFC must therefore state plainly, and not soften:** the capability token is a
correlation and authenticity mechanism within a cooperating adapter, not a defence against a
hostile process running as the same user.

## What this does *not* require, correcting the record

**`TerminalEnvironmentPolicy::ExplicitAllowlist` is not needed**, and an earlier version of
the delivery plan wrongly said it was.

Delivering the token *sets* a value Tekstide generated. `ExplicitAllowlist(Vec<String>)` is a
list of **names with no values**, which can only mean *inheriting* variables from Tekstide's
own environment. A generated token's value cannot be expressed as a name in a `Vec<String>`,
so that variant is structurally incapable of delivering it.

The runtime already does `.env_clear()` plus five fixed `.env(...)` calls. The token is a
sixth, inheriting nothing. **`ExplicitAllowlist` stays rejected** until something genuinely
wants inherited environment — a separate question, sharpened by RFC-004's redaction policy
having no implemented pattern set.

## Scope

1. **An adapter spawn path**, distinct from `spawn_shell`: launches an AI CLI from an
   RFC-010 profile, inside RFC-009's boundary, with the token set and nothing inherited.
2. **Token delivery**, wiring `inject_token_into_environment`'s existing, tested code to its
   first production caller.
3. **A production `AgentRun` creation path** — `launch_agent_run_with_runtime` gains its
   first non-test caller.
4. **The approval dialog**, rendered in trusted chrome, escaped, under the modal exclusivity
   rules RFC-018 established.
5. **A route from the GUI** to start an agent run and to reach a run's detail — the concept
   RFC-020's surface work found missing.
6. **A reference adapter** — a small, purpose-built program that speaks RFC-021's protocol,
   added per open question 1's answer. It is the thing that makes every other item in this
   list demonstrable: without it there is no adapter to spawn, no token to deliver, and no
   approval request for the dialog to answer. It is a test-and-proof artifact, not a
   product feature, and the RFC must not present it as one.

## Non-goals

- **Enforcement.** Approval stays cooperative. This RFC must not imply Tekstide can prevent
  an adapter from executing something.
- **Environment inheritance.** See above.
- **Multiple concurrent agent runs**, unless it falls out for free. One is enough to make
  the mechanism reachable.
- **RFC-020's surfaces.** They become *unblocked* by this work; they are not part of it.
- **Audit producer completion.** Separated out, as stated.

## Open questions for the owner

1. ~~**Which AI CLI is the first real adapter, and does it exist?**~~ **Answered
   2026-08-16 by the architect, not the owner — it was answerable from this repository and
   should not have been raised as an owner question.**

   **No shipping AI CLI speaks this protocol, because the protocol is this project's own
   invention.** RFC-021 defined the socket, the `CommandProposal` encoding and the
   capability token; nothing external implements a Tekstide-specific handshake, and nothing
   would without adoption this project does not have.

   **The codebase already assumes this.** `validate_compatibility`
   (`agent/launch.rs:651-658`) rejects a `Managed` profile unless it declares
   `adapter_capabilities.structured_action_approval`, and RFC-010 §"Labels describe proven
   behavior" requires Managed be *"rejected or downgraded before launch if the selected
   adapter cannot prove the required capability."* Both were written anticipating exactly
   this: that no adapter proves it.

   **Consequence — a scope item, added below:** the first adapter is a **reference adapter**
   written by this project. Deliberately *not* an integration with any particular AI CLI:
   the goal is to make the pathway reachable and testable end to end, and coupling that to
   a third party's interface would put this RFC's provability at the mercy of their release
   cycle. Integrating a real AI CLI is later, separate work that this reference
   implementation makes possible.
2. **What happens when an approval request goes unanswered** — the user is away, the dialog
   is open, the adapter is blocked. Timeout and deny, or block indefinitely? RFC-021 defines
   the protocol but not the human-absent case.
3. **Does the approval dialog interrupt whatever the user is doing?** It is a modal in a
   workbench where the user may be mid-edit. RFC-018's paste dialog is user-initiated;
   this one is not.

## Risks

- **Overclaiming enforcement.** The single largest risk in this RFC. Mitigated by stating
  the cooperative limit in the surface, not only in documentation.
- ~~The first adapter not existing.~~ Resolved by open question 1's answer: it does not
  exist, this RFC writes one, and that is now scope item 6 rather than an unquantified risk.
- **The reference adapter being mistaken for a product feature**, or for evidence that real
  AI CLIs are supported. It is neither. The closeout must say so.
- **A dialog nobody can reach**, repeating the failure this project has hit three times.
  Mitigated by scope items 3 and 5 being in the same RFC as the dialog.
