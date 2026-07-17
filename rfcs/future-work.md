# Tekstide Future Work Themes

This file tracks deferred themes after the `0.1.0` foundation release scope. It is not a substitute for detailed RFCs or issues; it is the durable index that prevents deferred work from disappearing.

## Post-0.1.0 Product Themes

### Terminal / PTY Runtime

Status: partially implemented by RFC-007/RFC-008/RFC-009; product UI and GUI evidence remain.

- Linux project-owned PTY shell lifecycle foundation is implemented by RFC-008 with documented limitations.
- Background terminal sessions, mode-switch preservation, visible-slot policy, and project-close assessment are implemented at the core/project layer.
- Terminal output containment, conservative ANSI/VT/OSC policy, paste input policy, and model-level trusted UI/spoofing boundaries are implemented by RFC-009 with documented limitations.
- Add app/UI commands for launching, selecting, and closing terminals.
- Add app-wide close aggregation for running terminals.
- Add terminate/keep confirmation actions and visible terminal consequence text.
- Wire paste protection into real app/UI paste events and rendered confirmation dialogs.
- Implement safe GUI terminal rendering and screenshot-backed spoofing evidence in the GUI milestone.
- Add macOS/Windows terminal runtime evidence before claiming cross-platform terminal support.

### AgentRun And AI CLI Execution

Status: deferred after `0.1.0`.

- Define executable AI CLI profiles.
- Launch AgentRuns from project/profile context.
- Track runtime lifecycle from real process state.
- Preserve managed/supervised/plain compatibility labels without overclaiming command interception.
- Add command approval only where an adapter can actually support it.

### Transcript And Review Workflow

Status: deferred after `0.1.0`.

- Capture bounded transcript/output for Tekstide-created AgentRuns.
- Provide visible retention and purge controls.
- Link generated diffs/artifacts to AgentRuns when detectable.
- Add review surfaces for transcript and generated changes.

### Durable Audit Storage

Status: deferred after `0.1.0`.

- Persist security-relevant audit events.
- Record trust decisions, approvals, process launches, blocked root/symlink access, and destructive confirmations.
- Keep audit records local and avoid storing unnecessary file contents or private output.

### Desktop GUI Runtime

Status: deferred after `0.1.0`.

- Select and implement the desktop GUI runtime.
- Replace shell-visible evidence with real Project Board, file tree, and editor surfaces.
- Add focus, keyboard, mouse, dialog, and confirmation flows.
- Validate responsive layout, accessibility, and visual polish.

### File Workflow Follow-Up

Status: deferred after `0.1.0`.

- File watcher integration.
- Overwrite-confirmation UI for externally changed files.
- Multi-document tabs or another explicit multi-document model.
- Richer editor internals if `String`-backed buffers become limiting.

### Release Process

Status: active after `0.1.0`.

- Keep the release checklist current.
- Add release build, package, and package smoke evidence before each release.
- Decide whether future releases need scripts, `xtask`, or CI gates.
- Keep the changelog aligned with implemented and deferred scope.

## Milestone Roadmap

See [`../ROADMAP.md`](../ROADMAP.md) for the milestone schedule.
