---
title: "RFC-024: Diff Preview Policy - QA Evidence"
rfc: "RFC-024"
rfc_file: "../../proposed/024-diff-preview-policy.md"
status: "Accepted 2026-08-11 — not started"
target_milestone: "M10"
created: "2026-08-11"
---

# QA Evidence

Record results here as each slice lands: gate output, ablations with the exact failure
they produced, findings, and limitations.

**This file is where results go. It is not where obligations go.** If a slice discovers
something a later slice must handle, put it in that slice's entry in
`task-breakdown-pr-plan.md` as well. Four obligations have been lost to that gap.

## Recording conventions

- **Ablations name the exact failure**, not "the test failed."
- **One ablation per property.** An ablation breaking two things proves neither.
- **A green ablation is a defect in the ablation**, not a pass.
- **Measured beats estimated.** Two bounds in this project were wrong until measured.
- **Retire obligations explicitly.** When a carried item stops applying, say so and why,
  where it was recorded.
- **Correct by annotation, not rewrite.** If a claim here is later found wrong, annotate
  it in place — the record of *when* something was discovered is part of the evidence.

## PR-024-A — Design and handoff acceptance

Granted by the human owner 2026-08-11 with RFC-024. Handoff pack authored the same day.

## PR-024-B — Gating and bounds

Pending implementation.

## PR-024-C — Content access with a bounded lifetime

Pending implementation.

## PR-024-D — Baseline authority, and closeout

Pending implementation.

## Known Limitations

Consolidated at closeout. Carried in from RFC-024's own text:

- **This RFC renders nothing.** RFC-020 owns the surfaces; content produced here is
  unescaped by design.
- **No action on a change** — no accept, revert, or stage. Detection and preview only.
- **Git-backed detection is unchanged**, and still gated behind RFC-012's own safety
  evidence.
- **The diff algorithm is not this RFC's contribution.** Its value is the policy around a
  solved problem.

## What this RFC hands to RFC-020

To be filled in at closeout — RFC-020's handoff will be written from this section:

- the produced diff's shape, and that it is unescaped;
- the refusal's shape, so a surface can render one rather than showing nothing;
- the stale-baseline signal's shape;
- the bound, so RFC-020 does not introduce a second one.
