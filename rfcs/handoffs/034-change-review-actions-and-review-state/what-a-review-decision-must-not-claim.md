---
title: "What a review decision must not claim"
rfc: "RFC-034"
rfc_file: "../../done/034-change-review-actions-and-review-state.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-034 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-26"
---

# What a review decision must not claim

**Required reading before writing code.** This slice adds two buttons and calls one function that
already exists. Almost none of the work is code. **All of the risk is in what the buttons appear
to mean**, and this surface has less margin for that than any other in the product, because it is
the one a user opens specifically to decide whether to trust an AI agent.

## §1 The first claim: that something happened

A button labelled **Reject** on a screen showing files an agent changed reads as *undo*.

It is not. Nothing is reverted, nothing is staged, no file is touched. A rejected change set is a
change set with a different value in a field. The agent's edits are still on disk, exactly as they
were one moment before the click.

There is no before-side to revert to — filesystem-snapshot detection captured metadata, and
RFC-041 records that the before-bytes are gone by preview time. Reverting needs Git-backed
detection, which is RFC-030 and does not exist.

**So the words must carry it.** Not a tooltip, not the changelog, not a doc page: the surface, at
the point of the click. A user who clicks Reject and believes the change was undone will act on
that belief — leave the file in place, ship it, stop looking.

**The falsifiable form, which the closeout must be able to state:** *rejecting a change set does
not modify any file.* A test can hold that. Write it.

## §2 The second claim: that the record persists

This is newer than the RFC and it is the one that would be missed.

`ChangeSet` derives no `Serialize`. `ProjectSession::change_sets` is a `Vec` in memory. **Closing
Tekstide discards every change set and every decision about one.** Not archived, not degraded —
gone, along with the change sets themselves.

A user who marks twelve change sets reviewed across an afternoon, closes the application, and
reopens it, finds no change sets at all. If the surface never said so, they will reasonably
conclude they did something wrong, or worse, will not check.

RFC-034 was accepted on the premise that the value here is *"the audit trail plus not being asked
again."* **Neither exists.** D2 says no audit record. D0 says the decision lives one session. What
remains is real but small: working through one sitting's changes without losing your place. That
is worth building, and it is worth saying at that size.

## §3 The third claim: that the click can be taken back

`can_transition_review_state` permits `Accepted → Superseded` and `Rejected → Superseded`, and
nothing else out of either. **`Accepted → Rejected` is illegal. So is the reverse.**

The controls are one-way. A user who clicks Accept to see what happens has permanently spent that
change set's decision for the session.

D4 decides how that is handled: **say it before the click**, while the controls are live.
Withdrawing them afterwards and letting the user infer the rule from their absence teaches it by
loss.

## §4 The real design work: three more sentences on a surface that has 28

`en.ftl` already carries **28** `change-review-*` strings. Detection disclosure, detection status,
two distinct omission counts, review state, and six separate content refusals — each one correct,
each one added because a reviewer proved the surface would otherwise overclaim.

You are about to add: a finality sentence, a session-scope sentence, a stale-tree sentence, and
two button labels.

**A surface where every line is a caveat is a surface where no line is read.** Being technically
honest and practically ignored is not the same as being honest, and this project has never had to
confront that trade because it has never had this many disclosures in one place.

**This is the design work of the slice.** Not a formatting preference — the difference between a
user who knows their decision is session-scoped and one who was told in a sentence they had
already stopped reading.

Some things that are legitimate answers, none of which this document mandates:

- Attaching a claim to the control it qualifies rather than adding a line to a stack of lines.
- Saying two facts in one sentence where they are genuinely one fact (final **and** session-scoped
  is arguably one fact: *this is a note to yourself for now*).
- Reviewing whether any existing 28 have earned their place, which is in scope precisely because
  you are the one making the surface worse.

**What is not a legitimate answer** is adding three more lines to the bottom of `pinned_middle`
and calling the requirement met because the words are present.

## §5 What you may not do

- **Do not offer `PartiallyAccepted` or `Superseded`.** D1. The first cannot be true without
  per-file review; the second is a fact about the world, not an opinion, and a user pressing a
  button is not its source of truth.
- **Do not block the decision when the tree has moved.** D3. Refusing would leave a change set
  permanently undecidable because an unrelated file changed. Say the tree moved; let them decide.
- **Do not reuse the content-stale wording for the tree-moved case.** Two different facts, two
  different sentences — the rule that split the two omission counts in `0.14.0`. The content
  refusal means *these bytes are from a moment that has passed*; the decision notice means *what
  you are deciding about is older than what is on disk*.
- **Do not invent an audit family.** D2. If it seems obviously right, that is the argument for its
  own RFC, and the successor question is already written down.
- **Do not answer §1 or §3 with a confirmation modal.** A modal after the click is not a claim
  before it, and this project's modal-exclusivity machinery is already carrying seventeen
  hand-written guards.

## §6 If the honest answer is "this is not worth shipping"

It is a legitimate finding, and this document would rather receive it than a shipped control
nobody should trust.

If, having written the sentences §1–§3 require, the feature reads as *a note to yourself that
vanishes when you close the window, about files nothing will change, which you cannot take back* —
then say so, in writing, and the right response may be to hold RFC-034 until D0's successor
question is answered and a decision can outlive the session.

**That would not be a failure of this slice.** It would be the slice discovering that its value
was carried by a premise the RFC was accepted on and the code does not support. Better found here
than by a user.
