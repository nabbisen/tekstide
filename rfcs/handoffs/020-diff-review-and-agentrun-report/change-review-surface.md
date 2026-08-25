---
title: "RFC-020: the change review surface — slice handoff"
rfc: "RFC-020"
rfc_file: "../../accepted/020-diff-review-and-agentrun-report.md"
source_rfc_status: "Accepted, partially implemented — the AgentRun report shipped in 0.12.0"
target_milestone: "M12"
created: "2026-08-25"
---

# The change review surface

**Named by surface, not by letter, deliberately.** RFC-020's Slices section says PR-020-B is the
change review surface and PR-020-C is the AgentRun report. The opposite is what happened: the
AgentRun report shipped as PR-020-B in `0.12.0`. The architect already mislabelled these two once
when recommending scope. This slice is **the change review surface**; do not refer to it by a
letter.

Scheduled second of three, after RFC-040 and before the minimal user documentation.

## What it is

Since `0.11.0` this product has been able to detect what an agent run changed and unable to show
it. `attempt_generated_change_detection` creates a real `ChangeSet` when an agent run's terminal
exits. Nothing renders it.

## Why it is unblocked now, and what is actually left

Scoped 2026-08-25 (see RFC-020's own scoping addendum). Checked rather than assumed:

| Leg | State |
| --- | --- |
| A `ChangeSet` exists in production | **Yes** — `attempt_generated_change_detection`, wired 2026-08-18 |
| The GUI can read them | **Yes** — `ProjectSession::change_sets()` |
| A bounded projection | **Yes** — `ChangeSet::bounded_summary(limit)` → `ChangeSetSummary` |
| A route | **No** — `OpenDiffReview` is `Configurable` with no binding |
| A render arm | **No** |
| A visible control | **No** |

The last three are this slice's work. **The projection is better placed than the RFC's age
suggests**: `ChangeSetSummary` already carries `changed_file_count`, `shown_changed_files`,
`omitted_changed_file_count` and `detection_status`. You are rendering distinctions core already
makes, not inventing them.

## What the surface must not claim — from RFC-020's own text

- **A change set is not "all the changes."** Detection is metadata-only and conservative;
  `.git/`, `target/` and `node_modules/` are excluded by design, so a change an agent makes in a
  git hook is not reported. **State the limitation on the surface**, not only in documentation —
  the RFC requires this explicitly.
- **A truncated scan is not "nothing changed."** `omitted_changed_file_count` and
  `detection_status` exist to keep those distinct. Render them distinctly; collapsing them is the
  failure this project has now avoided in three separate surfaces.
- **Never present a change as safe.** The surface shows what was detected. It does not review,
  approve, or imply review.
- **File paths are untrusted.** They are filesystem-derived and attacker-influenceable, escaped
  like every other such value. The bidi fixture applies.
- **Do not become a second retention policy.** Render from what core holds; keep no copy.

## The control, not only a binding

`OpenDiffReview` needs a binding **and** a visible control. RFC-039's third reachability
principle — *naming a keystroke is not naming the path a user takes; name the control the user
sees* — applies here, and **RFC-040 closed on 2026-08-25 having established the pattern**, so
follow it rather than inventing one:

- A real `iced::widget::button` with `.on_press`, and a keyboard accelerator, **converging on the
  same function** — `open_folder_browser`'s "one setup, two routes" shape.
- Placed on the surface its action applies to (D2: per-surface, not a toolbar).
- Every new click message classified in `click_message_kind` as `BackgroundControl` or
  `ModalDecision`, and every new action given a `control_coverage` entry.

## What will fail, and should

Adding a live action trips five things by design. None is an obstacle; each is a decision point
someone must pass through deliberately, which is the whole reason they exist:

| What fails | Why, and what to do |
| --- | --- |
| `control_coverage`'s exhaustive match | Does not compile until `OpenDiffReview` is classified. Give it `VisibleControl` with a real `on_press_snippet` — which the coverage test then greps for in live source |
| `every_live_binding_is_described_to_the_user` | Asserts exactly **13**. It becomes 14. Update the count deliberately; do not loosen it to `>=` |
| `no_action_without_a_working_binding_is_advertised` | Names three dead actions explicitly. `OpenDiffReview` leaves that list — **RFC-036's dead-action count goes three to two**, worth stating in the evidence since RFC-036 is still open and tracking it |
| `click_message_kind`'s exhaustive match | Does not compile until the new button's message is classified |
| The keyboard-help catalog | A new live action with no `en.ftl` entry renders as its own catalog key, caught by the description check |

Prove the binding unclaimed **mechanically** against `KeybindingPolicy`, not by reading the list —
`open_project_entry_field_shortcut_is_a_candidate_that_collides_with_no_other_rule` is the shape.

Prove the binding unclaimed mechanically against `KeybindingPolicy`, not by reading the list.
Giving `OpenDiffReview` a real route takes RFC-036's dead-action count from three to two.

## Evidence

A cold start in which an agent run produces a real change set and a user reaches the surface and
sees it — **from a visible control**, not only a keystroke. If exercising a real agent run in
evidence is impractical, say so and state what you used instead rather than quietly substituting.

## What this slice is not

RFC-034's job — acting on a change set, transitioning review state — is a separate accepted RFC
blocked on this one. Render only. If you find yourself adding an approve button, stop.
