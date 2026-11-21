---
title: "Crate organization and hierarchy — measured status, 2026-11-21"
status: "Assessment only, written at the owner's question after 0.41.0 published. No RFC opened, nothing scheduled."
target_milestone: "unscheduled"
created: "2026-11-21"
---

# Crate organization and hierarchy — where this stands after RFC-077

The owner asked how crate organization and hierarchy stand. RFC-074 and RFC-077 reorganized
**modules inside one crate**; the crate layout itself has not been touched since the workspace was
created. This is the measured state, what it costs today, and what the one real optimization is —
measured rather than argued.

## The workspace as it is

Two members. `tekstide-core` **27,083 lines of production code across 30 top-level modules**;
`tekstide` 14,753 across the GUI. `tekstide-core` is published to crates.io; `tekstide` is the
binary.

The module graph inside `tekstide-core` is acyclic and tiered — this is RFC-074/077's output, and
it is in good shape:

| tier | modules | code |
|---|---|---|
| **T0, zero outgoing** | `domain` 1330, `navigation` 512, `recovery` 411, `close` 160, `test_support` 142, `project_lock` 138, `text_safety` 83, `interface_scale` 54, `route` 7 | ~2,837 |
| one dependency | `content`, `change_detection`, `recent`, `watch`, `security`, `metadata`, `command` | ~2,819 |
| middle | `transcript`, `approval`, `root`, `explorer_tree`, `workspace`, `diff` | ~5,679 |
| orchestration | `runtime` 4640, `agent` 1321, `audit` 4430, `config` 1659, `app` 638, `shell` 379, `project_board` 293, `project` 2356 | ~15,716 |

`project`'s apparent out-degree of 15 is 7 genuine plus 8 compatibility re-exports — the façade
RFC-077 kept so no published path moved. Any naive measurement will call `project` the densest node
in the crate; that is the façade, which is exactly why the gate needed the genuine-reach
distinction (`MAX_PROJECT_GENUINE_REACH`, review 703).

## The one measured optimization: three heavy dependencies, each used by exactly one module

| dependency | modules that use it | transitive subtree |
|---|---|---|
| **`rusqlite`** | **`audit` only** | 7 crates, and a bundled C library |
| **`notify`** | **`watch` only** | 11 crates |
| **`toml`** | **`config` only** | 7 crates |
| `libc` | `approval`, `project_lock`, `recovery`, `runtime`, `test_support` | genuinely cross-cutting |
| `serde` / `serde_json` | 7 / 6 modules | genuinely cross-cutting |
| `uuid` | `approval`, `domain`, `transcript` | cross-cutting enough |

`tekstide-core` has **36 unique transitive dependencies**. Three of them anchor subtrees that one
module each needs, and SQLite is the heaviest single thing in the tree. **A downstream consumer who
wants only the approval protocol today compiles SQLite, a filesystem watcher and a TOML parser.**
That is the whole of the optimization case, and it is real.

(For scale: the GUI crate pulls **270** transitive dependencies, overwhelmingly `iced`. Nothing in
`tekstide-core`'s layout moves that number.)

## What *not* to do first

**Not a crate split.** Breaking `tekstide-core` into several published crates would:

- break every published path unless each new crate is re-exported from a façade crate, which is the
  same trick RFC-077 used at module level and which adds a real dependency edge at crate level;
- multiply the publishing surface — each crate its own `0.x`, each a version to keep in step. This
  project declined `snora` as a dependency partly on release-churn grounds (`future-work.md`); it
  should not inflict the same shape on its own consumers without a reason features cannot serve;
- turn every remaining tangle into a hard compile error rather than a gate finding. That is a
  benefit *after* the graph is known clean, not a way to discover whether it is.

The module graph is already acyclic and tiered, so a crate split would buy **no structural
discipline the gate does not already enforce**. It would buy dependency isolation — which features
buy more cheaply.

## What to consider, if anything

**Cargo features, not crates**: `audit` behind a feature that gates `rusqlite`, `watch` behind one
gating `notify`, `config` behind one gating `toml`. One published crate, no path changes, reversible.

Two things to settle before anyone starts, neither of which is settled now:

1. **Measure the prize first.** Nobody has measured what those three subtrees actually cost — clean
   build time, incremental build time, or `tekstide-core`'s own compile time with each removed. "36
   dependencies, three of them narrow" is a structural fact; it is not yet a number anyone has paid.
   An optimization RFC that cannot state the saving should not open.
2. **The gate has a known blind spot here.** `capability_graph_gate` scans files by name and cannot
   see `#[cfg(feature = …)]` any more than it could see `#[cfg(test)]` — the class that reached it
   three times during D7. Feature-gating whole modules would put module files in the production scan
   that a default build never compiles, and the bounds would measure a graph nobody ships. That
   needs an answer before the first `#[cfg(feature)]`, not after.

**Who is served** is the question under all of this. `tekstide-core` has one known consumer: the
`tekstide` binary in this workspace, which uses everything. Dependency isolation pays off for a
downstream consumer who wants a subset — and this project has never recorded one. Until there is
one, or until the measurement in (1) shows the saving is large enough to matter to the binary's own
build, this is a real finding with no demonstrated beneficiary.

## Status

**Nothing is wrong and nothing is scheduled.** The hierarchy is acyclic, tiered, gate-enforced and
freshly reorganized; the crate layout is two members with one published; the one measured
inefficiency is three single-module dependency subtrees, addressable by features rather than by a
split. Recorded here so the question has an answer with numbers behind it, and so that whoever
reaches for a crate split later has to argue past the measurement rather than from the shape of the
directory tree.
