---
title: "RFC-007: Runtime Substrate and PTY Feasibility Gate — Acceptance / QA Checklist"
rfc: "RFC-007"
rfc_file: "../../proposed/007-runtime-substrate-pty-feasibility.md"
status: "Proposed"
target_milestone: "M4 feasibility gate"
source_rfc_status: "Proposed"
created: "2026-07-09"
---

# RFC-007: Runtime Substrate and PTY Feasibility Gate — Acceptance / QA Checklist

## Acceptance Status

This checklist is prepared for implementation review. RFC-007 is accepted only as a feasibility-gate design; completion means the spike evidence is sufficient for a Go/No-Go decision, not that production terminal behavior is implemented.

## Scope Checklist

- [ ] Spike location is chosen and recorded.
- [ ] Spike code is marked as temporary, quarantined, or deletion-ready.
- [ ] No production TerminalSession behavior is introduced.
- [ ] No AgentRun launch is introduced.
- [ ] No transcript persistence is introduced.
- [ ] No command approval implementation is introduced.
- [ ] No durable audit storage is introduced.
- [ ] New dependencies are justified and labeled spike-only or candidate production input.

## Required PTY Evidence

- [ ] Real PTY-backed shell is used.
- [ ] Safe spike shell profile is documented.
- [ ] Synthetic or explicitly non-sensitive test root is used.
- [ ] Minimal or documented environment policy is used.
- [ ] Shell executable and arguments are recorded.
- [ ] Output renders in the TUI harness.
- [ ] Keyboard input reaches the PTY.
- [ ] Simple command output is visible.
- [ ] Resize is sent to the PTY.
- [ ] Child process observes resized rows/columns.

## Termination Evidence

- [ ] Shell termination is observed.
- [ ] Foreground child process scenario is run.
- [ ] Signal sequence is recorded.
- [ ] Timeout behavior is recorded.
- [ ] Process-group/session behavior is recorded where exposed.
- [ ] Orphan detection result is recorded.
- [ ] Unresolved termination/process-group questions are listed as RFC-008 blockers.

## Output Flood and Performance Evidence

- [ ] Output-flood test produces at least 10,000 lines or 1 MiB.
- [ ] Temporary buffer cap is enforced before the output-flood test.
- [ ] Cap value is recorded.
- [ ] Dropped/truncated behavior is recorded.
- [ ] Truncation marker behavior is recorded.
- [ ] Memory before/after output flood is recorded.
- [ ] Recovery behavior after output flood exits is recorded.
- [ ] Input/echo latency procedure is recorded.
- [ ] p50, p95, and worst observed latency are recorded where measurable.
- [ ] Measurement limitations are documented.

## Security Evidence

- [ ] Evidence avoids secrets, environment dumps, private project contents, and unnecessary absolute private paths.
- [ ] Terminal output is observed as constrained to the terminal surface, or limitations are recorded.
- [ ] Unsupported control-sequence behavior is observed where practical.
- [ ] Application chrome, trust state, approvals, clipboard, and command history are not modified by terminal output in the spike, or limitations are recorded.
- [ ] Multiline paste interception before PTY write is observed or identified as an RFC-009 blocker.
- [ ] Future native approval/paste-dialog visual separation remains plausible, or the risk is recorded.
- [ ] No complete ANSI/VT safety claim is made.

## Decision Checklist

- [ ] Evidence package is complete.
- [ ] Known Linux-only assumptions are recorded.
- [ ] Expected Windows/macOS risks are recorded.
- [ ] Cleanup/quarantine/promotion decision is recorded.
- [ ] Recommendation is one of: Go to RFC-008, No-Go, revise RFC-007, or change substrate direction.
- [ ] Review accepts the recommendation before RFC-008 implementation begins.

## Final Acceptance Decision

- [ ] Go to RFC-008.
- [ ] No-Go.
- [ ] Revise RFC-007.
- [ ] Change substrate direction.
- [ ] Blocked pending missing evidence.

Reviewer notes:

```text
Pending implementation evidence.
```

