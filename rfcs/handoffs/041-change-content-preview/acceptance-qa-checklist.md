---
title: "RFC-041: Acceptance / QA Checklist"
rfc: "RFC-041"
rfc_file: "../../accepted/041-change-content-preview.md"
source_rfc_status: "Accepted 2026-08-25 — M12, third of three"
target_milestone: "M12"
created: "2026-08-25"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The acceptance criterion

- [ ] **A user opening a changed file on the change review surface sees its content**, gated and
      bounded by RFC-024's existing policy, reached from a **visible control**.
- [ ] What they see is labelled for what it is — **"not a diff"** on screen for a modified file.

## PR-041-A — retention and reaching the gate

- [ ] `DetectedChanges` retained session-scoped, keyed by `ChangeSetId`; not a field on the
      persisted `ChangeSet`.
- [ ] `gate_diff_content_read` / `read_diff_content` have production callers. **No second gating
      path.**
- [ ] RFC-024's bounds, refusal semantics, non-text classification and staleness untouched.
- [ ] Ablated: retention dropped → the content test fails.
- [ ] Ablated separately: a change set whose retention was dropped **still renders its metadata** —
      D1's own claim.

## PR-041-B — rendering — `what-a-content-preview-must-not-claim.md`

- [ ] Per change kind: Added whole content; Modified current content; Deleted the fact.
- [ ] **"Not a diff" rendered on the surface**, and **ablated** — remove the label, a test fails.
- [ ] Content escaped via `quote_untrusted`; bidi fixture tested; the impossible half not
      re-asserted.
- [ ] A stale baseline refuses **and names the reason**.
- [ ] Content never reaches `ProjectSession`, a `Clone` state struct, or an audit record.
- [ ] `DiffContent`'s `Debug` hand-implemented — kind and length, never bytes.
- [ ] The move-out gap documented **at the type**.
- [ ] The new control is in `control_coverage` and `click_message_kind`.

## Closeout

- [ ] `README.md` corrected; **RFC-020 corrected with a dated note, not a rewrite** — it is closed.
- [ ] Deferrals stated: no two-sided diff (RFC-030), the move-out gap, absence-of-visible-change.
- [ ] Gates: `fmt`, `clippy -D warnings`, full suite three runs, `git diff --check`.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
