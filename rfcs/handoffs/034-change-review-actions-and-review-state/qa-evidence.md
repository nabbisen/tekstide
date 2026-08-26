---
title: "RFC-034: QA evidence"
rfc: "RFC-034"
rfc_file: "../../accepted/034-change-review-actions-and-review-state.md"
source_rfc_status: "Accepted 2026-08-18, amended 2026-08-26 — M12, second of three for 0.15.0"
target_milestone: "M12"
created: "2026-08-26"
---

# QA evidence

## §6 was checked and did not trigger

Read `what-a-review-decision-must-not-claim.md` §6 before writing any code, per its own
requirement. Having written the sentences §1–§3 require, the feature does **not** read as
worthless: "a note to yourself that vanishes when you close the window, about files nothing will
change, which you cannot take back" is an accurate but not a damning description — working
through one sitting's changes without losing your place, honestly labelled at that size, is a
real and modest value. PR-034-B was built. This is stated because the checklist requires it be
stated either way, not only when the answer is "stop."

## §4 — the disclosure-density decision, written down before implementing it

`en.ftl` carried 28 `change-review-*` strings before this slice. The security document's own
menu of legitimate answers was used, not a fourth line stacked onto the existing pile:

- **D0 (session-scoped) and D4 (finality) collapsed into one sentence**, per §4's own suggestion
  that they are "arguably one fact: this is a note to yourself for now." One Fluent key
  (`change-review-decision-notice`) carries all three of: no file is modified (§1's falsifiable
  claim), the decision is final (§3), and it does not survive closing Tekstide (D0) — one
  sentence, not three.
- **The claim is attached to the control it qualifies** — rendered directly beside the two
  buttons in `pinned_middle`, not appended to the bottom of the existing stack.
- **The button labels themselves carry part of the claim**, redundantly by design: "Mark
  accepted"/"Mark rejected" reads as recording a note, not performing an action, independent of
  whether the adjacent sentence gets read. Two independent channels for the same fact, the same
  "defence in depth" shape this project's own modal-exclusivity machinery already uses elsewhere.
- **No existing `change-review-*` string was reworded or removed.** Reviewed the other 28
  against this slice's own addition; none was found to have stopped earning its place. Stated
  here since the checklist requires naming this decision either way.
- **The stale-tree notice is the one sentence that is genuinely conditional** — it renders only
  when the tree has actually moved, so it costs nothing in the common case rather than being a
  fourth permanent line.

Net addition when the tree has not moved: **one sentence plus one button row.** With a stale
tree: one more sentence. Not "a finality sentence, a session-scope sentence, a stale-tree
sentence, and two button labels" (four permanent additions) — two, most of the time.

## Implementation note: A and B landed together, not sequentially

The pack's own instruction — "do the wording before the wiring" — is a *process* discipline (decide
what the surface says before deciding how big the buttons are), not strictly a commit-ordering
one. Both were designed in that order (the combined-sentence wording above was decided, then
measured, then wired), but landed in one continuous implementation pass rather than two separate
gated commits, given the size of this slice relative to RFC-042's. The evidence below still
answers each slice's own required items separately.

## Layout: the controls' home, decided, and measured

**Home: `pinned_middle`**, per the pack README's own recommendation — beside the review-state
line it updates, so a user reads what happened and what they can do about it in the same place.
Options 2 (pick up the RFC-042 file-row-collapse item) and 3 (a different home) were not taken:
option 2 is unrelated scope this slice does not need to touch to ship its own value, and no
argument for a different home improved on "beside the state it changes."

