---
title: "RFC-020: Diff Review and AgentRun Report Surfaces — Task Breakdown / PR Plan"
rfc: "RFC-020"
rfc_file: "../../proposed/020-diff-review-and-agentrun-report.md"
status: "PR-020-B's core (transcript reader) implemented 2026-08-15; reviewed (response 198), three required corrections applied same day, not yet re-reviewed — surface, PR-020-C, and PR-020-D not started"
target_milestone: "M10"
created: "2026-08-15"
---

# RFC-020 Task Breakdown

Four slices. **[`the-window-boundary.md`](./the-window-boundary.md) is required reading
before any of them.**

## PR-020-A — Design and handoff acceptance

Granted 2026-08-12 with the pack. Nothing to implement. RFC-020's four open questions are
answered in the README; raise a disagreement with evidence rather than implementing around
one.

## PR-020-B — The transcript reader, and the AgentRun report surface

Ordered first because it carries the security-critical decision, and because the reader
does not exist. Core work then surface work, in one slice, because a reader with no
consumer cannot be shown to be correct.

**Core** — the bounded reader, per RFC-011 Amendment 1's D1-D5.

**Surface** — the AgentRun report, escaped at the widget.

Review gate:

- **Window resynchronization proven**: a window starting inside a control sequence
  classifies identically to the same content read whole, against a real sequence boundary
  in real captured output.
- **Ablated**: resynchronization removed, the *specific* divergence shown with the exact
  wrong value, and the delivered-offset report shown to differ.
- **The delivered start offset is reported**, not the requested one.
- **No UTF-8 scalar split** at either edge.
- **Reader window vs. writer truncation render differently**, and a test pins the
  distinction. Conflating them is the failure mode.
- **Complete vs. still-being-written expressed in the type**, not a doc comment.
- **Read-only, by enumeration**: every production call site that opens a transcript for
  reading is named; a new one fails the test by name; no reader path reaches a mutating
  call.
- **Raw bytes survive the reader**, proven against `text_safety`'s own bidi probe.
- **The window size is measured** against the real 32 MiB ceiling, not estimated. Two
  estimated figures in this project were wrong once measured.
- **Escaping happens at the widget**, and no double-escaping — content containing the
  literal text `<U+202E>` is distinguishable from a real override.

**Core implemented 2026-08-15 (commits `c229781`, `1c7b980`), reviewed (response 198).
Surface not started — this slice is not complete.** Every core-side review gate item above
is met; full detail in `qa-evidence.md`. **One item found and fixed before the reader could
be built at all, not part of this slice's own gate**: `TerminalSecurityParser::parse`
(RFC-017) panicked on a CSI sequence truncated to a bare `ESC [` at a buffer's own end —
reachable the moment anything calls it on a buffer that was not guaranteed complete, which
nothing did until this slice's own resynchronization proof needed to. Fixed and disclosed
as its own commit (`c229781`), separate from the reader (`1c7b980`), since it is a defect
in a different RFC's already-shipped module.

**Response 198 accepted the panic fix and the reader outright, with three required
corrections before surface work starts — all three applied 2026-08-15 (commit `b74d8d5`),
not yet re-reviewed**:

1. **Refuse when `total_len > MAX_SCAN_BYTES`.** The reader had no guard; a transcript
   larger than the scan ceiling silently returned a window from the middle of the file,
   mislabelled as the tail. Fixed with `TranscriptReadErrorReason::TranscriptExceedsScanLimit`
   and a regression test.
2. **Re-measure D1's peak memory against the real 32 MiB ceiling.** The original sweep
   measured only the window in isolation and never allocated the mandatory scan buffer,
   understating real peak RSS by roughly an order of magnitude. Corrected measurement: real
   peak is ~33-50 MiB, dominated by the fixed 32 MiB scan buffer regardless of window size.
   1 MiB remains the chosen window, on the grounds the corrected figure supports (a window,
   not "basically the whole transcript"), not the "trivial memory cost" reasoning the wrong
   figure gave.
3. **Document why `read_window` scans from byte 0 rather than seeking near the tail.**
   Doc comment added: resynchronization needs a position guaranteed to be a sound parse
   origin, which only the file's true start provides.

Remaining for this slice: the AgentRun report widget, the reader-window-vs-writer-truncation
rendered distinction (needs the widget to exist), and the no-double-escaping proof (needs
the widget's own escaping call site).

## PR-020-C — The change review surface

Depends on B only for the escaping pattern it establishes. All model work exists already.

Review gate:

- **Rendered per `ChangeLifecycle`, never inferred from `ChangePathKind`** — the
  distinction RFC-012 Amendment 1 exists to provide.
- **The `Modified` case is labelled as not-a-diff where the user reads it.** Quote the
  exact wording chosen and justify it. This is the highest-consequence sentence in the
  slice.
- **No heading, label, or affordance implies a two-sided comparison** anywhere on this
  surface.
- **Every refusal renders**: `TooLarge`, non-text, path-not-detected, symlink escape,
  unreadable. A refused path must be distinguishable from a file with no changes.
- **A stale baseline renders as stale**, distinct from both an error and an empty diff,
  proven against a real file changed on disk after capture.
- **Detection's metadata-only limitation appears on the surface**, not only in
  documentation.
- **The falsifiable claim, tested**: a generated change containing a bidi override renders
  it visibly. Stated as a claim that could be false.
- **No second bound introduced.** If a display limit exists, it is named as a display
  concern and cannot silently show less than RFC-024's policy allowed.
- **Read-only stated on the surface** if a user might expect an action.

## PR-020-D — Closeout

Review gate:

- **Claim statement checked against RFC-020's own text**, not only against the evidence
  file. RFC-017 shipped two false statements because only the review response was
  corrected and the document was left wrong.
- **No claim that this renders a diff for a modified file.**
- **No claim about diff quality or algorithm** — neither is this RFC's contribution.
- **No claim that detection coverage improved.** RFC-012's limitations are unchanged.
- **`DiffContent`'s non-retention described accurately** — it blocks two specific storage
  paths, not general retention. Do not repeat the stronger claim.
- **What M10 delivered and did not**, consolidated, since this closes the milestone.
- Every unchecked line in the acceptance checklist carries a stated reason.

## Sequencing

```
A ─→ B ─→ C ─→ D
```

**B before C** is deliberate. B establishes the escaping pattern and carries the
security-critical work; C reuses the pattern. Doing C first would set the escaping
precedent in the surface with the *weaker* justification, and B would inherit it.

## What this hands forward

Record at closeout, because the next RFC's handoff is written from it:

- the escaping pattern for a reviewed-not-edited surface, and where it lives;
- what a refusal and a stale baseline look like rendered;
- the reader's window semantics and delivered-offset contract;
- whatever the `Modified`-case wording turns out to be, since RFC-030 (Git integration) is
  the RFC that could make a real two-sided diff possible and will have to replace it.
