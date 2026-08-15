---
title: "RFC-011 Amendment 2: Re-homing transcript capture - QA Evidence"
rfc: "RFC-011 Amendment 2"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Open - no slices implemented yet"
target_milestone: "M11 prerequisite"
created: "2026-08-15"
---

# QA Evidence

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. **A
  green ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does not
  find.
- **Real conditions, not synthesised values**: a real child process, a really unwritable
  file. The failure this amendment handles is a real-world one.
- State what each piece of evidence **does not** prove.

## Starting state, recorded before any change

- `LinuxTerminalRuntime::read_available_bounded_for` (`runtime/terminal/launch.rs:115`) is
  the **only** non-test writer of a `BoundedTranscriptWriter`: `.append(` at `:131-136`,
  `.flush(` at `:162-169`.
- It has **zero production callers** — the only non-test reference is its own definition.
- `runtime/terminal/reader.rs` contains the string "transcript" **zero times**.
- Nothing in production creates an `AgentRun`, so no transcript writer is ever configured
  and nothing fails today.

## PR-A2-A - Capture in the reader thread, and the ordering

*Not started.*

## PR-A2-B - The failure policy

*Not started.*

## PR-A2-C - Closeout

*Not started.*

## Known limitations going in

- **Correct before reachable.** Nothing creates an `AgentRun`, so this cannot be
  demonstrated end-to-end through a real agent run. That is deliberate: the capability must
  be right *before* adapter-spawn depends on it, not proven *through* it.
- **Backpressure now includes the disk** (D4). A stalled disk stalls the child whenever
  capture is on.
