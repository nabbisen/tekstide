# RFC-037: Adopt the 5-Folder RFC Lifecycle

Status: **Implemented and closed 2026-08-19.** Accepted by the human owner and migrated in the
same commit, per RFC-000 §Self-application's own recommended pattern. `rfcs/accepted/` exists and
holds the five RFCs that were accepted but unfinished; `rfcs/proposed/` is empty, which is the
correct state when nothing is under review. This RFC lives in `done/` for the same reason RFC-000
does: the policy it defines is now in effect.
Target milestone: M12
Date: 2026-08-19

Related baseline documents:

- none — this is a process RFC.

Depends on:

- [RFC-000](../done/000-rfc-lifecycle-policy.md) — the policy this refines. RFC-000 §Open
  questions names the mechanism: *"Future refinements … will, if needed, land as follow-up RFCs
  referencing this one."* **This is that follow-up.** RFC-000 is not edited; it is closed, it
  correctly describes both variants, and it is not made wrong by a project moving between them.

## Summary

Add `rfcs/accepted/`, and move every RFC the owner has accepted but no one has finished
implementing into it.

## Why now, rather than as a tidy-up whenever

RFC-000 gives the criterion for adopting this variant, and a warning against adopting it
prematurely. **Both now point the same way.**

> Use this variant if "the maintainer signed off" is a meaningful event distinct from "the
> implementer finished." Skip it otherwise — `accepted/` will sit empty in projects where the
> two events collapse, and an empty folder is a maintenance burden with no payoff.

**The two events are emphatically distinct here.** Acceptance is an explicit act by the human
owner, recorded per RFC ("The six are accepted", 2026-08-18), and it is routinely days or weeks
ahead of implementation. RFC-034 and RFC-036 are accepted and unstarted. RFC-035 is accepted and
unstarted. RFC-023 was accepted on the 18th and is four slices in.

**And the warning is inverted.** `accepted/` would not sit empty — it would hold **five** files
on day one. What would sit empty is `proposed/`.

### The stronger argument: the folder is currently lying

RFC-000 §Folder layout:

> **The folder is the source of truth for the state.** A file's location is what determines its
> state, not the Status field inside the file.

Every file in `rfcs/proposed/` today has a Status of **Accepted**:

| RFC | Status inside the file | Folder says |
| --- | --- | --- |
| 020 | Accepted 2026-08-12, partially implemented | Proposed |
| 023 | Accepted 2026-08-18, four slices in | Proposed |
| 034 | Accepted 2026-08-18 | Proposed |
| 035 | Accepted 2026-08-18 | Proposed |
| 036 | Accepted 2026-08-18 | Proposed |

**Zero RFCs in `proposed/` are proposed.** By RFC-000's own rule the folder wins, so the
repository is currently telling every reader that five accepted RFCs are still open for review.
That is not a cosmetic misfit — it is the policy's central invariant being violated, in the
direction that most misleads someone deciding what to pick up.

## What changes

```
rfcs/
  proposed/    ← under review              (empty on landing, and that is correct)
  accepted/    ← review complete; implementer may start   (020, 023, 034, 035, 036)
  done/        ← shipped
  archive/     ← withdrawn or superseded
```

No `draft/`. RFC-000 says to add it only when multiple authors regularly need a shared place for
drafts, and this project has one author of RFCs.

`handoffs/` is unchanged and does not move — RFC-000 is explicit that handoff status is inherited
from the RFC's folder, never managed separately.

## Where a partially-implemented RFC goes

**`accepted/`, not `done/`.** RFC-020 has one slice shipped and two outstanding; RFC-023 has four
slices in and two to go. `done/` means *shipped*, and neither is. This is not a gap in the
variant — it is the variant working: under the 4-folder layout there was nowhere honest to put
them, which is part of why `proposed/` filled with non-proposals.

## Scope

1. Create `rfcs/accepted/`.
2. `git mv` the five accepted RFCs into it.
3. Repoint every reference — **30 occurrences of `rfcs/proposed/NNN` outside `.git-exclude/`**,
   including the `rfc_file:` front matter in every handoff pack whose RFC moves.
4. `rfcs/README.md`: an Accepted table between Proposed and Implemented; Proposed retained and
   empty, with a line saying an empty Proposed folder is the expected state when nothing is under
   review.
5. `rfcs/delivery-plan.md`'s "How to pick up work" step 2 currently says to read the RFC in
   `rfcs/proposed/` or `rfcs/done/`. It gains `accepted/` — and `accepted/` becomes the *first*
   place to look, since it is by definition where startable work lives.

**Atomically, in one commit.** RFC-000 §Self-application records that its own adoption combined
the policy and the migration in a single change and calls that the recommended pattern. This
follows it.

## Non-goals

- **Editing RFC-000.** It is closed, it is correct, and this project does not edit closed
  documents to match a later state. It documents the variant it was written for; this RFC records
  the project's move to the other one.
- A `draft/` folder.
- Any change to what acceptance *means*, or to who grants it.
- Renumbering anything. RFC-000 §Anti-patterns names renumbering during reorganisation
  explicitly.

## Risks

- **A missed reference.** Mitigated mechanically: `grep -rn "proposed/0"` must return only
  `.git-exclude/` hits and this RFC's own prose after the move, and that check belongs in the
  commit's own evidence rather than being run once and forgotten.
- **A review request in flight pointing at an old path.** Mitigated by timing — see below.
- **`proposed/` sitting empty reads as a mistake.** Mitigated by saying so in `rfcs/README.md`:
  an empty `proposed/` means nothing is awaiting review, which is a *good* state, not a missing
  folder.

## Timing

**Now is unusually clean, and the window is not permanent.** RFC-033 closed today; RFC-023's
PR-023-E has not started; no review request is open. Every additional slice landed before the
migration is another handoff front matter and another cross-reference to repoint, and every day
`proposed/` misdescribes five RFCs is a day someone could pick up work believing it still needs
review.

The one thing that would argue for waiting is a large in-flight change touching `rfcs/` — there
is none.

## Open questions

1. Should `proposed/` keep a `.gitkeep`, or is an absent-until-needed folder acceptable? Git does
   not track empty directories, so without one the folder disappears on clone — which would make
   the "empty is correct" note in the README describe something a reader cannot see. Recommend a
   `.gitkeep` with a one-line comment.
