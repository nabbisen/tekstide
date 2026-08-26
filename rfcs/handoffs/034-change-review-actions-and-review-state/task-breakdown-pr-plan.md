---
title: "RFC-034 task breakdown and PR plan"
rfc: "RFC-034"
rfc_file: "../../done/034-change-review-actions-and-review-state.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-034 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-26"
---

# Two slices and a closeout

**Do the wording before the wiring.** The code in this RFC is a function call that already exists;
the difficulty is entirely in what the surface says. A slice that lands the buttons first and
writes the sentences afterwards will write the sentences to fit the layout, which is the wrong way
round.

## PR-034-A — the words, and where they go

**No new behaviour. No button that does anything yet.** This is the slice the security document
is about.

1. **Write the sentences** the security document's §1–§3 require: what a decision does not do to
   any file, that it is final, and that it does not survive closing Tekstide. `en.ftl`, reviewed
   as text before it is reviewed as code.
2. **Answer §4** — the disclosure-density question. `en.ftl` already carries 28 `change-review-*`
   strings and you are adding to them. Decide how these claims reach a reader who has stopped
   reading, write the decision down in `qa-evidence.md`, and implement that decision rather than
   a stack of lines.
3. **Decide where the controls live**, per the pack README's three options, and **measure it** by
   extending RFC-042's headless layout test rather than reasoning about pixels.

**Evidence:** the release binary, a `mktemp -d` fixture, a screenshot of the surface with the
claims rendered as they will ship — before any control works. If they cannot be read at a glance
in that screenshot, §4 is not answered yet.

**Gate:** full suite green; no behaviour changed.

## PR-034-B — the controls

Now the wiring.

1. **Two controls, `Accepted` and `Rejected`** (D1), offered from `Unreviewed` and
   `PartiallyAccepted` only. Never `PartiallyAccepted` or `Superseded` — a control may record an
   opinion, it may not assert a fact.
2. **Withdrawn once a decision is recorded** (D4). The state line carries what was decided.
3. **Both handlers guarded against modal exclusivity**, in the shape the seventeen existing
   guards already use. Copy `clicking_a_change_review_row_while_a_modal_is_open_has_no_effect`'s
   test, do not invent a new shape.
4. **The stale-tree notice** (D3), reusing `diff_content_is_stale`. The decision stays live; the
   notice is its own sentence, distinct from `change-review-content-stale`.

**Required tests, at minimum:**

- **`rejecting_a_change_set_does_not_modify_any_file`** — the security document's own falsifiable
  form. Real files on disk, real bytes compared before and after a real `Rejected` transition
  through the real message path. **This is the test the slice exists to be able to pass.**
- Accept and Reject each reach `transition_change_set_review_state` for real and the state line
  changes.
- After a decision, neither control is offered.
- `PartiallyAccepted` and `Superseded` are never offered from any reachable state.
- Both controls are inert while a modal is open.
- A stale tree shows the notice **and** leaves the controls live.

**Ablations, separately:**

- Offer `Superseded` → the "never offered" test fails.
- Keep the controls after a decision → the withdrawal test fails.
- Drop the modal guard on one handler → that handler's inertness test fails.
- Remove the session-scoped sentence → its own test fails. *(If no test fails, §4 was answered by
  putting words on screen that nothing holds in place. Write the test.)*

**Evidence:** the walkthrough the pack README describes — controls live with both the finality and
session-scope claims visible **at the same time**, then the surface after a decision.

## Closeout

- **`CHANGELOG.md` says what this is at its real size**: a note to yourself, for this session,
  that changes no file and cannot be taken back. Do not describe it as review workflow.
- **D2 stated plainly**: a review decision produces **no** audit record, so the audit store's
  silence is never read as evidence that no decision was made.
- **The successor question restated** where someone will find it: *should the audit store record a
  user's decision about generated code?*
- `ARCHITECTURE.md` gains D1's rule if it has earned it: **a control may record an opinion; it may
  not assert a fact.**
- README's change-review paragraph updated — it currently describes a read-only surface.

## Not in this plan

- Revert, stage, apply (RFC-030). Per-file decisions. An audit family. Persistence.
- Changing `ReviewState`'s variants or `can_transition_review_state`. If either looks wrong, that
  is an RFC-012 amendment and is reviewed as one.

## If §6 happens

The security document's §6 allows the honest outcome that this is not worth shipping as scoped.
**If PR-034-A's own evidence makes that case, stop there and say so.** Do not build PR-034-B to
avoid an awkward report. A held slice with a written reason is a good outcome; a shipped control
nobody should trust is not.
