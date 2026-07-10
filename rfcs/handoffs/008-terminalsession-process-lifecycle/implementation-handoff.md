---
title: "RFC-008: TerminalSession and Process Lifecycle — Implementation Handoff"
rfc: "RFC-008"
rfc_file: "../../proposed/008-terminalsession-process-lifecycle.md"
status: "Proposed"
target_milestone: "M4"
source_rfc_status: "Proposed"
created: "2026-07-10"
---

# RFC-008: TerminalSession and Process Lifecycle — Implementation Handoff

## Purpose

This handoff translates RFC-008 into developer-facing guidance for implementing the first production TerminalSession/process lifecycle foundation.

This handoff does not authorize AgentRun launch, transcript retention, command approval, durable audit storage, production ANSI/VT safety, clipboard policy, or a final desktop GUI terminal widget.

## Source RFC Summary

RFC-008 defines project-owned local terminal runtime behavior on Linux: start a plain shell TerminalSession, track real process lifecycle state, preserve hidden/background sessions across mode switches, enforce visible slot policy, terminate process groups with timeout/fallback semantics, and feed safe-close summaries from real running terminals.

RFC-009 must run alongside this work for terminal security policy. RFC-008 may expose hooks for paste/ANSI/clipboard/approval boundaries, but must not claim those policies are complete.

## Dependencies and Sequencing

- Target milestone: **M4**
- Source RFC status: **Proposed**
- Required predecessor RFCs: RFC-002, RFC-003, RFC-004, RFC-005, RFC-006, RFC-007.
- RFC-007 closeout accepted the Go recommendation on 2026-07-10.
- RFC-008 design/handoff review accepted the runtime boundary and PR sequence on 2026-07-10.
- RFC-009 security design must be reviewed before production terminal security claims are shipped.

## Implementation Scope

The implementation must deliver:

- a project-owned plain shell TerminalSession on Linux;
- a runtime boundary that keeps PTY/process handles out of persisted domain metadata;
- lifecycle transitions from observed process facts;
- bounded output/input plumbing sufficient for shell-visible evidence;
- PTY resize routing;
- process-group termination with SIGTERM, timeout, SIGKILL fallback, and orphan/unknown observation;
- hidden/primary/secondary visible slot policy with at most two visible terminals per project;
- mode-switch preservation of running terminals;
- safe-close summaries for real running terminals.

The implementation is not complete until these outcomes are covered by automated tests and Linux smoke evidence.

## Recommended Module Boundaries

Recommended boundaries:

- `domain::terminal`: persistent/reference metadata and lifecycle vocabulary only.
- `project::session`: owns TerminalSession collections and derives runtime summaries.
- `terminal` or `runtime::terminal`: runtime handle registry, PTY/process ownership, IO loop, resize, termination, and event production.
- `shell`/`app`: orchestration and shell-visible evidence rendering.
- future RFC-009 module: paste/ANSI/clipboard/security policy.

Do not put PTY file descriptors, child handles, reader threads, or process IDs as durable truth inside `TerminalSession`.

If the runtime boundary becomes large enough to justify a separate crate, stop and request review before splitting workspace crates. The first implementation can remain internal if it stays testable and isolated.

## Key Design Decisions to Preserve

- Plain user terminals are not AgentRuns.
- Plain terminals must not claim managed command approval.
- Restricted Mode may allow user-started plain terminal commands, but must not auto-start workspace automation.
- Process cleanup must use process-group/session semantics on Linux.
- Orphaned/unknown is a valid honest state when cleanup cannot be proven.
- Safe close must be based on real runtime state, not placeholder counts.

## Launch Policy

Initial launch policy:

- shell executable: `/bin/sh`;
- shell arguments: none unless design review approves otherwise;
- cwd: ProjectSession canonical root by default;
- environment: minimal documented environment;
- startup files: no explicit login shell or startup-file loading;
- terminal kind: `Plain`;
- compatibility label: no managed command approval guarantee.

Launch must reject:

- unknown project id;
- missing project root;
- cwd outside canonical root;
- PTY creation failure;
- any path that would require workspace-local automation startup in Restricted Mode.

## Lifecycle Requirements

Implement transition helpers or equivalent policy that prevents free-form status mutation.

Required states:

- `Starting`
- `Running`
- `Terminating`
- `Exited`
- `Failed`
- `OrphanedUnknown`

Required observations:

- launch accepted;
- PTY/process created;
- output seen or process running;
- exit status known;
- termination requested;
- SIGTERM sent;
- timeout occurred or not;
- SIGKILL fallback sent if needed;
- process group remains observable or returns ESRCH;
- cleanup failed or runtime lost ownership.

## Termination Requirements

Linux termination must:

- start terminal process as a session/process-group leader where practical;
- send SIGTERM to the process group;
- wait a bounded timeout;
- send SIGKILL to the process group if still alive;
- wait a bounded timeout again;
- check whether the process group remains observable;
- record final state and bounded user-visible consequence.

The implementation must not copy RFC-007's child-only timeout cleanup path into production lifecycle behavior.

## Security and Privacy Requirements

- Do not capture environment dumps, shell history, private file contents, or token-like values in logs or review evidence.
- Do not persist terminal output or transcript bytes in RFC-008.
- Bound terminal output buffers before reading output.
- Keep terminal output from changing trust state, approvals, clipboard, command history, or app chrome.
- Keep all terminal input addressed by `TerminalId` and `ProjectId`; reject cross-project routing.
- Make RFC-009 follow-up visible anywhere paste, ANSI/VT, clipboard, or approval-spoofing behavior is mentioned.

## Observability and Evidence

Every implementation slice must record:

- tests added or updated;
- manual/Linux smoke commands run;
- lifecycle observations;
- security-impact note;
- migration note or "no migration";
- known limitations;
- review request path.

The Linux smoke evidence must avoid secrets and private project output. Use synthetic roots or harmless commands.

## Stop Conditions

Pause and request review if:

- implementation requires AgentRun launch;
- process cleanup cannot avoid child-only kill semantics;
- terminal state crosses ProjectSession ownership boundaries;
- output buffering cannot be bounded;
- implementation needs transcript persistence or durable audit;
- security policy work becomes necessary beyond RFC-009 hooks;
- workspace-local automation starts implicitly in Restricted Mode;
- new dependencies materially change the workspace or release posture.
