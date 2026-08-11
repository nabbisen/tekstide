---
title: "RFC-024: Diff Preview Policy - Task Breakdown / PR Plan"
rfc: "RFC-024"
rfc_file: "../../proposed/024-diff-preview-policy.md"
status: "Accepted 2026-08-11 — ready for implementation"
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

## PR-024-C — Content access with a bounded lifetime

Scope: reading content for an approved path, with Decision 1's third clause enforced
structurally.

Review gate:

- **Content cannot outlive the request, by type rather than by convention.** State which
  mechanism and why; a comment saying "do not retain" does not satisfy this.
- **Enumeration test** naming every production call site that reads content; a new one
  fails by name.
- Content does not enter `ProjectSession` state, and does not reach `AuditCoordinator`.
- **Not pre-escaped** — a test asserting the raw bytes survive, since escaping is
  RFC-020's job and a model that escaped would hide file contents from non-rendering
  consumers.

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
