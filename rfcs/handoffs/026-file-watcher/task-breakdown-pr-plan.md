---
title: "RFC-026 — task breakdown and PR plan"
rfc: "RFC-026"
rfc_file: "../../done/026-file-watcher.md"
source_rfc_status: "Implemented and closed 2026-10-07 — released as 0.29.0"
target_milestone: "M13"
created: "2026-09-30"
---

# Task breakdown and PR plan

Four slices. **A adds no dependency and wires no watcher**, and it comes first so that B's evaluation
is judged against a number that already exists.

## PR-026-A — the batching, proved against a simulated stream

`tekstide-core`. **No dependency, no watcher, no surface change.**

- A debouncer: events in, at most **one scan request per directory per window**, out. The window is a
  stated constant, not a tuned one, and the book names it.
- **The falsification is the slice**: a simulated burst of 1,000 events into one directory yields one
  scan request per window. Counted, written down, and ablated — remove the batching and the count
  becomes 1,000.
- A second count that matters just as much: **git subprocesses**. A scan is a subprocess (risk §1),
  so the evidence carries both numbers.
- Coalescing rules stated: several events for one directory collapse; events for different
  directories do not silently merge.

## PR-026-B — the watcher, and the dependency that comes with it

- **D4's evaluation, recorded before adoption.** `notify 8.2.0` as of today: MSRV 1.77 (under our
  1.90 floor), licence **CC0-1.0** — unlike every other direct dependency — last stable
  **2025-08-03**. None disqualifies it; all three go in `dependency-advisories.md`. The question that
  decides it is **what it does when the kernel refuses another watch**, read from what it does.
- **Scope from `ExplorerTree::expanded`** plus the project root plus the folders of open documents
  (D1). Expanding adds watches, collapsing removes them, closing a project removes all of its —
  **counted before and after**, because a scope that only grows is risk §2 arriving on its own.
- **Budget exhaustion forced** (risk §2): no crash, today's behaviour, and a sentence on screen.
- **The hostile fixture** (risk §3): a symlink leaving the root is not watched; a loop does not
  recurse.
- A subscription in the shape `explorer_scan_subscription` already has (D10). Nothing on the render
  thread.

## PR-026-C — the change arrives without reopening anything

- **`REQ-FILE-003`**: a file created, deleted or renamed in an expanded folder appears without the
  user reopening it. **Captured live** — this is the requirement's evidence.
- **`REQ-FILE-004`**: an editor keystroke's latency under a watched burst, measured against RFC-057's
  own baseline harness. Measured, not asserted.
- **D8, and risk §4**: an external change reaches an open document through the states that already
  exist. Unsaved edits survive; no silent reload; a deleted open file is a state the product can say.
- **This is the split point.** If A–C have been more than one release's work, say so here rather than
  at the candidate — the RFC names `0.29.0`/`0.30.0` as the planned division, and taking it is the
  architect's call on evidence.

## PR-026-D — the open set becomes plural

- **The two counts** (D9): `open_buffer_count()` and `dirty_file_count()`, each `u32::from(<one
  Option>)` today at `project/content.rs:359`/`:363`, each with one reader at
  `session.rs:1736–1737`. They count the whole open set. **A test fails if either counts one when two
  are open.** If a third counter exists by then, report it rather than absorbing it.
- **`active_document` keeps its meaning** (D6): one *active* document, a plural *open* set. A rename
  across 74 call sites to prove a point is how this slice goes wrong.
- **The cap** (D5), said when reached — each document is up to 4 MiB plus two 500-deep undo stacks.
- The close dialog and save-all count every open document, not the active one.

## Order, and why

A first because the batching is ours and must be falsifiable before a dependency can be judged
against it. B before C because there is nothing to deliver events until it exists. C before D because
the watcher is the release's subject and the document model is the part that can move. D last, where
it can be split off without unpicking anything.
