---
title: "RFC-024: Diff Preview Policy - Task Breakdown / PR Plan"
rfc: "RFC-024"
rfc_file: "../../proposed/024-diff-preview-policy.md"
status: "PR-024-B implemented 2026-08-11, review resubmitted as request 192 (never actually reviewed — see qa-evidence.md) — RFC-012 Amendment 1 accepted (response 190) — PR-024-C content access accepted (response 191) — PR-024-D not yet started"
target_milestone: "M10"
created: "2026-08-11"
---

# RFC-024 Task Breakdown

Four slices. **[`the-four-decisions.md`](./the-four-decisions.md) is required reading
before any of them.** All work is in `tekstide-core`.

## PR-024-A — Design and handoff acceptance

Granted 2026-08-11 with the RFC. Nothing to implement.

## PR-024-B — Gating and bounds

Scope: the decision layer, before any content is read. Size check against metadata, kind
classification, and the refusal type. **No diffing yet** — this slice decides what may be
read and produces a refusal or a go-ahead.

Review gate:

- **Starting state confirmed**: nothing in `tekstide-core` currently reads
  generated-change content, shown by enumeration.
- **Refusal happens before any content read**, proven — not merely ordered correctly in
  the source.
- Boundary test: `== bound` accepted, `bound + 1` refused.
- **Binary classified before a text read is attempted**, with the negative proven rather
  than asserted.
- Open questions 1 and 3 answered, with the bound **measured**, and the reasoning recorded.
- Reads are for already-detected paths only; a path not in `DetectedChanges` is refused.

