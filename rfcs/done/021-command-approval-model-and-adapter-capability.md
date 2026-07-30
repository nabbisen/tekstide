# RFC-021: Command Approval Model and Adapter Capability

Status: Implemented headless with documented limitations — **not reachable by any user** (no adapter-spawn pathway, no dialog). Closed 2026-07-30 by PR-021-F; one required regression test outstanding ("no timeout approves"). See `../handoffs/021-command-approval-model-and-adapter-capability/qa-evidence.md` §PR-021-F for exactly what may be claimed.
Target milestone: M11 (headless model implemented ahead of it — see Scheduling)
Date: 2026-07-28

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md), [`delivery-plan.md`](../delivery-plan.md)

Depends on:

- [RFC-002](../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md)
- [RFC-009](../done/009-terminal-security-boundary.md)
- [RFC-010](../done/010-agentrun-launch-model-and-ai-cli-profiles.md)
- [RFC-013](../done/013-durable-audit-store-and-local-data-policy.md)

Blocks:

- the rendered approval dialog (RFC-022);
- the `command_approval` durable audit producer;
- any honest use of the `Managed` compatibility label.

## Summary

RFC-021 defines the command approval model: how an AI CLI proposes a command before executing it, how Tekstide decides, and what Tekstide may honestly claim as a result.

This is the product's central safety promise — *approve before risk*. It is also the single easiest thing to overclaim, because Tekstide spawns AI CLIs as ordinary child processes and **cannot intercept what they execute**. Approval therefore works only where the AI CLI cooperates through an adapter. This RFC defines that adapter contract and states the limits plainly.

The headless model is implementable now. The dialog belongs to RFC-022.

## Motivation

`REQ-AGENT-012` requires an approval UI/API for adapter-mediated commands. `REQ-SEC-012` and `REQ-SEC-013` specify when approval is required and what must be shown. RFC-013 froze a `command_approval` audit family with four action kinds. The domain vocabulary exists — `ApprovalRequest`, `ApprovalDecision`, `RiskLevel`.

What does not exist is the mechanism: nothing proposes a command, nothing decides, nothing executes or refuses. Until that lands, `Managed` remains a label no profile may honestly claim, and four audit families stay unwired.

## The Enforcement Boundary — Stated First

Tekstide launches an AI CLI in a PTY. That process may fork, exec, and write files with the user's full privileges. Intercepting that would require OS-level sandboxing, which the threat model lists as an explicit non-goal (§3.2: *"secure isolation equivalent to containers, VMs, SELinux, AppArmor, pledge/unveil, or Windows AppContainer"*).

Therefore:

> **Command approval is a cooperative protocol, not an enforcement mechanism.** It constrains AI CLIs that choose to ask. It does not and cannot constrain one that does not.

Consequences that this RFC treats as binding:

- `Plain` and `Supervised` AgentRuns get **no approval capability whatsoever**. Not a degraded version — none.
- `Managed` requires proven adapter capability, per RFC-010. A profile claiming `Managed` without a working adapter must be rejected before launch, as RFC-010 already requires.
- No UI, README, or release note may state or imply that Tekstide approves commands generally. It approves commands *an adapter submits*.

## Goals

- Define the adapter capability contract: how a proposed command reaches Tekstide and how a decision returns.
- Define which proposed commands require approval, and the risk classification behind that.
- Define approval decision semantics, including edit-and-approve.
- Correlate approvals with the frozen RFC-013 `command_approval` audit family.
- Keep the exact command available to the decision surface while keeping it out of durable audit.
- Define timeout, disconnect, and failure behavior — every one of which must fail closed.

## Non-Goals

- OS-level sandboxing or syscall interception.
- Approval for `Plain`/`Supervised` runs.
- Pattern-based "always allow" rules. The threat model (§8.3) permits these only if a later RFC defines a safe pattern language and audit policy; this RFC deliberately does not.
- The rendered approval dialog (RFC-022).
- Approval for user-typed terminal commands. `REQ-SEC-011` treats user-created terminal sessions as explicit user action; paste protection already covers the accidental case.
- Shipping a specific vendor adapter. This RFC defines the contract; adopting a concrete AI CLI is separate work.

## Adapter Channel — Out of Band, Mandatory

**The approval channel must not run over the PTY stream.** This is the central security decision in this RFC.

RFC-009 established that terminal output is untrusted and cannot reach trusted UI. T-026 names approval spoofing as a specific threat. An in-band protocol — the CLI emitting a marked escape sequence on stdout — would give untrusted bytes a direct path to trusted decisions, and any process writing to that PTY could forge a request or, worse, a response.

