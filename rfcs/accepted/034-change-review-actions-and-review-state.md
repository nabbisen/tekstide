# RFC-034: Change Review Actions and Review State

Status: **Accepted by the human owner 2026-08-18. UNBLOCKED 2026-08-25. AMENDED 2026-08-26** — see the Amendment at the end: one of the two values this RFC was accepted for does not exist, and D1/D2/D3 are decided.
Original status line: **Accepted by the human owner 2026-08-18. UNBLOCKED 2026-08-25** — RFC-020 shipped the change review surface, so a user can now see a change set. The original blocker read: *"Blocked on RFC-020: a user cannot act on a change set they cannot see."* Note what RFC-020 shipped is **metadata only**: file paths, counts, detection status, review state. Diff *content* is still not rendered, so an action taken here is taken on the same metadata the surface shows, not on inspected content — decide deliberately whether that is sufficient for the actions this RFC defines.
Target milestone: M12
Date: 2026-08-18

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`

Depends on:

- [RFC-012](../done/012-generated-change-review-foundations.md) — `ReviewState` and
  `transition_change_set_review_state`, both built, neither reachable.
- [RFC-020](../done/020-diff-review-and-agentrun-report.md) — **hard prerequisite.** A user cannot
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

---

## Amendment, 2026-08-26 — the premise, then the decisions

Written while scoping `0.15.0`. Checked against the code before scheduling, per this project's
own rule that a claim about behaviour cites what produced it.

### The premise that no longer holds

This RFC's own text says a review record's "value is the audit trail plus not being asked again."

**Neither half exists today.**

- **There is no audit trail for it.** `AuditEventFamily` has twelve frozen variants
  (`crates/tekstide-core/src/audit/record.rs`): `ProjectAdded`, `TrustChange`, `CommandApproval`,
  `ManagedProcessLifecycle`, `PlainTerminalObservation`, `PasteBlocked`, `RestrictedModeBlocked`,
  `RootAccessBlocked`, `SafeCloseDecision`, `SensitiveConfigChanged`, `TranscriptPurge`,
  `AuditStoreRecovery`. None is about a user's opinion of generated code. D2 said to check before
  assuming; checked, and nothing fits.
- **"Not being asked again" holds for one session only.** `ChangeSet` derives
  `Clone, Debug, Eq, PartialEq` and **not** `Serialize`
  (`crates/tekstide-core/src/domain/changeset.rs`). `ProjectSession::change_sets` is a
  `Vec<ChangeSet>` in memory and nothing writes it anywhere — the only things this application
  persists are `recent-projects.json` and the audit store. **Close Tekstide and every change set
  is gone, review decision included.**

So RFC-034 as accepted would ship a control that records an opinion which evaporates on exit,
with no durable trace anywhere. That is a control implying more than it does — the exact risk
this RFC names, one level deeper than it anticipated.

**Not a reason to drop it.** A reason to decide, deliberately, which product it is.

### D0 (new) — where does a review decision live?

- **(a) Session-scoped, and the surface says so.** Cheapest and fully honest. Helps a user work
  through one sitting's changes and claims nothing past it.
- **(b) A thirteenth audit family.** Makes the audit store the durable record. An RFC-013 schema
  change, with a migration question and a frozen-families argument to win.
- **(c) Persist change sets.** Largest. A change set carries **file paths** from a real project,
  so this is a local-data-policy question (retention, purge, what a user can delete) before it is
  a storage question.

**Decided: (a) — session-scoped, stated on the surface.** In order:

1. **(c) is a data-policy decision wearing a storage costume.** This project has twice been
   corrected for storing something whose retention nobody had decided. Persisting real project
   paths as a side effect of adding a button would be the third.
2. **(b) may well be right, and is not this slice's to take.** A thirteenth family changes what
   the audit store is *for*: today every family records a security boundary crossed, refused or
   authorised, and none records a preference. That argument deserves its own RFC.
3. **(a) is honest at the size it is.** A session-scoped decision that says it is session-scoped
   claims exactly what it delivers.

**(b) is recorded as the successor question, in these words:** *should the audit store record a
user's decision about generated code?* Whoever takes RFC-030 or the next audit slice should find
that already written down rather than rediscover it.

### D1 — which transitions are offered? **`Accepted` and `Rejected` only.**

`can_transition_review_state` permits, from `Unreviewed`: `Accepted`, `PartiallyAccepted`,
`Rejected`, `Superseded`; from `PartiallyAccepted`: `Accepted`, `Rejected`, `Superseded`; from
`Accepted` or `Rejected`: `Superseded`.

Offer **`Accepted`** and **`Rejected`**, from `Unreviewed` and from `PartiallyAccepted`. Not the
other two, for two different reasons:

- **`PartiallyAccepted` has no way to be true.** It means some files accepted and some not. There
  is no per-file decision model, so a whole-change-set button setting it would record a state
  nothing in the product can express or act on. It becomes offerable when per-file review exists.
- **`Superseded` is not an opinion.** It is a fact about a later change set replacing this one. A
  user asserting it by button press asserts something they are not the source of truth for.

**The general line this draws: a control may record an opinion; it may not assert a fact.**
Carry it into `ARCHITECTURE.md` at closeout — it is the same failure shape as a field named
`fully_confirmed` that could not confirm, arriving from the other direction.

### D2 — is a review decision audited? **No, this slice.**

Answered by D0. No family fits, and inventing one is (b). The closeout must state plainly that a
review decision produces **no** audit record, so nobody later reads the audit store's silence as
evidence that no decision was made.

### D3 — a change set whose files changed again since detection? **Say so; still allow the decision.**

Reuse `diff_content_is_stale`, which already backs RFC-041's content refusal. Do not invent a
second staleness notion.

But **refuse the content and still allow the decision**, deliberately the opposite of what the
content preview does:

- RFC-041 refuses *content* when stale, because bytes from a moment that has passed would
  misrepresent what the user is looking at.
- A *decision* is about the change set as detected — a historical fact staleness does not
  invalidate. Blocking it would leave a change set permanently undecidable because an unrelated
  file moved.

The decision controls stay live; the surface says the tree has moved since detection, in its own
words, distinct from the content refusal's. **Two different facts, two different sentences** —
the rule that split the two omission counts in `0.14.0`.

### Scope, as amended

Scope item 3 ("an audit event for the decision, if D2 below says so") is **out** — D2 says no.
Item 2 grows: the surface must say both that the decision is session-scoped and, when true, that
the tree has moved since detection.

### Dependency added

**RFC-042 (Change Content Legibility) lands first.** Content is inspectable as of `0.14.0` and
not readable — a real source file previews as one escaped line. An approval control over a review
surface a user cannot read is a control implying more than it delivers, and would be the fourth
of that shape in this project.

### D4 (added 2026-08-26, while writing the handoff pack) — a decision is final. Say so **before** the click.

Found by reading `can_transition_review_state` again with D1's answer in hand. Once a change set
is `Accepted`, the only legal transition is `Superseded`. Once it is `Rejected`, likewise.
**`Accepted → Rejected` is not legal, and neither is the reverse.**

So the two controls D1 offers are **one-way**, and nothing in D1 said so.

Three ways that could go, and only one is honest:

- **Offer the buttons anyway and refuse on click.** A control that will always refuse is a control
  that implies an action it does not have. This project has corrected that shape four times.
- **Withdraw the buttons after a decision, silently.** The user clicks Accept, the controls
  vanish, and nothing ever said the click was irreversible. They learn the rule by losing to it.
- **Say it before the click, and withdraw after.** ← **decided.**

**The surface states that a review decision cannot be changed once recorded, while the controls
are still live** — not in a confirmation modal after the fact, and not only as an explanation of
why the buttons are gone. After a decision, the controls are no longer offered and the review
state line carries what was decided.

**This compounds with D0.** A decision is *final* and *session-scoped*: it cannot be changed, and
it does not survive closing Tekstide. Both are true, both are surprising, and a user told only one
of them will infer the other wrongly in whichever direction is worse. **Both sentences ship or
neither control does.**
