---
title: "RFC-010: AgentRun Launch Model and AI CLI Profiles — Acceptance / QA Checklist"
rfc: "RFC-010"
rfc_file: "../../proposed/010-agentrun-launch-model-and-ai-cli-profiles.md"
status: "Proposed"
target_milestone: "M5"
source_rfc_status: "Proposed"
created: "2026-07-17"
---

# RFC-010: AgentRun Launch Model and AI CLI Profiles — Acceptance / QA Checklist

## Acceptance Status

This checklist is proposed. It becomes the implementation acceptance checklist only after RFC-010 design review accepts or amends the scope.

## Scope Checklist

- [ ] AI CLI profiles are modeled as launch contracts.
- [ ] Launch validates project, root, cwd, profile, executable provenance, implicit CLI workspace-discovery behavior, environment, transcript policy, compatibility, and active-file safety before process start.
- [ ] AgentRuns launch through project-owned TerminalSessions.
- [ ] AgentRun lifecycle follows runtime/terminal observations.
- [ ] Plain/Supervised/Managed labels remain honest.
- [ ] Active-document dirty/external-change/conflict state is surfaced before launch and while AgentRuns are active.
- [ ] No transcript byte retention is introduced.
- [ ] No durable audit storage is introduced.
- [ ] No final GUI launch/review surface claim is introduced.
- [ ] No general command approval claim is introduced.

## Profile / Launch Checklist

- [ ] Unknown profile is rejected before process start.
- [ ] Unavailable or non-executable profile executable is rejected.
- [ ] Workspace-local executable provenance is rejected in Restricted Mode.
- [ ] Workspace-local wrappers, shims, or symlink targets are rejected in Restricted Mode.
- [ ] Reviewed `PATH` lookup does not prefer project-local directories in Restricted Mode.
- [ ] Unknown or mismatched project is rejected.
- [ ] Missing project root is rejected.
- [ ] Missing cwd or cwd outside canonical root is rejected.
- [ ] Profile source is validated against workspace trust state.
- [ ] Workspace-local profile loading is blocked in Restricted Mode.
- [ ] Workspace-local prompt loading is blocked in Restricted Mode.
- [ ] Workspace-local environment loading is blocked in Restricted Mode.
- [ ] Implicit CLI workspace-local config/tool/prompt/plugin/instruction discovery is disabled or rejected in Restricted Mode.
- [ ] Built-in profile evidence documents CLI auto-discovery behavior and Restricted Mode disabling/blocking behavior.
- [ ] Managed compatibility is rejected or downgraded without adapter capability evidence.

## Attachment / Lifecycle Checklist

- [ ] AgentRun attaches only to a TerminalSession from the same ProjectSession.
- [ ] Duplicate same-terminal attachment is idempotent.
- [ ] Second terminal attachment to the same AgentRun is rejected.
- [ ] Cross-project attachment is rejected.
- [ ] Failed validation leaves no optimistic running AgentRun.
- [ ] Failed runtime launch is represented as failed metadata only when explicitly modeled.
- [ ] Terminal exit/failure/termination/orphaned state maps to truthful AgentRun lifecycle state.
- [ ] Runtime handles and process ids are not persisted as durable truth.

## Security and Privacy Checklist

- [ ] Environment summaries list policy/source/names only, not values.
- [ ] Launch errors are bounded.
- [ ] Launch diagnostics do not include raw prompts.
- [ ] Launch diagnostics do not include terminal output.
- [ ] Launch diagnostics do not include file contents.
- [ ] Transcript policy remains metadata-only pending RFC-011.
- [ ] Durable audit remains deferred to RFC-012.
- [ ] Terminal output cannot approve launch, mutate trust, synthesize approvals, or dismiss trusted UI.

## Active-File Safety Checklist

- [ ] Clean active document permits launch.
- [ ] Dirty active document is surfaced before launch.
- [ ] Externally changed active document is surfaced before launch.
- [ ] Conflict state is surfaced before launch.
- [ ] Reviewed decision path is required when policy blocks launch.
- [ ] Active-document external state can be refreshed while AgentRuns are active.
- [ ] Safe-save external-change blocking still prevents overwrite conflicts while AgentRuns are active.
- [ ] No full watcher or multi-document UI is claimed unless separately implemented and reviewed.

## Automated Test Checklist

- [ ] Profile validation tests cover accepted and rejected sources plus executable provenance.
- [ ] Restricted Mode tests cover workspace profile/prompt/env/executable blocking.
- [ ] Restricted Mode tests cover wrapper/shim/symlink and project-local `PATH` rejection.
- [ ] Restricted Mode tests cover implicit CLI workspace-config discovery rejection when it cannot be disabled.
- [ ] Environment policy tests prove values are not stored in summaries.
- [ ] Launch validation tests cover unknown project, wrong project, missing root, invalid cwd, escaped cwd, unknown profile, and unsupported Managed capability.
- [ ] AgentRun lifecycle tests cover validation success/failure, runtime launch success/failure, terminal exit, cancellation, and detached/orphaned cases.
- [ ] Attachment tests cover same-project success, duplicate idempotence, duplicate-invalid rejection, and cross-project rejection.
- [ ] Transcript tests prove no transcript byte persistence is enabled.
- [ ] Active-file safety tests cover clean, dirty, externally changed, and conflict states.
- [ ] Terminal security regression tests prove terminal output cannot mutate launch/trust/approval/file/project state.

## Release Evidence Required

Attach or link the following evidence before marking RFC-010 implemented:

- [ ] Commit/PR list.
- [ ] Test command output.
- [ ] Profile/source/trust validation summary.
- [ ] Environment policy evidence.
- [ ] AgentRun/TerminalSession lifecycle and attachment evidence.
- [ ] Active-file safety evidence.
- [ ] Security/privacy note.
- [ ] Migration note or "no migration" statement.
- [ ] Known limitations.
- [ ] Follow-up RFCs/issues for transcript retention, durable audit, GUI surfaces, watcher/multi-document workflows, and review surfaces.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [ ] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Pending design review.
```
