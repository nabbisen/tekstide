# RFC-034: Change Review Actions and Review State

Status: **Proposed — awaiting the human owner's acceptance.** Authored 2026-08-18.
Target milestone: M12
Date: 2026-08-18

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`

Depends on:

- [RFC-012](../done/012-generated-change-review-foundations.md) — `ReviewState` and
  `transition_change_set_review_state`, both built, neither reachable.
- [RFC-020](./020-diff-review-and-agentrun-report.md) — **hard prerequisite.** A user cannot
  act on a change set they cannot see. RFC-020's own answer to its Q3 is "read-only," and
  this RFC is the follow-on that question deferred.

## Summary

Let a user record a decision about a detected change set, and decide what that decision means.

## Why this is scheduled

As of `0.11.0` a real `ChangeSet` exists in production for the first time, carrying a
`review_state`. Nothing can change it. `transition_change_set_review_state` has been on the
reachability audit's orphan list since the audit ran, and the reason it stayed there was
honest: there was nothing to review.

Now there is. A change set is created, sits at its initial `ReviewState`, and stays there
forever.

## The question that makes this an RFC rather than a wiring slice

**What does "accepted" mean?**

RFC-012's foundations are detection-only. A review state is *metadata about the user's
opinion*, not an operation on the working tree. So there are two coherent products here and
they are not the same:

- **A record.** Marking a change set reviewed/accepted/rejected records what the user decided
  and nothing else. Cheap, honest, and its value is the audit trail plus not being asked
  again.
- **An operation.** Rejecting a change set reverts it. That requires a before-side, which
  **does not exist** — filesystem-snapshot detection captured metadata only, and RFC-020's
  own scope correction records that the before-bytes are gone by preview time. Reverting
  would need Git-backed detection, which is RFC-030 and is gated behind RFC-012's unmet
  safety evidence.

**Recommend the record, explicitly, and say so on the surface.** A "Reject" button that does
not undo anything is a trap unless it is labelled as a decision rather than an action. This is
the same class as RFC-020's no-two-sided-diff problem: a control whose name implies more than
it does.

## Scope

1. A route from RFC-020's change review surface to `transition_change_set_review_state`.
2. Surface wording that distinguishes recording a decision from performing one.
3. An audit event for the decision, if D2 below says so.

## Non-goals

- **Reverting, staging, or applying changes.** No before-side exists. Anything of this shape
  waits for RFC-030.
- Changing `ReviewState`'s variants. RFC-012 froze them; if they turn out wrong, that is an
  RFC-012 amendment and should be reviewed as one.

## Decisions required

**D1 — which transitions are offered?** RFC-012 defines the state machine; not every legal
transition needs a button. Recommend the minimum that closes the loop and no more.

**D2 — is a review decision audited?** It is a user decision about generated code, which is
the kind of thing an audit trail exists for. But RFC-013's frozen families do not obviously
include one, and inventing a family is a schema change. **Check before assuming**, and if
none fits, say so rather than forcing a decision into a family that means something else.

**D3 — what happens to a change set whose files changed again since detection?** The recorded
decision is about a state of the tree that no longer holds. Recommend the surface says the
change set is stale rather than silently accepting a decision about a vanished state — the
same reasoning as `theme-contrast-verification`'s "truncated is not the same fact as clean."

## Risks

- **A control that implies an operation.** Mitigated by D-level wording review, and by making
  the claim falsifiable in the closeout: *rejecting a change set does not modify any file*, a
  statement a test can hold.
- **Reviewing a stale change set.** D3.
