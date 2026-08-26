---
title: "RFC-034: Change Review Actions and Review State — implementation handoff"
rfc: "RFC-034"
rfc_file: "../../done/034-change-review-actions-and-review-state.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-034 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-26"
---

# Let a user record a decision, and never let the record claim more than it is

Source RFC: [RFC-034](../../done/034-change-review-actions-and-review-state.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-034](../../done/034-change-review-actions-and-review-state.md) | **Read the Amendment at the end first.** D0–D4 are decided; do not re-open them |
| 2 | [`what-a-review-decision-must-not-claim.md`](./what-a-review-decision-must-not-claim.md) | **Required.** Every risk in this slice is a wording risk |
| 3 | [RFC-012](../../done/012-generated-change-review-foundations.md) | Froze `ReviewState` and built `transition_change_set_review_state`. You are calling it, not changing it |
| 4 | [RFC-042](../../done/042-change-content-legibility.md) | Owns the four-region layout your controls land in, and the module boundary around content |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Two slices and a closeout |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |

## The one-sentence version

`transition_change_set_review_state` has existed since RFC-012 and has never had a caller. Give it
one — and make the surface say, before the user clicks, the two things about that click that would
otherwise surprise them.

## What is already built

- `ProjectSession::transition_change_set_review_state(&mut self, &ChangeSetId, ReviewState)` —
  `crates/tekstide-core/src/project/session.rs`. Delegates to `ChangeSet::transition_review_to`,
  records activity, refreshes the runtime summary. **Do not change it.**
- `can_transition_review_state` — `crates/tekstide-core/src/domain/changeset.rs`. The legality
  table. Read it before writing any control; D1 and D4 both come straight out of it.
- `change_review_state_line` — already renders the current state on the surface.
- `diff_content_is_stale` — RFC-024/041's staleness check, which D3 says to reuse rather than
  reinvent.

## The decisions, in one place

All five are settled in the RFC's Amendment. Restated here only so nobody implements from memory:

- **D0 — a decision is session-scoped, and the surface says so.** `ChangeSet` derives no
  `Serialize`; `ProjectSession::change_sets` is a `Vec` in memory. Closing Tekstide discards every
  change set and every decision about one. Persisting them is a local-data-policy question (real
  project paths on disk) and a thirteenth audit family is its own RFC. Neither is this slice.
- **D1 — `Accepted` and `Rejected` only**, from `Unreviewed` and from `PartiallyAccepted`.
  `PartiallyAccepted` has no way to be true without per-file review. `Superseded` is a fact about
  a later change set, not a user's opinion. **A control may record an opinion; it may not assert a
  fact.**
- **D2 — no audit record**, and the closeout says so plainly, so nobody later reads the audit
  store's silence as evidence that no decision was made.
- **D3 — a stale tree is disclosed, and does not block the decision.** The opposite of what
  content preview does, deliberately: content refuses when stale because bytes from a passed
  moment misrepresent what you are looking at; a *decision* is about the change set as detected,
  which staleness does not invalidate.
- **D4 — a decision is final; say so before the click**, while the controls are still live. After
  a decision the controls are withdrawn and the state line carries what was decided.

**D0 and D4 ship together or the controls do not ship.** Final and session-scoped are both
surprising, and a user told only one will infer the other wrongly.

## Where this lands, and the one thing that will bite

RFC-042 restructured this surface into four regions: `pinned_top`, the file-row `scrollable`,
`pinned_middle`, and the content `scrollable`. Your controls are Tekstide's own chrome, so they
belong in **`pinned_middle`**, beside the review-state line.

**`pinned_middle` does not scroll, by design.** That is D1-of-RFC-042 doing its job — the "not a
diff" label must not be scrollable away from the content it qualifies. It also means every line
you add there is a line that can push the region past a short window's edge. At 380px it already
clips today, and the file-row region already collapses to zero height there (a real item, flagged
in review 332 and deliberately left unscheduled).

**Adding two buttons and two sentences to that region makes it measurably worse.** You have three
honest options and this pack does not choose for you:

1. Add to `pinned_middle` and accept it, having measured how much worse at the heights that
   matter.
2. Pick up the deferred file-row-collapse item as part of this slice, since you are in the same
   code.
3. Propose a different home for the controls, in writing, with the reason.

**Whichever you choose, measure it with the harness RFC-042 built.**
`change_review_layout_pins_fixed_regions_regardless_of_list_length` already computes a real
`layout::Node` tree headlessly; extend it rather than reasoning about pixels in prose.

## Traps this surface has already set for people

- **Modal exclusivity.** Seventeen per-handler guards exist because `click_message_kind` returns
  `None` for `Message::Input(_)`, so a message-level guard cannot see every path. Two new
  handlers need the same treatment, and there are existing tests
  (`clicking_a_change_review_row_while_a_modal_is_open_has_no_effect`) to copy the shape from.
- **Disclosure density.** `en.ftl` already carries **28** `change-review-*` strings. You are
  adding more. A surface where every sentence is a caveat is a surface where none is read — see
  the security document's §4, which is the real design work in this slice.
- **The content module boundary.** `ChangeReviewContentLine` and its renderer live in
  `mod change_review_content` with a private field and a `cfg(test)`-only accessor. Nothing here
  needs to reach past that. If you find yourself wanting to, stop and say why.

## Live GUI evidence

Required. **Capture against a `mktemp -d` fixture project** — fixture file names, fixture content,
never a path under `$HOME`, per `ARCHITECTURE.md`. State whether a real mouse click was sent
either way; silence is not an answer.

The walkthrough must show, on screen: the controls before a decision **with both the final and
session-scoped sentences visible at the same time**, and the surface after a decision with the
controls withdrawn and the state line updated.

## Deferrals to state, not to solve

- No revert, stage, or apply. No before-side exists; that is RFC-030.
- No per-file decisions, which is what would make `PartiallyAccepted` reachable.
- No audit record (D2), and no thirteenth family.
- No persistence (D0). **The successor question is already written down** in the RFC's Amendment:
  *should the audit store record a user's decision about generated code?*
