# RFC-024: Diff Preview Policy

Status: Accepted by the human owner 2026-08-11 — ready for a handoff pack
Target milestone: M10 (`0.6.x`), prerequisite for RFC-020
Date: 2026-08-11

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md) §M10

Depends on:

- [RFC-012](../done/012-generated-change-review-foundations.md) — the detection model this extends, and **the RFC that named this policy**.
- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md) — the snapshot and external-change machinery this reuses rather than duplicates.
- [RFC-019](../done/019-editor-and-explorer-surfaces.md) — the editable-size bound and the escaping asymmetry.

Blocks:

- [RFC-020](./020-diff-review-and-agentrun-report.md) — the diff review surface has nothing to render until this exists.

## Why this RFC exists

RFC-012 declined to read file content and named what would authorise it (§Detection scope):

> The detector must not read or store file contents **unless a later reviewed diff preview policy allows it.** Path-only and metadata-only detection is sufficient for RFC-012.

**This is that policy.** It is a separate RFC rather than an RFC-012 amendment because an amendment would be RFC-012 authorising itself to do the thing it deliberately declined to do.

RFC-020 §"Correction, 2026-08-11" records why this is design work rather than an accessor addition: the decisions below are the ones an amendment-shaped change would skip, and they are the ones that matter in production.

## Correction, 2026-08-11 — this RFC cannot deliver a two-sided diff for a modified file

**Found by the implementer before writing PR-024-C, and the gap is the architect's.** This RFC governs how a diff is read, bounded and invalidated without ever asking whether the *inputs* to a diff exist. Decision 3 is written entirely about a baseline being **stale**; it never asks whether the baseline has content to be stale *about*.

It does not. `ReviewBaselineEntry` holds `relative_path`, `kind`, `len`, `modified_unix_nanos` — **no content, no hash** — and that is deliberate. RFC-012 §Design Principles 2: *"Metadata first. Summaries must not include file contents…"* So for a file the filesystem-snapshot detector reports as **modified**, the before-bytes were never captured, and by diff-request time the run has overwritten them. They are gone, not merely unretained.

**Capturing content at baseline time is rejected**, and not only because it contradicts Decision 1's "never speculatively" and "never retained beyond the request." It contradicts RFC-012's stated principle directly, and would mean speculatively storing the contents of every scanned path for every AgentRun, held until a request that may never come. That is a new retention surface requiring RFC-012 **and** RFC-011 amendments plus the owner's authorisation — not a choice available to an implementation slice.

**What this RFC therefore delivers:**

| Change kind | Available | Delivered |
| --- | --- | --- |
| Added | full content is the whole change | content, bounded and gated |
| Deleted | nothing to read | the fact of deletion, from metadata |
| **Modified** | **current content only** | current content, **explicitly not a diff** |

**The modified row must be stated on the surface, not only in a closeout.** A user shown a file's current content under a heading implying a diff will believe they have seen what changed. RFC-020 already forbids overclaiming what *detection* can see; this is the same rule applied to what *preview* can see.

**A two-sided diff needs a before-source, and the only one this project has designed is Git-backed detection** — which holds blob history, and is gated behind RFC-012's own unmet safety evidence. So the two-sided case is not cancelled; it is blocked on an existing, already-gated dependency.

The RFC keeps its name: "diff preview policy" is the phrase RFC-012 used to name this document, and changing it would break the link back to the clause that authorises it.

## Scope

**In:** when generated-change content may be read, how it is bounded, when a baseline stops being authoritative, and what is delivered for a non-text change. **Per the correction above, "a diff" means a two-sided comparison only where a before-source exists.**

**Out:** rendering (RFC-020), any *action* on a change — accept, revert, stage — and Git-backed detection, which RFC-012 already gates behind its own safety evidence.

## Decision 1 — content is read only on demand, only for already-detected paths

Three constraints, and the third is the one that keeps this bounded:

1. **Only paths RFC-012's detector already reported as changed.** This policy authorises reading content for a known change; it does not authorise scanning.
2. **Only on explicit user request** — opening the review surface for a change. Never speculatively, never in the background, never as a side effect of detection.
3. **Never retained beyond the request.** A diff is computed, rendered, and dropped. Content does not enter `ProjectSession` state, and it does not enter the audit store — RFC-013's families have no field for it, and this policy must not become the reason one is added.

