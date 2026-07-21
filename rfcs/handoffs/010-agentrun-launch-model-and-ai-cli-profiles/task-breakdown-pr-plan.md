---
title: "RFC-010: AgentRun Launch Model and AI CLI Profiles — Task Breakdown / PR Plan"
rfc: "RFC-010"
rfc_file: "../../done/010-agentrun-launch-model-and-ai-cli-profiles.md"
status: "Proposed"
target_milestone: "M5"
source_rfc_status: "Proposed"
created: "2026-07-17"
---

# RFC-010: AgentRun Launch Model and AI CLI Profiles — Task Breakdown / PR Plan

## Planning Assumptions

- RFC-010 starts after the 0.2.0 terminal/security foundation is released.
- The first implementation target is `tekstide-core` model/runtime behavior.
- Review must accept the RFC/handoff before implementation begins.
- Final GUI surfaces are not required for RFC-010 acceptance.
- Transcript retention and durable audit remain later RFCs.

## PR Sequence Overview

- PR-010-A: Design/handoff acceptance.
- PR-010-B: AI CLI profile model and launch validation.
- PR-010-C: AgentRun launch spec and terminal attachment orchestration.
- PR-010-D: Runtime-backed AgentRun launch lifecycle.
- PR-010-E: Active-file safety integration.
- PR-010-F: Closeout evidence and release-scope alignment.

## PR-010-A — Design / Handoff Acceptance

Purpose:

- Review and accept RFC-010 before implementation begins.

Developer tasks:

- Create proposed RFC and handoff pack.
- Ask review on scope, trust boundaries, profile model, active-file safety, and implementation sequencing.
- Apply review feedback before implementation.

Review focus:

- Scope is suitable for M5.
- RFC-011/RFC-012/RFC-013 dependencies are not overclaimed.
- Active-file safety is placed correctly.
- Implementation slices are reviewable.

## PR-010-B — AI CLI Profile Model and Launch Validation

Purpose:

- Establish profiles as launch contracts and reject invalid launch requests before process start.

Developer tasks:

- Add profile source, executable provenance, compatibility, cwd, prompt, environment, implicit CLI workspace-discovery, and transcript policy vocabulary.
- Add launch request/spec validation.
- Block workspace-local profile/prompt/env/executable loading and implicit CLI workspace-config discovery in Restricted Mode.
- Validate executable availability, executable provenance, cwd containment, project ownership, reviewed `PATH` lookup behavior, and Managed adapter capability.
- Add bounded metadata-only launch rejection summaries.

Review focus:

- Profiles do not become hidden workspace automation.
- Built-in/user-global profiles cannot resolve to workspace-local executables or auto-load workspace-local config in Restricted Mode.
- Restricted Mode behavior matches RFC-004.
- Environment values and prompt text are not leaked in diagnostics.
- Managed labels do not overclaim.

## PR-010-C — AgentRun Launch Spec and Terminal Attachment

Purpose:

- Connect validated AgentRun launch metadata to project-owned TerminalSessions.

Developer tasks:

- Build `AgentRunLaunchSpec` or equivalent from validated profile/context.
- Construct matching `TerminalLaunchSpec`.
- Add project-level orchestration for creating AgentRun metadata, adding terminal sessions, and attaching references.
- Reject cross-project and duplicate-invalid AgentRun/TerminalSession attachments.
- Keep runtime handles out of persisted metadata.

Review focus:

- TerminalSession remains process truth.
- Attachment rules are strict and tested.
- No optimistic running AgentRun survives failed validation.

## PR-010-D — Runtime-Backed AgentRun Launch Lifecycle

Purpose:

- Launch AgentRuns through the existing terminal runtime and map runtime observations into AgentRun lifecycle.

Developer tasks:

- Start the selected AI CLI through the terminal runtime.
- Transition AgentRun from Draft/Ready/Preparing/Running based on validation and runtime events.
- Map terminal exit/failure/termination/orphaned states into truthful AgentRun summaries.
- Add Linux smoke where practical for the first supported profile flow.

Review focus:

- Lifecycle follows observed process facts.
- Failed launch is represented without private output or environment dumps.
- Plain/Supervised/Managed labels remain honest.

## PR-010-E — Active-File Safety Integration

Purpose:

- Surface active-document dirty/external-change/conflict state before launch and while AgentRuns are active.

Developer tasks:

- Refresh active-document external state before launch when an active document exists.
- Add active-file launch assessment/read model.
- Block or require a decision when active document state is dirty, externally changed, or conflicted according to reviewed policy.
- Preserve safe-save external-change blocking while AgentRuns are active.
- Add model/harness tests for clean, dirty, external-change, and conflict cases.

Review focus:

- Launch does not hide editor conflicts.
- Existing RFC-006 safe-save behavior remains intact.
- Scope does not balloon into full watcher/multi-document UI.

## PR-010-F — Closeout Evidence

Purpose:

- Convert implementation into reviewed M5 evidence.

Developer tasks:

- Run formatting, workspace checks, and targeted tests.
- Record profile validation and launch rejection evidence.
- Record Restricted Mode, executable-provenance, CLI workspace-discovery, and environment-policy evidence.
- Record lifecycle/attachment evidence.
- Record active-file safety evidence.
- Record known limitations and follow-up RFCs for transcript, audit, GUI, watcher, and review surfaces.

Review focus:

- Evidence supports every accepted RFC-010 claim.
- Deferred transcript/audit/GUI claims remain visible.
- The next RFC, RFC-011, can proceed without ambiguous AgentRun launch behavior.

## Suggested Review Gates

1. Design gate: RFC-010 and handoff accepted.
2. Profile gate: profile model and launch validation reviewed.
3. Attachment gate: AgentRun/TerminalSession ownership reviewed.
4. Runtime gate: launch lifecycle evidence reviewed.
5. Active-file gate: editor safety behavior reviewed.
6. Closeout gate: evidence package and known limitations accepted.

## Stop Conditions

Pause and request RFC amendment or design review if:

- a profile needs to load workspace-local config in Restricted Mode;
- a profile resolves to a workspace-local executable, wrapper, shim, or project-local `PATH` entry in Restricted Mode;
- a selected AI CLI performs implicit workspace-local config/tool/prompt/plugin discovery that cannot be disabled in Restricted Mode;
- Managed launch requires claims without adapter capability evidence;
- transcript bytes must be stored before RFC-011;
- durable audit must be introduced before RFC-013;
- launch diagnostics need environment values, raw prompts, terminal output, or file contents;
- AgentRun state would need to claim process truth independent of TerminalSession/runtime facts;
- active-file safety requires a full watcher or multi-document UI to be credible;
- terminal output would need to mutate launch, trust, approval, audit, file, or Project Board state.
