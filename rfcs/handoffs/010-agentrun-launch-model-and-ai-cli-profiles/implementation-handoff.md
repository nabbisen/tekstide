---
title: "RFC-010: AgentRun Launch Model and AI CLI Profiles — Implementation Handoff"
rfc: "RFC-010"
rfc_file: "../../done/010-agentrun-launch-model-and-ai-cli-profiles.md"
status: "Proposed"
target_milestone: "M5"
source_rfc_status: "Proposed"
created: "2026-07-17"
---

# RFC-010: AgentRun Launch Model and AI CLI Profiles — Implementation Handoff

## Purpose

This handoff translates RFC-010 into implementation guidance for Tekstide's first AgentRun launch model.

Implementation must wait for RFC-010 design review acceptance. This handoff does not authorize transcript byte retention, durable audit storage, final GUI launch/review surfaces, general command approval, or provider-specific AI cloud integration.

## Source RFC Summary

RFC-010 introduces AI CLI profiles as explicit launch contracts. A launch validates project ownership, canonical root/cwd, trust state, profile source, executable provenance, implicit CLI workspace-config discovery behavior, argv/prompt policy, environment policy, compatibility label, transcript policy, and active-file safety before process start.

Launched AgentRuns attach to project-owned TerminalSessions. TerminalSession remains the process/runtime truth; AgentRun stores metadata, references, and lifecycle summary.

## Dependencies and Sequencing

- Target milestone: **M5**.
- Required predecessors: RFC-002, RFC-004, RFC-006, RFC-008, RFC-009.
- RFC-011 owns transcript retention.
- RFC-013 owns durable audit persistence.
- M8 or later GUI work owns rendered launch, approval, transcript, and review surfaces.

## Implementation Boundaries

Keep the first implementation small:

- profile model and validation;
- launch context validation;
- AgentRun launch spec and terminal launch spec construction;
- runtime-backed launch through the existing terminal runtime;
- AgentRun-to-TerminalSession attachment;
- active-document safety assessment;
- model/harness evidence before GUI surfaces.

Do not add:

- transcript byte storage;
- durable audit database;
- automatic workspace profile/env/prompt loading in Restricted Mode;
- workspace-local executables, wrappers, shims, project-local `PATH` resolution, or implicit CLI workspace-config discovery in Restricted Mode;
- managed command approval without adapter capability evidence;
- terminal-output-driven approval/trust decisions;
- raw environment, prompt, terminal output, or file-content diagnostics.

## Suggested Module Shape

Prefer existing boundaries:

- `domain::agent` for AgentRun lifecycle extensions only if necessary;
- `security` for Restricted Mode/profile/environment policy vocabulary if it is shared;
- `runtime::terminal` for terminal-backed launch integration;
- `project::session` for ProjectSession collection updates and attachment orchestration;
- a new `agent` or `project::agent` module only if profile/launch code grows beyond what existing modules can read clearly.

Do not split modules for aesthetics alone. Split when profile validation, launch orchestration, or active-file safety logic becomes large enough to make tests/readability suffer.

## Data Model Guidance

Prefer explicit enums and small structs:

- profile source: built-in, user-global, workspace-local;
- executable provenance: built-in-reviewed, user-global, system-path-reviewed, workspace-local;
- compatibility: Plain, Supervised, Managed;
- cwd policy: project root, in-root relative cwd;
- prompt policy: argv, stdin, interactive/no initial prompt;
- environment policy: minimal, named, allowlist;
- transcript policy: metadata-only until RFC-011;
- active-file launch decision: proceed, requires decision, blocked;
- launch rejection reason: bounded and metadata-only.

Avoid booleans such as `trusted`, `managed`, or `safe` where a caller needs to know the reason or evidence boundary.

## Security Notes

- Restricted Mode must block workspace-local AI profile, prompt, environment, executable, wrapper/shim, project-local `PATH`, and implicit CLI workspace-config loading.
- Built-in profile evidence must document whether the selected AI CLI auto-discovers workspace-local config, tool definitions, prompts, plugins, or agent instruction files.
- In Restricted Mode, CLI workspace auto-discovery must be disabled through reviewed flags/environment or the launch must be rejected.
- `PATH` lookup, if used, is a reviewed lookup result and must not prefer project-local directories in Restricted Mode.
- Symlink or wrapper resolution must treat project-root targets as workspace-local executable provenance.
- Environment summaries may include variable names and policy names, not values.
- Managed launch requires adapter capability evidence; otherwise reject or use a lower compatibility label before launch.
- Terminal output is untrusted and cannot mutate launch, trust, approval, audit, file, or Project Board state.
- Transcript remains metadata-only until RFC-011.
- Durable audit remains deferred to RFC-013.

## Active-File Safety Notes

Before launching an AgentRun, refresh active-document external state when an active text document exists.

Launch may proceed only when the active-document assessment is clean/unchanged or when a reviewed decision path explicitly permits launch with dirty/external-change state. While AgentRuns are active, project/content summaries must continue to expose external-change/conflict state and safe-save must still block overwrite conflicts.

This does not require a full watcher or multi-document UI in RFC-010.

## Review Expectations

Every implementation slice should get a review request before closeout. Reviewers should be able to inspect:

- profile validation and source/trust policy;
- executable provenance and implicit CLI workspace-discovery policy;
- launch rejection before process start;
- AgentRun/TerminalSession attachment rules;
- lifecycle mapping from runtime events;
- Restricted Mode behavior;
- environment/privacy behavior;
- transcript non-persistence;
- active-file safety behavior.
