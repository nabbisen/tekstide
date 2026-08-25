---
title: "RFC-041: Change Content Preview — implementation handoff"
rfc: "RFC-041"
rfc_file: "../../done/041-change-content-preview.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-041 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-25"
---

# Reach what `0.7.0` already built

Source RFC: [RFC-041](../../done/041-change-content-preview.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-041](../../done/041-change-content-preview.md) | Three decisions already made; do not re-open them |
| 2 | [RFC-024](../../done/024-diff-preview-policy.md) | **Read this before writing code.** It decided the policy and built the machinery; your job is to call it |
| 3 | [`what-a-content-preview-must-not-claim.md`](./what-a-content-preview-must-not-claim.md) | **Required.** The naming risk is the largest one here |
| 4 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Two slices |
| 5 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |
| 6 | [`qa-evidence.md`](./qa-evidence.md) | Where evidence goes |

## What this is

`0.13.0` shows **which** files an agent run touched and cannot show **what it did to them**.

The model is not missing. `read_diff_content` and `gate_diff_content_read`, both in
`project/diff.rs`, were built, gated, reviewed across four slices and shipped in
`0.7.0` — with bounds, non-text classification before any read, and staleness detection. **Both
have zero production callers.**

The single missing link: `add_detected_generated_change_set` **discards** the `DetectedChanges`
that `read_diff_content` takes as its first argument.

So this slice retains what is discarded and renders what is already gated. Most of the work is
reading RFC-024 carefully enough not to rebuild it.

## The thing that will be tempting, and is wrong

**Do not compute a diff.** For a modified file the before-bytes were never captured —
`ReviewBaselineEntry` holds `relative_path`, `kind`, `len`, `modified_unix_nanos` and no content,
by RFC-012's own design principle. They are *gone, not merely unretained*. RFC-024 found this
before building and corrected its own scope for it.

What ships is **current content, labelled not a diff**. If that feels unsatisfying, that is the
correct feeling and the honest response is RFC-030, not an approximation.

## What "done" means

A user opening a changed file on the change review surface sees its content, bounded and gated by
RFC-024's existing policy, **labelled for what it actually is** — and content is never retained
beyond the request, per RFC-024 Decision 1, which is binding and not reopened.

## Scope boundaries

**In:** retaining `DetectedChanges` (D1), calling the existing gate and reader, rendering per
change kind, the staleness refusal (D2), and `DiffContent`'s `Debug` (D3).

**Out:** re-deciding any of RFC-024 — bounds, refusal-not-truncation, non-text classification,
baseline authority, escaping. A two-sided diff. Retaining content. Acting on a change, which is
RFC-034's and should follow this.

## Premise re-verified 2026-08-25, after RFC-035 landed

RFC-035 modified `add_detected_generated_change_set` — the exact function this slice's premise
rests on — so the premise was re-checked rather than assumed still true:

- `add_detected_generated_change_set` still takes `detected: &DetectedChanges` **by reference and
  keeps none of it**. RFC-035 added an omitted *count* through it; the value itself is still
  dropped at the end of the call.
- `read_diff_content` and `gate_diff_content_read` still have **zero production callers**.

Both premises hold. Citations here name functions rather than line numbers, deliberately: a
sibling handoff written the same morning had two line citations go stale within hours when
RFC-035 shifted them. **If a citation in this document does not resolve, the citation is wrong,
not the code** — find it by name and say so in your evidence.
