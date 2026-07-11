---
title: "RFC-008: TerminalSession and Process Lifecycle — Task Breakdown / PR Plan"
rfc: "RFC-008"
rfc_file: "../../done/008-terminalsession-process-lifecycle.md"
status: "Implemented with documented limitations"
target_milestone: "M4"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-10"
---

# RFC-008: TerminalSession and Process Lifecycle — Task Breakdown / PR Plan

## Planning Assumptions

- RFC-008 starts only after RFC-007 feasibility closeout is accepted.
- The first production target is Linux.
- The first terminal kind is `Plain`.
- RFC-009 security policy is a parallel design dependency.
- AgentRun launch, transcript retention, durable audit, and command approval remain out of scope.
- Implementation should be split into small reviewable slices with a review request after each good checkpoint.

## PR Sequence Overview

- PR-008-A: Runtime boundary and lifecycle model refinement.
- PR-008-B: Project-owned PTY shell launch.
- PR-008-C: Output/input/resize event plumbing with bounded buffers.
- PR-008-D: Process-group termination and orphan/unknown handling.
- PR-008-E: ProjectSession integration, visible slots, and mode-switch preservation.
- PR-008-F: Safe-close integration for real running terminals.
- PR-008-G: QA evidence and RFC-008 closeout.

The sequence starts with state boundaries before process behavior, then integrates user-visible summaries only after lifecycle facts are credible.

## PR-008-A — Runtime Boundary and Lifecycle Model

Purpose:

- Establish where runtime handles live and how lifecycle transitions are represented.

Developer tasks:

- Add or refine lifecycle transition helpers for TerminalSession.
- Define runtime handle/snapshot/event types without persisting process handles.
- Define launch spec, termination request, termination outcome, and bounded error summary types.
- Add tests for valid/invalid status transitions.
- Add tests that runtime-only fields are not part of persistent metadata.

Review focus:

- Domain metadata and runtime handles are clearly separated.
- Status transitions cannot be mutated arbitrarily.
- No AgentRun, transcript, durable audit, or GUI security feature enters the slice.

## PR-008-B — Project-Owned PTY Shell Launch

Purpose:

- Start a real local shell TerminalSession under a ProjectSession root.

Developer tasks:

- Launch `/bin/sh` through a Linux PTY from a ProjectSession canonical root.
- Use minimal documented environment.
- Bind runtime handle to ProjectId and TerminalId.
- Reject unknown project, missing root, cwd escape, and PTY creation failure.
- Mark launched terminal as `Plain`.
- Add shell-visible or test harness evidence for start and simple output.

Review focus:

- Process is PTY-backed.
- Launch is project-owned and root-bound.
- Restricted Mode does not auto-start workspace automation.

## PR-008-C — Output, Input, and Resize Plumbing

Purpose:

- Make terminal IO event flow usable without claiming full terminal security policy.

Developer tasks:

- Read PTY output through a bounded buffer before data is accepted.
- Route input writes by TerminalId/ProjectId.
- Route resize events to the PTY.
- Record truncation/drop state where output exceeds temporary bounds.
- Add tests for cross-project input rejection and buffer cap behavior.
- Add Linux smoke for output, input, and resize.

Review focus:

- Output is bounded before it is read.
- Input cannot target another project's terminal.
- ANSI/VT security policy remains deferred to RFC-009.

## PR-008-D — Process-Group Termination

Purpose:

- Promote RFC-007 termination observations into production lifecycle policy.

Developer tasks:

- Start terminal process as a session/process-group leader where practical.
- Send SIGTERM to process group on terminate request.
- Wait bounded timeout.
- Send SIGKILL fallback when required.
- Check whether process group remains observable.
- Mark exited, failed, or orphaned/unknown honestly.
- Record user-visible termination consequence.
- Add smoke for foreground child such as `sleep 60`.

Review focus:

- Cleanup is process-group/session based, not child-only.
- Timeout/fallback/orphan outcomes are explicit.
- User-visible state does not imply stronger supervision than observed.

## PR-008-E — ProjectSession Integration and Visibility

Purpose:

- Preserve running terminals across mode switches and expose honest project state.

Developer tasks:

- Attach started terminals to owning ProjectSession.
- Preserve terminal state across Content Mode and Terminal Mode switches.
- Enforce hidden/primary/secondary visible slots with at most two visible terminals.
- Update Project Board/workspace summaries from real terminal runtime state.
- Add tests for mode switches, visible slot caps, and cross-project isolation.

Review focus:

- Hidden terminals keep running.
- Visible terminal count policy matches RFC-003.
- Project summaries derive from real state.

## PR-008-F — Safe-Close Integration

Purpose:

- Make real running terminals participate in safe close.

Developer tasks:

- Feed close-resource summaries from real running terminal sessions.
- Require confirmation for project/app close with running terminals.
- Expose terminate/keep behavior according to current UI capability.
- Represent failed or orphaned/unknown terminals visibly.
- Add tests for active resources, termination decisions, and summary labels.

Review focus:

- User is not surprised by killed or left-running processes.
- Running terminals are not counted from stale placeholders.
- Unknown/orphaned state remains visible.

## PR-008-G — QA Evidence and Closeout

Purpose:

- Convert RFC-008 implementation into reviewed milestone evidence.

Developer tasks:

- Run formatting, clippy, workspace checks, tests, and Linux smoke.
- Record launch, output, input, resize, termination, foreground child, safe-close, visible slot, and mode-switch evidence.
- Record security impact and RFC-009 dependencies.
- Record migration note or no-migration statement.
- Record known limitations and cross-platform risks.
- Recommend complete, complete with limitations, or blocked.

Review focus:

- Evidence supports every accepted RFC-008 claim.
- Deferred RFC-009/RFC-010/RFC-011/RFC-012 scope remains visible.
- The implementation is ready for the next milestone slice.

## Suggested Review Gates

1. **Design gate:** RFC-008 and handoff accepted.
2. **Boundary gate:** runtime/domain separation reviewed.
3. **PTY gate:** project-owned shell launch, output, input, and resize work.
4. **Lifecycle gate:** termination and orphan/unknown semantics reviewed.
5. **Integration gate:** ProjectSession summaries, visible slots, and mode-switch persistence work.
6. **Safe-close gate:** running terminal close behavior reviewed.
7. **Closeout gate:** evidence package and known limitations accepted.

## Stop Conditions

Pause and request RFC amendment or design review if:

- implementation requires AgentRun launch or AI profile execution;
- process cleanup cannot be proven for foreground children;
- runtime handles need to be persisted;
- output buffer bounds fail under smoke;
- GUI terminal security policy is needed before RFC-009;
- Restricted Mode would need to auto-load workspace configuration;
- cross-platform behavior becomes necessary before Linux lifecycle is credible.
