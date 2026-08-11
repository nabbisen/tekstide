# RFC-020: Diff Review and AgentRun Report Surfaces

Status: Proposed — awaiting the human owner's acceptance
Target milestone: M10 (`0.6.x`), second half
Date: 2026-08-11

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md) §M10

Depends on:

- [RFC-011](../done/011-transcript-retention-and-local-data-policy.md) — transcript capture, retention, and purge policy.
- [RFC-012](../done/012-generated-change-review-foundations.md) — the generated-change detection model.
- [RFC-015](../done/015-application-shell-and-rendered-surface-model.md) — the surface contract.
- [RFC-016](../done/016-internationalization-and-localization.md) — text safety, and the two exceptions this RFC does **not** inherit.
- [RFC-019](../done/019-editor-and-explorer-surfaces.md) — the escaping asymmetry this RFC extends with a third position.

## Summary

Render two report surfaces: what an AgentRun changed, and what it said.

## This RFC is *not* the same shape as the last three, and that is the most important thing in it

RFC-017, RFC-018 and RFC-019 were each "the model is complete in `tekstide-core`, nothing calls it — write call sites and rendering." **RFC-020 is not.** I checked before writing, and two pieces the surfaces need do not exist:

**1. There is no diff content model.** RFC-012 gives `GeneratedChangeDetector`, `ReviewBaseline`, `DetectedChanges`, `DetectedChangedPath` and `ChangePathKind` — all of which answer *which paths changed*. `future-work.md` describes this accurately as "conservative metadata-only association foundations." **Nothing produces before/after content or hunks.** A diff review surface cannot render a diff that no model computes.

**2. There is no transcript reader.** `transcript/` has `path.rs`, `policy.rs` and `writer.rs`. There is no reader, no bounded replay, no model of a transcript as a thing that can be displayed. Transcripts are written and never read back.

**I under-estimated the terminal-launch-UX slice once by assuming reviewed code implied reachable code, and said so.** This is the same trap one level up: an RFC whose dependencies are "implemented with documented limitations" is not necessarily an RFC that only has to render them. **RFC-020 requires new `tekstide-core` model work**, and any plan that treats it as a rendering exercise is mis-sized from the start.

## What this means for scope

Two honest options. **I recommend the second.**

**Option A — one RFC covering model and rendering.** Larger, and it mixes two kinds of review: a model decision (what is a diff, how bounded) and a rendering decision (how it looks and escapes). This project's experience is that mixed slices get reviewed less well than separated ones.

**Option B — sequence the model work first, as amendments to the RFCs that own it.** RFC-012 owns the change model; a diff-content amendment belongs there. RFC-011 owns transcripts; a bounded-reader amendment belongs there. RFC-020 then becomes what it should be — a rendering RFC over models that exist — and inherits the same shape as its three predecessors.

Option B also puts each decision in front of the RFC that already reasoned about the surrounding constraints: RFC-011 already decided retention bounds and purge scopes, and a reader that ignores those would be a second policy.

**Recommended: Option B**, with RFC-020 blocked on the prerequisite model work and this document standing as its design in the meantime.

### Correction, 2026-08-11 — the two prerequisites are not the same size

The owner asked whether Option B preserves security and extensibility, noting that stable performance and data integrity in production matter more than minimalism. That pushback found a real flaw in the framing above, which called both prerequisites "amendments" and so invited the shape of RFC-006 Amendment 1 — one accessor for state that already existed. That fits one of them and badly misfits the other.

**The transcript reader is genuinely amendment-shaped.** RFC-011 has already decided capture mode, retention limits, budget scope and purge. A bounded reader is "read back what policy already governs," and the constraints to review it against already exist.

**The diff content model is not.** It is new state and new I/O, and an accessor-sized amendment would skip exactly the decisions that matter in production:

