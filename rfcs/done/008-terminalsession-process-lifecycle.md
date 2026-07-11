# RFC-008: TerminalSession and Process Lifecycle

Status: Implemented with documented limitations
Target milestone: M4
Date: 2026-07-10

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-roadmap-milestones-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md)

Depends on:

- [RFC-002](../done/002-core-domain-model-projectsession-terminalsession-agentrun-auditevent.md)
- [RFC-003](../done/003-information-architecture-and-ui-mode-model.md)
- [RFC-004](../done/004-security-baseline-and-restricted-mode.md)
- [RFC-005](../done/005-application-shell-and-project-board.md)
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md)
- [RFC-007](../done/007-runtime-substrate-pty-feasibility.md)

Parallel dependency:

- RFC-009 Terminal Security Boundary must be designed before production terminal security claims, paste-protection claims, clipboard behavior, or native approval-dialog spoofing boundaries are shipped.

## Summary

This RFC defines Tekstide's first production TerminalSession/process lifecycle foundation. It turns the RFC-007 Linux PTY feasibility evidence into project-owned terminal runtime behavior: launch a local shell for a ProjectSession, track lifecycle state from real process observations, preserve hidden/background sessions across Content Mode and Terminal / Agent Immersion Mode switches, and provide safe-close resource summaries for running terminals.

This RFC does not implement AgentRun launch, transcript retention, durable audit persistence, complete ANSI/VT security policy, command approval, or the final desktop GUI terminal widget.

## Motivation

Tekstide cannot build AgentRun execution, transcripts, approvals, review workflows, or durable audit on top of placeholder terminal state. RFC-002 already provides TerminalSession vocabulary, and RFC-007 proved a minimum Linux PTY loop. RFC-008 defines the production boundary between ProjectSession state and local process runtime so later AgentRun and security RFCs have stable lifecycle facts.

## Goals

- Start a project-owned local shell TerminalSession on Linux.
- Bind each terminal to exactly one ProjectSession root.
- Track lifecycle states: starting, running, terminating, exited, failed, orphaned/unknown.
- Keep runtime handles out of persisted domain metadata.
- Preserve running terminal sessions while switching between Content Mode and Terminal / Agent Immersion Mode.
- Maintain hidden/primary/secondary visible-slot policy with at most two visible panes.
- Provide project summaries and safe-close summaries from real terminal runtime state.
- Define termination behavior using process-group/session cleanup, timeout, fallback, and user-visible consequences.
- Keep Restricted Mode honest: user-started plain terminals may run, but workspace automation and AI launch remain blocked unless later RFCs authorize them.
- Expose security hooks that RFC-009 must fill before product security claims.

## Non-Goals

- AgentRun launch or AI CLI profile execution.
- Transcript capture or retention.
- Durable audit storage.
- Command approval.
- Production ANSI/VT parser or renderer policy.
- Production paste approval UI.
- Clipboard integration.
- Desktop GUI terminal widget acceptance.
- Cross-platform implementation beyond documenting Linux assumptions and portability seams.
- Multiplexing arbitrary external terminals not launched by Tekstide.

## Design Principles

1. **Project ownership is explicit.** Every terminal launch carries `ProjectId`, canonical root, working directory policy, and visible compatibility label.
2. **Runtime handles are runtime-only.** PIDs, process handles, PTY file descriptors, reader threads, output buffers, and shutdown tokens must not be persisted through domain metadata.
3. **Lifecycle state follows observed process facts.** UI and Project Board summaries must derive from runtime state transitions, not optimistic launch intent.
4. **Termination is a policy, not a syscall.** The runtime must record signal sequence, timeout, fallback, and final outcome.
5. **Security claims stay bounded.** Plain terminal sessions do not imply command interception or managed approval.

## Runtime Architecture

Introduce a runtime boundary equivalent to:

