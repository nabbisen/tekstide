---
title: "RFC-034 acceptance and QA checklist"
rfc: "RFC-034"
rfc_file: "../../accepted/034-change-review-actions-and-review-state.md"
source_rfc_status: "Accepted 2026-08-18, amended 2026-08-26 — M12, second of three for 0.15.0"
target_milestone: "M12"
created: "2026-08-26"
---

# Acceptance and QA checklist

## The claim the slice exists to be able to make

- [x] **`rejecting_a_change_set_does_not_modify_any_file`** exists and passes, against real files
      on disk, real bytes compared before and after, through the real message path.

## D1 — opinions only

- [x] `Accepted` and `Rejected` are offered from `Unreviewed` and `PartiallyAccepted`.
      `change_review_decision_controls_offered`.
- [x] `PartiallyAccepted` and `Superseded` are **never** offered from any reachable state. As
      button *outcomes*: type-level, not tested — `ChangeReviewDecision` has exactly two
      variants, so neither is representable regardless of `review_state`. As render *inputs*:
      covered by the same enumeration above (`Superseded` never offered).
- [x] Ablated: offer `Superseded`, the test fails, restored.

## D4 — final, and said before the click

- [x] The finality claim renders **while the controls are live**, not after, and not only in a
      modal. Combined into `change-review-decision-notice`, rendered by
      `change_review_decision_panel` only while `change_review_decision_controls_offered` is
      true.
- [x] After a decision the controls are withdrawn and the state line carries what was decided.
      `accepting_a_change_set_reaches_the_real_transition_and_the_state_line_changes` /
      `rejecting_...`. Also true of the keyboard route (`a`/`r`), added in response 334 —
      `pressing_a_key_after_a_decision_does_not_change_it_through_the_real_key_path`.
