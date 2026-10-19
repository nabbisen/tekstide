---
title: "RFC-052: A File Explorer A User Can Read — implementation handoff"
rfc: "RFC-052"
rfc_file: "../../done/052-a-file-explorer-a-user-can-read.md"
source_rfc_status: "Implemented and closed 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# The sidebar is debug text, and it is not a tree

Source RFC: [RFC-052](../../done/052-a-file-explorer-a-user-can-read.md)

## What this is

Two defects, one surface. The rows read `[FILE] scratch.txt [untracked]` because **every row is one
Fluent string** — and underneath that, **the explorer is not a tree at all**: one directory scan, a
`Parent` row to walk back out, so a change inside `src/` is invisible until you step into `src/`.
`REQ-FILE-001` asks for the project root tree.

**The one-string row is not sloppiness.** It centralises the escaping of attacker-controlled names,
keeps every label in the catalog, and keeps the row assertable without `iced`. Whatever replaces it
keeps all three, or the change is a trade, not a fix.

## Read these first

1. [`what-a-file-tree-must-not-do.md`](./what-a-file-tree-must-not-do.md) — **required before
   writing code.**
2. The RFC's **"Decided on acceptance"**, especially **D3**'s rule and **D7**'s collapse list.
3. `surface/explorer.rs`'s `node_line` and `project/root/explorer.rs`'s scan policy — the two
   things this slice replaces and the guarantees they carry.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A measures and decides the mechanism,
B builds the tree, C renders it. Checklist:
[`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## Not in this slice

`.gitignore` handling and an `ignored` badge (next slice), a hidden-file toggle, the file watcher and
multi-document model (RFC-026), drag-and-drop, multi-select, and re-platforming onto `snora`.

## The measurement harness

[`measurement/`](./measurement/) is a standalone crate (its own `Cargo.lock`, not a member of the
main workspace, by deliberate choice — see that `Cargo.lock`'s own commit history) that `#[path]`-
includes `tekstide-core`'s own `hostile_fixture.rs` and drives both the third-party `DirectoryTree`
widget and our composition against it, measuring the D3/D8 properties this RFC decided on. Run it
with `cargo run --release` from inside `measurement/`. Its `Cargo.lock` is a build input, not a
historical record — `cargo metadata --locked` must exit 0 — and the `results-run*.txt` files beside
it are the actual historical record: full, captured output from real runs, one per entry, diffed
against each other to catch a behavior or performance change between them rather than relying on
memory of what the numbers used to be.
