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

- [x] **A user opening a changed file on the change review surface sees its content**, gated and
      bounded by RFC-024's existing policy, reached from a **visible control**.
      `change_review_surface_shows_real_content_from_a_real_agent_run`: real managed launch, real
      approval, real exit, a real `Message::ChangeReviewFileRowPressed` dispatched through
      `update`, real rendered content asserted against the real bytes written. Live-verified via
      the keyboard route (`Ctrl+Alt+D`, `Enter`) — see `qa-evidence.md`'s own disclosed caveat
      about mouse clicks in this session's automation environment.
- [x] What they see is labelled for what it is — **"not a diff"** on screen for a modified file.
      `change_review_content_modified_content_is_labelled_not_a_diff`, ablated; live-verified.

## PR-041-A — retention and reaching the gate

- [x] `DetectedChanges` retained session-scoped, keyed by `ChangeSetId`; not a field on the
      persisted `ChangeSet`. `state.detected_changes_by_change_set` (`shell.rs`), not
      `ProjectSession`, not `tekstide-core` at all.
- [x] `gate_diff_content_read` / `read_diff_content` have production callers. **No second gating
      path.** Called from `select_change_review_file`/`change_review_content_lines` exactly as
      RFC-024 built them.
- [x] RFC-024's bounds, refusal semantics, non-text classification and staleness untouched.
      `DiffPreviewPolicy::default()` used directly; `diff.rs`'s existing 24 tests all pass
      unchanged.
- [x] Ablated: retention dropped → the content test fails.
      `change_review_content_is_unavailable_when_retention_was_dropped` — the ablation is the
      test itself (a real `HashMap::remove`, not a hand-edit cycle, since this is genuinely
      runtime-driven unlike RFC-035's non-configurable const).
- [x] Ablated separately: a change set whose retention was dropped **still renders its metadata** —
      D1's own claim. Same test: `summary.changed_file_count` asserted unaffected.

## PR-041-B — rendering — `what-a-content-preview-must-not-claim.md`

- [x] Per change kind: Added whole content; Modified current content; Deleted the fact.
      `change_review_content_added_content_has_no_not_a_diff_label` /
      `change_review_content_modified_content_is_labelled_not_a_diff`; `Deleted`/`NonTextContent`/
      `NonFile` each get their own named line, reusing RFC-024's own classification.
- [x] **"Not a diff" rendered on the surface**, and **ablated** — remove the label, a test fails.
      Confirmed: removed the label push, `change_review_content_modified_content_is_labelled_not_a_diff`
      failed with the exact missing-line value; reverted, green.
- [x] Content escaped via `quote_untrusted`; bidi fixture tested; the impossible half not
      re-asserted. `change_review_content_escapes_a_bidi_override_in_file_content` — a real bidi
      override in real file content renders as `<U+202E>`, never raw. Only the achievable half
      asserted, per §5's own carve-out.
- [x] A stale baseline refuses **and names the reason**.
      `change_review_content_refuses_when_the_file_changes_after_selection` — a real, later write
      after selection produces the real refusal naming "no longer authoritative", and the newer
      content never renders.
- [x] Content never reaches `ProjectSession`, a `Clone` state struct, or an audit record.
      Structural: `DiffContent` derives neither `Clone` nor `Serialize`; `ChangeReviewSelection`
      (the only thing `State` retains past a selection) holds a path and an `Option<FileSnapshot>`
      only, never bytes.
- [x] `DiffContent`'s `Debug` hand-implemented — kind and length, never bytes.
      `diff_content_debug_never_prints_file_bytes`, ablated by hand (temporarily printing `bytes`
      directly): a 5,200-byte payload produced a 21,047-character debug string versus the
      redacted 255; reverted, green.
- [x] The move-out gap documented **at the type**. `DiffContent`'s own doc comment, `diff.rs`.
- [x] The new control is in `click_message_kind`. **Not `control_coverage`**, correctly — that
      table is keyed by `NavigationAction` (global keybindings), and a single row's own
      activation is not one, the same reason explorer/approval-history row activation have no
      entry there either. Stated explicitly rather than silently done differently from what the
      handoff's own generic phrasing suggested.

## Closeout

- [x] `README.md` corrected; **RFC-020 corrected with a dated note, not a rewrite** — it is closed.
      Three README mentions corrected; RFC-020's own "Closeout, 2026-08-25" section gets a new,
      dated "Note, 2026-08-25 (RFC-041)" paragraph rather than an edit to the original prose.
- [x] Deferrals stated: no two-sided diff (RFC-030), the move-out gap, absence-of-visible-change.
      All three, in `README.md`, `qa-evidence.md`'s own "Known limitations", and (the latter two)
      on the surface itself.
- [x] Gates: `fmt`, `clippy -D warnings`, full suite three runs, `git diff --check`. Clean: 426
      tekstide + 742 tekstide-core, all three runs, no flake.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
