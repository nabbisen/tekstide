---
title: "RFC-065: The Multi-Document Model — implementation handoff"
rfc: "RFC-065"
rfc_file: "../../done/065-the-multi-document-model.md"
source_rfc_status: "Implemented and closed 2026-10-08 — released as 0.30.0 2026-10-08"
target_milestone: "M13"
created: "2026-10-07"
---

# Opening a second file throws the first one's edits away

Source RFC: [RFC-065](../../done/065-the-multi-document-model.md)

## What this is

Four slices. **A repairs a data-loss defect that is live in `0.29.0`** and ships first and alone. B is
the model — the open set, the two counts, the bound, the watcher's scope. C is a way to reach the
second document. D is save-all.

| | |
| --- | --- |
| Release | `0.30.0` |
| Depends on | RFC-006 (the document), RFC-026 (the watcher and its harness), RFC-057 (undo, per-document state) |
| Requirements | `REQ-EDIT-004`'s own plural; the roadmap's 1.0 list. **No `REQ-` names multi-document** — D9 records the gap and mints nothing |
| Slices | [A](./task-breakdown-pr-plan.md#pr-065-a), [B](./task-breakdown-pr-plan.md#pr-065-b), [C](./task-breakdown-pr-plan.md#pr-065-c), [D](./task-breakdown-pr-plan.md#pr-065-d) |

## Read these first, in this order

1. [The RFC](../../done/065-the-multi-document-model.md) — ten measurements, D1–D9, and *Decided
   on acceptance* (D10–D13). **D10 and D13 remove work; read them before planning any.**
2. [What losing a buffer looks like](./what-the-open-set-must-not-do.md) — the risk document.
3. [The PR plan](./task-breakdown-pr-plan.md).
4. [The acceptance checklist](./acceptance-qa-checklist.md) — write `qa-evidence.md` as you go.

## The shape, in one paragraph

`Action::Open(path)` reaches `self.active_document = Some(document)` through four functions, **none of
which mentions `dirty`, `unsaved` or `confirm`** — so a user who edits a file and then opens another
loses the first silently, with its undo history. The open set is the repair, because a set has nothing
to replace. `active` keeps its meaning: one *active* document, a plural *open* set, and of the 83
sites that read it, the two that **count** are `open_buffer_count` and `dirty_file_count`, each
`u32::from(<one Option>)` with one reader. `REQ-EDIT-004` has said *dirty **buffers*** since it was
written; this is what makes the plural true.

## What is not in this slice

Crash recovery of the open set (RFC-027, `0.31.0`, which this unblocks). Split views. Per-document
terminals. Syntax highlighting. A tab bar, if something simpler reaches the second document.