- **Baseline invalidation (integrity).** RFC-012 computes against a captured `ReviewBaseline`. Nothing yet decides when that baseline goes stale, and a diff rendered against a stale baseline shows changes that are not there. This is the same class as the defect RFC-019 PR-019-E found, where a blocked save reported a conflict for a document with no local edits — a status derived from a source that had stopped being authoritative.
- **Binary content (integrity).** `ChangePathKind` exists, but nothing decides what a diff *is* for a non-text change. Preventing an attempt to diff a binary is the model's job; a renderer discovering it is a renderer that has already read the file.
- **Bounding (performance).** RFC-019 bounds editable files at 4 MiB. A diff holds two versions plus the computed difference. Whether that is bounded, streamed, or refused above a threshold is a model decision with a memory profile attached.

So the corrected sequencing is: **the transcript reader as an RFC-011 amendment; the diff content model as design work reviewed on its own terms** — a new RFC or a substantial RFC-012 amendment reviewed as a design rather than as an addition.

**The general principle, worth keeping past this RFC**: minimalism belongs in surfaces, which can grow. A model chosen minimally to satisfy one caller has the wrong shape for the second, and replacing it takes everything built on it along.

### The form is settled by RFC-012's own text, not by judgement

Approved by the owner 2026-08-11. I was going to argue the diff model is "too large for an amendment"; checking RFC-012 first produced a better answer, because **RFC-012 already anticipated this and named what would authorise it** (§Detection scope):

> The detector must not read or store file contents **unless a later reviewed diff preview policy allows it.** Path-only and metadata-only detection is sufficient for RFC-012.

So this is not an amendment question at all. RFC-012 deliberately deferred content reading to a *separate reviewed policy*, and an amendment to RFC-012 would be that RFC authorising itself to do the thing it declined to authorise.

**Therefore: a new RFC — the diff preview policy RFC-012 named.** It owns:

- when file content may be read at all, and by what;
- baseline invalidation — when a `ReviewBaseline` stops being authoritative;
- what a diff is for a non-text change;
- the memory bound: two versions plus a computed difference, bounded, streamed, or refused.

`022` is unused but reserved by convention for Security Dialogs throughout the delivery plan, so the next free number is **024**.

**Prerequisite order for RFC-020**, now settled:

1. **RFC-024** — diff preview policy (new RFC, the one RFC-012 named).
2. **RFC-011 Amendment** — a bounded transcript reader, reviewed against RFC-011's existing retention and purge constraints.
3. **RFC-020** — the two rendering surfaces, unblocked once both land.

## Scope correction, 2026-08-11 — the change surface may not promise a diff

RFC-024's own §Correction records that a **two-sided diff for a modified file is
unavailable** under filesystem-snapshot detection: `ReviewBaselineEntry` captures metadata
only, by RFC-012's stated principle, so the before-bytes were never stored and the run has
overwritten them by preview time.

So this surface renders, per change kind: full content for an added file, the fact of
deletion for a removed one, and **current content only — explicitly not a diff — for a
modified one**. That last case is the common one, and **the surface must say so where the
user reads it**, not only in a closeout. A user shown current content under a heading that
implies a diff will believe they have seen what changed.

A two-sided diff needs a before-source, and the only one designed is Git-backed detection,
gated behind RFC-012's unmet safety evidence. This RFC must not imply otherwise, and
PR-020-B's gate carries it.

## The security core — a third position in the escaping asymmetry

RFC-019 established two positions. This RFC adds the third, and the reasoning is what makes it defensible rather than arbitrary.

| Surface | Treatment | Why |
| --- | --- | --- |
| Terminal grid | raw | Escaping would corrupt it — control sequences *are* the rendering |
| Editor text area | raw | The user is editing these bytes; an editor that rewrites what it shows is broken |
| Chrome everywhere | escaped | Tekstide describing something, not the user's content |
| **Diff review** | **escaped** | **New** — see below |
| **AgentRun transcript** | **escaped** | **New** — see below |

**Both new surfaces escape, and neither inherits an existing exception.**

The editor exception is justified by *editing*: you must see bytes as they are because you are about to change them and save them. **A diff is reviewed, not edited.** The justification does not transfer.

The grid exception is justified by *corruption*: escaping terminal output would destroy the thing being rendered, because the escape sequences drive the grid. **A transcript report is not a grid.** That justification does not transfer either.

