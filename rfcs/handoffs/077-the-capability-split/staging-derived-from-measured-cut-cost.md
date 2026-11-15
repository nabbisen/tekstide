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
| 6 | 5 → 4 | `approval -> project` **and** `approval -> agent` | 2 | **blocked — see "Stage 6 does not happen as scoped" below** |
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

## Stage 6 does not happen as scoped (2026-11-16, review 682)

The dev team held stage 6 rather than forcing it, which is RFC-077's own rule, and the finding is
correct. **One of stage 6's two references cannot be cut in any shape without weakening a
documented security guarantee.**

`approval -> agent` is the single import `use crate::agent::VerifiedCwd;`. That type exists so that
`approval::coordinator::receive_proposal` cannot typecheck against an *unverified* path — concretely
`CommandProposal::cwd()`, an adapter's own untrusted claim — and the guarantee rests on its
constructor being private to `agent/launch.rs`, which holds its one call site, immediately after the
containment check. All three of D5's moves were tried and none fits:

- **Move 1** (type → T0) does not compile. `pub(in crate::agent)` on an item defined under
  `crate::domain::agent` is **E0742**: visibilities restrict only to *ancestor* modules, and the two
  are siblings. Reproduced in isolation on rustc 1.100.0 in a scratch crate containing none of this
  project. The only visibility that compiles is `pub(crate)`, which takes minting from one file to
  the whole crate.
- **Move 2** (consumer trait, provider impl) is *mechanically* available under RFC-074 D5 as amended
  — the impl would sit on `agent -> approval`, an edge that already exists. It fails because a trait
  moves the privilege from *who may construct* to *who may implement*, which inside one crate is no
  narrower; and sealing it requires `approval` to implement the seal for `crate::agent::VerifiedCwd`,
  re-creating the edge being cut.
- **Move 3** (need is orchestration's) has nothing to relocate: verification already happens in the
  caller, before `approval` sees the value.

Two further shapes were searched and are also dead. Moving the type **together with its minting
check** into `domain::agent` compiles and keeps minting file-local, but turns the containment root
into a caller-supplied `&Path`, so `approval` itself could mint a token from `proposal.cwd()` — the
same laundering, now reading as safe code. Typing that parameter as `ProjectRootHandle` to make the
root's provenance unforgeable fails because the handle lives in `project/root.rs` and is minted only
from a `&ProjectSession`, which can never be T0.

**The generalization, which closes the question rather than leaving it open:** in a single crate,
*"only code under subtree X may mint this value"* is expressible only when the type is **defined**
inside X. `approval` must name the type and `agent` must own the minting, so the only construction
satisfying both is a dependency from `approval` to `agent`. **This edge is not a reference that
resists D5; it is the structural expression of the guarantee.** No fourth shape exists.

### What each answer actually costs, measured

Both halves of stage 6 are necessary and neither is sufficient — dropping either alone leaves the
component at 5. Carrying each answer through stages 7–9:

| | after 6 | +7 | +7,8 | +7,8,9 |
| --- | --- | --- | --- | --- |
| `pub(crate)` accepted, stage 6 lands | 4 | 3 | 2 | **1 (acyclic)** |
| `approval -> agent` kept as a residual | 5 | 5 | 4 | **4** |

The residual floors at `{agent, approval, project, runtime}`, and **stage 7 becomes a complete no-op
on the bound.** Cutting `agent -> approval` from the other side of the 2-cycle does not help (still
4), and no 1- or 2-edge cut anywhere in that component reaches acyclic; at least three further cuts
would be needed, none of them scoped here. Note also that `agent -> approval` counts as one
reference only because the counter counts `crate::X` occurrences and that one is a grouped `use`
naming **six** symbols — the reference counts order the stages and were never what judges one
complete, so the bound derivation is unaffected, but that row is not cheap.

**What the residual forfeits is smaller than it looks, and it is this document's target, not the
RFC's.** RFC-077 nowhere decides acyclicity: D6 is the capability rule, D7 the `project/` split, D8
staging-by-measured-cost. "To acyclic" is this document's heading — and the section below already
records that the bound **cannot witness D7 anyway**. D7 does not require an acyclic graph to proceed.

**Reviewer's recommendation: keep the edge, record it as a named residual.** A documented,
threat-model-anchored construction privilege outranks an internal graph metric that is invisible to
users, is not an accepted decision, and stops being evidence exactly where the remaining work
begins. If `pub(crate)` is accepted instead, it needs a compile-time witness — a test asserting
exactly one `from_validated` call site — and `approval/coordinator.rs`'s module doc corrected, since
its claim that the laundering is *structurally* unreachable becomes false.

**Decision pending with the owner.** Until it is answered, stage 7 should not start: under the
residual it moves nothing and must be re-derived first. Independent of the answer, the other half of
stage 6 — `crate::project::ProjectId` → `crate::domain::ProjectId` in `approval/coordinator.rs:71`
— lands on its own as housekeeping (60 edges / 139 refs, bound unchanged at 5), committed as a
reference removed and not as a stage.

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
