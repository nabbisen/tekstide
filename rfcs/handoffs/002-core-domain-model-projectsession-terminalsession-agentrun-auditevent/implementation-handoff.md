# RFC-002 Implementation Handoff

Source RFC: [RFC-002](../../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
Target: core domain model

## Purpose

Create stable implementation vocabulary and ownership boundaries before app shell, terminal, AgentRun, persistence, or security implementation begins.

## Required Concepts

- `ProjectSession`
- `TerminalSession`
- `AgentRun`
- `ApprovalRequest`
- `Transcript`
- `ChangeSet`
- `AuditEvent`

## Decisions To Preserve

- Every project-owned entity must carry explicit project ownership.
- Stable IDs are required for persisted entities.
- Paths and display names are not primary identities.
- Runtime handles must be separated from persisted metadata.
- `TerminalSession` and `AgentRun` are related but not conflated.
- Canonical AgentRun states are `Draft`, `Ready`, `Preparing`, `Running`, `AwaitingApproval`, `ReviewReady`, `Completed`, `Failed`, `Cancelled`, and `Detached`.
- `Detached` means Tekstide no longer supervises the process and must not imply recovery or management guarantees.

## Implementation Guidance

- Implement domain types and transition helpers before GUI-heavy workflows.
- Keep invalid state transitions explicit and testable.
- Add cross-project isolation tests early.
- Use in-memory models first if persistence is not ready, but do not erase the persistent/runtime distinction.

## Stop Conditions

Pause for RFC amendment if implementation needs cross-project shared mutable state, path-derived primary IDs, additional persisted AgentRun states, or behavior that treats a plain terminal as a managed AgentRun.
