---
title: "RFC-020: Diff Review and AgentRun Report Surfaces - Acceptance / QA Checklist"
rfc: "RFC-020"
rfc_file: "../../proposed/020-diff-review-and-agentrun-report.md"
status: "Open"
target_milestone: "M10"
created: "2026-08-15"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason. An absence with a reason is
evidence; an absence without one is a gap.

## The window boundary (PR-020-B)

- [ ] A window starting inside a control sequence classifies identically to the same
      content read whole, against a real sequence boundary in real captured output.
- [ ] Resynchronization ablated; the **specific** divergence shown with the exact wrong
      value.
- [ ] The delivered start offset is reported, and differs from the requested one under
      ablation.
- [ ] No UTF-8 scalar split at either edge.
- [ ] The window size is **measured** against the real 32 MiB ceiling, not estimated.

## The transcript reader (PR-020-B)

- [ ] Reader window and writer truncation render differently; a test pins the distinction.
- [ ] Complete vs. still-being-written expressed **in the type**, not a doc comment.
- [ ] Read-only proven by enumeration; a new reading call site fails by name.
- [ ] No reader path reaches a mutating call; retention metadata untouched.
- [ ] Raw bytes survive the reader, proven against `text_safety`'s own bidi probe.

## Escaping (PR-020-B and PR-020-C)

- [ ] Escaping happens at the widget; models return raw bytes.
- [ ] No second escaping primitive introduced.
- [ ] No double-escaping — literal `<U+202E>` text is distinguishable from a real override.
- [ ] **The falsifiable claim, tested**: a generated change containing a bidi override
      renders it visibly in the diff surface.

## The change review surface (PR-020-C)

- [ ] Rendered per `ChangeLifecycle`; **never** inferred from `ChangePathKind`.
- [ ] The `Modified` case is labelled as not-a-diff **where the user reads it**; the exact
      wording is quoted and justified.
- [ ] No heading, label, or affordance implies a two-sided comparison.
- [ ] Every refusal renders and is distinguishable from a file with no changes:
      `TooLarge`, non-text, path-not-detected, symlink escape, unreadable.
- [ ] A stale baseline renders as stale — distinct from an error and from an empty diff —
      proven against a real file changed on disk after capture.
- [ ] Detection's metadata-only limitation appears **on the surface**.
- [ ] No second bound; any display limit is named as a display concern.
- [ ] Read-only stated on the surface where a user might expect an action.

## Honesty checklist (PR-020-D)

- [ ] Claim statement checked **against RFC-020's own text**, not only the evidence file.
- [ ] **No claim that this renders a diff for a modified file.**
- [ ] **No claim about diff quality or algorithm.**
- [ ] No claim that detection coverage improved.
- [ ] `DiffContent`'s non-retention described accurately — two specific storage paths
      blocked, not general retention.
- [ ] No claim that a Git-backed before-source exists.
- [ ] Every unchecked line above carries a stated reason.

## Evidence Required

- [ ] Commit/PR list and gate output.
- [ ] The enumeration tests and their ablations, with exact failing values.
- [ ] The measured window size and how it was measured.
- [ ] GUI evidence for both surfaces: real screenshots, real keystrokes individually
      dispatched, stating what each proves **and does not**.
- [ ] What M10 delivered and did not, consolidated.
- [ ] What this hands forward to RFC-030.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
