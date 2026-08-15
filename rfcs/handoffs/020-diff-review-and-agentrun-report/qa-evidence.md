---
title: "RFC-020: Diff Review and AgentRun Report Surfaces - QA Evidence"
rfc: "RFC-020"
rfc_file: "../../proposed/020-diff-review-and-agentrun-report.md"
status: "Open — no slices implemented yet"
target_milestone: "M10"
created: "2026-08-15"
---

# QA Evidence

Record results here as each slice lands, with the reasoning that produced them.

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap — an item recorded only in an evidence file is an item
the next implementer does not read. If a slice discovers a new obligation, put it in the
task breakdown, not only here.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. One
  ablation per property. **A green ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does
  not find.
- **GUI evidence**: `niri msg action screenshot-window`; synthetic input with
  `env -u WAYLAND_DISPLAY`, `xdotool windowfocus` (not `windowactivate`), always
  `--clearmodifiers`. Compare captures at one window geometry — comparing across
  geometries, or across different *screens*, has produced wrong claims here twice.
- State what each piece of evidence **does not** prove, alongside what it does.

## PR-020-A — Design and handoff acceptance

Granted 2026-08-12 with the pack. RFC-020's four open questions answered in the pack's
README: Option B (owner's decision), no second bound (RFC-024's measured 4 MiB stands),
read-only, and `DiffContent` left owned with its limitation carried forward accurately.

## PR-020-B — The transcript reader, and the AgentRun report surface

*Not started.*

## PR-020-C — The change review surface

*Not started.*

## PR-020-D — Closeout

*Not started.*

## Known limitations, consolidated

To be filled at closeout. The ones already known going in, which must survive into the
closeout rather than being rediscovered:

- **No two-sided diff for a modified file.** The before-bytes were never captured
  (`ReviewBaselineEntry` is metadata-only by RFC-012 §Design Principles 2) and are gone,
  not merely unretained, by preview time.
- **Detection is metadata-only and conservative**; the change set may be incomplete.
- **`DiffContent` blocks two specific storage paths**, not general retention — a consumer
  can destructure it and keep the bytes.
- **The transcript window is a view, not the whole transcript**, and is distinct from the
  writer's retention truncation.
- **No Git-backed before-source exists**; it is gated behind RFC-012's unmet safety
  evidence.
