---
title: "RFC-021: Command Approval Model and Adapter Capability - Task Breakdown and PR Plan"
rfc: "RFC-021"
rfc_file: "../../done/021-command-approval-model-and-adapter-capability.md"
target_milestone: "M11"
created: "2026-07-28"
updated: "2026-07-28"
---

# RFC-021 Task Breakdown and PR Plan

Six slices. All of B through E are headless and may run in parallel with RFC-014.

## PR-021-A — Design and handoff acceptance

Maintainer sign-off on the enforcement boundary, the out-of-band channel decision, and the approve-everything policy.

## PR-021-B — Protocol types and validation

Scope:

- `CommandProposal` / `CommandDecision` types with bounded fields.
- Strict decoding modelled on `audit/record.rs`: unknown version rejected, malformed rejected, oversized rejected, argv required as a vector.
- Adapter-declared effects accepted but marked untrusted at the type level if practical.

Review gate:

- Every malformed-input path fails closed with a bounded, content-free diagnostic.
- A shell-string proposal is rejected rather than split.
- No permissive default anywhere in decoding.

## PR-021-C — Risk classifier

Scope:

- Structural classifier over argv and cwd.
- All escalation rules from `implementation-handoff.md` §4.
- Fixture corpus with expected classifications.

Review gate:

- Unclassifiable input classifies `High`.
- Corpus covers each escalation rule and both directions — escalating cases escalate, ordinary cases do not.
- No shell-grammar interpretation.

Reviewer focus: I will add argv forms your corpus omits, particularly indirect paths that escape the project root.

## PR-021-D — Sideband channel

Scope:

- Unix domain socket endpoint per AgentRun, under the state root, outside project roots.
- Per-run capability token; environment-allowlist delivery.
- Listener lifecycle bound to AgentRun start and termination.
- Socket permissions restricted to the owning user.

Review gate:

- **Impersonation rejected** — a non-adapter process cannot submit a proposal or a decision.
- Token from another run rejected.
- Path validation matches the RFC-011/RFC-013 discipline; symlink redirection rejected.
- Endpoint destroyed on termination; no orphaned sockets.
- Token never persisted and never audited.

This is the slice most likely to contain a real vulnerability. Expect empirical probing.

## PR-021-E — Approval coordinator and audit correlation

Scope:

- Proposal → `ApprovalRequest` → decision → adapter response.
- Audit events via `AuditCoordinator`, conforming to the frozen `command_approval` family.
- Full fail-closed matrix.
- Edit-and-approve re-classification.

Review gate:

- Audit conforms to the schema CHECK constraints without amendment.
- One operation id per approval; authorization precedes execution.
- Audit-append failure blocks execution.
- **No command text in the durable store** — sentinel test.
- Pending proposals never execute on timeout, disconnect, or termination.

## PR-021-F — Closeout evidence

Scope: checklist, QA evidence, known limitations, decision on the open questions, and a statement of exactly what Tekstide may claim about approval.

Review gate: the claim statement must survive the honesty test — it may not imply enforcement.

## Sequencing

B → C can be parallel. D is independent of both. E needs B, C, and D.

If PR-021-D cannot establish impersonation resistance, **stop and escalate** rather than proceeding to E. An approval channel that can be impersonated is worse than no approval channel, because it manufactures false confidence.
