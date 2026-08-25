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
- [ ] ~~**The falsifiable claim, tested**: a generated change containing a bidi override
      renders it visibly in the diff surface.~~ Still not this item's slice, now for a
      confirmed rather than provisional reason: PR-020-C shipped 2026-08-25 scoped to
      metadata only (file paths, counts, detection status, review state), never diff
      *content* — there is no diff surface for a bidi override in file *content* to render
      into. What PR-020-C **does** carry is the narrower, applicable claim — a bidi
      override in a detected file *path* renders visibly — and that one is checked:
      `change_review_file_entry_line_escapes_a_bidi_override_in_the_path`. The original
      claim (an override inside changed file *content*) remains genuinely blocked on the
      same `DetectedChanges`-retention future work as `ChangeLifecycle` above.

## The change review surface (PR-020-C)

**Scoped down 2026-08-25** (RFC-020's own scoping addendum, then `change-review-surface.md`):
diff *content* rendering (`ChangeLifecycle`, per-path refusal reasons, stale-baseline-vs-error
distinctions) requires retaining `DetectedChanges` past `add_detected_generated_change_set`,
which currently discards it. Not built in this slice — recorded as future work, not a gap in
this checklist. The items below are marked accordingly rather than left as open pending work
this slice was never scoped to do.

- [ ] ~~Rendered per `ChangeLifecycle`; never inferred from `ChangePathKind`.~~ Out of scope —
      no diff content is rendered at all in this slice; `ChangeLifecycle` is not reachable from
      `ChangeSetSummary`. Future work, per the scoping addendum.
- [ ] ~~The `Modified` case is labelled as not-a-diff where the user reads it.~~ Out of scope —
      no per-file `Modified`/`Added`/`Deleted` case is rendered; the surface names the file
      list only, not lifecycle. Future work.
- [x] No heading, label, or affordance implies a two-sided comparison. Confirmed by reading:
      `change_review_view` renders a heading ("Change Review"), a disclosure, detection status,
      a file count, a file list, and a review-state line — no diff/comparison language anywhere.
- [ ] ~~Every refusal renders and is distinguishable from a file with no changes.~~ Out of scope
      — no per-path refusal reasons (`TooLarge`, non-text, symlink escape, unreadable) exist in
      `ChangeSetSummary`; those belong to the `DetectedChanges` projection this slice does not
      read. Future work.
- [ ] ~~A stale baseline renders as stale.~~ Out of scope — no per-file staleness concept exists
      at the `ChangeSetSummary` level. Future work.
- [x] Detection's metadata-only limitation appears **on the surface**.
      `change-review-disclosure` (`en.ftl`): "Detected changes only, not all changes:
      detection is metadata-only and conservative, and excludes .git/, target/, and
      node_modules/ by design. This is not a review, an approval, or a claim that a change is
      safe." Rendered unconditionally, not only when a `ChangeSet` exists.
- [x] No second bound; any display limit is named as a display concern.
      `omitted_changed_file_count` (a *display* truncation) and
      `ChangeDetectionStatus::Partial{limit}` (a *scan* truncation) render as two distinct
      lines when both are true, never collapsed —
      `change_review_detection_status_line_renders_each_status_distinctly` (also the
      reviewer's own ablation, review response 322: collapsing `Partial`'s symbol into
      `complete` failed the test as designed).
- [x] Read-only stated on the surface where a user might expect an action. The disclosure
      states "not a review, an approval" explicitly; no approve/reject/accept control exists
      anywhere on the surface (RFC-034's own job).
- [x] **(Added, not in the original list)** File paths are untrusted and escaped —
      `change_review_file_entry_line_escapes_a_bidi_override_in_the_path`, the bidi fixture
      this RFC's own falsifiable claim requires.
- [x] **(Added)** A real, visible control, not only a keystroke — RFC-039's third
      reachability principle. The "Change Review" button on `trust_settings_view`, and
      `Ctrl+Alt+D` converging on the same function (`open_diff_review`); both proven
      click- and keyboard-reachable.

## Honesty checklist (PR-020-D)

- [x] Claim statement checked **against RFC-020's own text**, not only the evidence file —
      re-read `020-diff-review-and-agentrun-report.md`'s own scoping addendum
      (2026-08-25) before implementing, not only `change-review-surface.md`'s summary of it.
- [x] **No claim that this renders a diff for a modified file.** Confirmed by reading
      `change_review_view` and the checklist item above.
- [x] **No claim about diff quality or algorithm.** No diff exists to make a claim about.
- [x] No claim that detection coverage improved. Unchanged — same `GeneratedChangeDetector`,
      same exclusions.
- [x] `DiffContent`'s non-retention described accurately — untouched by this slice
      (`ChangeSetSummary` never holds a `DiffContent`); the existing description in RFC-020's
      own open-questions section stands as before.
- [x] No claim that a Git-backed before-source exists.
- [x] Every unchecked line above carries a stated reason (the strikethrough items in "The
      change review surface" section, each with an explicit "out of scope" / "future work"
      reason and a pointer to what would unblock it).

## Evidence Required

- [x] Commit/PR list and gate output. `2da223d` (RFC-020: render the change review surface)
      plus the closeout commit adding `TEKSTIDE_CHANGESET_DEMO` and this document set. Gate
      output: `qa-evidence.md`, PR-020-C section, "Gates" paragraph.
- [x] The enumeration tests and their ablations, with exact failing values. The reviewer's
      own ablation (review response 322): collapsing `ChangeDetectionStatus::Partial`'s
      symbol into `complete` failed
      `change_review_detection_status_line_renders_each_status_distinctly` by design; restored,
      green.
- [x] The measured window size and how it was measured. PR-020-B's own item (transcript
      window, unrelated to this slice) — already checked under "The window boundary" above;
      not re-measured here since nothing about it changed.
- [x] GUI evidence for both surfaces: real screenshots, real keystrokes individually
      dispatched, stating what each proves **and does not**. PR-020-B: attempted, environment
      issue, disclosed substitution (see PR-020-B's own entry above). PR-020-C: two full
      rounds, empty state and populated state, both disclosed precisely (see PR-020-C's own
      entry above).
- [x] What M10 delivered and did not, consolidated. See "Known limitations, consolidated" in
      `qa-evidence.md` — both surfaces now implemented and reachable; diff *content*
      rendering remains future work, disclosed rather than silently dropped.
- [ ] What this hands forward to RFC-030. Not assessed in this slice — RFC-030 was not
      re-read against the delivered surface's actual shape; flagged for whoever picks up
      RFC-030 rather than guessed at here.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
