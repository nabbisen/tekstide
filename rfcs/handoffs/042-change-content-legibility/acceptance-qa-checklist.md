---
title: "RFC-042 acceptance and QA checklist"
rfc: "RFC-042"
rfc_file: "../../done/042-change-content-legibility.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-042 is in rfcs/done/"
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
      confused with the real lines. `change_review_content_spoof_lines_are_never_rendered_as_chrome`
      — response 331 corrected this test's own doc comment: it proves the *data-level*
      classification, not the *render-level* placement (see below).
- [x] The renderer no longer discriminates chrome from content by index.
      `ChangeReviewContentPreview`'s three fields (`heading`/`chrome`/`content`) replace the old
      `if index == 0` position check.
- [x] **Response 332 Required 3, actually closed**: response 331's own fix was a source-scan of
      `change_review_view`'s body, and the reviewer defeated it by extracting a second helper
      function and calling `.as_str()` from there instead — a scan of one function cannot see a
      call from another. `ChangeReviewContentLine` and the render function now live in their own
      module; the struct's field and its `as_str` accessor are both private to it (the accessor
      additionally `#[cfg(test)]`-gated). Nothing outside the module can read a content line's
      own text under any name, at any distance. The stale scan test is removed — there is
      nothing left to test at the render level. Both of the reviewer's own exact exploits
      confirmed as compile errors (`E0616` private field; `E0599` method does not exist outside
      `cfg(test)`), then reverted.

## D1 — the frame does not scroll

- [x] Heading, detection disclosure, detection status, both omission counts, review state and the
      "not a diff" label are outside the scroll region.
- [x] **Response 331 Required 1, D1 amended**: the file-row list (variable length, up to
      `DEFAULT_CHANGESET_PATH_SUMMARY_LIMIT`) was still living in the pinned region, making the
      label *unreachable* (not merely scrollable-past) on a window shorter than the frame's own
      content-dependent height. `change_review_view` restructured into four independent regions
      (`assemble_change_review_layout`): `pinned_top`, the file-row list in its own `scrollable`,
      `pinned_middle` (including the label), the content region. Pinned regions' own height no
      longer depends on file-row count.
- [x] A test asserts the label is present with content long enough to scroll.
      `change_review_content_label_survives_content_long_enough_to_scroll` — a real ~100KB file.
- [x] Ablated: label back inside the scroll region, that test fails, restored.
      **Response 331 Required 2**: the previous guard (a source-text scan) was defeated by the
      reviewer reformatting the same defect (`scrollable(column![...])` instead of
      `scrollable(column(lines)`) — replaced with
      `change_review_layout_pins_fixed_regions_regardless_of_list_length`, which computes a real
      `layout::Node` tree via `()` (iced's headless test renderer) against the real production
      `assemble_change_review_layout`. Both of the reviewer's own attacks reproduced and confirmed
      failing against this new test, then reverted.
- [x] **Response 332 Required 1: the ordering invariant, stated and tested.** The reviewer
      accepted the bounded-height fix but found the deeper guarantee ("content is never visible
      without the claim that qualifies it") held only "by accident of ordering" — nothing tested
      it. Added: `pinned_middle`'s own bottom must sit at or above the content region's own top,
      at four viewport heights including tiny ones. **A real gap found ablating this**: read as
      fixed-index bounds, the check is vacuous (`Column` always preserves declaration order, so
      it holds for *whatever* occupies the two slots) — confirmed by swapping `pinned_middle` and
      `content`'s declaration order, which left it green. Strengthened with a `Tree::tag`
      (`iced_core::widget::tree::Tag`) check confirming positions 0/2 are stateless (`Column`) and
      1/3 are not (`Scrollable`) — the same swap then fails immediately. Both checks together
      close the gap the first version left open.
- [ ] **Left open, at the architect's own request from response 331**: at the 380px reproduction
      height, the file-row list's own scroll region collapses to zero visible height (not
      scrolled, not clipped, absent) — flagged by the reviewer as a real but non-blocking
      readability issue, not required in this slice.

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
      a file** rather than filtered live, any flake named against `test-process-leak.md`. Response
      331's own re-gate hit disclosed, transient PTY exhaustion (the reviewer's) then the
      already-documented `command_approval_family_...` flake (mine) across six total runs — see
      `qa-evidence.md`'s own Response 331 section. Response 332's own re-gate: three consecutive
      runs, all clean, 433 tekstide + 742 tekstide-core (one fewer than response 331's 434 — the
      stale source-scan test was removed, not replaced with a new one).
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

- [x] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Accepted 2026-08-26 at review 333, after three rounds: requests 331, 332, 333
(commits 75a6850, f746eb5, e475f60, c0a035f, ee3a94d).

Verified by the reviewer, by attack rather than by reading:

Round 1 (response 331) -- two guards passed while the property they named was
broken:
  - D1's guard was a source-text scan asserting exact indentation. Wrapping the
    whole surface back in one scrollable -- the pre-RFC-042 defect in full --
    left all four assertions green, because the negative one looked for the
    literal "scrollable(column(lines)" and the defeat spells it
    "scrollable(column![".
  - D2's spoof test asserted the data classification, which PR-042-A had already
    made true by construction. Pushing content into the chrome frame via
    as_str() left 11 of 11 tests green.
  - D1's own decision was also wrong, and that was the reviewer's fault: the
    frame held up to 32 file-row buttons, so at a 380px window height the "not a
    diff" label became unreachable. Reproduced in the release binary. D1 amended
    to "pin the claims, scroll the lists."

Round 2 (response 332) -- Required 2 closed properly; Required 3 reported closed
and was not. An extracted helper one level away from change_review_view defeated
the .as_str() scan with 29 of 29 tests green.

Round 3 (this request) -- both closed, verified:
  - Module boundary: the reviewer's two exploits are now compile errors, E0599
    (no such method outside cfg(test)) and E0616 (private field). The field stays
    private even in a test build. Both `cargo test --all-targets` and
    `cargo clippy --all-targets` reject them, so the project's own gate enforces
    the boundary -- not only a release build.
  - Layout ordering: transposing pinned_middle and content inside
    assemble_change_review_layout fails on the Tree::tag assertion, naming the
    transposition.

The dev team found that the reviewer's own required assertion was VACUOUS before
reporting it closed. A Column lays children out in declaration order, so
children[2].bottom <= children[3].top holds for whatever occupies those slots --
the assertion as specified could never fail. The Tree::tag check is what makes it
real. That correction is the most valuable thing in this arc: it runs against the
reviewer, and it was found by ablating a green test rather than trusting it.

Gates verified independently by the reviewer: three consecutive full-workspace
runs, logged to files, all clean -- 433 tekstide + 742 tekstide-core + 2
doc-invariant, no flake. fmt clean.

Not done, by the reviewer's own decision: the file-row list region collapses to
zero height at 380px -- real, not a correctness issue, left for a follow-up.
```
