---
title: "RFC-008: TerminalSession and Process Lifecycle — Acceptance / QA Checklist"
rfc: "RFC-008"
rfc_file: "../../done/008-terminalsession-process-lifecycle.md"
status: "Accepted with documented limitations"
target_milestone: "M4"
source_rfc_status: "Implemented with documented limitations"
created: "2026-07-10"
---

# RFC-008: TerminalSession and Process Lifecycle — Acceptance / QA Checklist

## Acceptance Status

This checklist records accepted RFC-008 implementation closeout evidence. Acceptance means the project has reviewed the Linux terminal/process lifecycle foundation and its documented limitations, not that final GUI terminal behavior or later AgentRun/security RFCs are implemented.

## Scope Checklist

- [x] Runtime boundary keeps PTY/process handles out of persisted domain metadata.
- [x] Project-owned plain shell TerminalSession can start on Linux.
- [x] Terminal launch is bound to ProjectId and canonical project root.
- [x] Launch rejects unknown project, missing root, and cwd escape.
- [x] Restricted Mode does not auto-start workspace automation.
- [x] Plain terminal labels do not imply managed command approval.
- [x] No AgentRun launch is introduced.
- [x] No transcript retention is introduced.
- [x] No durable audit storage is introduced.
- [x] No production ANSI/VT, paste, clipboard, or approval-dialog security claim is introduced.

## Lifecycle Checklist

- [x] `Starting` state is created only after runtime accepts launch.
- [x] `Running` state is reached only after PTY/process creation succeeds.
- [x] `Failed` records bounded error summary without private output.
- [x] `Terminating` records request source and policy path.
- [x] `Exited` records known exit status.
- [x] `OrphanedUnknown` records unresolved cleanup/ownership state.
- [x] Invalid lifecycle transitions are rejected or idempotently ignored.
- [x] Terminal status updates cannot mutate another ProjectSession.

## PTY / IO Checklist

- [x] Real PTY-backed process is used on Linux.
- [x] Shell executable, args, cwd, and environment policy are recorded.
- [x] Output is read through a bounded buffer.
- [x] Truncation/drop behavior is recorded when output exceeds the bound.
- [x] Input writes are addressed by TerminalId and ProjectId.
- [x] Cross-project input routing is rejected.
- [x] Resize is sent to PTY.
- [x] Child process observes resized rows/columns.

## Termination Checklist

- [x] Terminal process starts as session/process-group leader where practical.
- [x] SIGTERM is sent to process group on termination request.
- [x] Bounded SIGTERM timeout is recorded.
- [x] SIGKILL fallback is sent when timeout requires it.
- [x] Bounded SIGKILL timeout is recorded.
- [x] Process-group alive/ESRCH observation is recorded.
- [x] Foreground child scenario is run.
- [x] Final state distinguishes exited, killed-after-timeout, failed, and orphaned/unknown.
- [x] Production cleanup does not use child-only kill semantics.

## ProjectSession / UI State Checklist

- [x] Started terminal attaches to owning ProjectSession.
- [x] Running terminal remains running across Content Mode / Terminal Mode switches.
- [x] Visible slots are limited to hidden, primary, and secondary.
- [x] At most two terminals are visible for a project.
- [x] Hidden terminals remain visible in summaries.
- [x] Project Board/runtime summary uses real terminal counts.
- [x] Failed and orphaned/unknown terminals remain attached and counted until resolved by explicit policy.

## Safe-Close Checklist

- [x] Project close with running terminals requires confirmation.
- [ ] App close with running terminals requires confirmation.
- [ ] Safe-close prompt identifies project, terminal title/status, and consequence.
- [ ] Terminate action updates lifecycle state from real process outcome.
- [x] Unknown/orphaned terminals are not hidden as safe.
- [x] Safe-close summary is not derived from stale placeholder counts.

## Security and Privacy Checklist

- [x] No secrets, environment dumps, shell history, or private file contents are printed in evidence.
- [x] Terminal output does not mutate trust state, approvals, clipboard, command history, or app chrome.
- [x] Terminal errors/logs use bounded summaries.
- [x] No transcript bytes are persisted by RFC-008.
- [x] No process IDs or handles are persisted as durable truth.
- [x] RFC-009 dependency is visible for ANSI/VT, paste protection, clipboard behavior, and approval-dialog spoofing boundary.
- [x] Managed/Supervised/Plain labels remain honest.

## Automated Test Checklist

- [x] Unit tests cover lifecycle transition helpers.
- [x] Unit tests cover invalid transitions.
- [x] Cross-project isolation tests reject terminal/input/state mutation against another project.
- [x] Launch validation tests reject root/cwd errors.
- [x] Visible slot tests enforce at most two visible terminals.
- [x] Mode-switch tests preserve running terminal state.
- [x] Safe-close tests count real running terminals.
- [x] Restricted Mode tests verify no workspace automation startup.
- [x] Runtime-handle persistence boundary is covered by tests or evidence.

## Manual / Linux Smoke Checklist

- [x] Start shell.
- [x] Render output.
- [x] Send input.
- [x] Run harmless command.
- [x] Resize terminal and observe child dimensions.
- [x] Run foreground child.
- [x] Terminate session and record signal/fallback/orphan behavior.
- [x] Exercise output cap/truncation.
- [x] Switch modes while terminal remains running.
- [x] Close project with running terminal and observe safe-close behavior.
- [ ] App-wide close with running terminal and observe aggregate safe-close behavior.

## Release Evidence Required

Attach or link the following evidence before marking RFC-008 implemented:

- [x] Commit/PR list.
- [x] Test command output.
- [x] Linux smoke output.
- [x] Manual QA notes.
- [x] Security impact note.
- [x] Migration note or "no migration" statement.
- [x] Known limitations.
- [x] RFC-009 dependency status.
- [x] Follow-up RFCs/issues for deferred AgentRun, transcript, audit, GUI, and cross-platform work.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [x] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Closeout accepted with documented limitations on 2026-07-11.

Implemented foundation:
- Linux project-owned PTY shell launch.
- Bounded output/input/resize plumbing.
- Process-group termination with timeout and fallback semantics.
- ProjectSession terminal collection integration, visible-slot policy, and mode-switch preservation.
- Project close assessment for real running terminals.

Documented limitations:
- No app-wide close aggregate surface yet.
- No terminate/keep confirmation action UI yet.
- No final GUI terminal widget.
- No AgentRun launch, transcript retention, durable audit storage, or command approval.
- No RFC-009 terminal security boundary, ANSI/VT policy, paste protection, clipboard behavior, or approval-dialog spoofing boundary.
- Linux-only runtime evidence; macOS/Windows terminal behavior remains future work.
```
