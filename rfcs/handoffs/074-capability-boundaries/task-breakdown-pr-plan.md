---
title: "RFC-074 — task breakdown and PR plan"
rfc: "RFC-074"
rfc_file: "../../accepted/074-capability-boundaries.md"
source_rfc_status: "Accepted by the human owner 2026-11-03; narrowed to D1–D4 by the 2026-11-12 split and scheduled as 0.40.0 — RFC-074 is in rfcs/accepted/"
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

**Updated 2026-11-12, at the split.** D5 through D8 are no longer part of RFC-074 at all — they
were moved to [RFC-077](../../accepted/077-the-capability-split.md), carried forward as
already-accepted decisions, so that RFC-074 is a thing `0.40.0` can actually close. That changes
nothing in this pack's own two slices: PR-074-A is still the gate and nothing else, PR-074-B is
still Stages 1 and 2, and the bound still ratchets in the same commit as the move that earns it.
The later staging is scoped in RFC-077's own handoff, only after this gate has behaved on two real
stages.
