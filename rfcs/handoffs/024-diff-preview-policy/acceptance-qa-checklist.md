---
title: "RFC-024: Diff Preview Policy - Acceptance / QA Checklist"
rfc: "RFC-024"
rfc_file: "../../proposed/024-diff-preview-policy.md"
status: "PR-024-B implemented 2026-08-11, review resubmitted as request 192 (never actually reviewed — see qa-evidence.md) — RFC-012 Amendment 1 (ChangeLifecycle) accepted (response 190) — PR-024-C content access accepted (response 191) — PR-024-D not yet started"
target_milestone: "M10"
created: "2026-08-11"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason. An absence with a reason is
evidence; an absence without one is a gap.

## Gating and Bounds (PR-024-B)

- [ ] Starting state: nothing in `tekstide-core` read generated-change content before this slice, shown by enumeration.
- [ ] Refusal occurs **before** any content read — proven, not merely ordered in source.
- [ ] Boundary exact: `== bound` accepted, `bound + 1` refused.
- [ ] The refusal's identity distinguishes it from other outcomes (PR-018-D's `TooLarge` shape).
- [ ] **No truncation behaviour exists anywhere** — no truncation test remains.
- [ ] Binary classified **before** any text read; the negative proven, not asserted.
- [ ] Only paths already in `DetectedChanges` are eligible; others refused.
- [ ] Open question 1 answered with the memory profile **measured**, not estimated.
- [ ] Open question 3 answered, and its interaction with the bound's shape stated.

## Content Access (PR-024-C)

- [ ] Content **cannot outlive the request by type**, not by convention; mechanism named and justified.
- [ ] Enumeration test names every production content-read call site; a new one fails by name.
- [ ] Content does not enter `ProjectSession` state.
- [ ] Content does not reach `AuditCoordinator` or the durable store.
- [ ] Content is **not pre-escaped**, proven by a test asserting raw bytes survive.

## Baseline Authority (PR-024-D)

- [ ] `FileSnapshot`/`ExternalChangeDecision` reused; **no second staleness mechanism**, shown by enumeration.
- [ ] A stale baseline is reported as stale, **proven against a real file changed on disk after capture**.
- [ ] Open question 2 answered with reasoning against the real cost of a per-path check.

## Honesty Checklist (PR-024-D closeout)

- [ ] Claim statement checked **against RFC-024's own text**, not only the evidence file.
- [ ] **No claim that this RFC renders anything.**
- [ ] **No claim about diff quality or algorithm** — not this RFC's contribution.
- [ ] No claim that detection coverage improved; RFC-012's limitations are unchanged.
- [ ] Every unchecked line above carries a stated reason.

## Evidence Required

- [ ] Commit/PR list and gate output.
- [ ] The enumeration tests and their ablations, with exact failing values.
- [ ] The measured memory profile behind the bound.
- [ ] Answers to the three open questions, with reasoning.
- [ ] **What this RFC hands to RFC-020** — diff shape, refusal shape, stale-baseline shape, and the bound.
- [ ] Known limitations, consolidated.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