That third constraint is what stops "read content for review" from becoming "the project now holds copies of everything an agent touched."

## Decision 2 — refuse above the bound; never truncate

**A diff whose inputs exceed the bound is refused whole, with a stated reason. It is never truncated.**

This project has now made this mistake once and caught it once, in the same subsystem family:

- The terminal's 64 KiB per-poll cap **truncates mid-stream and discards the event that says so** — recorded in `future-work.md` as a defect requiring a real block/grow/report decision, and currently masked only by an unrelated sleep.
- RFC-018 PR-018-B's paste path originally truncated at 64 KiB **before classification**, which let truncation change the classification. The fix was to refuse the whole paste rather than shorten it.

The paste fix is the precedent to follow, and for the same reason: a truncated diff is not a smaller true answer, it is a **different and false** one. A reviewer shown 400 of 900 changed lines, without being told, believes they have reviewed the change.

**The bound applies to inputs, not output**, and covers both versions plus the computed difference. RFC-019 bounds an editable file at 4 MiB; a diff naturally costs more than one file, so this RFC sets its own number rather than inheriting that one. **State the number and the reasoning**, the way RFC-017's terminal-session limit had to be re-derived once its basis was understood.

## Decision 3 — baseline authority, reusing what already exists

A diff is computed against a `ReviewBaseline` captured at some earlier moment. **A baseline that no longer matches on-disk state produces a diff describing changes that are not there** — the same failure class as the defect RFC-019 PR-019-E found, where a status derived from a source that had stopped being authoritative told a user their local changes would be discarded when they had none.

**Reuse `TextDocument`'s existing snapshot machinery — do not build a second one.** `FileSnapshot`, `last_known_snapshot()` and `ExternalChangeDecision` already answer "has this file changed underneath what I last saw." That machinery is reviewed, tested, and has already caught a real defect. A parallel staleness mechanism for diffs would be a second source of truth about the same question.

The rule: **a diff states the baseline it was computed against, and a stale baseline is reported as stale rather than silently diffed.** Whether staleness invalidates the whole review or only the affected path is this RFC's to decide during implementation, with the reasoning recorded.

## Decision 4 — non-text changes are classified before content is read

`ChangePathKind` exists. What a diff *is* for a non-text change does not.

**Binary detection happens before the file is read as text**, not by attempting a UTF-8 read and handling failure — that ordering reads the whole file to answer a question a bounded sniff answers. A non-text change is reported as a change with its size and kind, and no diff is attempted.

The ordering matters for the same reason it mattered for paste: **a classification made after reading is a classification made on data you have already committed to handling.**

## The escaping position, inherited and stated

Diff content is **escaped**, per RFC-020's §The security core. It does not inherit the editor's raw-rendering exception, because a diff is reviewed rather than edited, and escaping is the stronger position here: a reviewer deciding whether to accept a generated change wants to see that it introduces `U+202E`.

This RFC owns producing the content; RFC-020 owns rendering it. **Content produced here is not pre-escaped** — escaping is a rendering concern, and a model that returned escaped text would prevent any future non-rendering consumer from seeing what the file actually contains.

## Risks

- **Content retention creeping past the request.** Mitigated by Decision 1's third constraint, which should be enforced structurally rather than by convention — a type that cannot outlive the request is better than a rule that says it must not.
- **Truncation reappearing as a convenience.** Mitigated by Decision 2 and by the two recorded precedents.
- **A second staleness mechanism.** Mitigated by Decision 3's reuse requirement.
- **Binary read before classification.** Mitigated by Decision 4's explicit ordering.
- **This RFC quietly becoming a diff *engine*.** Producing a difference between two texts is a solved problem with well-understood libraries; this RFC's value is in the policy around it, not in the algorithm. If implementation starts designing a diff algorithm, that is a signal the scope has drifted.

## Open questions

1. **The bound's number**, and whether it is per-file or per-review. Decide with the memory profile measured, not estimated — this project has twice found that an estimated bound was wrong once measured.
2. **Does a stale baseline invalidate the whole review or one path?** Whole-review is safer and cruder; per-path is more useful and needs a per-path staleness check. Decide against the real cost of the check.
3. **Is a diff computed lazily per opened path, or eagerly for the whole change set?** Lazy bounds memory naturally and matches Decision 1's on-demand rule; eager gives an accurate up-front count of what changed. These may both be wanted, and the answer affects Decision 2's bound shape.
