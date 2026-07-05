# RFC-001: Product Scope, Foundation Release, and Non-Goals

Status: Proposed  
Target release: 0.1.0 foundation
Date: 2026-07-04

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-appendix-a-extensibility-plugin-v0.md`
- `tekstide-roadmap-milestones-v0.md`


## Summary

This RFC defines Tekstide `0.1.0` as a foundation release through RFC-006. Tekstide remains a local-first, multi-project workbench for supervising terminal-based AI development workflows, but `0.1.0` does not claim the full AI CLI workbench. It proves the core model: project sessions, project board state, navigation modes, restricted-mode policy, root-bound file access, bounded explorer state, text document buffers, and shell-visible Content Mode evidence.

The broader AI CLI workbench remains the product direction for later milestones. Terminal/PTY runtime, AgentRun launch, transcript/review flows, durable audit storage, and the desktop GUI are tracked as future work rather than release blockers for `0.1.0`.

## Motivation

The initial concept began as a dual-mode editor/terminal. Review identified that the project goal is larger: concurrent development workflows for multiple projects with AI CLI tools. Without a scope freeze, implementation may over-index on editor features or terminal features while missing the differentiator: project-level orchestration and safe AI CLI supervision.

## Goals

- Define Tekstide's product identity.
- Make multi-project workflow first-class.
- Make AI CLI run supervision first-class.
- Preserve the dual-mode focused workspace inside each project.
- Define explicit non-goals to prevent scope creep.
- Establish the release target for `0.1.0`.
- Track deferred product themes clearly so a foundation release does not erase the broader workbench goal.

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

The long-term product provides:

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

The `0.1.0` foundation release implements the first durable slice of that product in core/shell form. It is not yet a release-quality GUI IDE or AI CLI runner.

## 2. `0.1.0` foundation feature set

The `0.1.0` foundation release includes:

### 2.1. Project Board foundation

- Add local project roots.
- Display multiple ProjectSessions.
- Show trust state, dirty-file/runtime summaries, placeholder terminal/AgentRun counts, and last activity without probing workspace processes.
- Switch active project quickly.
- Surface projects requiring attention without noisy dashboards.

### 2.2. Active Project Workspace foundation

- Content Mode: explorer + one main content surface.
- Terminal / Agent Immersion Mode: navigation model and placeholder shell evidence.
- Instant mode switching by keyboard.
- State retention across mode switches.

### 2.3. Editor and file explorer

- Open, view, edit, and save text/source files under the project root.
- Show basic file tree.
- Detect external changes.
- Avoid silent symlink escape.
- Reject invalid UTF-8, NUL-containing, and over-cap editable files conservatively.
- Preserve one primary Content Mode surface without arbitrary splits.

### 2.4. Domain and safety foundation

- Define ProjectSession, TerminalSession, AgentRun, approval, transcript metadata, change set, and audit-event domain vocabulary.
- Keep unknown projects effectively Restricted.
- Represent managed/supervised/plain compatibility labels honestly.
- Block workspace-local automation paths in Restricted Mode.
- Avoid process launch, network clients, Git probing, LSP startup, formatter startup, task execution, plugin loading, workspace AI profile loading, and `.env` loading.
- Provide close-readiness seams without claiming real running-process safe close.

## 3. Deferred post-`0.1.0` themes

The following themes remain product-critical but are not part of the `0.1.0` foundation release:

### 3.1. Desktop GUI runtime

- Release-quality desktop shell.
- Real file tree and editor widgets.
- GUI focus, mouse, keyboard, dialog, and confirmation flows.
- User-facing visual polish and accessibility checks.

### 3.2. Terminal / PTY runtime

- Start local shell sessions per project.
- Maintain background terminal sessions.
- Show at most two visible terminals in immersion mode.
- Preserve running processes across mode switches.
- Provide running-process safe close behavior.
- Implement paste protection for terminal multi-line paste.

### 3.3. AI CLI profiles and AgentRuns

- Define executable local AI CLI profiles in config.
- Start an AgentRun from a profile and project.
- Track runtime lifecycle through real process state.
- Capture bounded transcript/output locally by default for Tekstide-created AgentRuns, with visible retention settings and per-run opt-out before launch.
- Display AgentRun detail in Content Mode.
- Link generated diffs/artifacts to an AgentRun when detectable.
- Provide command approval for managed adapters where possible.

### 3.4. Durable review and audit

- Durable audit storage.
- Transcript retention/purge policy and storage.
- Generated diff/artifact review.
- Release-quality safe-close evidence for running work.
- Follow-up issue or RFC tracking for each deferred theme before a later MVP/beta milestone.

## 4. Non-goals

The `0.1.0` foundation release excludes:

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
- AI provider accounts or model management;
- release-quality terminal/PTY runtime;
- release-quality AgentRun execution;
- transcript/review workflow;
- durable audit storage;
- desktop GUI runtime.

## 5. Compatibility promise

Tekstide must support arbitrary terminal-based AI CLIs as supervised or plain terminal processes. However, deep command approval is only promised for CLIs with adapter support. The UI must never imply that Tekstide can intercept commands it cannot see.

For `0.1.0`, this is a product-direction promise and domain-model constraint, not a runtime execution claim.

## 6. First target platform

The product should initially target one primary desktop platform for implementation feasibility. Cross-platform architecture should be preserved, but release-quality support for all platforms is not required before the first internal alpha.

Recommended first target: Linux desktop, because PTY/file watching behavior is easier to validate and aligns with likely early development usage. Windows/macOS portability should be accounted for in abstractions.

Cross-platform support remains an architectural requirement, but Windows and macOS release-quality support are not gates for `0.1.0`.

## User Experience Impact

The long-term UX must communicate three layers clearly:

1. Global project overview.
2. Focused project workspace.
3. Terminal/agent immersion.

The user should always be able to answer:

- Which projects are running?
- Which project needs attention?
- Which commands are waiting for approval?
- Which generated changes need review?
- Which processes will continue or stop if I close the app?

For `0.1.0`, user-facing text and release notes must be explicit that the executable is a CLI/text foundation harness, not the final desktop GUI.

## Security and Privacy Impact

Security-sensitive capabilities must not be misrepresented. `0.1.0` includes Restricted Mode policy/read-model behavior, root-bound file policy, no workspace automation startup, and honest compatibility labels. Paste protection and running-process safe close remain deferred because `0.1.0` does not launch terminals or AI CLI processes.

## Test Plan

- Scenario tests for adding/removing/switching projects.
- Scenario tests for Content Mode explorer/text document behavior.
- Security acceptance tests for Restricted Mode policy, root-bound file access, symlink escape blocking, and no workspace automation startup.
- Release-readiness tests for packaging, release build, and package smoke behavior before tagging.

## Acceptance Criteria

- A user can open at least three local ProjectSessions and switch between them.
- A user can inspect Project Board state without workspace process/Git/network probing.
- A user can open, edit, and save a UTF-8 text file under a project root.
- Dirty state is visible through shell/project summaries.
- External file modifications are detected and conflicted saves do not overwrite silently.
- Symlink escape and root traversal do not silently open or save outside the project root.
- Restricted Mode does not auto-run project-specific automation.
- Release notes clearly identify deferred terminal/PTY, AgentRun, transcript/review, GUI, watcher, overwrite-confirmation, and durable-audit work.

Deferred acceptance criteria for later milestones:

- A user can edit a file in one project while terminals/AgentRuns continue in other projects.
- A user can start an AI CLI profile as an AgentRun.
- The UI distinguishes managed/supervised/plain running sessions.
- The user can review transcript and detected changes from an AgentRun.
- Closing Tekstide with running processes triggers an explicit safe-close decision.
- Untrusted projects do not auto-run project-specific automation.

## Risks and Mitigations

- Foundation release may be mistaken for the full AI CLI workbench. Mitigation: README, release notes, and RFC status must list implemented and deferred scope explicitly.
- Future work may drift after a foundation release. Mitigation: keep deferred themes visible in this RFC and release notes, then create follow-up RFCs or issue records for terminal/PTY, AgentRun, transcript/review, durable audit, GUI, and release process.
- AI CLI differences may make command approval inconsistent. Mitigation: compatibility levels remain part of the product contract for later runtime work.

## Open Questions

- Which AI CLI should be used as the first managed adapter?
- Which deferred theme should start immediately after `0.1.0`: terminal/PTY runtime, AgentRun launch, durable audit, or desktop GUI?
- Should follow-up work be tracked as RFCs, issue files, or both?

## Implementation Handoff Checklist

- Treat this RFC as the scope freeze for the `0.1.0` foundation release.
- Do not pull deferred plugin, remote, debugger, marketplace, or cloud features into `0.1.0` unless a later RFC explicitly reprioritizes them.
- Preserve the future-work list when creating release notes, review requests, and later RFCs.
