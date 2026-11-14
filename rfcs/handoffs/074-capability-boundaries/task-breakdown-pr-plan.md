---
title: "RFC-074 — task breakdown and PR plan"
rfc: "RFC-074"
rfc_file: "../../done/074-capability-boundaries.md"
source_rfc_status: "Implemented and closed 2026-11-14, as part of the 0.40.0 release, published 2026-11-14 — D1 through D5, the first slice; D6–D8 carried forward to RFC-077 — RFC-074 is in rfcs/done/"
target_milestone: "M14"
created: "2026-11-03"
---

# Task breakdown and PR plan

**Two slices in this first pass, A → B, and A is not a code move.** The RFC's accepted scope is
D1–D8 plus Stages 1 and 2; later stages are deliberately unscoped until the gate has behaved on
two real ones. This file covers only A and B. Do not extend it.

## PR-074-A — the gate, before anything moves

**Deliverable**: one test in `tekstide-core` that derives the module graph from the source and
asserts three bounds. Nothing else in this slice. No module moves, no `use` is rewritten.

What it asserts, in this order:

1. **The largest strongly connected component is at most 10 modules.** Today's measured value, so
   this slice is green on landing and the bound is a ceiling, not a goal.
2. **`domain` has zero outgoing cross-module references.** Fails today — **so it is written
   `#[ignore]`d with the reason, and PR-074-B un-ignores it.** That is deliberate: the assertion
   that proves Stage 1 must exist before Stage 1, or Stage 1's own claim has no witness.
3. **No production module reaches `test_support`.** Also fails today; same treatment, un-ignored
   by the slice that earns it.

**The method is fixed here and must match the RFC's own, because the RFC's numbers were measured
with it**: production `.rs` files under `crates/tekstide-core/src` only (`tests.rs`, `*_tests.rs`
and `tests/` excluded), full-line comments stripped, an edge counted for each occurrence of
`crate::<module>` naming a top-level module other than the file's own, 21 top-level modules
(directories plus top-level `.rs` files, `bin/` excluded). **If your implementation measures
something else, the numbers in the RFC stop being checkable against it** — which is the whole
point of the gate — so if you think the method is wrong, say so and stop rather than silently
using a better one.

Expect the measured SCC to be exactly `agent`, `approval`, `audit`, `content`, `domain`,
`project`, `runtime`, `security`, `test_support`, `transcript`. If your run disagrees with that
membership, that is the finding, not a number to adjust to.

**Where it lives**: `crates/tekstide-core`'s own `tests/`, not in `tekstide`. The thing measured is
`tekstide-core`'s internal graph, and the crate that owns a property owns its test — the same rule
every capability in the RFC's D2 is about. Note this is the first integration test in that crate;
`crates/tekstide-core/tests/` does not exist yet.

**Not a `cfg(test)` module inside a source file.** It reads the crate's own source text from disk,
so it must not be inside the thing it reads.

## PR-074-B — Stage 1 and Stage 2, each with its bound lowered in the same commit

### B1. `ProjectId` moves to T0

Move `project/identity.rs` (52 lines) into `domain/`, and re-export `ProjectId` from its current
path so no caller and no published-API consumer changes. Then delete the seven
`use crate::project::ProjectId;` lines in `domain/` and import locally instead.

**The seven, complete** — `ownership.rs:1`, `audit.rs:6`, `terminal.rs:2`, `approval.rs:2`,
`transcript.rs:2`, `changeset.rs:2`, `agent.rs:6`. Re-grep before you start: if the count is not
seven, the tree moved under us and the stage needs re-measuring, not patching.

In the same commit: **lower the gate's bound from 10 to 9, and un-ignore assertion 2.** Both are
the stage's evidence. A commit that moves the file without moving the bound has not landed
Stage 1; it has made the gate lie by one.

### B2. `test_support` leaves the production graph

Two references: `runtime → test_support` and `test_support → runtime`, one each. Both go by one of
D5's three legal moves — and **say which of the three you used, per reference.** "Moved it" is not
a disposition here; the RFC's isolation rule is the thing being exercised for the first time, and
how it is exercised on two trivial references is the precedent for how it is exercised on forty.

**Amended 2026-11-14, review 675, after the stage landed: the question above rests on a premise that
does not hold here, and the right answer is the one the round gave — "neither of the three, and
why".** D5 governs what one *capability* may do when it needs something from another; both ends of
both references are test-only code, which is not a capability pair D2's tier model speaks about at
all. So the standard here is not "pick one of three labels" but **state the disposition and why it
is structurally right** — which is what D5's three moves exist to enforce in the cases they do
cover. A later stage between two real capabilities is back under the original wording, unchanged.

In the same commit: **bound 9 → 8, assertion 3 un-ignored.**

## Order, and what makes a stage finished

A → B1 → B2, each its own commit, each with the full gate green on both platforms. **A stage is
finished when the gate's own bound has moved**, not when the code has. If a stage cannot lower the
bound, it is not a stage — fold it into one that can, or report that the measured cut cost was
wrong.

