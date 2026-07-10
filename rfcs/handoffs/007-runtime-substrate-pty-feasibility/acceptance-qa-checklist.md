---
title: "RFC-007: Runtime Substrate and PTY Feasibility Gate — Acceptance / QA Checklist"
rfc: "RFC-007"
rfc_file: "../../done/007-runtime-substrate-pty-feasibility.md"
status: "Implemented"
target_milestone: "M4 feasibility gate"
source_rfc_status: "Implemented"
created: "2026-07-09"
---

# RFC-007: Runtime Substrate and PTY Feasibility Gate — Acceptance / QA Checklist

## Acceptance Status

This checklist records accepted closeout evidence. RFC-007 is complete only as a feasibility gate; it does not mean production terminal behavior is implemented.

## Scope Checklist

- [x] Spike location is chosen and recorded.
- [x] Spike code is marked as temporary, quarantined, or deletion-ready.
- [x] No production TerminalSession behavior is introduced.
- [x] No AgentRun launch is introduced.
- [x] No transcript persistence is introduced.
- [x] No command approval implementation is introduced.
- [x] No durable audit storage is introduced.
- [x] New dependencies are justified and labeled spike-only or candidate production input.

## Required PTY Evidence

- [x] Real PTY-backed shell is used.
- [x] Safe spike shell profile is documented.
- [x] Synthetic or explicitly non-sensitive test root is used.
- [x] Minimal or documented environment policy is used.
- [x] Shell executable and arguments are recorded.
- [x] Output renders in the TUI harness.
- [x] Keyboard input reaches the PTY.
- [x] Simple command output is visible.
- [x] Resize is sent to the PTY.
- [x] Child process observes resized rows/columns.

## Termination Evidence

- [x] Shell termination is observed.
- [x] Foreground child process scenario is run.
- [x] Signal sequence is recorded.
- [x] Timeout behavior is recorded.
- [x] Process-group/session behavior is recorded where exposed.
- [x] Orphan detection result is recorded.
- [x] Unresolved termination/process-group questions are listed as RFC-008 blockers.

## Output Flood and Performance Evidence

- [x] Output-flood test produces at least 10,000 lines or 1 MiB.
- [x] Temporary buffer cap is enforced before the output-flood test.
- [x] Cap value is recorded.
- [x] Dropped/truncated behavior is recorded.
- [x] Truncation marker behavior is recorded.
- [x] Memory before/after output flood is recorded.
- [x] Recovery behavior after output flood exits is recorded.
- [x] Input/echo latency procedure is recorded.
- [x] p50, p95, and worst observed latency are recorded where measurable.
- [x] Measurement limitations are documented.

## Security Evidence

- [x] Evidence avoids secrets, environment dumps, private project contents, and unnecessary absolute private paths.
- [x] Terminal output is observed as constrained to the terminal surface, or limitations are recorded.
- [x] Unsupported control-sequence behavior is observed where practical.
- [x] Application chrome, trust state, approvals, clipboard, and command history are not modified by terminal output in the spike, or limitations are recorded.
- [x] Multiline paste interception before PTY write is observed or identified as an RFC-009 blocker.
- [x] Future native approval/paste-dialog visual separation remains plausible, or the risk is recorded.
- [x] No complete ANSI/VT safety claim is made.

## Decision Checklist

- [x] Evidence package is complete.
- [x] Known Linux-only assumptions are recorded.
- [x] Expected Windows/macOS risks are recorded.
- [x] Cleanup/quarantine/promotion decision is recorded.
- [x] Recommendation is one of: Go to RFC-008, No-Go, revise RFC-007, or change substrate direction.
- [x] Review accepts the recommendation before RFC-008 implementation begins.

## Final Acceptance Decision

- [x] Go to RFC-008.
- [ ] No-Go.
- [ ] Revise RFC-007.
- [ ] Change substrate direction.
- [ ] Blocked pending missing evidence.

Reviewer notes:

```text
Accepted with notes on 2026-07-10. See qa-evidence.md and review response 037. RFC-008 may proceed, with RFC-009 security policy required alongside lifecycle work.
```
