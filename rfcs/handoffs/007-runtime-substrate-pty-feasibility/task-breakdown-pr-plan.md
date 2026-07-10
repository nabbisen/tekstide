---
title: "RFC-007: Runtime Substrate and PTY Feasibility Gate — Task Breakdown / PR Plan"
rfc: "RFC-007"
rfc_file: "../../done/007-runtime-substrate-pty-feasibility.md"
status: "Implemented"
target_milestone: "M4 feasibility gate"
source_rfc_status: "Implemented"
created: "2026-07-09"
---

# RFC-007: Runtime Substrate and PTY Feasibility Gate — Task Breakdown / PR Plan

## Planning Assumptions

- This plan is for a spike-only feasibility gate.
- The selected direction is a narrow TUI-first harness.
- The spike should be reviewable as one small implementation slice unless dependency or process behavior forces a split.
- RFC-008, RFC-009, RFC-010, RFC-011, and RFC-012 remain out of scope.

## Recommended Sequence

- PR-007-A: Spike harness shell and dependency quarantine.
- PR-007-B: PTY start, output render, and input loop.
- PR-007-C: Resize, termination, foreground-child, and orphan observations.
- PR-007-D: Output-flood bounds and latency measurements.
- PR-007-E: Security observations, evidence package, and Go/No-Go recommendation.

## PR-007-A — Spike Harness Shell

Purpose:

- Create the smallest isolated location for the feasibility harness.

Developer tasks:

- Choose and record spike location.
- Add only dependencies needed for the first harness shell.
- Mark the crate/module/example as spike-only.
- Add a short command or documented manual path to run the spike.
- Add an initial `qa-evidence.md` entry for location and dependency policy.

Review focus:

- Spike code cannot be mistaken for stable production API.
- Dependencies are justified and quarantined.
- No unrelated runtime, AgentRun, transcript, or audit work enters the slice.

## PR-007-B — PTY Start, Output, and Input

Purpose:

- Prove the minimum PTY loop.

Developer tasks:

- Start a shell with the safe spike shell profile.
- Render output in the TUI harness.
- Send keyboard input to the PTY.
- Run harmless commands such as `printf 'tekstide-pty-ok\n'`.
- Record shell executable, arguments, root category, environment policy, and terminal dimensions.

Review focus:

- The process is PTY-backed, not a mocked command runner.
- Evidence avoids secrets and private paths.
- Input/output behavior is repeatable enough for review.

## PR-007-C — Resize and Termination

Purpose:

- Prove lifecycle observations needed before RFC-008.

Developer tasks:

- Resize the terminal and verify `stty size` or equivalent observes the new rows/columns.
- Terminate the shell cleanly.
- Start a foreground child such as `sleep 60`.
- Terminate the PTY/session and record signal sequence, timeout behavior, process-group/session behavior where exposed, and orphan detection.

Review focus:

- Resize is observable by the child process.
- Foreground-child behavior is recorded, not assumed.
- Any unresolved process-group semantics are identified as RFC-008 blockers.

## PR-007-D — Output Flood and Latency

Purpose:

- Make responsiveness and bounded buffering reviewable.

Developer tasks:

- Run at least 10,000 lines or 1 MiB of output.
- Enforce an explicit temporary buffer cap before the test.
- Record truncation marker behavior.
- Record memory before/after output flood.
- Record whether the harness remains usable after truncation.
- Measure input/echo latency with a repeatable local procedure and report p50, p95, and worst observed values where measurable.

Review focus:

- Output flood is bounded before it runs.
- Memory and recovery behavior are recorded.
- Latency evidence is honest about method limits.

## PR-007-E — Security Observations and Closeout

Purpose:

- Convert the spike into a Go/No-Go decision for RFC-008.

Developer tasks:

- Record whether terminal output is constrained to the terminal surface.
- Record handling of unsupported control sequences where observable.
- Record whether multiline paste can be intercepted before PTY write.
- Record whether native approval/paste-dialog separation remains plausible for the future GUI.
- Complete `qa-evidence.md`.
- Recommend Go, No-Go, or redesign before RFC-008.

Review focus:

- Security observations inform RFC-009 without overclaiming ANSI/VT safety.
- The recommendation is backed by observed evidence.
- Spike cleanup/quarantine decision is explicit.

## Suggested Review Gates

1. **Scope gate:** handoff package reviewed; spike-only guardrails accepted.
2. **Dependency gate:** new crates are justified as spike-only or candidate production inputs.
3. **PTY gate:** shell start, output, input, and resize work.
4. **Lifecycle gate:** termination, foreground child, timeout, and orphan evidence recorded.
5. **Performance gate:** output flood and latency evidence recorded.
6. **Security gate:** terminal containment and paste-interception observations recorded.
7. **Decision gate:** Go/No-Go recommendation reviewed before RFC-008 starts.
