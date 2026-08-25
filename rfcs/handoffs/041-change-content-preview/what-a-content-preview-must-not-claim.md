---
title: "What a content preview must not claim"
rfc: "RFC-041"
rfc_file: "../../done/041-change-content-preview.md"
source_rfc_status: "Implemented and closed 2026-08-26 — RFC-041 is in rfcs/done/"
status: "Required reading before any RFC-041 code"
created: "2026-08-25"
---

# What a content preview must not claim

The security policy for reading this content was decided and implemented in RFC-024. What is
*not* settled, and is this slice's real risk, is what the screen tells a user it is looking at.

## 1. It is not a diff, and the word must not appear as though it were

For a modified file this shows **current content**. Not a comparison, not "what changed" — the
file as it stands now, which is also what it would look like if the agent had changed nothing.

`ProjectOpenSurface::DiffReview` and `OpenDiffReview` are existing internal names and stay.
**User-facing text is a different matter**: a heading, label or button that a reader parses as
"the diff" is a claim this product cannot support, and it would be a claim on a security-review
surface, which is where users are least able to afford one.

RFC-024 labelled it *"not a diff"* at the model layer. **Carry that to the screen**, in the same
breath as the content, not in a tooltip and not in documentation.

## 2. Absence of visible change is not absence of change

A user comparing "current content" against their memory may conclude nothing happened. For a
modified file this surface **cannot tell them whether the agent's edit is still there, was
reverted, or was overwritten by something else**. It shows a file, at a time, that some run
touched.

State that limit where it applies. It is the direct consequence of §1 and the one a user is most
likely to get wrong in a way that matters.

## 3. Content is read on demand and never retained — RFC-024 Decision 1, binding

> Never retained beyond the request. A diff is computed, rendered, and dropped. Content does not
> enter `ProjectSession` state, and it does not enter the audit store.

**Required:** content does not reach `ProjectSession`, any `Clone` state struct, or any audit
record. What this slice retains is `DetectedChanges` — paths and kinds, not content.

If rendering seems to need content in state, that is the design telling you something: render
from a value that lives for the request.

## 4. `DiffContent`'s `Debug` derive goes (D3)

Unredacted `Debug` on a type holding file content is one `dbg!`, one panic message or one
`tracing` field from putting a user's source into a log. Hand-implement `Debug` printing kind and
length, never bytes — `BoundedRuntimeSummary` and `DisplayText` are the shapes.

**The move-out gap stays open and documented at the type**: non-retention protects the wrapper,
not bytes a consumer moves out after a pattern match. Closing it means a lifetime-bound
`DiffContent`, which is larger than this slice. Document it where the next consumer reads it — a
comment on the definition is worth more than a sentence in a closed RFC nobody opens.

## 5. File content is untrusted, and it is not the grid

RFC-016's grid exception covers terminal output. **This is not the grid.** File content written by
an AI CLI is attacker-influenceable text rendered in trusted chrome, exactly like a path or a
project name.

`quote_untrusted` applies. Test with the bidi fixture — and note RFC-020's own correction: content
containing the literal text `<U+202E>` is **not** distinguishable from a real override, and cannot
be under this project's escaping design. The achievable, security-relevant half stands: a real
override always renders as a visible marker and never reaches a widget raw. Assert that; do not
re-assert the impossible half.

## 6. Refuse on a stale baseline, and say which (D2)

Rendering content for a change set whose baseline is no longer authoritative shows a file that may
have nothing to do with that run. A plausible wrong answer is worse than none on a surface whose
entire purpose is answering "what did the agent do".

Reuse `diff_content_is_stale`. Refuse, and name the reason — "this change set's baseline is no
longer authoritative" is actionable; "cannot show content" is not.