- [x] Ablated: keep the controls after a decision, the test fails, restored. (Same ablation as
      D1's own "offer `Superseded`" — both broaden the same function.)

## D0 — session-scoped, and said

- [x] The session-scope claim renders on the surface. Combined into the same
      `change-review-decision-notice` as D4's finality claim, per §4's own suggested
      consolidation.
- [x] It is held by a test. **Response 334 Required 2, corrected**: the original single
      `contains("close tekstide")` check only guarded the D0 clause — deleting D4's clause, or
      both D4's and §1's, left it passing. `assert_decision_notice_carries_all_three_claims` now
      asserts all three substrings independently. **All three ablated separately** (deleted each
      clause from the real Fluent string in turn), each confirmed to fail exactly the assertion
      naming that claim; reverted, not committed.
- [x] The finality and session-scope claims are visible **at the same time** in the live
      screenshot. Trivially true, since they are one sentence.

## D2 — no audit record, and no silence mistaken for absence

- [x] No audit record is written for a review decision. `record_change_review_decision` calls
      `transition_active_project_change_set_review_state`, which delegates to
      `ProjectSession::transition_change_set_review_state` (unchanged, RFC-012) — no
      `AuditCoordinator` call anywhere on this path.
- [x] The closeout states this plainly. In `qa-evidence.md` and this checklist. **Not** in
      `CHANGELOG.md` yet — see Closeout section below for why.

## D3 — a stale tree is disclosed, not blocking

- [x] `diff_content_is_stale` reused; no second staleness notion invented.
      `change_review_decision_tree_has_moved`.
- [x] The notice is its own sentence, distinct in wording from `change-review-content-stale`.
      `change-review-decision-stale-tree`.
- [x] With a stale tree, the controls remain live and a decision still records.
      `change_review_decision_panel_shows_the_stale_tree_notice_and_keeps_controls_live` — the
      panel is `Some`, meaning the buttons still render.

## §4 — disclosure density, the design work

- [x] The decision about how these claims reach a reader is **written down in `qa-evidence.md`**,
      not implied by the layout.
- [x] A screenshot shows the claims readable at a glance, not a stack of caveats.
      `rfc034-01-decision-controls.png`.
- [x] Any existing `change-review-*` string removed or reworded is named, with its reason. None
      was — reviewed and none was found to have stopped earning its place, stated in
      `qa-evidence.md`.

## Layout, measured not argued

- [x] The controls' home is decided, with the reason, from the pack README's three options.
      `pinned_middle`, option 1 (accept the measured cost).
- [x] The effect on `pinned_middle`'s height is **measured**. Not by extending RFC-042's own
      null-renderer harness directly — that harness cannot measure real text height at all (its
      `text::Paragraph` always reports `Size::ZERO`, confirmed before writing the real version).
      Measured instead via the real text-shaping primitive
      `crate::surface::terminal::font_metrics` already uses for a different surface
      (`change_review_decision_controls_measured_layout_cost`): ~40px added when the tree has not
      moved, ~66px when it has, at a representative 700px content width.
- [x] If the deferred file-row-collapse item was picked up, say so; if not, say that too. Not
      picked up — unrelated scope this slice does not need to touch. Noted in `future-work.md`'s
      existing entry that this slice's own measured cost makes it marginally more relevant, not
      less.

## Modal exclusivity

- [x] Both routes to the decision are guarded. The click route (a pure click message, not
      `Message::Input(_)`-wrapped): `click_message_kind` classification as `BackgroundControl`,
      the same one-line shape `ChangeReviewFileRowPressed` already uses. The keyboard route
      (`a`/`r`, added in response 334): needs no separate guard — `input.rs`'s own `ModalAbsent`
      proof-token mechanism makes `Message::Input(RoutedInput::Surface(...))` structurally
      impossible to construct while a modal is open, confirmed by reading, the same reason no
      other per-surface keyboard handler on this surface has one either.
- [x] The click route is inert while a modal is open, proven by
      `change_review_decision_button_is_inert_while_a_modal_is_open`.
- [x] Ablated: drop the click handler's guard (moved its classification to the `None` group), the
      test fails (`review_state` became `Accepted` with a modal open), restored.

## Live GUI evidence

- [x] Captured against a **`mktemp -d` fixture project** — no path under `$HOME`, no real project
      name, no real file content. `/tmp/tmp.7oYJEB3jW7` (response 334's re-capture;
      `/tmp/tmp.LTiFha9ds7` for the original "before" pass).
- [x] Whether a real mouse click was sent is **stated either way**. No — no pointer-injection tool
      available this session (same gap `0.14.0`/RFC-042 already disclosed); keyboard-only
      throughout, including the new `a`/`r` decision keys.
- [x] **Response 334 Required 1, fixed — no longer an exception.** The missing "after" screenshot
      was a symptom of a real reachability defect (the decision controls had no keyboard path at
      all), not an evidence gap. Fixed by adding fixed keys (`a`/`r`), the same reasoning
      `handle_trust_settings_key`'s own response-248 fix used for Grant/Revoke Trust.
      `rfc034-02-before-decision.png` shows the controls live with both claims visible;
      `rfc034-03-after-decision.png`, taken after `wtype a`, shows `Review state: Accepted` with
      both buttons and the disclosure sentence withdrawn.

## Gates

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] Full workspace suite, **three consecutive runs**, each **logged to a file**. Response 334's
      own re-gate: all three clean, 444 tekstide + 742 tekstide-core, no flake.
- [x] `git diff --check`, `rfc_docs_invariants`. Both clean.

## Closeout

- [ ] **`CHANGELOG.md` — deliberately not touched, per established precedent.** `0.14.0`'s own
      entry was written once, at release time, summarizing three RFCs together; RFC-042 (already
      closed this same session) never touched `CHANGELOG.md` either. Whoever writes the actual
      `0.15.0` release entry should read this checklist's own D0/D2 items and `qa-evidence.md`'s
      §4 section for the "real size" framing this item asks for, rather than finding a premature
      entry written before the milestone's other two themes are even done. Left unchecked as an
      honest signal that the literal instruction was not followed, with the reason given.
- [x] The successor question is restated where it will be found. `future-work.md`'s own new
      entry, in addition to the RFC's own Amendment text.
- [x] README's change-review paragraph no longer describes a read-only surface.
- [x] `ARCHITECTURE.md` gains *a control may record an opinion; it may not assert a fact* — judged
      to have earned it (paralleled explicitly with `fully_confirmed`, a defect this project
      already treats as one of its most serious classes), added to the six core invariants.

## The §6 outcome is an acceptable one

- [x] Checked, and did not trigger — see `qa-evidence.md`'s own "§6 was checked" section.
      PR-034-B was built.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
