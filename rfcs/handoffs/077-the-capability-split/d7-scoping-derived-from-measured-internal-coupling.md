---
title: "RFC-077 D7 scoping — `project/` splits, derived from measured internal coupling"
status: "Scoped 2026-11-18 by the reviewer, at the owner's direction; **authorized by the owner the same day** — the instrument refinement recommended in §5 is taken, API compatibility is kept. §5 and §6 were then corrected by the reviewer before handoff: reference weight alone is not a witness every stage can move. No slice begun; the release slot remains the owner's."
rfc_file: "../../accepted/077-the-capability-split.md"
target_milestone: "M14"
created: "2026-11-18"
---

# D7 scoping: `project/` splits into orchestration plus its capabilities

D7 is *"`project/` splits: orchestration plus its capabilities"*, decided on RFC-077's acceptance
and never scoped, because the staging document parked it behind one explicit blocker: *"D7's own
scoping… needs the second measurement named above before it can be written honestly."* That
measurement landed in reviews 686–694 (reference weight, not edge count: `project` 7 edges / **37
refs** / in-degree 5, against `app`'s 8 / 17 / 2). This document is the scoping it unblocks.

**Everything below is measured, not estimated.** The scripts are a Tarjan/reference scan over
`crate::<module>` occurrences (the gate's own method, reimplemented separately) plus a resolved
intra-module import graph. Where a number here disagrees with an earlier document, this one was
taken on 2026-11-18 against `4e5197e`.

## 1. What is actually in `project/`

23 production files, **6,074 lines of code** (comment and blank lines excluded; the staging
document's "~9,077" counts every line). Clustered by the shape the code already has:

| Cluster | Files | Code LOC | Share |
|---|---|---|---|
| **`session`** | `session.rs`, `session/run_records.rs` | **2,270** | 37% |
| `content` | `content.rs`, `diff.rs` | 986 | 16% |
| `root` | `root.rs`, `root/explorer.rs` | 773 | 13% |
| `change_detection` | `change_detection.rs` | 594 | 10% |
| `recent` | `recent.rs` + 4 | 490 | 8% |
| `watch` | `watch.rs` + 4 | 434 | 7% |
| `explorer_tree` | `explorer_tree.rs` | 273 | 4% |
| `metadata` | `metadata.rs` | 170 | 3% |
| `project.rs` (façade) | `project.rs` | 70 | 1% |
| `runtime`, `ignored_directories` | 2 files | 14 | <1% |

Plus `root/explorer/hostile_fixture.rs`, 127 LOC, `#[cfg(test)]`-gated but **scanned as production**
by the gate (its filename matches no exclusion rule) — a live instance of the blind spot
`no_production_file_declares_a_module_with_a_body` cannot see. Harmless today: it contains zero
`crate::` references, so it perturbs no measurement. Excluded from `root`'s 773 above and named here
so the next person counting does not count it twice.

**`session` is 37% of the module and the only file that reaches seven external modules at once**
(`agent:4, domain:4, runtime:2, transcript:2, approval:1, close:1, content:1`). Every other file
reaches at most two. That asymmetry is the hub D7 exists to remove, and it is also why the split is
tractable: the hub is one file, not a diffusion.

## 2. The internal coupling, resolved

Importer → definer, resolved by name through `use super::`/`use crate::project::`/`use self::`:

| From | To | References |
|---|---|---|
| `content` | `root` | 9 |
| `explorer_tree` | `root` | 9 |
| `session` | `content` | 5 |
| `session` | `metadata` | 5 |
| `content` | `explorer_tree` | 4 |
| `session` | `change_detection` | 4 |
| `content` | `change_detection` | 3 |
| `session` | `root` | 2 |
| `watch` | `root` | 2 |
| `session` | `explorer_tree` | 1 |
| `session` | `runtime` | 1 |
| **`change_detection`** | **`session`** | **1 import, 12 signatures** |
| **`root`** | **`session`** | **1 import, 1 constructor** |

Out-degree / in-degree by cluster: `session` 6/2 · `content` 3/1 · `root` 1/**4** · `explorer_tree`
1/2 · `change_detection` 1/2 · `watch` 1/0 · `metadata` 0/1 · `runtime` 0/1 · **`recent` 0/0** ·
`ignored_directories` 0/0.

Three things follow, and none of them was obvious before measuring:

- **`root` is the internal base, not `session`.** In-degree 4 (`content`, `explorer_tree`,
  `session`, `watch`), out-degree 1. A capability others build on.
- **`recent` and `ignored_directories` are already detached.** Zero coupling in either direction.
  They can leave with no cut at all.
- **The split is blocked by exactly two back-edges into `session`** — `change_detection → session`
  and `root → session`. Everything else is already a DAG. **This is the whole cost of D7's first
  stage**, and it is why a naive split must not be attempted: `session → change_detection` and
  `change_detection → session` is a 2-cycle, as is the pair with `root`. The gate's bound is **0**
  with `==`, so the first naive stage would red it immediately.

## 3. What the two back-edges actually want — and why one move cuts both

Measured, not inferred from the signatures' shape:

- **`change_detection` reads exactly two things from `ProjectSession`**: `canonical_root_path()`
  (11 call sites) and `id()` (7). Nothing else, across all 12 signatures.
- **`root::ProjectRootHandle::from_project_session` reads four**: `id()`, `display_name()`,
  `root_path()`, `canonical_root_path()`.

The union is four values, and they are all the same kind of thing: **which project this is, and
where it is rooted.** That is not orchestration state — it is the project's own identity
vocabulary, which makes this **D5 move 1** (vocabulary to T0), the cheapest of the three legal
moves, rather than move 2 applied twice.

**Directed shape: one new `domain` type, not two consumer traits.** A `ProjectRootIdentity`
(or similar) carrying id, display name, selected path and canonical path, defined in `domain`,
held by `ProjectSession`, and handed to both consumers in place of `&ProjectSession`. Compared with
the two-trait alternative (which has precedent here — `AgentRunLaunchProject` and
`TerminalLaunchProject` are both consumer-defined traits implemented on `ProjectSession` at
`session.rs:135` and `:154`):

- one new type instead of two traits, and both back-edges fall at once;
- it composes — the next capability needing "which project, rooted where" takes the same value
  rather than defining a third trait;
- `change_detection`'s 12 signatures lose their `&ProjectSession` parameter entirely rather than
  gaining a generic parameter.

**Not assumed, and flagged for the implementing slice rather than decided here:** whether
`canonical_root_path` inside that type needs a *minting* restriction. It is security-relevant —
`validate_launch_spec` enforces `cwd.starts_with(root)` against it — so it is in the same family as
`VerifiedCwd` (ARCHITECTURE.md's seventh invariant), but `VerifiedCwd`'s guarantee comes from a
private constructor in the subtree that validates, and this value is produced by whoever opens the
project. Whether that needs the same treatment is a real question with a real answer, and it is not
answered by this measurement. **Do not give the type a public constructor without settling it.**

## 4. The derived shape

Once those two back-edges are cut, the internal graph is a DAG and the split reads off it:

- **T0, vocabulary (to `domain`):** the project identity/root type above.
- **Capabilities** (each with its own external contract, grant decision and threat surface, per D6):
  `root` (the base), `explorer_tree`, `content` (+`diff`), `change_detection`, `watch`, `recent`,
  `metadata`.
- **Orchestration (stays `project`):** `session` (+`run_records`), `runtime`, `ignored_directories`,
  and the façade.

`session` keeps its seven external edges, because they are what orchestration *is* — it is the one
place entitled to name `agent`, `runtime`, `approval`, `transcript`, `content` and `close` together,
which is precisely what Stages 8 and 9 established by putting the terminal-spec assembly and the
approval binding there. D7 does not reduce that; it removes everything that is *not* that.

## 5. The gate property D7 needs — and the façade interaction, which inverts the obvious metric

The SCC bound is 0 and cannot witness D7: an acyclic module does not become more acyclic by
splitting. The property that *can* be held is reference weight — but **not** the way it first reads,
and getting this wrong would make the gate reward keeping the hub.

- **Total references rise by construction when a module splits.** `content → root`'s 9 internal
  references are invisible to the gate today (`super::`, inside one top-level module). Move `root`
  out and they become 9 `crate::root` references. The crate-wide total is therefore the **wrong**
  metric: D7 makes it worse, necessarily.
- **So the bound must be per-module: no single module's outgoing reference weight exceeds N**,
  ratcheted downward as each capability leaves, exactly as `MAX_LARGEST_SCC_SIZE` was. Today's
  worst is `project` at 37; `app` is next at 17.
- **And the façade must be excluded, or stage 1 reds the bound it is meant to lower.** Every
  cluster is currently re-exported from `project.rs` (`pub use change_detection::{…}`, `pub mod
  recent`, …). Keeping API compatibility — which this RFC has done at every prior stage, three times
  in `project.rs`'s own comments ("re-exported from here unchanged so no caller and no published-API
  consumer has to move with it") — means `project.rs` gains a `pub use crate::root::{…}` line per
  departing cluster. The gate's `crate_path_references` counts `pub use` exactly like `use`, so
  **`project`'s measured weight would go up as its code goes out.**

**Recommendation: refine the instrument, do not break the API. — Authorized by the owner,
2026-11-18.** Count only non-`pub use` references toward the per-module weight bound. A pure
re-export is not a dependency — the façade names a module without reaching into it — and this
project has held API compatibility through nine stages; inverting that priority for a metric's
convenience is the wrong trade. The refinement is small and must carry its own positive control, in
the shape review 686 established: assert that a known façade line is excluded **and** that a known
real reference is still counted, so the exclusion cannot silently widen into "nothing counts".

Measured, so the bound has a real starting value: `project` carries **37** references, **4** of them
façade, so **33 in its own code**. The next-densest module is `app` at 17, all own-code.

### 5a. Correction before handoff: reference weight cannot be D7's primary bound

Written into this document an hour after §5 and §6, by the reviewer, from doing the arithmetic
RFC-074's own standing rule demands ("a stage that cannot lower the bound is not a stage") against
the metric §5 had just recommended. **It does not survive that test.** Per-stage, with the
capabilities' own external references counted from §1's inventory:

| Stage | `mod` declarations in `project` | `project`'s own-code refs | refs moved |
|---|---|---|---|
| today | 11 | 33 | — |
| D7.2 `recent` | 10 | 32 | 1 |
| D7.3 `watch` | 9 | 32 | **0** |
| D7.4 `root` + `explorer_tree` | 7 | 27 | 5 |
| D7.5 `content` + `diff` | 5 | 25 | 2 |
| D7.6 `change_detection` | 4 | 24 | 1 |
| D7.7 `metadata` | 3 | 23 | 1 |

`watch` carries **no** external references at all, so a correct, complete stage moves the weight by
zero and would be "not a stage" under the standing rule. The cause is structural, not incidental:
**`session` holds 17 of the 33, and `project.rs`'s own `read_transcript_window` body holds 5** — the
capabilities leaving were never what made the hub dense, so a metric built on their references
cannot witness them leaving.

**So the primary bound is the count of capability submodules `project` declares** — 11 today, 3 when
D7 is done. It falls by construction with every stage, it is exact, and it cannot be satisfied by
anything other than a capability actually leaving: a `pub use crate::recent;` façade line is not a
`mod` declaration, so compatibility costs the metric nothing. **Reference weight stays as a
secondary guard with the opposite polarity — it must not *rise***, which is the recurring finding of
stages 6 through 9 (the gate rewarded lowering the SCC bound by making the hub denser, three stages
running). Together: one bound that every stage must lower, and one that no stage may raise.

**The alternative is the owner's to take if preferred:** drop the façade, accept a published-API
break, and keep the instrument unchanged. RFC-077's own D7 note already reserves this ("The
`project/` split may not be API-compatible — a separate question from the one answered for
`0.40.0`"). This measurement narrows that note: compatibility is *achievable* at every stage, so the
question is no longer "can it be compatible" but "is a façade worth an instrument refinement."

## 6. Staging, derived from the measured cut cost

Each stage must lower the new per-module bound and move it in the same commit, per RFC-074's
standing rule.

| Stage | Move | Cut cost | Why here |
|---|---|---|---|
| **D7.0 — the instrument, not a move** | Add both bounds of §5a to `capability_graph_gate.rs`: capability-submodule count at **11**, `project`'s own-code reference weight at **33** (façade-excluded, with the positive control §5 requires) | no production change | RFC-074's own precedent, in its delivery-plan row's words: *"the gate was built before any move and its bound ratcheted with the stage that earned it."* D7 has no witness until this lands |
| **D7.1 — a prerequisite, not a stage** | The identity type to `domain`; `change_detection` and `root` stop naming `ProjectSession` | 2 imports, 13 signatures, 4 accessors | Nothing else can start: both back-edges must fall before any capability leaves, or the first departure is a 2-cycle against a bound of 0. **It lowers neither bound** — the back-edges are `super::` references the gate cannot see, and no submodule leaves — so it is declared a prerequisite rather than dressed as a stage, which is what the standing rule requires of it |
| **D7.2** | `recent` out | **0 internal references** | Already detached in both directions. The cheapest possible first departure, and the one that proves the façade-plus-refined-bound mechanism on a cluster where nothing else can go wrong |
| **D7.3** | `watch` out | 2 (`watch → root`) | Out-degree 1, in-degree 0. One surviving edge to a module that has not moved yet, which is a DAG edge, not a cycle |
| **D7.4** | `root` + `explorer_tree` out, together | 9 + 9 inbound from `content`, 2 from `session` | `root`'s in-degree is 4; splitting it from `explorer_tree` (9 references, one direction) buys nothing and doubles the churn |
| **D7.5** | `content` (+`diff`) out | 5 from `session`, 3 to `change_detection` | Depends on three clusters, so it goes after them |
| **D7.6** | `change_detection` out | 4 from `session` | Last of the capabilities; its own back-edge is already gone from D7.1 |
| **D7.7** | `metadata` out, or stay | 5 from `session`, 0 out | 170 LOC of vocabulary-shaped state. Measure whether it is a capability or T0 vocabulary before moving it; this scoping does not decide it |

`ignored_directories` (1 LOC) and `runtime.rs` (13) stay with orchestration — a module boundary
around 14 lines is overhead, not a capability.

**Why this order and not by size:** every stage must leave the graph acyclic at a bound of 0, so the
order is forced by the measured dependency direction, not chosen. Each stage from D7.2 lowers the
capability-submodule bound by exactly the number of declarations it removes (§5a's table), which is
what makes each one a stage under the standing rule. D7.2 before D7.3 before D7.4 is
the only sequence in which each departing cluster's dependencies have either already left or are
still inside `project` — never half-out.

## 7. What this scoping does not decide

- **The release slot.** The owner's. `0.41.0` is open; nothing here gates it.
- **Façade versus API break** (§5). The owner's, with my recommendation stated.
- **Whether the identity type needs a minting restriction** (§3). A real question for D7.1's own
  review, not answerable from coupling measurements.
- **Whether `metadata` is a capability or vocabulary** (D7.7). Needs its own measurement.
- **How many slices each stage becomes.** The dev team's, from D7.1's actual churn.

## 8. Standing requirements carried from RFC-074/077

- Each bound move re-ablates: plant a reference that should fail the **new** bound, confirm red
  naming the module, restore. A stricter bound is not a re-proved measurement.
- The bound moves in the same commit as the move that earns it.
- A stage that cannot lower the bound is not a stage.
- Every disposition names which of D5's three legal moves it used, per reference.
- Every commit touching `crates/tekstide-core/src/` reports total edges and total references,
  before and after (review 687) — **and, from D7.0, both of §5a's bounds**, since those are what
  now move.

## 9. D7.0's brief, directed 2026-11-18 — the instrument, and nothing else

The dev team's first read of this document was of `3ccde61`, before §5a and §6's correction landed in
`8337778`. Three things changed, and all three change what to build first: the façade-versus-API
question **is decided** (refine the instrument, keep compatibility — the owner, 2026-11-18); the
primary bound is **not** reference weight; and **D7.0 exists**, before D7.1.

**Scope: `crates/tekstide-core/tests/capability_graph_gate.rs` only. No production change.** Two new
bounds, with opposite polarities, plus their controls.

### Bound 1 — capability submodules, the one every stage must lower

`project.rs` declares **11** production submodules (lines 1–11: `change_detection`, `content`,
`diff`, `explorer_tree`, `ignored_directories`, `metadata`, `recent`, `root`, `runtime`, `session`,
`watch`). **Off-by-one to avoid: a naive count returns 12** — `#[cfg(test)] mod tests;` at line 104
is the twelfth, and must be excluded, the same `#[cfg(test)]`-in-a-production-file class that has
now bitten this gate twice (review 675's inline test modules, review 687's accessor). Exclude it by
the `#[cfg(test)]` attribute on the preceding line, not by the module's name.

`MAX_PROJECT_CAPABILITY_SUBMODULES = 11`, asserted `==`, with the same reasoning the SCC bound's own
`==` carries: a shrink without the bound moving in the same commit is as much a lie as a growth.
Falls to 3 when D7 is done (`session`, `runtime`, `ignored_directories`).

### Bound 2 — own-code reference weight, the one no stage may raise

Measured today, and these three numbers are the control, not just the input: `project` carries **37**
`crate::<module>` references, **4** of them on `pub use` lines, **33** in its own code. The
next-densest module is `app` at **17**, all own-code.

- Façade rule: a reference on a line whose trimmed start is `pub use ` or `pub(crate) use ` is
  excluded. A pure re-export names a module without reaching into it.
- Assert the **maximum over all modules** of own-code references is `<= 33`. It must not fall as a
  condition of anything — D7.3 (`watch`) correctly moves it by zero — but no stage may raise it,
  which is the guard against stages 6–9's recurring failure of lowering one bound by densifying the
  hub.

### Controls, required, in review 686's shape

The exclusion must not be able to widen silently into "nothing counts":

1. Pin all three of `project`'s numbers separately — 37 total, 4 façade, 33 own-code — so the
   subtraction itself is measured rather than trusted.
2. Assert a known façade line **is** excluded and a known own-code reference **is** still counted.
   `project.rs` has both on hand: its three `pub use crate::domain::…` lines, and
   `read_transcript_window`'s body, which names `crate::transcript` and `crate::runtime` in real
   code.
3. Re-ablate both bounds on the committed tree via `rfcs/handoffs/ablate.sh` (it refuses a dirty
   tree; never hand-restore): for bound 1, plant a twelfth production submodule declaration and
   confirm red naming it; for bound 2, plant an own-code `crate::` reference in `project` and
   confirm red. Restore and confirm byte-identical.

### What D7.0 is not

Not a move, and it lowers nothing — that is the point. RFC-074's own precedent, from its
delivery-plan row: *"the gate was built before any move and its bound ratcheted with the stage that
earned it."* D7.1 follows and is a **prerequisite, not a stage**: it cuts two `super::` back-edges
the gate cannot see and moves no submodule, so it lowers neither bound, and must be reported that
way rather than dressed as a stage.

Report both bounds' values before and after, per the standing requirement — from here that means
four numbers, not two.

## 10. §3 corrected before implementation (review 696, 2026-11-18)

§3 directed "one new `domain` type carrying id, display name, selected path and canonical path,
cutting both back-edges at once." **Three measurements taken while answering review 696's minting
question overturn it**, and the corrected shape is smaller:

1. **No new type needs inventing.** `ValidProjectRoot` (`project/root.rs:27`) already carries three
   of the four values; `ProjectId` is the fourth.
2. **`ValidProjectRoot` already names a validation it does not enforce.** Its fields are `pub`, and
   `ProjectRootHandle::from_project_session` (`root.rs:40-48`) builds one **literally from the
   session's accessors**, bypassing `ProjectRootValidator`'s own sanctioned mint at `root.rs:311`.
   Pre-existing, in ARCHITECTURE.md's seventh-invariant family — and §3's move would have spread it
   into `domain`. Cost to close: **two production construction sites, both inside `root.rs`**; none
   in `crates/tekstide` (the GUI only obtains one from the validator); tests go through the validator
   or take a `#[cfg(test)]` constructor, as `VerifiedCwd::for_test` already does. The bypass is the
   same line D7.1 must touch anyway.
3. **Relocating it to `domain` and giving it a minting restriction are mutually exclusive** — Stage
   6's own finding: `pub(in crate::project::root)` on an item defined under `crate::domain` is
   **E0742**, the error that made `approval -> agent` permanent and became the seventh invariant.

**Corrected D7.1.** The four values are not all one thing, which §3 flattened:

- **`change_detection` reads exactly two** — `canonical_root_path()` (11 sites), `id()` (7) — and
  validates nothing. Give it a T0 vocabulary type in `domain` carrying **those two only**, public
  fields, a name claiming no validation (**not** one containing "Valid"). Move 1, no privilege, no
  E0742 exposure.
- **`root`'s back-edge needs no type — invert the constructor.** `session` calls a
  `ProjectRootHandle` constructor taking what `from_project_session` already reads, so `root` never
  names `ProjectSession`; if that constructor takes a `ValidProjectRoot` only the validator can
  mint, the bypass closes in the same edit. **`ValidProjectRoot` stays defined in `project::root`**,
  the only place it can carry its own guarantee.

Net: one small vocabulary type, one inverted constructor, one bypass closed, both back-edges cut, no
privileged type relocated — less movement than §3 proposed, and it keeps a guarantee §3 would have
lost. Published-API note for D7.1's own request: privatizing `ValidProjectRoot`'s fields is a change
for any downstream constructor (none in this workspace), and the owner's compatibility decision
covers re-exports, not field privacy.

## 11. D7.0 landed (`5b59ffa`, reviewed at 697) — the witness exists

Both bounds are in `capability_graph_gate.rs`, instrument-only, no production change. Gate 13/13;
CI green. The four numbers, re-measured independently by a script sharing no code with the gate:
`project` **total 37 / own-code 33 / façade 4**, `project` the densest own-code module, and
`project.rs` declaring **11** production submodules with one `#[cfg(test)]`-gated declaration
excluded.

- `MAX_PROJECT_CAPABILITY_SUBMODULES = 11`, asserted `==` — every D7 stage must lower it.
- `MAX_OWN_CODE_REFERENCE_WEIGHT = 33`, asserted `<=` over the maximum of all modules — no stage may
  raise it, and `watch` correctly moves it by zero.

**Three facts about this instrument that are load-bearing and were not obvious from building it**
(review 697):

1. **The pin is part of the ratchet.** `projects_three_reference_numbers_are_pinned_before_the_facade_exclusion`
   fixes 37 / 33 / 4, and **D7.2 moves all three** (`recent/timestamp.rs` carries one
   `crate::domain` reference out; the façade gains a line). A red there is the pin working, not a
   regression — the cheapest-looking fix, deleting it, is the wrong one.
2. **Bound 2's per-line scan is sound only because review 687's guard exists.** It counts
   `crate::` references line by line, where `measure_capability_graph` counts them over the whole
   file; a wrapped `crate::` would be invisible to both, and cannot exist because
   `every_crate_path_occurrence_names_a_known_top_level_module` reds on it.
3. **The pin is what saves bound 2 from vacuity.** The ceiling takes
   `max_by_key(..).unwrap_or_default()`, so a blind instrument yields `0 <= 33` and passes — the
   same vacuity the SCC bound has at 0. The pin's `own == 33` is the positive control that catches
   it.

**Ablation precedent set here:** bound 1's ablation needs two files (the `mod` line and its target),
which `ablate.sh` cannot express, so it was run in a detached `git worktree` instead — stronger than
patch-and-restore, since the main tree is never touched and there is nothing to restore wrongly. Use
a worktree whenever an ablation needs more than one file.