```text
TerminalRuntime
├─ start_terminal(ProjectId, TerminalLaunchSpec) -> TerminalId
├─ write_input(TerminalId, TerminalInput)
├─ resize(TerminalId, rows, cols)
├─ request_terminate(TerminalId, TerminationRequest)
├─ poll_events() -> TerminalRuntimeEvent[]
└─ snapshot(TerminalId) -> TerminalRuntimeSnapshot
```

The exact Rust names may differ, but the implementation must keep these roles separate:

- domain metadata: `TerminalSession`, ownership, visible slot, status summary;
- runtime handle: PTY/process/session handles and IO tasks;
- renderer/input adapter: temporary shell/TUI output path until the GUI terminal exists;
- app orchestration: maps runtime events into ProjectSession state and safe-close summaries.

## Terminal Launch

Launch policy:

- default shell: `/bin/sh` for the Linux MVP unless user configuration is explicitly introduced by a later slice;
- default cwd: ProjectSession canonical root;
- environment: documented minimal environment, plus only reviewed user/project additions in later RFCs;
- startup files: no explicit login shell or startup-file loading in the first slice;
- terminal kind: `Plain` for user-started terminals;
- command approval label: "Managed command approval not guaranteed";
- workspace automation: no tasks, hooks, `.env`, project AI profiles, plugins, or LSP/formatter startup as a side effect of opening a terminal.

Terminal launch must fail closed if:

- the project root is missing or no longer a directory;
- the requested cwd escapes the project root;
- the project id is unknown;
- the runtime cannot create a PTY;
- Restricted Mode would require launching workspace-provided automation.

## Lifecycle State

TerminalSession status transitions:

```text
Starting -> Running
Starting -> Failed
Running -> Terminating
Running -> Exited
Running -> Failed
Running -> OrphanedUnknown
Terminating -> Exited
Terminating -> Failed
Terminating -> OrphanedUnknown
OrphanedUnknown -> Exited
```

Rules:

- `Starting` begins only after the runtime accepts the launch request.
- `Running` begins only after the process and PTY are created.
- `Exited` records exit status when known.
- `Failed` records bounded error summary without dumping environment or private output.
- `Terminating` records user/system request, timestamp, and policy path.
- `OrphanedUnknown` is used when the runtime cannot prove the process group is gone or cannot map an observed process outcome to a safe terminal state.

Invalid transitions must be rejected or ignored idempotently with no cross-project mutation.

## Output and Input

RFC-008 must preserve enough output/input behavior to operate terminals, but it does not define full terminal security policy.

Minimum behavior:

- PTY output is read asynchronously or through a bounded event loop.
- Temporary scrollback/output buffer is bounded before output is read.
- Truncation/drop behavior is explicit in runtime events or state.
- Input writes are project/session-addressed and cannot be routed to another project's terminal.
- Resize events are routed to the PTY and reflected in runtime state.

RFC-009 owns:

- supported ANSI/VT subset;
- paste protection UX and rules;
- clipboard/OSC behavior;
- terminal-output containment guarantees;
- native approval-dialog spoofing boundary.

## Visible Slots and Mode Switching

Terminal visibility follows RFC-003:

- `Hidden`: running but not visible.
- `Primary`: visible primary pane.
- `Secondary`: visible secondary pane.

Rules:

- at most two terminals can be visible for a project;
- switching Content Mode / Terminal Mode does not terminate terminals;
- hidden terminals continue running;
- Project Board and workspace summaries expose running/failed terminal counts;
- selecting a terminal from another project requires switching active project context first.

The first implementation may show shell/text evidence rather than a final GUI widget, but state transitions must be real.

## Termination Policy

Termination request types:

- user-requested close terminal;
- app/project close safe-close action;
- runtime cleanup after failure;
- test/harness cleanup.

Linux MVP policy:

1. start terminal process as a session/process-group leader where practical;
2. send SIGTERM to the process group;
3. wait a bounded timeout;
4. if still alive, send SIGKILL to the process group;
5. wait again;
6. check whether the process group remains observable;
7. mark `Exited`, `Failed`, or `OrphanedUnknown` with bounded outcome details.

