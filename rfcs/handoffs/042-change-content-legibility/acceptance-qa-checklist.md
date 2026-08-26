---
title: "RFC-042 acceptance and QA checklist"
rfc: "RFC-042"
rfc_file: "../../accepted/042-change-content-legibility.md"
source_rfc_status: "Accepted 2026-08-26 — M12, first of three for 0.15.0"
target_milestone: "M12"
created: "2026-08-26"
---

# Acceptance and QA checklist

## D2 — impersonation is unrepresentable

- [x] A content value cannot be constructed or rendered where a chrome value is expected.
      **Evidenced by a compile failure**, with the error recorded, not by a runtime test.
      `E0308`, `qa-evidence.md` PR-042-A.
- [x] The spoof fixture exists — a file whose first lines are `Detection: Complete`,
      `Review state: Accepted`, `1 file changed` — and a test asserts none of them can be
      confused with the real lines. `change_review_content_spoof_lines_are_never_rendered_as_chrome`.
- [x] The renderer no longer discriminates chrome from content by index.
      `ChangeReviewContentPreview`'s three fields (`heading`/`chrome`/`content`) replace the old
      `if index == 0` position check.

## D1 — the frame does not scroll

- [x] Heading, detection disclosure, detection status, both omission counts, review state and the
      "not a diff" label are outside the scroll region. `column(lines)`, non-scrolling; only
      `content_body` (built from `content_elements` alone) feeds `scrollable(...)`.
- [x] A test asserts the label is present with content long enough to scroll.
      `change_review_content_label_survives_content_long_enough_to_scroll` — a real ~100KB file.
      Live evidence covers reachability and correct layout; genuine interactive scrolling is
      outside what a `frames()`-avoiding unit test or this session's live pass can observe —
      disclosed in `qa-evidence.md` rather than claimed.
- [x] Ablated: label back inside the scroll region, that test fails, restored.
      `change_review_frame_lines_never_feed_the_scrollable` (structural), confirmed failing then
      reverted.

## D3 — bounded, refusing, and distinct

- [x] A line bound exists in `DiffPreviewPolicy`, beside the byte bound. `max_lines`, alongside
      `max_input_bytes`.
- [x] Over the bound the preview **refuses**. It does not truncate.
      `change_review_content_refuses_over_the_line_bound_and_names_it`, and the truncate-instead
      ablation confirmed the refusal path is load-bearing.
- [x] The refusal names which bound it hit and is distinguishable from RFC-024's byte refusal, the
      stale-baseline refusal, `omitted_changed_file_count` and
      `changed_files_omitted_by_detection`. **Five facts, five sentences.**
      `change-review-content-error-too-many-lines`, its own Fluent key and wording.
- [x] The bound's value comes from a **measurement recorded in `qa-evidence.md`**, not from a
      choice. `change_review_content_view_build_cost_by_line_count_measurement`, both release and
      debug profiles, against `NFR-PERF-003`'s existing 16ms budget. `DEFAULT_MAX_DIFF_LINES = 4000`.

## Escaping is not weakened

- [x] A fixture containing a tab, a carriage return, an ANSI escape sequence and a bidi override
      renders all of them escaped.
      `change_review_content_line_split_does_not_weaken_escaping_of_other_control_characters`.
- [x] Ablated: relax `quote_untrusted` for one of those, that test fails, restored.
      Excluded tab from `is_untrusted_display_control`; confirmed failing, reverted.
- [x] The line break is the only character this slice stops escaping. Verified by the same test:
      `<U+000A>` never appears, every other marker still does.

## Fixtures

- [x] Multi-line ordinary source. `change_review_content_renders_real_line_structure_not_one_escaped_blob`.
- [x] Long enough to scroll (D1). `change_review_content_label_survives_content_long_enough_to_scroll`.
- [x] The spoof (D2) — **written first**. `change_review_content_spoof_lines_are_never_rendered_as_chrome`.
- [x] Over the bound (D3). `change_review_content_refuses_over_the_line_bound_and_names_it`.
- [x] Other control characters. `change_review_content_line_split_does_not_weaken_escaping_of_other_control_characters`.

## Live GUI evidence

- [x] Captured against a **`mktemp -d` fixture project**. No path under `$HOME`, no real project
      name, no real file content. See `ARCHITECTURE.md`, "A committed screenshot may only ever
      show throwaway state." `/tmp/tmp.pgzhKLaKI4`; screenshots kept in `.git-exclude/` rather
      than committed, since the persisted project list happens to include an unrelated row whose
      path encodes an operator identity — disclosed, not silently worked around.
- [x] Whether a real mouse click was sent is **stated either way**. No — keyboard only
      (`wtype`); no pointer-injection tool available this session, same gap `0.14.0`'s own
      release gate disclosed. The click path itself is unchanged from RFC-041 and already covered
      by two tests dispatching the real `Message` through `update`.

## Gates

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] Full workspace suite, **three consecutive runs** under default parallelism, each **logged to
      a file** rather than filtered live, any flake named against `test-process-leak.md`. Run 2
      hit the original, already-documented `bind_recovers_from_a_stale_socket_file` flake (row 1)
      — unrelated to this slice (no approval/socket code touched), assertion message captured
      this time (`ApprovalChannelError { reason: Io, source: None }`). Runs 1 and 3 clean, 433
      tekstide + 742 tekstide-core.
- [x] `git diff --check`, `rfc_docs_invariants`. Both clean.

## Closeout

- [x] `ARCHITECTURE.md` gains D1's rule: *a claim that qualifies content stays visible for as long
      as that content is visible.*
- [x] `ARCHITECTURE.md` gains the fixture rule: *a fixture that omits the shape under test proves
      nothing about that shape.*
- [x] README's change-review section corrected — it currently discloses the escaping as a shipped
      limitation, which this slice makes false.
- [x] `CHANGELOG.md`'s `0.14.0` entry is **not** rewritten; it was true when written.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
