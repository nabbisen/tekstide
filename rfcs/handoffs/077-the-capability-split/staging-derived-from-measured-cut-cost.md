---
title: "RFC-077 — staging, derived from measured cut cost"
rfc: "RFC-077"
rfc_file: "../../accepted/077-the-capability-split.md"
source_rfc_status: "Accepted 2026-11-03 by inheritance, split out of RFC-074 on 2026-11-12 — RFC-077 is in rfcs/accepted/"
target_milestone: "M14"
created: "2026-11-15"
---

# Staging, derived from measured cut cost

RFC-077's D8 says staging order is derived from measured cut cost, and its *Acceptance* defers the
enumeration until RFC-074's gate has behaved on two real stages. It has: Stages 1 and 2 landed in
`0.40.0`, each lowering the bound in the same commit as the move, and the gate was ablated at every
bound change. **This is that enumeration.**

Measured 2026-11-15 against `87de992`, with RFC-074's own method — production `.rs` under
`crates/tekstide-core/src`, `tests.rs`/`*_tests.rs`/`tests/` and `bin/` excluded, full-line
comments stripped, one reference per `crate::<module>` occurrence naming another top-level module.
Starting point: **largest SCC 8** — `agent`, `approval`, `audit`, `content`, `project`, `runtime`,
`security`, `transcript` — with 27 edges and 61 references inside it.

## The derived path: seven stages, 22 references, to acyclic

Each row is the cheapest cut (or cheapest coherent group, where no single cut shrinks the
component) that lowers the bound by one, computed by removing the edge and re-deriving the largest
component:

| Stage | Bound | Cut | Refs | Where |
| --- | --- | --- | --- | --- |
| 3 | 8 → 7 | `security -> project` | **1** | `security.rs` |
| 4 | 7 → 6 | `content -> project` | 4 | `content/{document,open,save,snapshot}.rs` |
| 5 | 6 → 5 | `approval -> audit` | 5 | `approval/coordinator.rs` |
| 6 | 5 → 4 | `approval -> project` **and** `approval -> agent` | 2 | a coherent group: both of `approval`'s remaining reaches out |
| 7 | 4 → 3 | `project -> agent` | 3 | |
| 8 | 3 → 2 | `transcript -> runtime` **and** `transcript -> project` | 4 | a coherent group |
| 9 | 2 → 1 | `runtime -> project` | 3 | |

**Stage 3 is one reference in one file.** Stage 6 is the first place D8's *"one cut or one coherent
group"* clause is load-bearing: at a component of 5 (`agent`, `approval`, `project`, `runtime`,
`transcript`) **no single cut shrinks it at all** — measured, all fifteen inner edges tried. The
cheapest pair is both of `approval`'s outgoing edges, two references, which is also the most
coherent reading of the move: `approval` stops reaching into `project` and `agent` together, or not
at all.

Alternatives at each step were computed and the cheapest taken; at stage 3 the other two candidates
were `content -> project` (4) and `approval -> audit` (5), which is why they become stages 4 and 5
rather than disappearing.

## The finding that matters more than the table

**Twenty-two references is small.** Stages 3 through 9 together are about the size of a single
ordinary slice — smaller, in references, than PR-074-B's own two stages were in files touched. The
multi-release problem this reviewer flagged when the split was made may simply not arise: **if
stages 3–9 land in one release, RFC-077 closes in that release and never has to be named by a
section it did not ship in.** That should be checked against real effort before being relied on,
but plan for one release and split only if the work argues for it.

## And the finding that should stop someone declaring victory at stage 9

**After stage 9 the graph is acyclic, the bound is 1, and D7 has not started.**

D7 is *"`project/` splits: orchestration plus its capabilities."* `project/` is **9,077 production
lines** across 17 entries today, `session.rs` alone 2,696. Cutting 22 references makes the module
graph acyclic; it does not split `project/` into anything. **Splitting an already-acyclic module
into siblings does not lower the largest-SCC bound** — it cannot, because the bound is already 1.

So RFC-074's ratchet, which has been the whole evidence mechanism since D1, **stops being evidence
exactly where D7's work begins.** Whoever scopes D7 must bring a second measurement with it — a
property the gate can hold that a `project/` split actually moves — or D7 lands with no witness,
which is the condition D1 was written to make impossible. This is not a reason to defer D7; it is
the thing to settle before starting it.

## Standing requirements carried from RFC-074

- **Each bound move re-ablates** (review 672's requirement, applied at every stage of PR-074-B):
  plant a reference that should fail the *new* bound, confirm red naming the module, restore.
  Lowering a bound makes the assertion stricter; it does not re-prove the measurement underneath
  still discriminates.
- **The bound moves in the same commit as the move that earns it.** A commit that cuts the edge
  without lowering the bound has not landed the stage; it has made the gate lie by one.
- **A stage that cannot lower the bound is not a stage** — fold it into one that can, or report
  that the measured cut cost was wrong.
- **Every disposition says which of D5's three legal moves it used, per reference** — and where the
  premise does not hold, says so rather than picking a label that misdescribes it, as PR-074-B's
  own B2 did for a test-only pair.

## What is not in this document

Which release these take, how many slices they become, and D7's own scoping. The first two are the
owner's and the reviewer's; the third needs the second measurement named above before it can be
written honestly.