## Verification each slice needs

The ordinary gate (fmt, clippy `-D warnings`, `cargo doc -D warnings`, doc tests, full workspace
suite under a short fixed `TMPDIR`, zero fixture leftovers) plus, for B1 specifically:
**`cargo doc` proves the re-export** — a `ProjectId` that stops resolving at its old path is a
published-API break, and D3's whole claim is that this stage has none.

## What is not in this pass

The `project/` split (D7), the remaining cuts in D8's table, any crate split, any rename, and the
GUI's breadth. The RFC scopes them out or defers them; this pack does not reopen them.

**Updated 2026-11-12, at the split.** D6 through D8 are no longer part of RFC-074 at all — they
were moved to [RFC-077](../../accepted/077-the-capability-split.md), carried forward as
already-accepted decisions, so that RFC-074 is a thing `0.40.0` can actually close. That changes
nothing in this pack's own two slices: PR-074-A is still the gate and nothing else, PR-074-B is
still Stages 1 and 2, and the bound still ratchets in the same commit as the move that earns it.
The later staging is scoped in RFC-077's own handoff, only after this gate has behaved on two real
stages.

## Final Acceptance Decision

**Accepted 2026-11-14 by the reviewer, for release as `0.40.0`** — RFC-074 as narrowed by the
2026-11-12 split (D1–D5; D6–D8 carried forward as already-accepted decisions in RFC-077, which is
deliberately unscheduled and is **not** named by this release's own section).

**The rounds.** Two slices, reviews **672** and **675**, seven rounds between them.

| Review | Slice | Required changes, and what they were |
| --- | --- | --- |
| 672 | PR-074-A, the gate | None required. The reviewer closed one gap himself: assertion 1 passed today and had never been shown capable of failing, so he ablated it (`close -> project` planted, red at 11 modules). Carried forward as a standing requirement: **each bound move re-ablates**. Also raised then: assertion 3 witnesses only one of B2's two references. |
| 675 r1 | PR-074-B, Stages 1 and 2 | 1 — the round claimed `test_support.rs` held the only inline `mod tests` block; **seven more existed**, and the gate's exclusion is by filename, so each was scanned as production. Measured: 5 references across 4 edges, SCC unchanged — latent, not live. |
| 675 r2 | — | All seven extracted (the stronger of the two options: it keeps the RFC's stated method rather than re-defining it), plus a fifth assertion. **New required**: that assertion matched the literal line `mod tests {`. The reviewer planted `mod unit_checks` and all five assertions passed. |
| 675 r3 | — | Rewritten to key on `#[cfg(test)]` + an inline body. **New required**: `#[cfg(any(test, feature = "test-support"))]` still evaded — the attribute form this crate itself uses for `test_support` and `domain::shell_readiness`. Proven by planting it and watching the gate pass. |
| 675 r4 | — | Attribute match dropped entirely: no production file may declare **any** module with a body. Measured at zero first. **New required**: an intermittent disclosed in that round was filed against flake-register row 35, whose every prior occurrence is macOS, while this one was Linux — evidence bearing on that row's own open macOS-vs-not question. |
| 675 r5 | — | Appended to row 35, dated, platform named, recorded as a datum and explicitly **not** a disposition. Accepted. |

**What the acceptance rests on**, re-derived by the reviewer from source rather than read:

| | baseline `d868ab2` | Stage 1 `53e47f7` | Stage 2 `e573cbc` | after the guard rounds |
| --- | --- | --- | --- | --- |
| edges / references | 66 / 151 | 65 / 145 | 63 / 144 | **63 / 139** |
| largest SCC | 10 | 9 | **8** | **8** |
| `domain` outgoing | `[project]` | `[]` | `[]` | `[]` |
| `test_support` | both directions | both directions | none | **none** |

Each stage lowered the bound by exactly one, in the same commit as the move that earned it, and in
each the module that left is the one D3 and D4 predicted. The final reference drop (144 → 139) is
the inline test blocks ceasing to be counted as production — a real change, not a re-definition:
the reviewer had computed that counterfactual before the fix existed, and the measured result
matched it.

`ProjectId` still resolves at its old path (D3's API-compatibility claim, checked), and
`process_is_alive` landed `#[cfg(test)]` — **narrower** than the `any(test, feature = "test-support")`
it came from.

**Deferred rather than fixed:**

- **D6–D8** are RFC-077's, unscheduled until this gate has behaved on two real stages. It now has.
- **B2's dispositions are not one of D5's three named moves**, and the pack's question asking which
  was amended rather than answered: both ends of both references were test-only code, not a
  capability pair D2's tier model governs. The pack carries that amendment dated 2026-11-14.
- **A text-scanning gate cannot see a module produced by a macro.** Known limit, not a gap chased.

**The lesson this pack paid four rounds for, recorded for RFC-077's own gate work**: a guard that
matches a spelling invites exactly as many rounds as there are spellings. The version that ended it
asserts a property with nothing left to vary.
