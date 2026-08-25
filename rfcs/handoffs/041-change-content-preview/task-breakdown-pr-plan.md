---
title: "RFC-041: task breakdown and PR plan"
rfc: "RFC-041"
rfc_file: "../../done/041-change-content-preview.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-041 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-25"
---

# Two slices, A then B

## PR-041-A — retain `DetectedChanges`, and reach the gate

**Build (core):** a session-scoped retention of `DetectedChanges` keyed by `ChangeSetId`, per D1
— **not** a field on the persisted `ChangeSet`. The retained value must be droppable without the
`ChangeSet` becoming wrong; that relationship is the whole reason for the shape.

**Build (the call):** `attempt_generated_change_detection` currently discards the
`DetectedChanges` it computes. Retain it there, at the one site that has it.

Then a production caller for `gate_diff_content_read`/`read_diff_content` — **reusing** them, not
deriving a second gating path. RFC-024 ablated the ordering (classify non-text *before* any read)
rather than asserting it; a second path would not inherit that.

**Do not touch:** RFC-024's bounds, refusal semantics, non-text classification, or
`diff_content_is_stale`.

**Ablate:** drop the retention, confirm the test that reaches real content fails. Separately,
confirm a change set whose retention was dropped still renders its metadata unharmed — that is
D1's claim and it needs its own test.

## PR-041-B — render it, labelled for what it is

**Read [`what-a-content-preview-must-not-claim.md`](./what-a-content-preview-must-not-claim.md)
first.**

**Build:** per-change-kind rendering on the existing change review surface — Added: whole content;
Modified: **current content, labelled not a diff**; Deleted: the fact of deletion.

- **The "not a diff" label is on the screen**, beside the content, not in a tooltip or the README.
- Content is untrusted text in trusted chrome: `quote_untrusted`, bidi fixture tested. Assert the
  achievable half (a real override renders as a visible marker); do not re-assert the half
  RFC-020 established is impossible.
- A stale baseline **refuses and names the reason** (D2).
- Reaching content needs a **visible control**, not only a keystroke — RFC-040's pattern, and
  RFC-039's third reachability principle. A row you can activate is the obvious shape; whatever
  you choose, it goes in `control_coverage` and `click_message_kind` like every other control.
- Content never enters `ProjectSession`, a `Clone` state struct, or an audit record.

**`DiffContent`'s `Debug`** (D3): hand-implemented, kind and length, never bytes. The move-out gap
documented at the type, not closed.

**Ablate:** remove the "not a diff" label and confirm a test fails — that claim is the one this
slice most needs guarded, and a label nothing tests is a label someone deletes while tidying.

## Closeout

Fold into PR-041-B.

- Correct what this falsifies: `README.md` and RFC-020's own text both say content is not
  rendered. **RFC-020 is closed** — correct it with a dated note, not a rewrite.
- State the deferrals: no two-sided diff (blocked on RFC-030), the `DiffContent` move-out gap,
  and that absence of visible change is not absence of change.

## Standing expectations

- Single-variable ablations, the unit being the design decision.
- Three consecutive full-suite runs under default parallelism.
- Disclose flakes against `test-process-leak.md`'s three causes.
- If your slice makes a shipped statement false, correcting it is part of your slice.
