# RFC-020: Diff Review and AgentRun Report Surfaces

Status: **Accepted 2026-08-12** (the status line said "awaiting acceptance" until 2026-08-18,
stale against `rfcs/README.md` and this RFC's own handoff pack, both of which recorded the
acceptance at the time). Surface work stopped 2026-08-15 by response 200 for want of a
producer; **re-scoped 2026-08-18**, see the scoping section at the end. The AgentRun report
surface shipped in `0.12.0`; the change review surface shipped 2026-08-25, reviewed and
accepted with one required closeout item, also delivered 2026-08-25 — see the closeout section
near the end. Both of this RFC's surfaces are now implemented and reachable.
Target milestone: M10, slipping to M11
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

**Correction, 2026-08-18 — one half of the gate I wrote is unachievable, and PR-020-C must not
inherit it.** The PR-020-B gate also required that *"content containing the literal text
`<U+202E>` is distinguishable from a real override."* It is not, and cannot be under this
project's escaping design: `escape_untrusted_chars` rewrites only Control, Format and
Default-Ignorable characters, so a real `U+202E` becomes the ASCII text `<U+202E>` while
content that already *was* that ASCII text passes through unchanged. Both render identically.
Found by the dev team while building PR-020-B's surface, disclosed rather than worked around.

**What is achievable, and is the security-relevant half, stands unchanged**: a real override
is always rendered as a visible marker and never reaches a widget raw. That is what protects a
reviewer, and it is tested.

**What the unachievable half would have bought is small**, which is why the answer is to
correct the gate rather than change the escaping. An attacker writing literal `<U+202E>` into
generated output can make a reviewer believe an override is present where none is — a **false
alarm**, not false safety. The dangerous direction is closed. Making the marker unmimicable
means escaping the marker's own delimiters across every escaped surface in the product, which
is an RFC-016 change with far wider blast radius than the failure it prevents.

