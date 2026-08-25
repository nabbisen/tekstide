# RFC-041: Change Content Preview

Status: **Implemented and closed 2026-08-26.** Proposed and accepted 2026-08-25, scoped at the
human owner's request as the second theme for `0.14.0`, after RFC-035. Shipped in `de07dbd`;
accepted at review 330.
Target milestone: **M12** — accepted by the human owner 2026-08-25, third of three for `0.14.0`,
after RFC-035 and the safe-close honesty fix.

Its three open questions were **decided by the architect on acceptance**; an implementer must not
inherit an unresolved architecture decision.
Date: 2026-08-25

Related RFCs:

- [RFC-024](../done/024-diff-preview-policy.md) — **decided all of this in 2026-08-11 and built
  it.** This RFC does not re-decide any of it; it makes it reachable.
- [RFC-020](../done/020-diff-review-and-agentrun-report.md) — shipped the change review surface
  that renders metadata. This adds content to it.
- [RFC-012](../done/012-generated-change-review-foundations.md) — owns detection and the
  `ReviewBaseline` whose shape makes a two-sided diff impossible today.
- **RFC-030** — Git Integration, reserved in `delivery-plan.md` and unauthored. The only
  designed source of before-content, and therefore the only route to a real diff.

## Summary

**This is not a design RFC. Almost everything it needs was designed, reviewed and shipped fifteen
days ago, and has never been called.**

`0.13.0` shipped a change review surface that shows *which files* an agent run touched. It cannot
show *what it did to them*. The reason is not a missing model:

- `read_diff_content` exists in `project/diff.rs`, with per-change-kind delivery, bounds,
  non-text classification and staleness detection.
- `gate_diff_content_read` exists in the same module, refusing above a measured 4 MiB-per-side bound and
  classifying binary content **before any read**, with the ordering ablated rather than asserted.
- Both have **zero production callers.** Built in `0.7.0`, reviewed across four slices, unreached
  ever since.

The single missing link is that `add_detected_generated_change_set` **discards the
`DetectedChanges`** that `read_diff_content` requires as its first argument. RFC-020's own
scoping addendum names this exactly: reading content is *"blocked on retaining `DetectedChanges`
past `add_detected_generated_change_set`, which currently discards it."*

So this RFC is: retain what is discarded, and render what is already gated. It is the fifth
instance in three months of *an unwired capability hiding the work that would have exercised it*.

## What this delivers, named honestly

Per RFC-024's own per-change-kind decision, unchanged:

| Change kind | What a user sees |
| --- | --- |
| **Added** | The file's whole content |
| **Modified** | The file's **current** content, **explicitly labelled not a diff** |
| **Deleted** | The fact of deletion |

**It does not deliver a diff for a modified file, and must not be described as one.** RFC-024
found this before building, and the reason is structural rather than incidental:
`ReviewBaselineEntry` holds `relative_path`, `kind`, `len`, `modified_unix_nanos` — **no content
and no hash** — by RFC-012's design principle that summaries must not include file contents. For
a modified file the before-bytes were never captured and are overwritten by request time. They
are *gone, not merely unretained*.

A two-sided diff needs a before-source, and the only one this project has designed is Git-backed
detection, which holds blob history and is gated behind RFC-012's unmet safety evidence. **The
two-sided case is not cancelled; it is blocked on RFC-030**, and this RFC must not quietly
present current-content-preview as though it were the thing users mean by "diff".

That naming is the single largest risk here. A surface that shows current content under a heading
a user reads as "the diff" is a durable claim more than it knows — the failure class this project
has corrected in a privacy claim, a blocked-feature count, an audit field and an affordance audit.

## The privacy question is already answered

I asked the human owner whether rendering content means Tekstide starts retaining file content.
**RFC-024 Decision 1 answered that on 2026-08-11 and I should have read it before asking:**

> **Never retained beyond the request.** A diff is computed, rendered, and dropped. Content does
> not enter `ProjectSession` state, and it does not enter the audit store — RFC-013's families
> have no field for it, and this policy must not become the reason one is added.

That constraint is binding on this RFC and is not reopened. What this RFC retains is
`DetectedChanges` — **paths and change kinds, not content** — which `ChangeSet.changed_files`
already persists in substance. The retention delta is metadata this project already keeps.

## Goals

1. `read_diff_content` and `gate_diff_content_read` have production callers, reached from the
   change review surface a user can already open.
2. Content is delivered per change kind, labelled for what it is, with "not a diff" stated on the
   surface for the modified case rather than in documentation.
3. RFC-024's bounds, refusals and staleness checks are **reused, not re-derived** — a second
   gating path is a second policy.

## Non-goals

- Re-deciding anything in RFC-024: bounds, refusal-not-truncation, non-text classification,
  baseline authority, the escaping position. All settled and implemented.
- A two-sided diff. Blocked on RFC-030; say so, do not approximate it.
- Retaining content. Decision 1 forbids it.
- Acting on a change. RFC-034's job, and it should follow this rather than precede it — approving
  a change you cannot inspect is the ordering this RFC exists to fix.

## Decisions (were open questions; settled on acceptance)

**D1 — retain `DetectedChanges` beside the `ChangeSet`, keyed by `ChangeSetId`, session-scoped.**

Not as a field on the persisted `ChangeSet`. `ChangeSet` is a domain type that outlives the
session and is designed to hold metadata; `DetectedChanges` is a detector output whose usefulness
expires with its baseline's authority. Fusing them makes the second look as durable as the first.

A session-scoped map keyed by `ChangeSetId` makes the lifetime honest and makes the expiry
expressible: **the retained value may be dropped without the `ChangeSet` becoming wrong**, which
is exactly the relationship. Dropping it means content preview stops being offered for that change
set; the metadata the surface already shows is unaffected.

**D2 — a stale baseline refuses, and says which.**

`diff_content_is_stale` exists and RFC-024 deliberately did not reuse `ExternalChangeDecision`'s
variants, because its `Conflict` state cannot arise in a read-only flow. Reuse RFC-024's, not a
third shape.

Refuse rather than render-with-a-warning. A file's current content shown under a change set whose
baseline has expired is content that may have nothing to do with that run — and the whole point of
this surface is answering "what did the agent do", where a plausible wrong answer is worse than
none. RFC-019 PR-019-E already found this exact class once: a status derived from a source that
had stopped being authoritative.

**Say which**: "this change set's baseline is no longer authoritative" is actionable; "cannot show
content" is not.

**D3 — `DiffContent` loses its `Debug` derive, and the move-out gap is documented rather than
closed.**

RFC-024 recorded both as harmless while nothing called it. This RFC is what makes them reachable.

The `Debug` derive goes: an unredacted `Debug` on a type holding file content is one `dbg!`,
one panic message, or one `tracing` field away from putting a user's source into a log. Implement
`Debug` by hand printing kind and length, never bytes — the shape `BoundedRuntimeSummary` and
`DisplayText` already establish.

The move-out gap — non-retention protects the wrapper but not bytes a consumer moves out after a
pattern match — **stays open and is documented at the type**, not closed. Closing it properly
means a lifetime-bound `DiffContent`, which RFC-024 raised and is a larger change than this RFC
should smuggle. Documenting it at the definition, where the next consumer reads it, is the
proportionate answer; a comment on the type is worth more than a sentence in a closed RFC nobody
opens.

## Open questions

None. The three above were the open questions.
