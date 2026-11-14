# RFC-074: Capability Boundaries — The Gate, And The First Two Moves

Status: **Implemented and closed 2026-11-14, moved to `done/` as part of the `0.40.0` release,
published 2026-11-14.** D1 through D5 as written, with Stages 1
and 2 as the first slice and the later staging scoped in a handoff after the gate has proven
itself on two real stages, exactly as *Acceptance* asked. Two owner-level questions stood open
and neither blocked that slice: which release slot this takes, and whether
`tekstide-core`'s published API may change shape in that release — Stage 1 is API-compatible
by construction (D3), so it did not wait on the second. Accepted by the human owner 2026-11-03.

**Updated 2026-11-09, dated rather than silently edited.** The second question is **answered:
the owner accepted the `TerminationOutcome` variant proposal**, which was the concrete form that
question had taken (review 657's own narrowing: *"when a consumer needs this distinction, may that
release carry a breaking change to `TerminationOutcome`"*). So a release carrying this RFC's work
may also carry a shape change to `tekstide-core`'s published API. **The release-slot question stays
open**, and Stage 1 still does not depend on either. Separately, the macOS-gate hold that kept
PR-074-A from starting is **lifted** — see `.git-exclude/reviewed/`'s own dated note for the
attribution rule that replaces it; the three families that produced reds on code-free commits
(explorer frame budget, watch semantics, the approval-channel `EINVAL` sites) are all closed, and
the one still open has a greppable signature.

**Split 2026-11-12, at the owner's acceptance, and both open questions are answered with it.**
This RFC spanned more releases than one — D1–D5 are the gate, the tier model, Stages 1–2 and the
isolation rule Stage 2 is the first to exercise; D6–D8 are the capability split proper, whose
staging was deliberately unscoped until the gate had proven itself twice. **Corrected 2026-11-14
by its own author**: this sentence was written on 2026-11-12 as "D1–D4 … D5–D7", when D5 was still
going to RFC-077. D5 stayed, the registers and the moved-out heading below were updated, and this
clause was not — so the paragraph described one split and its own next sentence described another. A released section naming an RFC asserts that RFC shipped in that release
(`an_rfc_a_release_names_lives_in_done`), so a `0.40.0` carrying only the first slice could not
have named the RFC it was about — the same reason RFC-026 was split from RFC-065. **D6–D8 moved to
[RFC-077](../accepted/077-the-capability-split.md)**, carried forward as already-accepted decisions.
**The release slot is `0.40.0`**, and **the published API may change shape in it**: Stage 1 is
API-compatible by construction (D3), Stage 2 takes `test_support` out of the production graph.

**Implemented, 2026-11-14.** D1 through D4 are all implemented: the gate (`dc50c7a`), Stage 1 moving
`ProjectId` to T0 (`53e47f7`, bound 10 → 9), and Stage 2 taking `test_support` out of the
production graph (`e573cbc`, bound 9 → 8). D5's isolation rule is exercised, not separately built —
Stage 2 is the first move this RFC made that D5 governs, and review 675's three rounds hardening
the gate's own fifth assertion (`43f4e30`, `836577e`, `3abae15`) are what closed it against every
evasion found rather than the one first observed. D6–D8 remain out of scope, carried forward to
RFC-077 as already-accepted decisions, exactly as the split above states.

Proposed 2026-11-03. Approved to follow RFC-028 by the human owner (review 595), and drafted
only after RFC-028 closed and `0.38.0` shipped, as directed. The goal, the tier model and
three of the decisions below were settled first, in
`tekstide-structure-goal-definition.md` (2026-10-17, written at the owner's direction to **define
the goal before measuring the graph**, and derived from `specs/tekstide-requirements-v0.md`,
`specs/tekstide-external-design-v0.md`, `specs/tekstide-appendix-a-extensibility-plugin-v0.md`,
`ARCHITECTURE.md` and `ROADMAP.md`). This RFC carries those forward as decided, re-measures every
number against the code as it stands today rather than reusing October's figures, and adds the one
thing the goal document deliberately left open: **how the structure is held, stage by stage,
without a human re-reading the graph.**

## Summary

`tekstide-core`'s modules form **one strongly connected component of ten**. The vocabulary tier
that is supposed to depend on nothing depends on the hub. `project/` is not one capability — it is
an orchestrator plus six or seven separately-authored ones sharing a directory.

This RFC does not propose a rewrite. It proposes **a measured gate, then a sequence of small moves
whose effect on the graph is predicted before each one lands.** The gate is the deliverable that
matters: without it, every stage is undone by the next person who writes a `use`.

## Why now, measured today

Method, stated so it is reproducible and so a later reader can tell drift from a different
counting rule: production `.rs` files in `crates/tekstide-core/src` only (`tests.rs`, `*_tests.rs`
and `tests/` excluded), full-line comments stripped, cross-module references counted as
occurrences of `crate::<module>` where `<module>` is a top-level module other than the file's own.
21 top-level modules.

| Measurement | Today |
| --- | --- |
| Cross-module edges / references | **66 / 151** |
| Largest strongly connected component | **10 modules** — `agent`, `approval`, `audit`, `content`, `domain`, `project`, `runtime`, `security`, `test_support`, `transcript` |
| Edges / references inside that component | **37 / 104** |
| Inbound references to `project` | **43**, from 12 distinct modules |
| `project/` production lines | **~9,041**, across 11 submodules; `session.rs` alone is 2,676 |

Two of those rows are the whole argument:

**`domain` — the vocabulary tier, which by the goal document's own definition depends on
nothing — has exactly seven outgoing references, and they are the same line seven times:**

```
domain/ownership.rs:1    use crate::project::ProjectId;
domain/audit.rs:6        use crate::project::ProjectId;
domain/terminal.rs:2     use crate::project::ProjectId;
domain/approval.rs:2     use crate::project::ProjectId;
domain/transcript.rs:2   use crate::project::ProjectId;
domain/changeset.rs:2    use crate::project::ProjectId;
domain/agent.rs:6        use crate::project::ProjectId;
```

Seven of seven. Not a sample — the complete set, from a grep over the whole crate.

**`test_support` is a member of the production component.** Test scaffolding is inside the
strongly connected core, held there by two single references (`runtime → test_support` and
`test_support → runtime`, one each).

## The organising rule

From `ARCHITECTURE.md` — *"`tekstide-core` decides. `tekstide` renders the decision and collects
the answer"* — and Appendix A §3.2 — *"Plugins provide structured data. Tekstide core decides how
that data is rendered"* — one rule at two boundaries:

> **Authority and decisions stay in the host. Everything crossing a boundary passes structured
> data, never control.**

Every boundary below exists to make that rule enforceable at one more place. Appendix A names
seven host-owned responsibilities — UI rendering, project trust, process lifecycle, command
approval, transcript handling, audit logging, cross-project isolation. **A structure whose modules
do not correspond to that list cannot defend it**, and today they do not: filesystem safety
(requirements §7.5, with its own threat-model section) shares a directory with diff rendering,
recent-project bookkeeping and the file watcher. A reviewer cannot tell from a change's *location*
whether it touches a security boundary, and this project's entire review discipline rests on
being able to.

## Decisions

### D1. The gate is built first, and it ratchets. **This is the load-bearing decision.**

A test in `tekstide-core` measures the module graph from the source and asserts three bounds:

1. the largest strongly connected component is **at most N modules**;
2. `domain` has **zero** outgoing cross-module references;
3. no module outside a declared allow-set reaches `test_support`.

N starts at today's measured value and is **lowered by the stage that earns it, in the same
commit**. The test fails if the graph grows, so no stage can be silently undone, and it fails if a
stage's own claim is wrong, so no stage can be reported as landed when it has not.

Why first rather than last: every property in the goal document's table is checkable, and the
three new ones (acyclic, tiered, capability isolation) have no check today. A structural
improvement with no gate is a structural improvement with a half-life.

**The gate measures source text, not a build artifact.** It is the same shape as this project's
existing `rfc_docs_invariants` — read the files, derive the fact, compare against the stated
number — and it is subject to the same rule: the number in the test and the number in this
document are the same number, or the test fails.

### D2. The tier model is normative

T0 vocabulary (identities, enums, error taxonomy; no I/O, no dependencies) · T1 host-owned
capabilities, one per authored responsibility, **none importing another** · T2 orchestration
(`ProjectSession`, application state; may depend on every T1, **nothing may depend on it**) ·
T3 presentation (the GUI crate) · T4 the extension seam Appendix A §2.1 requires.

The mapping from each capability to the requirement that authorises it is the table in
`tekstide-structure-goal-definition.md` §3, adopted here unchanged. It is authored from the specs,
not read off the import graph, which is why it can be used to judge the graph.

### D3. `ProjectId` moves to T0, re-exported from `project`. **Stage 1.**

`project/identity.rs` is 52 lines and is vocabulary. Moving it into `domain/` and re-exporting
`ProjectId` from its current path removes **all seven** of T0's upward references, and no other
code has to change.

**Predicted effect, to be asserted by D1's gate in the same commit: the largest component goes
from 10 modules to 9**, because `domain` has no other outgoing edge. If it does not, the move was
wrong and the gate says so before a human reads the diff.

The re-export is what keeps `tekstide-core`'s published API unchanged — this stage is
API-compatible by construction, which is why it is first.

### D4. `test_support` leaves the production graph. **Stage 2.**

Two references hold test scaffolding inside the production component: `runtime → test_support`
and `test_support → runtime`, one each. Both go, by the three legal moves of D5. Expected effect:
largest component 9 → 8.

This is listed as its own stage despite being two lines because of what it is: the gate in D1
cannot honestly assert "no module reaches `test_support`" while production code does.

### D5. Capability isolation is absolute. No allow-list. **Decided (goal document D-G3).**

When capability A needs something from capability B there are exactly **three** legal moves, and
no fourth:

1. **The shared thing is vocabulary** → it belongs in T0. (`ProjectId` is the worked example.)
2. **A defines a trait; orchestration supplies B.** The trait is owned by the *consumer* and has
   only the methods that consumer calls — never a mirror of B's API.
3. **The need is really orchestration's** → the logic moves to T2.

An allow-list of permitted T1↔T1 pairs was considered and rejected: it is a list of exceptions,
exceptions grow, and this codebase has already paid for that exact shape once — the two
independently-maintained copies of `IGNORED_DIRECTORY_NAMES` whose own doc comment calls it *"a
defect the day someone edits one without the other."* Each allowed pair is also a place where the
authority rule can be violated with nothing failing.

**The cost accepted, stated plainly**: indirection where a direct call would read more simply, and
a T2 that grows because it becomes the only composer. The second is tolerable precisely because
T2 is the one tier where a reviewer expects breadth; the first is bounded by the
consumer-owned-trait rule, which keeps each trait as small as its single caller needs.

### D6–D8 — moved to RFC-077 by the 2026-11-12 split

The rule that generates the capability list (D6), the `project/` split itself (D7) and the staging
order derived from measured cut cost (D8) are **not in this RFC any more**. **D5 stays**, above:
its first real application is this RFC's own Stage 2, and a release should be able to cite the rule
it applied without naming an RFC that has not shipped. They were accepted on 2026-11-03 as part of it and are carried forward unchanged, as
accepted decisions, in [RFC-077](../accepted/077-the-capability-split.md). Nothing was re-opened, re-decided
or weakened: the split changes which release closes which decision, and nothing else.

What stays here is what `0.40.0` can actually close: the gate, the tier model it enforces, the
isolation rule its second move is the first to exercise, and the two moves whose effect on the graph
is predicted before they land.

## Out of scope, deliberately

- **Not a crate split.** Crates are a packaging decision, available once the properties hold. The
  value is the acyclicity, not the crate count, and `tekstide-core`'s published API argues for
  making any split late and deliberately.
- **Not the GUI's breadth.** Measured, so it is not mistaken for unexamined: `crates/tekstide`
  references **21 distinct `tekstide_core` modules**, `project` 390 times. That is wide, and it is
  a different question from this RFC's — the crate boundary is already clean of platform code, and
  the goal document places T3 outside this goal. Narrowing it needs its own RFC and its own reason.
- **Not a rename.** If a change only moves text, it is not this.
- **Not plugins.** The MVP ships no plugin runtime. This RFC only requires that the seams *could*
  be added without rewriting, which is Appendix A's own MVP position.

## What this buys, named against the requirement that demands it

`REQ-LSP-001..005` (deferred beyond 1.0) — adding language intelligence must not touch thirteen
other capabilities · Appendix A §2.1's internal extension seams — a capability that cannot be
addressed cannot be extended, sandboxed or granted · M13 multi-document (§3.4) · `NFR-PORT-001..003`
and Windows after RFC-028 — platform arms land inside the capability that owns the mechanism
rather than scattered through a hub · `REQ-SEC-023` secret redaction (RFC-068) — a transcript
capability that owns its own format is where a content scanner belongs.

## Acceptance

**What the owner decides**: whether this RFC or the remaining M13/M14 work takes the next release
slot, and whether `tekstide-core`'s published API may change shape in that release. Stage 1 is
API-compatible by construction (D3); the `project/` split may not be.

**Both questions are answered as of 2026-11-12**: `0.40.0`, and yes — see the Status amendment.
The text below is the question as it stood at acceptance, kept rather than rewritten.

**What I am asking to be accepted**: D1 through D8 as written, and Stages 1 and 2 as the first
slice — with the explicit understanding that the later staging is scoped in a handoff after the
gate has proven itself on two real stages, not now.
