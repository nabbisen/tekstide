# RFC-004: Security Baseline and Restricted Mode

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

This RFC defines the baseline security behavior for Tekstide MVP: local-first trust boundaries, Restricted Mode, AI CLI safety levels, command approval, paste protection, transcript privacy, safe close, and auditability.

## Motivation

Tekstide runs terminals and AI CLIs in local project directories. That makes it powerful and dangerous. The product must not auto-execute repository-provided behavior, must not imply protection it cannot provide, and must make risky operations explicit.

## Goals

- Define MVP security principles.
- Define workspace trust behavior.
- Define Restricted Mode restrictions.
- Define managed/supervised/plain AI CLI security levels.
- Define minimum approval/audit requirements.
- Define transcript privacy behavior.
- Define safe close as a security requirement.

## Non-Goals

- Provide VM/container-grade isolation.
- Detect malware.
- Prevent all user-approved destructive actions.
- Fully sandbox arbitrary shells.
- Verify every downloaded tool.
- Build plugin marketplace security.

## Detailed Design

## 1. Security principles

- Local-first: data stays local unless a user-run tool sends it elsewhere.
- Restricted by default: unknown projects do not auto-run project-specific automation.
- Explicit authority: risky actions require clear user decisions.
- Honest compatibility: Tekstide distinguishes what it can intercept from what it merely supervises.
- Project isolation: project state is not silently mixed.
- Auditability: security-relevant decisions are recorded locally.

## 2. Trust states

| State | Default behavior |
|---|---|
| Unknown | Treat as Restricted. |
| Restricted | Disable automation and workspace-local extensions. |
| Trusted | Allow configured project features. |
| Revoked | Return to Restricted and record audit event. |

Trust must be explicitly granted by the user. Opening a folder must not imply trust.

## 3. Restricted Mode restrictions

In Restricted Mode Tekstide must block or require explicit one-time confirmation for:

- automatic LSP startup;
- workspace-local AI CLI profile loading;
- workspace-local environment loading;
- workspace-local plugin loading;
- automatic formatter/test/build task execution;
- Git hooks initiated by Tekstide;
- command palette entries sourced from the workspace;
- background automation from project files.

Plain terminal use remains possible, because the user may intentionally run commands. The UI must label it as user-controlled and not protected by managed approval.

## 4. AI CLI security levels

| Level | Meaning | Security promise |
|---|---|---|
| Managed | Tekstide adapter can intercept structured command/action requests. | Command approval can be enforced for supported actions. |
| Supervised | Tekstide launches/captures process but cannot intercept all commands. | Transcript, lifecycle, and warning only. |
| Plain | Ordinary terminal session. | No AI-specific safety promise. |

Tekstide must not display supervised/plain sessions as if command approval is guaranteed.

Profile and adapter configuration may use more technical capability terms, but the UI should use the labels above.

| Internal/profile capability | User-facing mode |
|---|---|
| none / user-started terminal | Plain Terminal |
| `terminal-native` | Supervised AgentRun |
| `adapter-mediated` | Managed AgentRun |
| `structured-protocol` | Future Managed AgentRun, deferred from MVP |

## 5. Command approval baseline

For managed actions Tekstide must show:

- project name and root path;
- command/action text;
- working directory;
- risk classification;
- environment summary if available;
- approve/reject/edit options if supported;
- audit record after decision.

For supervised/plain sessions, Tekstide may provide paste protection and terminal warning labels but cannot claim command interception.

Normative non-claims:

- Tekstide does not guarantee command interception for Supervised or Plain sessions.
- Tekstide does not provide VM/container-grade isolation in MVP.
- Tekstide does not fully sandbox arbitrary shells.

## 6. Transcript privacy

Transcripts may include secrets. MVP requirements:

- local storage only;
- default-on bounded capture for Tekstide-created AgentRuns;
- visible retention setting and per-run opt-out before launch;
- configurable retention;
- purge command;
- clear transcript location policy;
- no automatic upload;
- avoid indexing transcripts for global search in MVP unless explicitly enabled.
- no unbounded silent transcript accumulation; storage must be bounded by a retention or size policy before transcript persistence ships.

