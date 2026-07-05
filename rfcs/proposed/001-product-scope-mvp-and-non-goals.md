# RFC-001: Product Scope, MVP, and Non-Goals

Status: Proposed  
Target milestone: M0  
Date: 2026-07-04

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-appendix-a-extensibility-plugin-v0.md`
- `tekstide-roadmap-milestones-v0.md`


## Summary

This RFC freezes the product scope for Tekstide v0.1.0 MVP. Tekstide is a local-first, multi-project GUI workbench for AI CLI-driven development. The MVP is not a full VS Code replacement, not a cloud IDE, not a plugin marketplace, and not a complete security sandbox. It must prove the core workflow: supervise several local projects and AI CLI runs safely from one fast GUI.

## Motivation

The initial concept began as a dual-mode editor/terminal. Review identified that the project goal is larger: concurrent development workflows for multiple projects with AI CLI tools. Without a scope freeze, implementation may over-index on editor features or terminal features while missing the differentiator: project-level orchestration and safe AI CLI supervision.

## Goals

- Define Tekstide's MVP identity.
- Make multi-project workflow first-class.
- Make AI CLI run supervision first-class.
- Preserve the dual-mode focused workspace inside each project.
- Define explicit non-goals to prevent scope creep.
- Establish the release target for v0.1.0.

## Non-Goals

- Build a complete IDE feature set.
- Replace every existing editor, terminal, Git client, or debugger.
- Provide a cloud service.
- Guarantee security isolation for arbitrary commands.
- Provide a public plugin marketplace.
- Provide remote SSH/container development.
- Provide deep integration with every AI CLI.

## Detailed Design

## 1. Product definition

Tekstide is a desktop application for developers who delegate development work to terminal-based AI CLIs while maintaining human oversight.

It provides:

- a global Project Board for concurrent projects;
- per-project focused editing;
- per-project terminal/agent immersion;
- AI CLI profile launch;
- AgentRun state tracking;
- transcript capture;
- generated diff/artifact review;
- command approval where supported;
- local audit history;
- workspace trust controls.

## 2. MVP feature set

The v0.1.0 MVP includes:

### 2.1. Project Board

- Add local project roots.
- Display multiple ProjectSessions.
- Show trust state, active terminals, active AgentRuns, pending approvals, dirty files, and last activity.
- Switch active project quickly.
- Surface projects requiring attention without noisy dashboards.

### 2.2. Active Project Workspace

- Content Mode: explorer + one main content surface.
- Terminal / Agent Immersion Mode: full-window terminal/agent view.
- Instant mode switching by keyboard.
- State retention across mode switches.

### 2.3. Editor and file explorer

- Open, view, edit, and save text/source files under the project root.
- Show basic file tree.
- Detect external changes.
- Avoid silent symlink escape.
- Provide basic syntax highlighting if feasible in v0.1.0, but editing correctness is higher priority than language richness.

### 2.4. Terminal sessions

- Start local shell sessions per project.
- Maintain multiple background terminal sessions.
- Show at most two visible terminals in immersion mode.
- Preserve running processes across mode switches.
- Provide safe close behavior.

### 2.5. AI CLI profiles and AgentRuns

- Define local AI CLI profiles in config.
- Start an AgentRun from a profile and project.
- Track lifecycle: Draft, Ready, Preparing, Running, AwaitingApproval, ReviewReady, Completed, Failed, Cancelled, and Detached.
- Capture bounded transcript/output locally by default for Tekstide-created AgentRuns, with visible retention settings and per-run opt-out before launch.
- Display AgentRun detail in Content Mode.
- Link generated diffs/artifacts to an AgentRun when detectable.

### 2.6. Safety and trust

- Restricted Mode for untrusted projects.
- Explicit Trust this Workspace action.
- Paste protection for terminal multi-line paste.
- Command approval for managed AI CLI adapters where possible.
- Clear labeling for managed, supervised, and plain terminal sessions.
- Local audit events for trust decisions, approvals, process launches, and destructive actions.

## 3. MVP non-goals

The MVP excludes:

- public plugin registry;
- user-installed third-party plugins;
- remote/container projects;
- multi-window orchestration;
- graphical settings panel beyond basic config viewing/editing;
- debugger;
- integrated package manager;
- complete Git GUI;
- automatic installation of LSPs, AI CLIs, or build tools;
- background cloud sync;
- collaborative editing;
- AI provider accounts or model management.

## 4. Compatibility promise

Tekstide must support arbitrary terminal-based AI CLIs as supervised or plain terminal processes. However, deep command approval is only promised for CLIs with adapter support. The UI must never imply that Tekstide can intercept commands it cannot see.

## 5. First target platform

The MVP should initially target one primary desktop platform for implementation feasibility. Cross-platform architecture should be preserved, but release-quality support for all platforms is not required before the first internal alpha.

Recommended first target: Linux desktop, because PTY/file watching behavior is easier to validate and aligns with likely early development usage. Windows/macOS portability should be accounted for in abstractions.

Cross-platform support remains an architectural requirement, but Windows and macOS release-quality support are not gates for v0.1.0.

## User Experience Impact

The UX must communicate three layers clearly:

1. Global project overview.
2. Focused project workspace.
3. Terminal/agent immersion.

The user should always be able to answer:

- Which projects are running?
- Which project needs attention?
- Which commands are waiting for approval?
- Which generated changes need review?
- Which processes will continue or stop if I close the app?

## Security and Privacy Impact

Security-sensitive capabilities must not be deferred out of the MVP if they are required to avoid misleading the user. At minimum, Restricted Mode, paste protection, safe close, and clear AI compatibility labeling are MVP requirements.

## Test Plan

- Scenario tests for adding/removing/switching projects.
- Scenario tests for editor + terminal + AgentRun coexistence.
- UX acceptance tests for visibility of pending approvals and running jobs.
- Security acceptance tests for Restricted Mode and paste protection.

## Acceptance Criteria

- A user can open at least three local ProjectSessions and switch between them.
- A user can edit a file in one project while terminals/AgentRuns continue in other projects.
- A user can start an AI CLI profile as an AgentRun.
- The UI distinguishes managed/supervised/plain sessions.
- The user can review transcript and detected changes from an AgentRun.
- Closing Tekstide with running processes triggers an explicit safe-close decision.
- Untrusted projects do not auto-run project-specific automation.

## Risks and Mitigations

- MVP may become too large. Mitigation: editor richness, Git richness, and plugins are explicitly secondary.
- AI CLI differences may make command approval inconsistent. Mitigation: compatibility levels are part of the product contract.

## Open Questions

- Which AI CLI should be used as the first managed adapter?
- Should v0.1.0 require syntax highlighting, or can it ship with plain text editing plus terminal/agent workflow?

## Implementation Handoff Checklist

- Treat this RFC as the scope freeze for implementation planning.
- Do not implement post-MVP plugin, remote, debugger, or marketplace features unless a later RFC reprioritizes them.
