# RFC-074: Capability Boundaries

Status: **Proposed 2026-11-03.** Approved to follow RFC-028 by the human owner (review 595), and
drafted only after RFC-028 closed and `0.38.0` shipped, as directed. The goal, the tier model and
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

### D6. The capability list is generated by a rule, not chosen by taste. **Decided (goal document D-G2).**

> **One capability = one external contract + one grant decision + one threat surface.**

Where any of the three differ, the capabilities are separate; where all three coincide, merging is
correct. Applied, this keeps filesystem safety, file watching and the explorer as **three**
(different external dependencies — none / `notify` / the `git` subprocess; different threat
surfaces; and decisively different Appendix A grant classes — `Read`, `Process`, and host safety
that is never granted at all). Merging them would mint exactly the broad capability Appendix A
§6.1 forbids.

### D7. `project/` splits: orchestration plus its capabilities. **Decided (goal document D-G1).**

Not because the hub is untidy, but because a hub is where safety invariants stop being locatable.
The cost of this split only ever rises — `project/` is ~9,041 production lines today, and every
capability added after 1.0 attaches to the hub.

### D8. Staging order is derived from measured cut cost, not from preference

After Stage 1, the cheapest single cuts that each take the component from 9 to 8, measured:

| Cut | References to remove |
| --- | --- |
| `runtime → test_support` | 1 |
| `security → project` | 1 |
| `test_support → runtime` | 1 |
| `content → project` | 4 |
| `approval → audit` | 5 |

Each stage: one cut or one coherent group, the gate's bound lowered in the same commit, the full
suite and both platforms green, and the six `ARCHITECTURE.md` invariants intact. **A stage that
cannot lower the bound is not a stage** — it is a move that should be folded into one that can.

The later stages (the `project/` split proper) are not enumerated here. They will be, in a
handoff pack, once Stages 1–2 have shown the gate behaves and the cut costs above survive contact.
Scoping twelve stages before the first one lands would be the kind of plan this project's own
history says gets rewritten.

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

**What I am asking to be accepted**: D1 through D8 as written, and Stages 1 and 2 as the first
slice — with the explicit understanding that the later staging is scoped in a handoff after the
gate has proven itself on two real stages, not now.
