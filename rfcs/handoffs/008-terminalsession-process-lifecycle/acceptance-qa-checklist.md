---
title: "RFC-008: TerminalSession and Process Lifecycle — Acceptance / QA Checklist"
rfc: "RFC-008"
rfc_file: "../../proposed/008-terminalsession-process-lifecycle.md"
status: "Proposed"
target_milestone: "M4"
source_rfc_status: "Proposed"
created: "2026-07-10"
---

# RFC-008: TerminalSession and Process Lifecycle — Acceptance / QA Checklist

## Acceptance Status

This checklist is prepared for RFC-008 design review. Acceptance means the project has reviewed the lifecycle design and implementation plan, not that production terminal behavior is already implemented.

## Scope Checklist

- [ ] Runtime boundary keeps PTY/process handles out of persisted domain metadata.
- [ ] Project-owned plain shell TerminalSession can start on Linux.
- [ ] Terminal launch is bound to ProjectId and canonical project root.
- [ ] Launch rejects unknown project, missing root, and cwd escape.
- [ ] Restricted Mode does not auto-start workspace automation.
- [ ] Plain terminal labels do not imply managed command approval.
- [ ] No AgentRun launch is introduced.
- [ ] No transcript retention is introduced.
- [ ] No durable audit storage is introduced.
- [ ] No production ANSI/VT, paste, clipboard, or approval-dialog security claim is introduced.

## Lifecycle Checklist

- [ ] `Starting` state is created only after runtime accepts launch.
- [ ] `Running` state is reached only after PTY/process creation succeeds.
- [ ] `Failed` records bounded error summary without private output.
- [ ] `Terminating` records request source and timestamp.
- [ ] `Exited` records known exit status.
- [ ] `OrphanedUnknown` records unresolved cleanup/ownership state.
- [ ] Invalid lifecycle transitions are rejected or idempotently ignored.
- [ ] Terminal status updates cannot mutate another ProjectSession.

## PTY / IO Checklist

- [ ] Real PTY-backed process is used on Linux.
- [ ] Shell executable, args, cwd, and environment policy are recorded.
- [ ] Output is read through a bounded buffer.
- [ ] Truncation/drop behavior is recorded when output exceeds the bound.
- [ ] Input writes are addressed by TerminalId and ProjectId.
- [ ] Cross-project input routing is rejected.
- [ ] Resize is sent to PTY.
- [ ] Child process observes resized rows/columns.

## Termination Checklist

- [ ] Terminal process starts as session/process-group leader where practical.
- [ ] SIGTERM is sent to process group on termination request.
- [ ] Bounded SIGTERM timeout is recorded.
- [ ] SIGKILL fallback is sent when timeout requires it.
- [ ] Bounded SIGKILL timeout is recorded.
- [ ] Process-group alive/ESRCH observation is recorded.
- [ ] Foreground child scenario is run.
- [ ] Final state distinguishes exited, killed-after-timeout, failed, and orphaned/unknown.
- [ ] Production cleanup does not use child-only kill semantics.

## ProjectSession / UI State Checklist

- [ ] Started terminal attaches to owning ProjectSession.
- [ ] Running terminal remains running across Content Mode / Terminal Mode switches.
- [ ] Visible slots are limited to hidden, primary, and secondary.
- [ ] At most two terminals are visible for a project.
- [ ] Hidden terminals remain visible in summaries.
- [ ] Project Board/runtime summary uses real terminal counts.
- [ ] Failed and orphaned/unknown terminals remain visible until resolved by explicit policy.

## Safe-Close Checklist

- [ ] Project close with running terminals requires confirmation.
- [ ] App close with running terminals requires confirmation.
- [ ] Safe-close prompt identifies project, terminal title/status, and consequence.
- [ ] Terminate action updates lifecycle state from real process outcome.
- [ ] Unknown/orphaned terminals are not hidden as safe.
- [ ] Safe-close summary is not derived from stale placeholder counts.

## Security and Privacy Checklist

- [ ] No secrets, environment dumps, shell history, or private file contents are printed in evidence.
- [ ] Terminal output does not mutate trust state, approvals, clipboard, command history, or app chrome.
- [ ] Terminal errors/logs use bounded summaries.
- [ ] No transcript bytes are persisted by RFC-008.
- [ ] No process IDs or handles are persisted as durable truth.
- [ ] RFC-009 dependency is visible for ANSI/VT, paste protection, clipboard behavior, and approval-dialog spoofing boundary.
- [ ] Managed/Supervised/Plain labels remain honest.

## Automated Test Checklist

- [ ] Unit tests cover lifecycle transition helpers.
- [ ] Unit tests cover invalid transitions.
- [ ] Cross-project isolation tests reject terminal/input/state mutation against another project.
- [ ] Launch validation tests reject root/cwd errors.
- [ ] Visible slot tests enforce at most two visible terminals.
- [ ] Mode-switch tests preserve running terminal state.
- [ ] Safe-close tests count real running terminals.
- [ ] Restricted Mode tests verify no workspace automation startup.
- [ ] Runtime-handle persistence boundary is covered by tests or evidence.

## Manual / Linux Smoke Checklist

- [ ] Start shell.
- [ ] Render output.
- [ ] Send input.
- [ ] Run harmless command.
- [ ] Resize terminal and observe child dimensions.
- [ ] Run foreground child.
- [ ] Terminate session and record signal/fallback/orphan behavior.
- [ ] Exercise output cap/truncation.
- [ ] Switch modes while terminal remains running.
- [ ] Close project/app with running terminal and observe safe-close behavior.

## Release Evidence Required

Attach or link the following evidence before marking RFC-008 implemented:

- [ ] Commit/PR list.
- [ ] Test command output.
- [ ] Linux smoke output.
- [ ] Manual QA notes.
- [ ] Security impact note.
- [ ] Migration note or "no migration" statement.
- [ ] Known limitations.
- [ ] RFC-009 dependency status.
- [ ] Follow-up RFCs/issues for deferred AgentRun, transcript, audit, GUI, and cross-platform work.

## Final Acceptance Decision

- [ ] Accepted as complete.
- [ ] Accepted with documented limitations.
- [ ] Blocked pending fixes.
- [ ] Requires RFC amendment.

Reviewer notes:

```text
Pending implementation evidence.
```
