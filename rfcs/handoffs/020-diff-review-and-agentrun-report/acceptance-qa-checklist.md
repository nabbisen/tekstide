---
title: "RFC-020: Diff Review and AgentRun Report Surfaces - Acceptance / QA Checklist"
rfc: "RFC-020"
rfc_file: "../../accepted/020-diff-review-and-agentrun-report.md"
status: "Open"
target_milestone: "M10"
created: "2026-08-15"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason. An absence with a reason is
evidence; an absence without one is a gap.

## The window boundary (PR-020-B)

- [x] A window starting inside a control sequence classifies identically to the same
      content read whole, against a real sequence boundary in real captured output.
      (`a_window_starting_inside_a_real_control_sequence_classifies_identically_to_the_whole`,
      core, 2026-08-15)
- [x] Resynchronization ablated; the **specific** divergence shown with the exact wrong
      value. (`ablation_without_resynchronization_the_split_misclassifies`)
- [x] The delivered start offset is reported, and differs from the requested one under
      ablation. (`TranscriptWindow::delivered_start()`/`requested_start()`)
- [x] No UTF-8 scalar split at either edge.
      (`resynchronization_never_splits_a_utf8_scalar`)
- [x] The window size is **measured** against the real 32 MiB ceiling, not estimated.
      `DEFAULT_TRANSCRIPT_WINDOW_BYTES`'s own doc comment; response 198 Finding 2 corrected
      an earlier, wrong sweep in place rather than silently.

## The transcript reader (PR-020-B)

- [x] Reader window and writer truncation render differently; a test pins the distinction.
      Surface-side, 2026-08-18: `reader_window_and_writer_truncation_render_as_distinct_notices`
      (`agent_run_detail_notices`) -- a full window produces 2 notices, a partial window
      produces a *different* 2nd notice, and independently marking the writer truncated adds
      a textually-distinct 3rd.
- [x] Complete vs. still-being-written expressed **in the type**, not a doc comment.
      (`TranscriptWindow::Complete`/`::StillBeingWritten`)
- [x] Read-only proven by enumeration; a new reading call site fails by name.
      (`only_this_module_opens_a_transcript_file_for_reading`)
- [x] No reader path reaches a mutating call; retention metadata untouched.
- [x] Raw bytes survive the reader, proven against `text_safety`'s own bidi probe.
      (`raw_bytes_survive_the_reader_including_bidi_and_format_characters`, core) --
      re-proved surviving through to the widget's own escaping call, surface-side, 2026-08-18
      (`transcript_body_escapes_a_real_override_and_does_not_double_escape_literal_marker_text`).

## Escaping (PR-020-B and PR-020-C)

- [x] Escaping happens at the widget; models return raw bytes. **PR-020-B side only** --
      `agent_run_detail_transcript_body` (`quote_untrusted`, the widget); the reader (D3)
      never escapes. PR-020-C's own widget does not exist yet.
- [x] No second escaping primitive introduced. **PR-020-B side**: reuses `quote_untrusted`.
- [x] No double-escaping — literal `<U+202E>` text is distinguishable from a real override.
      **PR-020-B side**, 2026-08-18:
      `transcript_body_escapes_a_real_override_and_does_not_double_escape_literal_marker_text`.
      See that test's own doc for the precise claim -- the two cases cannot render as
      *different visible text* (that is `quote_untrusted`'s own contract), what is proven is
      that the isolate marks are never themselves visible as escaped text, which is the
      concrete shape a second escaping pass would take.
- [ ] **The falsifiable claim, tested**: a generated change containing a bidi override
      renders it visibly in the diff surface. **Not this item's slice** -- this is
      PR-020-C's own diff-review widget, which does not exist yet (blocked on its own
      `DetectedChanges` projection). PR-020-B's own AgentRun report has no diff/change
      content to render; its equivalent claim ("a real override in transcript content
      renders visibly") is the item above, which is checked.

## The change review surface (PR-020-C)

- [ ] Rendered per `ChangeLifecycle`; **never** inferred from `ChangePathKind`.
- [ ] The `Modified` case is labelled as not-a-diff **where the user reads it**; the exact
      wording is quoted and justified.
- [ ] No heading, label, or affordance implies a two-sided comparison.
- [ ] Every refusal renders and is distinguishable from a file with no changes:
      `TooLarge`, non-text, path-not-detected, symlink escape, unreadable.
- [ ] A stale baseline renders as stale — distinct from an error and from an empty diff —
      proven against a real file changed on disk after capture.
- [ ] Detection's metadata-only limitation appears **on the surface**.
- [ ] No second bound; any display limit is named as a display concern.
- [ ] Read-only stated on the surface where a user might expect an action.

## Honesty checklist (PR-020-D)

- [ ] Claim statement checked **against RFC-020's own text**, not only the evidence file.
- [ ] **No claim that this renders a diff for a modified file.**
- [ ] **No claim about diff quality or algorithm.**
- [ ] No claim that detection coverage improved.
- [ ] `DiffContent`'s non-retention described accurately — two specific storage paths
      blocked, not general retention.
- [ ] No claim that a Git-backed before-source exists.
- [ ] Every unchecked line above carries a stated reason.

## Evidence Required

- [ ] Commit/PR list and gate output.
- [ ] The enumeration tests and their ablations, with exact failing values.
- [ ] The measured window size and how it was measured.
- [ ] GUI evidence for both surfaces: real screenshots, real keystrokes individually
      dispatched, stating what each proves **and does not**.
- [ ] What M10 delivered and did not, consolidated.
- [ ] What this hands forward to RFC-030.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
