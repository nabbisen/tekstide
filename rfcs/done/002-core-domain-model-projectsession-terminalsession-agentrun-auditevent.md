# RFC-002: Core Domain Model: ProjectSession, TerminalSession, AgentRun, AuditEvent

Status: Implemented  
Target milestone: M0-M1  
Date: 2026-07-04

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-appendix-a-extensibility-plugin-v0.md`
- `tekstide-roadmap-milestones-v0.md`


## Summary

This RFC defines Tekstide's core domain entities. The model makes multi-project state explicit and separates terminal processes, AI AgentRuns, approvals, transcripts, diffs, and audit events. This prevents the app from becoming a loose terminal multiplexer with an editor attached.

## Motivation

Implementation needs stable vocabulary before code begins. ProjectSession, TerminalSession, AgentRun, ApprovalRequest, Transcript, ChangeSet, and AuditEvent must have clear ownership and lifecycle semantics. Without this model, UI, persistence, security, and tests will diverge.

## Goals

- Define core entities and IDs.
- Define ownership relationships.
- Define lifecycle states.
- Define cross-project isolation rules.
- Define which data is persistent and which is runtime-only.
- Provide a base model for later implementation RFCs.

## Non-Goals

- Define database schema details.
- Choose a GUI framework.
- Define exact serialization format.
- Define plugin API types.
- Define remote project objects.

## Detailed Design

## 1. Entity overview

```text
AppState
├─ ProjectSession[]
│  ├─ WorkspaceTrust
│  ├─ OpenSurface
│  ├─ FileState
│  ├─ TerminalSession[]
│  ├─ AgentRun[]
│  ├─ ApprovalRequest[]
│  ├─ ChangeSet[]
│  └─ AuditEvent[]
└─ GlobalConfig
```

## 2. Identity rules

All persisted entities have stable IDs. IDs are local-only and need not be globally meaningful.

Recommended ID forms:

- `ProjectId`
- `TerminalId`
- `AgentRunId`
- `ApprovalId`
- `TranscriptId`
- `ChangeSetId`
- `AuditEventId`

IDs should not be derived solely from file paths because project roots may move. A project may store its canonical root path, display name, and optional stable fingerprint.

## 3. ProjectSession

A `ProjectSession` represents one local project root loaded in Tekstide.

Fields:

- `id`
- `display_name`
- `root_path`
- `canonical_root_path`
- `trust_state`
- `created_at`
- `last_opened_at`
- `last_activity_at`
- `open_surface`
- `mode`
- `resource_limits`
- `terminal_sessions`
- `agent_runs`
- `pending_approvals`
- `dirty_file_count`
- `git_summary`
- `warning_state`

A ProjectSession owns all terminals and AgentRuns launched from it. It must never implicitly share process state with another ProjectSession.

## 4. WorkspaceTrust

Trust states:

| State | Meaning |
|---|---|
| Unknown | Project has not been evaluated. Treat as Restricted. |
| Restricted | Project opened but automation is disabled. |
| Trusted | User explicitly trusts the workspace. |
| Revoked | Trust was previously granted and later removed. |

Trust must be project-root-specific. If the root path changes or resolves differently, Tekstide must avoid reusing trust silently.

## 5. TerminalSession

A `TerminalSession` represents a local PTY-backed process.

Fields:

- `id`
- `project_id`
- `kind`: plain, supervised, managed
- `title`
- `cwd`
- `command_line_summary`
- `pid` or platform process handle reference
- `status`: starting, running, exited, failed, terminating, orphaned_unknown
- `visible_slot`: hidden, primary, secondary
- `created_at`
- `last_output_at`
- `exit_status`
- `transcript_ref`
- `environment_policy_ref`

A TerminalSession may exist without an AgentRun. A plain user shell is a TerminalSession, not necessarily an AgentRun.

## 6. AgentRun

An `AgentRun` represents a supervised AI CLI work unit.

Fields:

- `id`
- `project_id`
- `profile_id`
- `terminal_id`
- `prompt_summary`
- `full_prompt_ref`
- `status`
- `compatibility_level`: managed, supervised, plain
- `started_at`
- `ended_at`
- `transcript_ref`
- `approval_ids`
- `change_set_ids`
- `artifact_refs`
- `audit_event_ids`

AgentRun status:

| Status | Meaning |
|---|---|
| Draft | Created but not launched. |
| Ready | Prompt, profile, working directory, environment policy, trust state, and transcript policy are validated for launch. |
| Preparing | Launch is in progress. This may be transient, but can be persisted for crash recovery. |
| Running | Process is active. |
| AwaitingApproval | Tekstide has intercepted or received a risky action request. |
| ReviewReady | Process produced changes or artifacts requiring review. |
| Completed | Process exited successfully and no required review remains. |
| Failed | Process failed. |
| Cancelled | User cancelled the run. |
| Detached | Tekstide no longer supervises the process. This must not imply active lifecycle guarantees. |

## 7. ApprovalRequest

An approval request represents a user decision about a command/action.

Fields:

- `id`
- `project_id`
- `agent_run_id`
- `requested_action_kind`
- `display_command`
- `risk_level`
- `cwd`
- `environment_summary`
- `created_at`
- `decision`: pending, approved_once, rejected, edited_and_approved
- `decided_at`
- `decision_audit_event_id`

Approval requests are append-only after decision except for user annotations.

## 8. Transcript

A transcript stores process input/output for review and recovery.

Fields:

- `id`
- `project_id`
- `terminal_id`
- `agent_run_id` optional
- `storage_path`
- `byte_count`
- `truncation_state`
- `retention_policy`
- `created_at`
- `last_write_at`

Transcript storage is local. Retention policy is configurable.

## 9. ChangeSet

A ChangeSet groups detected generated changes.

Fields:

- `id`
- `project_id`
- `agent_run_id` optional
- `baseline_snapshot_ref`
- `changed_files`
- `summary`
- `review_state`: unreviewed, accepted, partially_accepted, rejected, superseded
- `created_at`

## 10. AuditEvent

AuditEvent is an append-only local record of security- and lifecycle-significant actions.

Event classes:

- project_added
- trust_granted
- trust_revoked
- terminal_started
- agent_run_started
- command_approval_requested
- command_approved
- command_rejected
- paste_blocked
- process_terminated
- safe_close_decision
- config_changed
- transcript_purged

Audit events must be linked to project/session/run IDs where applicable.

## User Experience Impact

The model supports user-visible status summaries:

- Project Board cards derive counts from TerminalSession, AgentRun, ApprovalRequest, and ChangeSet.
- Content Mode can show AgentRun details.
- Safe Close can list TerminalSessions and AgentRuns by project.

## Security and Privacy Impact

The domain model enforces security boundaries by ownership. A TerminalSession, AgentRun, ApprovalRequest, Transcript, ChangeSet, and AuditEvent all belong to exactly one ProjectSession unless explicitly global. Cross-project operations must be modeled explicitly and audited.

Trust state must be checked before launching project-specific automation. ApprovalRequest decisions must be recorded before execution when Tekstide has command-interception capability.

## Test Plan

- Unit tests for lifecycle transitions.
- Unit tests preventing cross-project entity attachment.
- Serialization tests once persistence exists.
- Property-style tests for invalid state transitions if feasible.

## Acceptance Criteria

- The codebase has explicit types for ProjectSession, TerminalSession, AgentRun, ApprovalRequest, ChangeSet, Transcript metadata, and AuditEvent.
- Each entity has a stable ID.
- Project ownership is explicit and tested.
- Runtime-only and persistent fields are documented.
- AgentRun and TerminalSession are related but not conflated.

## Risks and Mitigations

- Over-modeling could slow early implementation. Mitigation: define the model but allow in-memory implementation first.
- Under-modeling approvals could weaken security later. Mitigation: ApprovalRequest is included from the start even if managed adapters arrive later.

## Open Questions

- Should ChangeSet snapshots be content-addressed immediately, or initially represented by file paths and timestamps?
- Should transcripts be line/event structured from v0.1.0, or raw stream plus metadata?

## Implementation Handoff Checklist

- Implement domain types before UI feature work.
- Keep constructors narrow enough to enforce project ownership.
- Add lifecycle transition helpers instead of free-form status mutation.