The channel is therefore a **sideband IPC endpoint**, one per AgentRun:

| Property | Requirement |
| --- | --- |
| Transport | Local IPC (Unix domain socket on Unix). Never the PTY stream. Never TCP. |
| Location | Under the Tekstide state root, outside any project root, following the RFC-011/RFC-013 path discipline |
| Lifetime | Created before process start, destroyed on AgentRun termination |
| Authentication | Per-run capability token, generated by Tekstide, passed to the adapter through the environment allowlist, never written to disk unencrypted or into audit |
| Scope | One endpoint serves exactly one AgentRun in one ProjectSession |

An adapter that cannot use the sideband channel does not have the capability, and its profile may not claim `Managed`.

## Protocol

Two message types. Bounded, versioned, and validated exactly as RFC-013 validates durable records.

**Adapter → Tekstide: `CommandProposal`**

- protocol version
- run capability token
- proposal id (adapter-generated, opaque, bounded)
- exact argv (not a shell string — see below)
- working directory
- declared intent, bounded free text for display only
- adapter-declared effects, optional and untrusted

**Tekstide → Adapter: `CommandDecision`**

- proposal id
- decision: `ApprovedOnce`, `Rejected`, or `EditedAndApproved`
- edited argv, present only for `EditedAndApproved`

Rules:

1. **argv, not a shell string.** A shell string invites quoting-based deception, where the rendered command and the executed command differ. The adapter submits an argument vector; the dialog renders it unambiguously.
2. **Adapter-declared effects are untrusted.** A proposal claiming "reads only" is a hint for display, never a basis for skipping approval or lowering risk.
3. **Unknown protocol version fails closed** — reject the proposal, do not guess.
4. **Malformed proposals fail closed** and produce a bounded diagnostic, never a permissive default.
5. **The token is not a secret in audit.** It never appears in a durable record.

## Risk Classification

`RiskLevel` (`Low`, `Medium`, `High`, `Destructive`) already exists and already matches RFC-013's audit vocabulary. This RFC defines how a proposal is classified.

Classification is **conservative and structural**. It inspects argv, not semantics — Tekstide does not interpret shell grammar, and `REQ-AGENT` non-goals exclude semantic command analysis.

Escalate to at least `High` when the proposal:

- names a path outside the canonical project root;
- requests privilege elevation (`sudo`, `doas`, `pkexec`, or platform equivalents);
- targets Git remote-mutating operations (`push`, `remote`, `tag -d`, force variants);
- references paths matching the secret-like patterns already defined for environment redaction;
- writes to the Tekstide state root.

Escalate to `Destructive` when the proposal names recursive deletion, disk-level operations, or history rewriting.

**Unknown is not safe.** A proposal that cannot be classified is `High`, never `Low`.

Classification affects *presentation and audit*, not whether approval is required — see next section.

## When Approval Is Required

**Every adapter-submitted proposal requires approval.** There is no risk threshold below which a proposal executes silently.

This is stricter than `REQ-SEC-012`, which lists conditions requiring approval. The stricter rule is chosen deliberately: a threshold implies Tekstide can reliably distinguish safe from unsafe commands by structural inspection, which it cannot. Risk classification exists to inform the user, not to bypass them.

If this proves unusably noisy in practice, the fix is a reviewed pattern language with its own RFC and audit policy — not a silent threshold.

## Decision Semantics

| Decision | Meaning | Audit action kind |
| --- | --- | --- |
| `ApprovedOnce` | This proposal, once. No future proposal is covered. | `command_approve` |
| `EditedAndApproved` | The user modified argv; the edited vector executes. | `command_edit_and_approve` |
| `Rejected` | The adapter must not execute. | `command_reject` |

Rules:

- Decisions are **single-use and non-transferable**. A decision binds to one proposal id.
- `ApprovalRequest::decide` already enforces append-only transition from `Pending`; reuse it.
- **Edit-and-approve re-classifies.** The edited argv is re-run through risk classification, and the audit record describes what was approved, not what was proposed.
- No decision may be inferred from silence.

## Failure Semantics — All Fail Closed

| Condition | Behavior |
| --- | --- |
| User does not respond | Proposal stays pending indefinitely; the adapter blocks. No timeout auto-approves. |
| Tekstide wants to cancel | Explicit `Rejected`, audited normally |
| Adapter disconnects while pending | Proposal is abandoned and audited as rejected-by-disconnect; no execution is implied |
| AgentRun terminates while pending | Same as disconnect |
| Audit append fails for the authorization | **Launch of the approved command is blocked.** Approval is a required audit write, matching the RFC-013 trust-grant precedent |
| Token invalid or absent | Proposal rejected without a dialog; bounded diagnostic; possible spoofing attempt |
| Two proposals share an id | Second rejected |

