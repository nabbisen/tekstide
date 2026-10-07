---
title: "RFC-026: File Watcher — implementation handoff"
rfc: "RFC-026"
rfc_file: "../../accepted/026-file-watcher.md"
source_rfc_status: "Accepted 2026-09-30 — M13"
target_milestone: "M13"
created: "2026-09-30"
---

# The explorer reads a folder once and never again

Source RFC: [RFC-026](../../accepted/026-file-watcher.md)

## What this is

Four slices. **A writes the batching and proves it before any watcher exists** — and before the
dependency, so the dependency is judged against a number that already exists. B evaluates and wires
the watcher. C makes the explorer and an open document see a change without the user reopening
anything. D makes the open set plural.

| | |
| --- | --- |
| Release | `0.29.0` (the split point, if taken, puts D in `0.30.0` — see the RFC's last section) |
| Depends on | RFC-052 (the tree), RFC-055 (the ignore answer, and why a scan is a subprocess), RFC-057 (the editor) |
| Requirements | `REQ-FILE-003`, `REQ-FILE-004`, `NFR-PERF-007`, and the single-document limit |
| Slices | [A](./task-breakdown-pr-plan.md#pr-026-a), [B](./task-breakdown-pr-plan.md#pr-026-b), [C](./task-breakdown-pr-plan.md#pr-026-c), [D](./task-breakdown-pr-plan.md#pr-026-d) |

## Read these first, in this order

1. [The RFC](../../accepted/026-file-watcher.md) — nine measurements, D1–D8,
   and *Decided on acceptance* (D9–D12). **D11 puts the batching before the dependency.**
2. [What a watcher must not do](./what-a-watcher-must-not-do.md) — the risk document.
3. [The PR plan](./task-breakdown-pr-plan.md).
4. [The acceptance checklist](./acceptance-qa-checklist.md) — write `qa-evidence.md` as you go.

## The shape, in one paragraph

Two facts decide this and both postdate the scheduling. **RFC-055 made every directory scan ask git**
— the gate's configuration half plus one `check-ignore` — so a re-scan is a subprocess and an event
storm is a subprocess storm. And **RFC-055's ignore answer makes the watch scope affordable**: this
repository is 18,974 directories and 1,485 without the ignored ones, while `ExplorerTree::expanded`
already tracks exactly what the user opened. So the watcher watches the project root, the expanded
directories and the folders holding open documents; events are batched into one scan per directory
per window; and a kernel that refuses another watch produces today's behaviour plus a sentence, never
a crash and never silence.

## What is not in this slice

Crash recovery and unsaved-buffer persistence (RFC-027). Watching outside the project root. Reacting
to files nobody has open in folders nobody expanded. Windows and macOS watching — M14's, per the
roadmap's own rule that cross-platform evidence is produced per platform and never inferred.
