---
title: "RFC-010: AgentRun Launch Model and AI CLI Profiles — Acceptance / QA Checklist"
rfc: "RFC-010"
rfc_file: "../../done/010-agentrun-launch-model-and-ai-cli-profiles.md"
status: "Accepted with documented limitations"
target_milestone: "M5"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-17"
updated: "2026-07-21"
---

# RFC-010: AgentRun Launch Model and AI CLI Profiles — Acceptance / QA Checklist

## Acceptance Status

This checklist records RFC-010 implementation evidence through PR-010-F. RFC-010 closeout evidence was accepted with documented limitations.

## Scope Checklist

- [x] AI CLI profiles are modeled as launch contracts.
- [x] Launch validates project, root, cwd, profile, executable provenance, implicit CLI workspace-discovery behavior, environment, transcript policy, compatibility, and active-file safety before process start.
- [x] AgentRuns launch through project-owned TerminalSessions.
- [x] AgentRun lifecycle follows runtime/terminal observations.
- [x] Plain/Supervised/Managed labels remain honest.
- [x] Active-document dirty/external-change/conflict state is surfaced before launch and while AgentRuns are active.
- [x] No transcript byte retention is introduced.
- [x] No durable audit storage is introduced.
- [x] No final GUI launch/review surface claim is introduced.
- [x] No general command approval claim is introduced.

## Profile / Launch Checklist

- [x] Unknown or mismatched profile is rejected before process start.
- [x] Unavailable or non-executable profile executable is rejected.
- [x] Workspace-local executable provenance is rejected in Restricted Mode.
- [x] Workspace-local wrappers, shims, or symlink targets are rejected in Restricted Mode.
- [x] Reviewed `PATH` lookup does not prefer project-local directories in Restricted Mode.
- [x] Unknown or mismatched project is rejected.
- [x] Missing project root is rejected.
- [x] Missing cwd or cwd outside canonical root is rejected.
- [x] Profile source is validated against workspace trust state.
- [x] Workspace-local profile loading is blocked in Restricted Mode.
- [x] Workspace-local prompt loading is blocked in Restricted Mode.
- [x] Workspace-local environment loading is blocked in Restricted Mode.
- [x] Implicit CLI workspace-local config/tool/prompt/plugin/instruction discovery is disabled or rejected in Restricted Mode.
- [ ] N/A for M5: no concrete built-in AI CLI profile ships yet, so no per-profile auto-discovery/security evidence is claimed.
- [x] Managed compatibility is rejected before launch without adapter capability evidence.

## Attachment / Lifecycle Checklist

- [x] AgentRun attaches only to a TerminalSession from the same ProjectSession.
- [x] Duplicate same-terminal attachment is idempotent.
- [x] Second terminal attachment to the same AgentRun is rejected.
- [x] Cross-project attachment is rejected.
- [x] Failed validation leaves no optimistic running AgentRun.
- [x] Failed runtime launch leaves no optimistic running AgentRun in this implementation.
- [x] Terminal exit/failure/termination/orphaned state maps to truthful AgentRun lifecycle state.
- [x] Runtime handles and process ids are not persisted as durable truth.

## Security and Privacy Checklist

- [x] Environment summaries list policy/source/names only, not values.
- [x] Launch errors are bounded.
- [x] Launch diagnostics do not include raw prompts.
- [x] Launch diagnostics do not include terminal output.
- [x] Launch diagnostics do not include file contents.
- [x] Transcript policy remains metadata-only pending RFC-011.
- [x] Durable audit remains deferred to RFC-012.
- [x] Terminal output cannot approve launch, mutate trust, synthesize approvals, or dismiss trusted UI.

## Active-File Safety Checklist

- [x] Clean active document permits launch.
- [x] Dirty active document is surfaced before launch.
- [x] Externally changed active document is surfaced before launch.
- [x] Conflict state is surfaced before launch.
- [ ] Reviewed decision path is required when policy blocks launch. Current implementation fails closed because no reviewed override path exists yet.
- [ ] Active-document external state can be refreshed while AgentRuns are active. The model API exists, but no while-running refresh test is claimed yet.
- [x] Safe-save external-change blocking still prevents overwrite conflicts while AgentRuns are active.
- [x] No full watcher or multi-document UI is claimed unless separately implemented and reviewed.

## Automated Test Checklist

- [x] Profile validation tests cover accepted and rejected sources plus executable provenance.
- [x] Restricted Mode tests cover workspace profile/prompt/env/executable blocking.
- [x] Restricted Mode tests cover wrapper/shim/symlink and project-local `PATH` rejection.
- [x] Restricted Mode tests cover implicit CLI workspace-config discovery rejection when it cannot be disabled.
- [x] Environment policy tests prove values are not stored in summaries.
- [x] Launch validation tests cover wrong project, missing root, invalid cwd, escaped cwd, mismatched profile, and unsupported Managed capability.
- [x] AgentRun lifecycle tests cover validation success/failure, runtime launch success/failure, terminal exit, cancellation, and detached/orphaned cases.
- [x] Attachment tests cover same-project success, duplicate idempotence, duplicate-invalid rejection, and cross-project rejection.
- [x] Transcript tests prove no transcript byte persistence is enabled.
- [x] Active-file safety tests cover clean, dirty, externally changed, conflict, save-error, and safe-save while AgentRun active states.
- [x] Terminal security regression tests prove terminal output remains untrusted terminal content and cannot synthesize trusted UI decisions or blocked app effects.

## Release Evidence Required

Attach or link the following evidence before marking RFC-010 implemented:

- [x] Commit/PR list. See `qa-evidence.md` implementation sections for PR-010-B through PR-010-F and review request references 066-073.
- [x] Test command output. See `qa-evidence.md` observed gates.
- [x] Profile/source/trust validation summary. See PR-010-B evidence.
- [x] Environment policy evidence. See PR-010-B, PR-010-C, and PR-010-D evidence.
- [x] AgentRun/TerminalSession lifecycle and attachment evidence. See PR-010-C and PR-010-D evidence.
- [x] Active-file safety evidence. See PR-010-E evidence.
- [x] Security/privacy note. See per-slice security/privacy notes in `qa-evidence.md`.
- [x] Migration note or "no migration" statement. No migration is required; data shapes are in-memory core/domain models only for this milestone.
- [x] Known limitations. See `qa-evidence.md`.
- [x] Follow-up RFCs/issues for transcript retention, durable audit, GUI surfaces, watcher/multi-document workflows, and review surfaces. See Known Limitations and follow-up notes in `qa-evidence.md`.
- [x] RFC lifecycle transition. RFC-010 moved from `rfcs/proposed/` to `rfcs/done/`, `rfcs/README.md` was updated, and inbound references were swept after closeout review accepted the implemented state.

## Final Acceptance Decision

- [x] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Accepted on 2026-07-21 by `.git-exclude/reviewed/tekstide-review-request-075-rfc010-pr010f-closeout-evidence-rereview-response.md`.
```