The user-visible consequence must distinguish:

- exited normally;
- terminated by request;
- killed after timeout;
- orphaned/unknown after failed cleanup.

The implementation must not promote RFC-007's child-only timeout helpers unchanged into production lifecycle behavior.

## Safe Close

Safe-close summaries must include real running terminals:

- project close with running terminals requires confirmation;
- app close with running terminals requires confirmation;
- terminal termination choices must identify project, terminal title, status, and consequence;
- failed or orphaned/unknown terminals remain visible until resolved or dismissed by explicit policy.

The first implementation may use existing text/shell rendering for evidence, but close-resource counts must be derived from actual TerminalSession/runtime state.

## Security and Privacy

- Do not read or print secrets, environment dumps, shell history, or private file contents in runtime logs or review evidence.
- Keep plain terminal labels honest: no command interception, managed approval, transcript retention, or audit durability claim.
- User-typed terminal commands are allowed in Trusted and Restricted projects as plain user action, but Tekstide must not auto-launch workspace automation in Restricted Mode.
- Terminal errors and logs must use bounded summaries.
- Terminal output must not mutate trust state, approvals, clipboard, command history, or app chrome in RFC-008.
- RFC-009 is required before shipping production paste/ANSI/clipboard/approval-spoofing claims.

## Persistence

RFC-008 may update in-memory ProjectSession collections and recent project summaries. It must not persist runtime handles, PTY data, transcript bytes, process IDs as durable truth, or terminal scrollback.

If any local state schema changes are needed, they require explicit migration evidence. The preferred first slice avoids schema changes.

## Test Plan

- Unit tests for TerminalSession lifecycle transitions and invalid transitions.
- Unit tests for project ownership and cross-project routing rejection.
- Tests for launch rejection when project root/cwd is invalid.
- Tests that Restricted Mode terminal launch does not imply workspace automation startup.
- Tests that visible slots are capped at two and mode switches preserve terminal state.
- Tests that safe-close summaries count real running terminals.
- Linux smoke for launch, output, input, resize, terminate, foreground child cleanup, and orphan/unknown outcome.
- Regression test that timeout cleanup uses process-group/session semantics, not child-only kill.

## Acceptance Criteria

- A project-owned plain shell TerminalSession can be started on Linux.
- Terminal state transitions are driven by observed runtime events.
- Running terminals persist across Content Mode and Terminal Mode switches.
- Visible slot policy allows at most two visible terminals.
- Safe-close summaries require confirmation for real running terminals.
- Termination records signal sequence, timeout, fallback, final observed outcome, and user-visible consequence.
- Runtime handles are not persisted through domain metadata.
- Restricted Mode does not auto-launch workspace automation.
- Security limitations and RFC-009 dependencies remain visible.

## Risks and Mitigations

- **Process cleanup is platform-specific.** Start with Linux and keep portability seams explicit.
- **Terminal output can become security-sensitive.** Bound output and defer security claims to RFC-009.
- **Runtime code may leak into domain state.** Keep process handles behind a runtime boundary and test metadata serialization boundaries.
- **AgentRun pressure may distort TerminalSession design.** Keep RFC-010 launch work out until TerminalSession lifecycle is credible.

## Open Questions

- Should the first runtime boundary live in `tekstide-core`, a new runtime crate, or a temporary internal module?
- What is the first acceptable terminal renderer surface before the desktop GUI widget exists?
- What timeout values should be default for SIGTERM and SIGKILL fallback?
- Should orphaned/unknown sessions be dismissible before durable audit exists?
- How much terminal runtime state should survive app restart before transcript/durable audit RFCs land?

## Implementation Handoff Checklist

- Start with model/runtime boundary review before adding broad process code.
- Keep the first implementation Linux-only and explicit.
- Add smoke evidence for every real process lifecycle claim.
- Create or update review request packages after each implementation slice.