Tekstide may redact known secret-like environment variable values from structured metadata, audit summaries, and UI summaries when those values are available through Tekstide-managed launch metadata. It must not claim complete redaction of arbitrary terminal output, transcripts, generated diffs, pasted prompts, or third-party AI CLI output.

## 7. Environment policy

Tekstide must avoid automatic `.env` loading in Restricted Mode. In Trusted mode, environment inheritance should be explicit and configurable.

Minimum environment policies:

- inherit host environment;
- clean/minimal environment;
- profile-defined environment additions;
- blocklist/allowlist for sensitive variables as future extension.

## 8. File-system policy

Editor, explorer, watcher, Git status, and diff detection operate under the project root. Symlinks escaping the root must be detected. MVP behavior may be conservative: show warning and require explicit open/permission rather than silently following.

## 9. Safe close policy

Closing Tekstide with running processes must show a decision dialog. The user must not be surprised by killed processes or by invisible processes left running.

Choices:

- cancel close;
- terminate selected processes;
- keep/detach selected processes if supported;
- save state and close after decisions.

## User Experience Impact

Security UX must be clear but not alarmist. Restricted Mode should explain blocked actions in plain language and provide a trust decision path.

## Security and Privacy Impact

This entire RFC is security-relevant. Implementations must add tests for each security promise. UI labels must be honest: a safety feature that only applies to managed adapters must not be described as universal.

## Test Plan

- Restricted Mode tests for blocked features.
- Approval decision tests.
- Transcript purge tests.
- Symlink escape tests.
- Safe close tests.
- UI tests for compatibility level labels.

## Acceptance Criteria

Closed by this RFC at the policy/read-model baseline:

- Newly opened projects enter Restricted/Unknown state.
- Restricted Mode policy prevents auto LSP/profile/plugin/env/task loading.
- Trust decisions are explicit and audited at the domain level.
- AI CLI sessions have managed/supervised/plain policy labels.
- Managed command approval is represented as Managed-only eligibility and audit vocabulary.
- Transcript persistence is blocked until local, bounded, opt-out-capable, purgeable storage exists.
- Safe-close assessment requires confirmation for running processes and other active resources.

Deferred runtime/storage/GUI enforcement:

- command approval execution UI and adapter integration;
- transcript byte storage and purge backend;
- terminal and AgentRun launch enforcement;
- paste protection behavior;
- LSP/plugin/profile/prompt/environment launch wiring;
- GUI trust dialogs and safe-close dialogs;
- audit persistence backend.

## Risks and Mitigations

- Too many warnings may train users to click through. Mitigation: show warnings only for meaningful risk and make Project Board attention calm.
- Users may expect full sandboxing. Mitigation: explicit non-goals and clear labels.

## Open Questions

- Should Trusted state expire when project root Git remote changes?
- Should Tekstide support per-action trust grants before full workspace trust?

## Implementation Handoff Checklist

- Implement Restricted Mode checks as policy functions, not scattered UI conditionals.
- Add audit event creation to trust and approval transitions.
- Make compatibility level visible in all AgentRun/terminal surfaces.

## Implementation Scope Closed By This RFC

RFC-004 is closed for the foundation stage as the security policy and read-model baseline.

Implemented:

- central Restricted Mode policy functions;
- trust-state vocabulary and effective trust mapping;
- display-facing Restricted Mode summaries;
- AI session security labels and Managed-only approval eligibility;
- transcript persistence guardrails;
- narrow redaction-scope vocabulary;
- trust/audit helper constructors;
- safe-close assessment vocabulary and conservative close-resource evaluation.

Deferred to later RFCs/slices:

- terminal process launch enforcement;
- AgentRun launch enforcement;
- command approval execution UI and adapter integration;
- paste protection behavior;
- LSP/plugin/profile/prompt/environment launch wiring;
- transcript byte storage and purge backend;
- GUI trust dialogs and safe-close dialogs;
- audit persistence backend;
- OS sandboxing/container isolation, which remains a non-goal unless separately designed.