**Discharged, implemented 2026-08-11 — full detail in `qa-evidence.md`.** Every gate item
met: starting state confirmed by enumeration (grep across both crates, excluding
`change_detection.rs` itself, found only metadata fields flowing into `ChangeSet`); the
refusal-before-read and sniff-boundedness properties both proven by real fixtures
(`chmod 000` + oversized file; a blocking FIFO) and both ablated with the exact wrong
value each produced; the boundary tested against the real default, not a substituted
policy; open questions 1 (4 MiB per side, measured via a real `/proc/self/status` RSS
sweep) and 3 (lazy per-path, required directly by Decision 1's own text) answered with
reasoning recorded.

**A real architectural question, raised and filed before any code was written, not
absorbed into this slice's own scope**: where does a diff's "before" content come from
at all, for the one detection source (`FilesystemSnapshot`) actually reachable in
production? Traced through `GeneratedChangeDetector`, RFC-012's own text, and
`ChangeSet::artifact_refs` and found no existing mechanism captures it anywhere — by
diff-request time an AgentRun has typically already overwritten the file. This slice does
not depend on the answer (gating/bounds apply symmetrically to either side of a diff), so
it proceeded; **PR-024-C is blocked on review request 187's answer**, since the two
plausible resolutions (baseline capture amended to also snapshot content, vs. a
single-sided view for this detection source) produce structurally different designs for
content access and cannot both be built toward.

## PR-024-C — Content access with a bounded lifetime

**Second prerequisite, added 2026-08-11 (response 189): RFC-012 Amendment 1 must land
before this slice.** `DetectedChangedPath` could not tell Added from Modified —
`changed_paths_between` computed the distinction and discarded it — so the corrected
three-row scope below was unfillable. Amendment 1 adds
`ChangeLifecycle { Added, Modified, Deleted }` and removes `Deleted` from
`ChangePathKind`, which is a **breaking** change to `tekstide-core`: the next release
carrying it is `0.7.0`, not `0.6.1`.

**Added to this slice's gate:** the Added / Modified / Deleted distinction is read from
`ChangeLifecycle`, **never inferred from `ChangePathKind`** — and the modified case's
"not a diff" statement reaches the surface, not only the closeout.

**Unblocked 2026-08-11 (response 187), with a scope correction landed in RFC-024's own
text (`f48d245`) before this slice starts.** This RFC cannot deliver a two-sided diff for
a modified file under `FilesystemSnapshot` detection — the before-bytes were never
captured (`ReviewBaselineEntry` is metadata-only, deliberately, per RFC-012 §Design
Principles 2) and are gone, not merely unretained, by the time a diff is requested.
Capturing content at baseline time was considered and rejected: it contradicts RFC-012's
own stated principle directly, not only this RFC's Decision 1, and needs RFC-012 **and**
RFC-011 amendments plus owner authorisation — not a choice available to this slice.

**Corrected scope, per change kind** (RFC-024 §Correction, 2026-08-11):

| Change kind | Available | Delivered |
| --- | --- | --- |
| Added | full content is the whole change | content, bounded and gated |
| Deleted | nothing to read | the fact of deletion, from metadata |
| **Modified** | **current content only** | current content, **explicitly not a diff** |

**The modified case must be labelled as not-a-diff in whatever this slice returns**, not
only disclosed in a closeout — RFC-020's own carried note says the surface must not
render current content under a heading that implies a two-sided comparison. This slice
owns making that distinction representable in the type it returns; RFC-020 owns choosing
the words a user reads.

Scope: reading content for an approved path, with Decision 1's third clause enforced
structurally. `gate_diff_content_read` (PR-024-B) already tells the caller `Readable`,
`NonTextContent`, or `NonFile` — this slice reads the approved case and shapes the
distinction above around it.

Review gate:

- **Content cannot outlive the request, by type rather than by convention.** State which
  mechanism and why; a comment saying "do not retain" does not satisfy this.
- **Enumeration test** naming every production call site that reads content; a new one
  fails by name.
- Content does not enter `ProjectSession` state, and does not reach `AuditCoordinator`.
- **Not pre-escaped** — a test asserting the raw bytes survive, since escaping is
  RFC-020's job and a model that escaped would hide file contents from non-rendering
  consumers.
- **The returned shape distinguishes "this is a diff" from "this is current content,
  presented because no before-version exists"** — not two functions returning the same
  type with the difference left to a doc comment.

**Blocked, found on starting this slice, filed as review request 189**: the table above
needs Added vs. Modified for `File`-kind changes, and `DetectedChangedPath` cannot
express it — `changed_paths_between` (RFC-012's own model, `change_detection.rs`)
computes the distinction and discards it before constructing the value this slice
receives. Full detail in `qa-evidence.md`'s PR-024-C entry. Recommended a minimal,
additive amendment (preserve the distinction already computed) rather than implementing
one unilaterally, since it touches a different closed RFC's model. *(Superseded below —
response 189 found a deeper defect than the recommendation above and RFC-012 Amendment 1
landed to fix it; see the amendment's own entry in `qa-evidence.md`.)*

**Unblocked and implemented 2026-08-11 (response 190 accepted RFC-012 Amendment 1).**
Every review gate item above met — full detail in `qa-evidence.md`'s "PR-024-C's own
content access" entry:

- Content-cannot-outlive-the-request made structural via `DiffContent` deriving neither
  `Clone` nor `Serialize` — a compile error if ever stored in `ProjectSession` (which
  derives `Clone` uniformly) or passed to `AuditCoordinator::record_*` (which requires
  `Serialize`), not a documentation promise.
- Enumeration test (`enumeration_confirms_only_the_closed_list_reads_full_file_content`)
  scans `tekstide-core` for raw full-file reads against a closed, disclosed allowlist;
  ablated by removing `project/diff.rs` from it, confirmed the failure names the file.
- Not pre-escaped: proven against the exact bidi probe `text_safety`'s own tests use,
  raw bytes survive unaltered.
- The Added/Modified distinction is two separate `DiffContent` constructors, not a
  shared shape with a lifecycle flag — satisfying "not left to a doc comment" directly.
- A design refinement made and disclosed while implementing: `DiffGateDecision`'s
  `lifecycle` field narrowed from `ChangeLifecycle` to a new two-variant
  `ContentLifecycle`, closing a smaller instance of the same representable-but-
  meaningless-state class the amendment itself just fixed one level down.

## PR-024-D — Baseline authority, and closeout

Scope: staleness via the existing snapshot machinery, plus the closeout.

Review gate:

- **`FileSnapshot`/`ExternalChangeDecision` reused; no second staleness mechanism**, shown
  by enumeration.
- A stale baseline is **reported as stale, not silently diffed**, proven against a real
  file changed on disk after capture — not a synthesised value.
- Open question 2 answered with reasoning.
- Claim statement checked **against RFC-024's own text**, not only the evidence file.
- **No claim that this RFC renders anything**, and no claim about diff *quality* — the
  algorithm is not this RFC's contribution.
- Known limitations consolidated, including anything the three open questions' answers
  constrain for RFC-020.

## Sequencing

**B → C is strict** — content access must not exist before the gate that decides whether
it is allowed. D needs C.

```
A ─→ B ─→ C ─→ D
```

## What this RFC hands to RFC-020

Record explicitly at closeout, because RFC-020's own handoff will be written from it:

- the shape of a produced diff, and that it is unescaped;
- what a refusal looks like, so the surface can render one rather than showing nothing;
- what a stale baseline looks like, for the same reason;
- the bound, so RFC-020 does not introduce a second one.