**Measured, not argued** (`change_review_decision_controls_measured_layout_cost`, extending
RFC-042's own headless approach). The RFC-042 null-renderer harness
(`change_review_layout_pins_fixed_regions_regardless_of_list_length`'s own technique) cannot
answer this question at all: its own `text::Paragraph` implementation
(`iced_core::renderer::null`) returns `Size::ZERO` unconditionally, so a `Text` widget always
measures zero height under it regardless of content — confirmed directly before writing the real
version (an early attempt reported `pinned_middle` height as `0px` for a page containing one real
sentence). Rebuilt using the real text-shaping primitive `crate::surface::terminal::font_metrics`
already uses for a different surface (`iced::advanced::graphics::text::Paragraph`, backed by
`cosmic-text` — no window, no GPU, real measured glyphs), reused rather than reinvented.

At a representative 700px content width (this session's own live evidence window: 1042px total,
~260px sidebar):

| element | height |
| --- | --- |
| review-state line (baseline, for comparison) | 13px |
| the combined D0/D4 notice | 26px (wraps to ~2 lines) |
| the stale-tree notice (conditional) | 26px |
| the button row | 14px |

**Net added cost: ~40px when the tree has not moved, ~66px when it has** (before
`.spacing(8)` between items, which adds roughly 16–24px more depending on how many new items
render). This is the "measurably worse" cost the pack's own README warned about, quantified
rather than guessed at.

**Decision: accept it, having measured it (option 1 of the pack's own three).** At the two
window heights already confirmed working for RFC-042's own surface (450px, and normal use), this
addition is well within tolerance. At the reviewer's own 380px reproduction height — already
known, before this slice, to clip `pinned_middle` — this slice makes an already-clipping surface
clip further. Not a new defect this slice introduces; the same one RFC-042 response 332 left
open, now with a slightly larger margin against it. Recorded, not hidden: see `future-work.md`'s
existing entry for the file-row-collapse item, which this slice's own measurement makes
marginally more relevant to pick up, not less.

## D1/D4 — opinions only, final, said before the click

`ChangeReviewDecision` (GUI-layer, `{ Accepted, Rejected }`) is narrower than
`tekstide_core::domain::ReviewState`'s five variants, the same "narrower than the domain type"
idiom `ContentLifecycle` already established ahead of `ChangeLifecycle`. **`PartiallyAccepted`
and `Superseded` are not offered as button outcomes by construction** — no test proves this,
because there is nothing left to test: the type cannot represent either, so no button this
surface builds can ever produce one, regardless of future edits to `change_review_view`.

**`change_review_decision_controls_offered(review_state)`** governs whether the controls render
at all — `true` for exactly `Unreviewed`/`PartiallyAccepted`, `false` for the other three.
`change_review_decision_controls_offered_exactly_from_unreviewed_and_partially_accepted` checks
all five variants in one enumeration.

**Two ablations, both confirmed failing then reverted:**

1. Broadened the match to also permit `Superseded` → the `Superseded` assertion failed.
2. Broadened it to always return `true` → the `Accepted`/`Rejected` assertions (the "withdrawn
   after a decision" case) failed.

`accepting_a_change_set_reaches_the_real_transition_and_the_state_line_changes` and its Reject
counterpart dispatch the real `Message` through `update`, assert `change_set.review_state`
actually changed, and assert `change_review_state_line`'s own resolved text reflects it.

## The claim the slice exists to be able to make

**`rejecting_a_change_set_does_not_modify_any_file`**: a real fixture (`existing.txt`, `new.txt`,
both real files on disk from `state_with_a_real_change_set_and_retained_detection`), real bytes
read before and after, a real `Message::ChangeReviewDecisionButtonPressed(Rejected)` dispatched
through the real `update` function — not `record_change_review_decision` called directly. Bytes
identical before and after; the change set's own `review_state` confirmed to have actually
transitioned to `Rejected`, so the "nothing changed" assertion is not trivially true for an
uninteresting reason (the decision never having recorded at all).

## D3 — the stale tree is disclosed, not blocking

**Scoping decision, stated plainly rather than left implicit**: `ChangeSet` itself carries no
per-file snapshot (only `changed_files: Vec<PathBuf>`), and `DetectedChanges` carries kind and
lifecycle but no `FileSnapshot` per path either. The only per-file baseline this application
retains anywhere is `ChangeReviewSelection::baseline`, captured once at *selection* time by
RFC-041/042's own content-preview machinery. `change_review_decision_tree_has_moved` reuses
exactly that — and exactly `diff_content_is_stale` (RFC-024) — against whichever file is
currently selected for the same change set. **This checks "has the selected file moved since it
was selected," not literally "has any file in the whole change set moved since detection."**
Re-scanning every changed file's own metadata would be inventing a second, heavier staleness
mechanism, which D3 explicitly says not to do; this is the mechanism that already exists, applied
honestly to the one file this application has a real baseline for. Stated here so the scoping is
a decision on record, not a gap discovered later.

Wording, distinct from `change-review-content-stale`'s own ("This change set's baseline is no
longer authoritative... Re-select it to see the current content"):
`change-review-decision-stale-tree` says "The files on disk have changed since this change set
was detected. You can still record your decision about what was detected." — a *decision* is
about a historical fact staleness does not invalidate; content preview's own refusal is about
*bytes from a moment that has passed*. Two different facts, two different sentences, the same
distinction that split the two omission counts in `0.14.0`.

**Tests**: `change_review_decision_panel_shows_the_stale_tree_notice_and_keeps_controls_live` (a
real, later write after a real selection; the stale-tree line renders, the panel is still
`Some` — controls remain offered) and
`change_review_decision_panel_has_no_stale_notice_when_nothing_has_moved` (the base case: no
stale line, the session/finality notice still renders).

## D0/D4's own required ablation

**`ChangeReviewDecisionPanel`** (new) carries the resolved disclosure text as `Vec<String>` — the
same "resolved string, not the `Element` tree" split every other rendered line on this surface
already uses, and specifically why this is independently testable: `change_review_view`'s own
conversion of `panel.lines` into `Element`s is a trivial, unconditional loop with nothing left to
decide, so testing `panel.lines` directly is testing exactly what reaches the screen — the same
reasoning RFC-042's own D1 fix relied on for `ChangeReviewContentPreview`.

**Ablated**: commented out `lines.push(state.catalog.get("change-review-decision-notice"))`
inside `change_review_decision_panel`. Both
`change_review_decision_panel_has_no_stale_notice_when_nothing_has_moved` and
`change_review_decision_panel_shows_the_stale_tree_notice_and_keeps_controls_live` failed (empty
`lines`, or missing the session-scope sentence respectively). Reverted, not committed.

## Modal exclusivity

`Message::ChangeReviewDecisionButtonPressed` classified as `ClickMessageKind::BackgroundControl`
in `click_message_kind`, the same one-line classification `ChangeReviewFileRowPressed` already
uses — the centralized `state.modal.is_some() && click_message_kind(...) == BackgroundControl`
check ahead of `update`'s own `match` is what governs it, not a new per-handler guard (this
message is a pure click message, not a `Message::Input(_)`-wrapped keyboard path, so it does not
need one).

**Ablated**: moved the message's classification arm to the `None`-returning group instead.
`change_review_decision_button_is_inert_while_a_modal_is_open` failed — the decision recorded
(`review_state` became `Accepted`) while a modal was open. Reverted, not committed.

## Live GUI evidence

Release binary, `mktemp -d` fixture project (`/tmp/tmp.LTiFha9ds7`), `TEKSTIDE_CHANGESET_DEMO=1`,
launched as `tekstide "$FIXTURE_DIR"`. `niri msg action focus-window --id` then `wtype` for
`Ctrl+Alt+D`/`Enter` (per `ARCHITECTURE.md`'s own convention). Screenshot:
`.git-exclude/tmp/rfc034-evidence/rfc034-01-decision-controls.png` (kept out of the repo per this
session's own established practice — the persisted project list includes rows whose paths encode
an operator identity, the same reason RFC-042's own screenshots were kept out).

Shows, in one frame: `Review state: Unreviewed`, the combined D0/D4 disclosure sentence
("Marking this here only records your own note about it: it changes no file, cannot be undone,
and disappears when you close Tekstide."), both real buttons ("Mark accepted", "Mark rejected"),
and — below — the preview heading and boxed content, all rendering correctly together. **Both the
finality and session-scope claims are visible at the same time**, trivially, since they are one
sentence.

**What this live pass does not show, disclosed rather than silently narrowed: the surface after
a decision.** Whether a real mouse click was sent: **no** — no pointer-injection tool was
available in this session (`ydotool`/`wlrctl`/`dotool` all absent, `wtype` is keyboard-only), the
same gap `0.14.0`'s own release gate and RFC-042's own live passes already disclosed. Unlike the
file-row button (which has both a click route and a keyboard route via
`handle_change_review_key`'s own `Enter` case), **the decision buttons have no keyboard path** —
the pack's own task breakdown does not ask for one, and adding keyboard navigation for exactly
two buttons (a highlight cursor, Tab/Arrow handling, a new piece of `State`) was judged out of
this slice's own scope, not something to build solely to make live evidence possible. The "after"
state is therefore proven by
`accepting_a_change_set_reaches_the_real_transition_and_the_state_line_changes`,
`rejecting_a_change_set_reaches_the_real_transition_and_the_state_line_changes`, and
`rejecting_a_change_set_does_not_modify_any_file` — all three dispatch the real
`Message::ChangeReviewDecisionButtonPressed` through the real `update` function, the same
click-message path a real mouse click would take, exactly the same substitution this project's
own review responses have already accepted for the file-row button's own click path.

## Response 334 — two required fixes

### Required 1: the decision controls were mouse-only -- a keyboard user could not record a decision at all

Correctly reframed by the reviewer as a **reachability defect**, not an evidence gap: the missing
"after" screenshot was a symptom, not the finding. `handle_change_review_key` handled
`ArrowUp`/`ArrowDown`/`Enter` for file rows only; there was no key that reached either decision
control. This project has fixed this exact shape twice before (`ApprovalHistory`, response 234;
`TrustSettings`, response 248) — this was the third occurrence, on the surface whose whole reason
to exist is offering these two controls.

**Fixed, adopting `handle_trust_settings_key`'s own reasoning rather than inventing a new one**:
`a`/`r` are **fixed keys**, not a shared highlight cursor -- the two decision controls are
independent actions (like Grant/Revoke Trust), not interchangeable list rows, so forcing a cursor
to move past one to reach the other would be the wrong shape. Checked before use, per the same
comment's own instruction: every global keybinding in `KeybindingPolicy::linux_mvp()` requires at
least `Ctrl+`, so a bare, unmodified character key never matches `matching_global_action`
(`input.rs`) and always reaches `handle_change_review_key`. Checked *before* the row-navigation
guard that returns early on zero rows -- a decision is about the whole change set, not the file
list, and must not become unreachable in that hypothetical case.

**No new modal-exclusivity guard needed, and this is itself worth recording**: `handle_change_review_key`
is reached via `Message::Input(RoutedInput::Surface(...))`, which `input.rs`'s own `ModalAbsent`
proof-token mechanism makes *structurally impossible to construct* while a modal is open (the
non-modal input-routing function requires a `ModalAbsent` token, obtainable only by checking
`modal.is_none()` immediately beforehand) -- confirmed by reading, not assumed, and consistent
with every other existing keyboard handler on this surface (`ArrowUp`/`ArrowDown`/`Enter` for file
rows) having no per-handler modal check either.

**Tests**, all dispatching through the real message path (`send_main_area_key`, the same
`Message::Input(RoutedInput::Surface(...))` route a real keystroke takes, not
`handle_change_review_key` called directly):

- `pressing_a_accepts_the_change_set_through_the_real_key_path`
- `pressing_r_rejects_the_change_set_through_the_real_key_path`
- `pressing_a_key_after_a_decision_does_not_change_it_through_the_real_key_path` -- proves the
  `offered` check inside the handler is load-bearing: presses `a` then `r`, asserts the change set
  stays `Accepted` (the second key must not overwrite the first decision).

**Live GUI evidence, redone**: same release binary, a fresh `mktemp -d` fixture
(`/tmp/tmp.7oYJEB3jW7`), keyboard-only throughout. `rfc034-02-before-decision.png` shows the
controls live, `wtype a` records the decision, `rfc034-03-after-decision.png` shows `Review
state: Accepted`, both buttons and the disclosure sentence withdrawn, exactly as D4 requires. The
checklist's own "partially met" item is now fully met; no exception remains.

### Required 2: consolidating three claims into one sentence had consolidated their guards into one

The reviewer ablated each of the combined sentence's three claims independently and found only
the D0 (session-scope) clause was actually guarded -- deleting the D4 (finality) clause, or both
D4 and §1 (no file is touched), left all 33 `change_review*` tests passing, since the only
assertion in either test checked for `"close tekstide"` alone.

**Fixed**: `assert_decision_notice_carries_all_three_claims` (new, shared by both tests that
inspect `panel.lines`) asserts on all three substrings independently -- `"it changes no file"`
(§1), `"cannot be undone"` (D4), `"close tekstide"` (D0) -- against the lines joined together, so
it does not matter which of `panel.lines`' entries carries which clause.

**All three ablated independently, each confirmed to fail exactly the assertion naming that
claim, then reverted**:

1. Deleted "it changes no file, " from `change-review-decision-notice` -- the §1 assertion failed
   in both tests, D4/D0 assertions still passed.
2. Deleted "cannot be undone, " -- the D4 assertion failed, §1/D0 still passed.
3. Deleted ", and disappears when you close Tekstide" -- the D0 assertion failed, §1/D4 still
   passed.

The sentence itself is unchanged -- §4's own consolidation was correct design; what was wrong was
testing it with one substring check standing in for three independent claims.

### Gate, re-run

Three consecutive full-workspace runs, all clean: 444 tekstide + 742 tekstide-core + 2
doc-invariant, no flake. `fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`
all clean.
