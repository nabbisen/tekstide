---
title: "M14 security review, Part 1 — the threat model's own open questions, disposed"
status: "Part 1 of the review opened in security-review-m14.md. Written 2026-11-15. Findings, not amendments: the amendments it argues for become an RFC."
rfc_file: "none — review output"
target_milestone: "M14"
created: "2026-11-15"
---

# Part 1 — the threat model's ten open security questions

`security-review-m14.md` makes this the first part because it is entirely checkable against
`rfcs/done/` and because it produces the spine of the threat-model update.

**The questions are referred to by subject, not reproduced.** The source document is gitignored and
the owner's; this file is tracked. Each row below names the section-14 question by what it asks
about, so a reader holding the document can follow along and a reader without it learns nothing
from this file that the project has not already published.

**Disposition vocabulary.** *Answered* means an accepted RFC decided it and the decision is live in
the shipped product. *Answered by standing position* means the product behaves one way and no dated
decision record closes the question. *Open* means neither.

## The ten, disposed

| # | Subject | Disposition | Where |
| --- | --- | --- | --- |
| 1 | Transcript persistence: default-on or opt-in | **Answered** | RFC-011 (M6): capture defaults to `LocalBounded` for Tekstide-created runs when the storage preflight passes, with a per-run opt-out to `Disabled`. Default-on with opt-out, not opt-in. |
| 2 | Audit logs purgeable, or append-only until project deletion | **Answered, and reachable** | RFC-013 (M7) defined project and global purge; RFC-059 (`0.35.0`) gave it a production call site. Verified: `crates/tekstide/src/shell.rs` calls `purge_project_records`/`purge_all_records`. |
| 3 | How a previously trusted project is identified | **Answered** | Canonical root, looked up from `recent-projects.json` (RFC-058) — not Git remote, not inode/device. **Carries a known failure mode**; see below. |
| 4 | Which AI CLI gets the first Managed adapter, *if any* | **Being answered now** | RFC-069, accepted 2026-11-15. Its first slice is a measurement that may yet answer *if any* in the negative, so this is not yet "answered". |
| 5 | Which command categories require approval | **Answered** | `RiskLevel::{Low, Medium, High, Destructive}` with a structural `RiskReason` enum, classified in `approval/risk.rs` (RFC-021), reachable since RFC-060 (`0.36.0`). |
| 6 | Minimal environment as the AgentRun default | **Answered — yes** | `AiCliProfile`'s default is `AiCliEnvironmentPolicy::Minimal`, with `Named`, `ExplicitAllowlist` and `WorkspaceLocalEnvFile` as deliberate opt-outs. |
| 7 | Optional OS-level sandbox profiles, per platform, later | **Open** | No RFC decides it. RFC-004's non-goal is *not* fully sandboxing arbitrary shells, which is a different statement from whether optional per-platform profiles should ever be offered. |
| 8 | Scrollback and transcript storage defaults | **Answered** | Scrollback default 2,000 lines, cap 12,000 (`config/terminal.rs`); transcript retention default 30 days (`transcript/policy.rs`). |
| 9 | Whether plugin design stays deferred | **Answered by standing position** | No plugin runtime ships; Appendix A's MVP position holds and RFC-074/RFC-077 restate it as out of scope. **No dated decision record closes the question**; it has been answered by not being reopened. |
| 10 | A safety banner on exported transcripts and handoffs | **Open** | No banner exists anywhere in the product. The subject belongs to RFC-068 (*Redacting What A Run Prints*), reserved `pre-1.0` and unwritten. |

## Findings

### F1. Three of ten are not answered, and two of those are the same hole

**7 and 10 are open; 9 is answered only by inertia.**

Questions 10 and the gap Part 2 will record in §13's recommended sequence are **the same hole seen
from two directions**: the threat model asked for a safety banner on exports, and its own
recommended RFC S5 (environment and secret policy, redaction posture) has no RFC covering its
redaction half. RFC-068 is the reservation for both and it is unwritten. A single RFC closes both,
and until it is written the product exports transcripts and handoffs with no statement about what
they might contain.

### F2. Question 3's answer carries a recorded failure mode the threat model does not know about

Project identity is the canonical root from `recent-projects.json`. `future-work.md` records,
found at RFC-050's acceptance, that **a corrupt `recent-projects.json` silently resets every
project's identity** — which, for a question the threat model asked in order to decide how trust is
remembered, is a property the baseline should carry. The decision is sound; the baseline is
incomplete without its failure mode.

### F3. Question 2's history is the cautionary case for the rest of this review

The answer to "are audit logs purgeable" was *yes* from RFC-013 in M7. For two milestones after
that it was yes in the API and no in the product: the purge functions existed with no production
call site, and `REQ-SEC-025` — that users must be able to purge local transcripts, logs and audit
data — was unmet for audit data the whole time. RFC-059 closed it in `0.35.0`.

**So a question answered in a design document is not a control that exists.** Part 3 verifies
behaviour, not decisions, and this is the reason it has to.

### F4. Question 9 should be closed deliberately or not at all

"Still deferred" is almost certainly the right answer, and nothing has decided it since 2026-07.
A standing position that has never been written down as a decision is indistinguishable, to a later
reader, from one nobody revisited. Either a dated line closes it or the threat model keeps it open
honestly; what it should not do is look closed because the code is quiet.

## Proposed amendments to the threat model

Stated as proposals. The source document is the owner's and this review does not edit it; the
mechanism for superseding it is an accepted RFC, as RFC-076 D2 did for external design §4.2.

1. **Mark questions 1, 2, 3, 5, 6 and 8 resolved**, each against the RFC named above, with its
   release.
2. **Record question 3's failure mode** alongside its answer.
3. **Leave 4 open** until RFC-069's measurement lands, and note that its own *"if any"* is live.
4. **Leave 7 and 10 open**, and name RFC-068 against 10 so the reservation and the question point
   at each other.
5. **Decide 9 explicitly**, either way.

## What this part does not claim

Nothing about whether any control *works*. Every disposition above is about a decision and its
record. Six questions are answered; that is a statement about `rfcs/done/`, not about the product's
behaviour under attack, and Part 3 is where that distinction gets paid for.