The deliberate absence of an approval timeout is a design choice: a timeout that denies is a hidden policy, and one that approves is a vulnerability. Blocking is honest.

## Audit Correlation

The RFC-013 `command_approval` family is frozen and this RFC conforms to it rather than amending it.

| Event | action_kind | operation_id | outcome | actor / source |
| --- | --- | --- | --- | --- |
| Proposal received | `command_request` | none | `requested` | `app_policy` / `adapter` |
| User approves | `command_approve` | required | `authorized` → `applied`/`failed` | `user` / `trusted_ui` |
| User edits and approves | `command_edit_and_approve` | required | `authorized` → `applied`/`failed` | `user` / `trusted_ui` |
| User rejects | `command_reject` | none | `applied` | `user` / `trusted_ui` |

Note the schema requires `approval_id` and `risk_level` present, `terminal_id` and `subject_kind` absent.

**The exact command is never persisted.** RFC-013 deliberately excludes commands, paths, and free text from durable records. `ApprovalRequest.display_command` lives in memory for the dialog; the durable record carries ids, risk level, and outcome only. Two retention policies for the same data, intentionally.

## Data Model Impact

- Adapter capability descriptor, extending RFC-010's `AiCliAdapterCapabilities`, which already has `structured_action_approval`.
- Sideband endpoint handle and per-run capability token (runtime, never persisted).
- `CommandProposal` and `CommandDecision` message types with bounded validation.
- Risk classifier over argv and cwd.
- Approval coordinator correlating proposal → `ApprovalRequest` → audit operation.

`ApprovalRequest` may need `proposal_id` and an argv-shaped field alongside `display_command`.

## Implementation Plan

1. **PR-021-A** — design and handoff acceptance.
2. **PR-021-B** — protocol message types, bounded validation, version negotiation, fail-closed paths.
3. **PR-021-C** — risk classifier with a fixture corpus.
4. **PR-021-D** — sideband endpoint, token generation, listener lifecycle bound to AgentRun.
5. **PR-021-E** — approval coordinator and audit correlation.
6. **PR-021-F** — closeout evidence.

PR-021-B through PR-021-E are headless and need no GUI. RFC-022 renders the dialog over the coordinator.

## Test and Evidence Requirements

- Protocol tests: unknown version, malformed payload, oversized fields, missing token, wrong token, duplicate proposal id — all fail closed.
- **Spoofing tests: a process that is not the adapter cannot submit a proposal or a decision.**
- Risk classifier fixtures covering each escalation rule, plus unclassifiable input yielding `High`.
- Decision tests: single-use binding, edit-and-approve re-classification, no decision from silence.
- Failure tests: disconnect while pending, AgentRun termination while pending, audit-append failure blocking execution.
- Audit tests: correct family/action/outcome per RFC-013, one operation id per approval, and **no command text in any durable record**.
- Regression: `Plain`/`Supervised` runs expose no approval path.

## Acceptance Criteria

- Approval works only over the authenticated sideband channel; the PTY stream is never an approval path.
- Every adapter-submitted proposal requires an explicit decision.
- All failure modes fail closed; no timeout approves.
- Audit conforms to the frozen `command_approval` family without amendment.
- No durable record contains command text.
- `Plain`/`Supervised` gain nothing; `Managed` still requires proven capability.
- Documentation states that approval is cooperative, not enforced.

## Risks

- **Overclaiming.** The largest risk is messaging, not code. Mitigation: the Enforcement Boundary section is normative, and release wording must reflect it.
- **Approval fatigue.** Approving everything may annoy users into inattention. Mitigation: accept for v1; a pattern language needs its own reviewed RFC rather than a quiet threshold.
- **Token leakage.** A leaked token permits proposal injection. Mitigation: environment-allowlist delivery, per-run scope, never persisted, never audited.
- **Adapter ecosystem may not exist.** No AI CLI may implement this contract. Mitigation: the contract is also implementable by a wrapper Tekstide ships later; nothing here assumes vendor cooperation.

## Open Questions

1. Should Tekstide ship a reference wrapper adapter, or define the contract and wait for CLI support?
2. Should `EditedAndApproved` be restricted for `Destructive` proposals, where editing may mask intent?
3. Should proposals persist across a Tekstide restart while an AgentRun is detached, or always be abandoned?

## Scheduling

The headless model (PR-021-B..E) has no GUI dependency and is recommended to start immediately, in parallel with RFC-014. Building the policy now means RFC-022 renders a dialog over reviewed behavior rather than designing safety semantics under UI schedule pressure.
