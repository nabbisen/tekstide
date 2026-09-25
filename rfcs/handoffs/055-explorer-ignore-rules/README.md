---
title: "RFC-055: Ignore Rules In The Explorer — implementation handoff"
rfc: "RFC-055"
rfc_file: "../../done/055-ignore-rules-in-the-explorer.md"
source_rfc_status: "Implemented and closed 2026-09-25 — M12 remainder tail"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# The explorer stops guessing what is ignored and asks git

Source RFC: [RFC-055](../../done/055-ignore-rules-in-the-explorer.md)

## What this is

Three slices that make the file explorer's idea of "ignored" come from git instead of from a
three-name array, give `REQ-FILE-002` the fifth category it has never had, and give a user one
setting to see the ignored rows anyway.

| | |
| --- | --- |
| Release | `0.26.0` |
| Depends on | RFC-052 (the tree and its expansion), RFC-030 (the git gate), RFC-054 (the configuration mechanism) |
| Requirements | `REQ-FILE-005`, `REQ-FILE-006`, the `ignored` category of `REQ-FILE-002` |
| Slices | [PR-055-A](./task-breakdown-pr-plan.md#pr-055-a), [PR-055-B](./task-breakdown-pr-plan.md#pr-055-b), [PR-055-C](./task-breakdown-pr-plan.md#pr-055-c) |

## Read these first, in this order

1. [The RFC](../../done/055-ignore-rules-in-the-explorer.md) — the eleven measurements, D1–D9,
   and *Decided on acceptance*, which corrects the per-expansion budget and forbids caching the gate.
2. [What the explorer must not claim](./what-the-explorer-must-not-claim.md) — the risk document.
   **The three things in its first section are what this slice is actually about.**
3. [The PR plan](./task-breakdown-pr-plan.md).
4. [The acceptance checklist](./acceptance-qa-checklist.md) — write `qa-evidence.md` as you go, not
   at the end.

## The shape, in one paragraph

The explorer scan already runs on a worker thread (`explorer_scan_subscription`, applied by
`ProjectSession::apply_explorer_scan` with newer-scan-wins). This RFC adds, to that same worker, one
`git check-ignore -z --stdin` call per directory scanned, asked only about the entries that scan is
about to return — at most `max_children_per_directory` of them, which is 256. Every path goes in
prefixed `./` and comes back with that prefix to strip, through **one function**, because a file a
repository names `:(glob)evil.log` otherwise aborts the whole batch. The answer becomes
`FileGitStatus::Ignored` on the node; the surface draws an `ignored` badge; `explorer.show_ignored`
decides whether ignored rows are drawn at all. Outside a repository, or when the query could not
answer, the old three-name list is still the floor — and the sidebar says which of the two it used.

## What is not in this slice

No `.gitignore` parser. No file watcher (that is RFC-026, `0.29.0`), and so no change to *when*
scans happen. No hiding of dotfiles. No change to `GeneratedChangeDetectionPolicy` or to the status
bar's changed-file count. No `--ignored` on the status invocation — the RFC's measurement 5 rules it
out, and it is not left open as an option.