**Substitute assertion, which PR-020-B adopted and PR-020-C should too**: the isolation
wrapping (`quote_untrusted`'s FSI/PDI marks) never itself appears as escaped text. Those are
Format characters, so a second escaping pass over already-escaped content would render them
visibly — which makes double-escaping concretely checkable even though marker-mimicry is not.

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

## Scoping addendum, 2026-08-25 — the remaining surface is unblocked

Requested by the human owner. Every structural prerequisite that blocked this RFC in August is
now resolved; what remains is the surface itself, which is ordinary slice work rather than a
blocker.

**Checked, not assumed:**

| Leg | State |
| --- | --- |
| A `ChangeSet` can exist in production | **Yes.** `attempt_generated_change_detection` (`shell.rs`) creates one for real when an agent run's terminal exits — wired 2026-08-18 by the change-detection-wiring handoff |
| The GUI can read them | **Yes.** `ProjectSession::change_sets()` is public |
| A bounded projection exists | **Yes.** `ChangeSet::bounded_summary(limit)` / `default_summary()` produce `ChangeSetSummary`, which already carries `changed_file_count`, `shown_changed_files`, **`omitted_changed_file_count`** and `detection_status` |
| A route to the surface | **No.** `OpenDiffReview` is `Configurable` with **no binding** — one of the three dead actions the RFC-039 affordance audit enumerates |
| A render arm | **No.** No change-review surface module exists |
| A visible control | **No.** Per the same audit: ten of thirteen live actions have none |

The projection is better placed than expected. `omitted_changed_file_count` makes truncation
explicit rather than silent, and `detection_status` carries the coverage limitation — which is
exactly what this RFC requires the surface to state ("a change surface that implies *these are
all the changes* would overclaim what detection can see"). The surface does not have to invent
that discipline; it has to render what the projection already distinguishes.

**Correction — this RFC's own slice lettering is wrong, and has already misled once.**

The Slices section below says PR-020-B is *the change review surface* and PR-020-C is *the
AgentRun report surface*. **What shipped as PR-020-B in `0.12.0` was the AgentRun report**
(`Ctrl+Alt+R`), the opposite of what that list says. The architect mislabelled the two once
already when recommending scope to the owner, on the strength of this same list.

The list is left as written — it is what was accepted — but **the remaining slice is the change
review surface, whatever letter is used for it.** Do not hand off "PR-020-C" without saying
which surface is meant. The next handoff should name the surface and drop the letter.

**What the remaining slice must carry** (from this RFC's own text, unchanged): AgentRun output
and file paths are untrusted and escaped; the surface must not present a change as safe, and
must state the metadata-only limitation **on the surface** rather than only in documentation;
the reader must not become a second retention policy. And, from RFC-039's audit: a visible
control, not only a keybinding — the third reachability principle applies to this surface as it
does to every other.

## Closeout, 2026-08-25 — the change review surface shipped; both RFC-020 surfaces are now reachable

`Ctrl+Alt+D` (or the "Change Review" button on `trust_settings_view`) opens the surface this
addendum unblocked, per `change-review-surface.md`. Reviewed and accepted, one required item
(review response 322): a live screenshot of the surface populated by a real `ChangeSet`, delivered
via `TEKSTIDE_CHANGESET_DEMO` rather than a live real-agent-run walkthrough judged too fragile to
orchestrate reliably through raw GUI automation — disclosed as a substitution, not presented as
equivalent. Full detail in `qa-evidence.md`'s PR-020-C entry.

**This RFC's own opening claim — "there is no diff review or change-review surface" — is no
longer true**, and both this document and `README.md` said so in several places; both have been
corrected rather than left stale. What remains true, and is now the accurate framing: the surface
renders *metadata* (file paths, a count, detection status, review state) — never diff *content*.
Reading actual diff content is still blocked, on retaining `DetectedChanges` past
`add_detected_generated_change_set` (which currently discards it), exactly as this addendum's own
table implied by never listing "diff content" among what exists. That remains future work, not
this slice's gap.

**Note, 2026-08-25 (RFC-041) — the paragraph above is now stale; left in place as the historical
record of this RFC's own closeout, corrected here rather than rewritten.** RFC-041 retained the
`DetectedChanges` this paragraph says is discarded, and reached the already-built
`read_diff_content`/`gate_diff_content_read` from this surface. Content is now shown — current
content, per change kind, explicitly labelled not a diff for the modified case. What remains
genuinely blocked is a **two-sided** diff for a modified file (no before-source exists short of
RFC-030), which this paragraph's own framing did not distinguish from "no content at all." See
RFC-041 and its own handoff pack for the corrected, current state.

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

---

## Scoping, 2026-08-18 — the two surfaces are not equally blocked, and the slice order inverts

Scoped at the owner's request after `0.11.0` wired change detection and `0.11.1` corrected the
transcript disclosure. Both prerequisites this RFC named — RFC-024 and RFC-011 Amendment 1 —
shipped long ago. What follows is what actually exists behind each surface, checked against the
code rather than against this document's own assumptions.

**The headline: this RFC's own ordering is wrong.** PR-020-B (change review) is listed first
and is the blocked one; PR-020-C (the AgentRun report) is listed second and is ready now.

### The AgentRun report surface is fully unblocked

Every link exists and is reached in production:

| link | where |
| --- | --- |
| a transcript is written for every AI CLI run | `runtime/terminal/launch.rs:47` |
| registered on the project | `project/session.rs:540` → `attach_agent_run_transcript` |
| discoverable from the run | `AgentRun.transcript_ref` → `Transcript.storage_path` |
| path reconstructable | `TranscriptPathResolver::resolve_agent_run` |
| bounded reader | `transcript::reader::read_window` (RFC-011 Amendment 1) |

This RFC assumed the opposite — that transcripts "are written and never read back," and that
nothing produced them from the GUI. Both were true when it was written and neither is true now.
**The `0.11.1` correction is what surfaced it**: the same investigation that found the false
privacy claim found that this surface's producer had been complete since `0.10.0`.

### The change review surface is blocked on a lossy projection, not on rendering

`read_diff_content` takes `&DetectedChanges`. Its gate refuses any path not present in it —
`DiffGateRefusal::PathNotDetected`, which is RFC-024's security boundary: content may be read
only for paths detection actually observed.

`DetectedChanges` carries, per path: **`relative_path`, `kind`, `lifecycle`**.
A stored `ChangeSet` carries: **`changed_files: Vec<PathBuf>`**.

**`kind` and `lifecycle` are dropped at that boundary**, and the `DetectedChanges` itself is
discarded once `add_detected_generated_change_set` returns. So a stored `ChangeSet` cannot
reconstruct the input `read_diff_content` requires — the surface cannot even say whether a path
was added, modified or deleted, let alone read its content.

**Fabricating a `DetectedChanges` from `changed_files` is not an option**, and this is the part
worth being explicit about: `PathNotDetected` exists to ensure content reads are confined to
what detection saw. Synthesising a gate's input from the very thing that was derived from it
defeats the gate. It would compile, pass, and remove the boundary RFC-024 was written to add.

**Recommended: retain the real `DetectedChanges` in the shell**, keyed by `ChangeSetId`, the
same session-scoped shape `agent_run_change_baselines` and `agent_run_change_detection_status`
already use. It preserves the gate (the genuine object is passed), costs no domain change, and
its lifetime matches reality — `ProjectSession::change_sets` is an in-memory `Vec` that does not
survive a restart either, so nothing is lost that currently persists.

**The eventual shape is different and should be recorded, not built now**: if change sets ever
persist, `ChangeSet` needs `kind` and `lifecycle` per path so the model is self-sufficient. That
is an RFC-012 domain change, and it should be made when persistence is designed rather than
speculatively.

### Neither surface is reachable, and both gaps differ

- **`OpenDiffReview` maps to no `AppCommand` at all** — it falls to `None` in `app_command_for`.
  It needs a command, a render arm, and a binding.
- **`OpenCurrentAgentRunDetail` has a real `AppCommand`** (`ProjectOpenSurface::AgentRunDetail`)
  and **no render arm** — `content_mode_view` falls through to the plain editor — and **no
  binding** (`Configurable`/`None`).

Naming this here because this project's own convention requires it before scheduling: a binding
alone would make `AgentRunDetail` silently render an editor, which is worse than dead.

### The scope correction still holds, and matters more now

This RFC's own §Scope correction stands unchanged: under filesystem-snapshot detection there is
**no two-sided diff for a modified file**. The before-bytes were never captured. `DiffContent`
says so in its own variant docs — `Modified { bytes, .. }` is *current content, explicitly not a
diff*.

The surface must say this **where the user reads it**. A reviewer shown current content under a
heading implying a diff will believe they have seen what changed, and the common case is exactly
the modified one.

### Open questions, answered against the real code

- **Q3 — does the change surface offer actions?** **Recommend read-only.**
  `transition_change_set_review_state` exists with no route; giving it one is a second decision
  with its own model questions, and read-only is the smaller, safer first answer this RFC
  already suggested.
- **Q4 — owned or borrowed `DiffContent`?** This RFC is the consumer the question was deferred
  to. **Recommend keeping it owned.** iced's update/view cycle means a surface holds its state
  across frames by construction; a borrow tied to a request-scoped buffer would force rendering
  inside the borrow and shape the whole surface around a retention risk that
  `DiffPreviewPolicy::max_input_bytes` already bounds. Decide it explicitly here rather than
  inheriting the owned form by default — which is what RFC-024 asked for.

### Slice lettering — this document's own list is not the operative one

**Correction, made the same day this section was written.** §Slices above calls itself
"provisional, pending the Option A/B decision" and letters the change review surface
**PR-020-B** and the report surface **PR-020-C**. The handoff pack letters them **the other
way round**, and the pack is what was built against: PR-020-B is *the transcript reader plus
the AgentRun report surface* (its reader half landed 2026-08-15), PR-020-C is *the change
review surface*.

**The pack's lettering is operative** — it is the plan work was executed against, and this
RFC's list is provisional by its own text. The first draft of this scoping section used this
document's lettering and so recommended "PR-020-C first" while meaning the report surface,
which under the operative lettering names the blocked slice. Corrected here rather than
leaving two readings in circulation.

### Revised order, in the pack's lettering

1. **Finish PR-020-B — the AgentRun report surface.** Its reader half is done. Its surface
   half was stopped in 2026-08-15 because no `AgentRun` could exist in production; that
   blocker is gone as of `0.10.0`. Needs the render arm, a binding, and an answer to the
   selection question the pack raised and never got: *which* run does "current" mean.
2. **Then PR-020-C — the change review surface**, preceded by retaining `DetectedChanges`.
3. **PR-020-D closeout**, with the no-two-sided-diff limitation stated on the surface and the
   bidi-visibility claim checked as this RFC already requires.