And for diff review specifically, **escaping is the stronger position, not a compromise**. A reviewer deciding whether to accept an AI-generated change *wants* to see that the change introduces `U+202E` — that is precisely the Trojan Source case, and it is why other review tools warn on bidi controls rather than rendering them faithfully. A diff that renders an override invisibly is a diff that hides the most dangerous thing it could contain.

**State this in the closeout as a claim that could be false**: a bidi override introduced by a generated change is visible in the diff surface. That is checkable.

## What the surfaces render, and what they must not claim

**AgentRun output is untrusted.** It is text produced by a third-party AI CLI, which may be quoting a file, an error, or an attacker-influenced input. It is rendered as data, never as chrome, and never as a basis for a decision the user did not make.

**Neither surface may present a change as safe.** RFC-012's detection is metadata-only and conservative; a change surface that implies "these are all the changes" would overclaim what detection can see. The closeout must state what detection does not cover.

**Transcript rendering inherits RFC-011's bounds.** Retention limits, capture mode, and purge scope are already decided. A reader that renders more than the policy retains, or that keeps its own copy, is a second retention policy.

## Slices — provisional, pending the Option A/B decision

Under Option B, with the amendments landed first:

- **PR-020-A** — design and handoff acceptance.
- **PR-020-B** — the change review surface. Renders detected changes and their diffs, escaped, with the metadata-only limitation stated on the surface rather than only in documentation.
- **PR-020-C** — the AgentRun report surface. Renders transcript content within RFC-011's bounds, escaped.
- **PR-020-D** — closeout, with the claim statement checked against this RFC's own text.

## Risks

- **Mis-sized as a rendering RFC.** The whole point of the section above. Mitigated by the Option A/B decision being made before implementation starts.
- **A diff surface that hides what it should reveal.** Mitigated by escaping, and by making the bidi-visibility claim falsifiable in the closeout.
- **A transcript reader that becomes a second retention policy.** Mitigated by the reader being an RFC-011 amendment, reviewed against RFC-011's own bounds.
- **Overclaiming detection coverage.** Mitigated by requiring the limitation on the surface, not only in the closeout.

## Open questions

1. **Option A or B?** My recommendation is B. The owner's call, since it changes M10's shape and possibly its release plan.
2. **How is a diff bounded?** A generated change can touch a large file. RFC-011 bounded transcripts and RFC-019 bounded editable files at 4 MiB; a diff needs its own answer, and it belongs in the RFC-012 amendment rather than here.
3. **Does the change surface offer any action** — accept, revert, stage — or is it read-only like the explorer? Read-only is the smaller and safer first answer, and RFC-012's foundations are detection-only, so anything else needs a model that does not exist yet.
4. **Should `tekstide_core::project::DiffContent` stay owned, or become lifetime-bound?**
   (Recorded 2026-08-11, RFC-024 PR-024-C response 191.) RFC-024's `DiffContent` derives
   neither `Clone` nor `Serialize`, which structurally blocks storing the wrapper in a
   `Clone` state struct or passing it to an audit producer — but a consumer can still
   pattern-match it, move the `Vec<u8>` out, and retain the unwrapped bytes indefinitely;
   general retention is not prevented, only those two specific paths are. A strictly
   stronger design, `DiffContent<'a> { bytes: &'a [u8], .. }` tied to a request-scoped
   buffer, would make retention genuinely unrepresentable — but it constrains *this RFC's*
   own rendering architecture: whatever surface consumes it would have to render inside the
   borrow, not hold the bytes across a later frame or async boundary. RFC-024 deliberately
   did not choose this without a real consumer to weigh the cost against (nothing outside
   `tekstide-core::project::diff` references `DiffContent` yet). **This RFC is that
   consumer.** Decide the owned-vs-borrowed question against this RFC's actual rendering
   shape (Option A/B, iced's own update/view cycle) before or during whichever slice first
   calls `read_diff_content`, rather than inheriting the owned form by default because it
   was already there.
