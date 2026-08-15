---
title: "RFC-017 Amendment 1: Readiness-driven terminal I/O - QA Evidence"
rfc: "RFC-017 Amendment 1"
rfc_file: "../../done/017-terminal-renderer-and-immersion-mode.md"
status: "Open - no slices implemented yet"
target_milestone: "M9 (carried), shipping in 0.8.0"
created: "2026-08-15"
---

# QA Evidence

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap. If a slice discovers a new obligation, put it in the
task breakdown, not only here.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. **A
  green ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does
  not find.
- **Measurement**: measure bounds, never estimate them. Two estimated figures here were
  wrong once measured, and a third measured the wrong quantity.
- **GUI evidence**: `niri msg action screenshot-window`; `env -u WAYLAND_DISPLAY`,
  `xdotool windowfocus`, always `--clearmodifiers`. One window geometry per comparison.

## Baseline figures this amendment replaces

Recorded here so the after-figures have something to be compared against:

- Poll tick: **50 ms**, contributing an expected p95 near **47.5 ms** against a 16 ms budget.
- `poll()` cost: **~10.3 ms** against the 50 ms period (21% duty) — not saturating.
- Throughput ceiling: **~374 KB/s** measured, against a reader sustaining ~69 MB/s while
  actually reading.
- Per-pane poll cost: **~10.1 ms**, measured linear, saturating at 5 panes — which is why
  `terminal_session_limit` is `Some(3)`.
- `dropped_bytes`: always `0` today, **only because the sleep starves the reader** —
  ~18.7 KB accumulates per poll against a 64 KiB cap.

## PR-A1-A — The reader thread and bounded channel

*Not started.*

## PR-A1-B — The ingress re-proof

*Not started.*

## PR-A1-C — Remove the tick and the sleep

*Not started.*

## PR-A1-D — Measurement and closeout

*Not started.*
