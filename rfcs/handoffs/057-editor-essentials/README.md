---
title: "RFC-057: Editor Essentials — implementation handoff"
rfc: "RFC-057"
rfc_file: "../../accepted/057-editor-essentials.md"
source_rfc_status: "Accepted 2026-09-26 — M10 remainder"
target_milestone: "M10 remainder"
created: "2026-09-26"
---

# The editor draws the whole file, every frame

Source RFC: [RFC-057](../../accepted/057-editor-essentials.md)

## What this is

Five slices. **A measures and changes nothing** — it is the baseline everything after it is judged
against, and it comes first for that reason. B makes the body rows bounded by the viewport that has
had no reader since RFC-006. C adds the gutter and a caret that is a widget, not a character. D adds
undo. E is the owner's two-row header, its own slice because it is not editor work.

| | |
| --- | --- |
| Release | `0.28.0` |
| Depends on | RFC-006 (the document model), RFC-019 (the editor surface) |
| Requirements | `REQ-EDIT-002`, `NFR-REL-005`, `NFR-PERF-003` |
| Slices | [A](./task-breakdown-pr-plan.md#pr-057-a), [B](./task-breakdown-pr-plan.md#pr-057-b), [C](./task-breakdown-pr-plan.md#pr-057-c), [D](./task-breakdown-pr-plan.md#pr-057-d), [E](./task-breakdown-pr-plan.md#pr-057-e) |

## Read these first, in this order

1. [The RFC](../../accepted/057-editor-essentials.md) — nine measurements, D1–D7, and *Decided on
   acceptance* (D8–D11). **D11 reverses the obvious order of the work.**
2. [What the editor must not do](./what-the-editor-must-not-do.md) — the risk document.
3. [The PR plan](./task-breakdown-pr-plan.md).
4. [The acceptance checklist](./acceptance-qa-checklist.md) — write `qa-evidence.md` as you go.

## The shape, in one paragraph

`body_text` is `document.text().to_string()`, handed to one `text` widget: the whole file, laid out
every frame. `TextViewport { first_visible_line }` has sat in the model since RFC-006 with no reader.
Every keystroke returns `EditResult { text: String, .. }` — a whole-document copy — against a 4 MiB
cap, while `NFR-PERF-003` asks for p95 ≤ 16 ms typing latency in a 100 000-line file that nobody has
ever measured. This RFC measures that first, then draws the viewport's window as rows, puts line
numbers beside them and a caret element on one of them, and records each of the four edit operations
so it can be taken back.

## What is not in this slice

Syntax highlighting. Selection, clipboard, delete-forward — the editor's vocabulary is four keys and
that is a recorded finding, not an invitation. Scrolling input (D9). Soft wrap. Multi-document
(RFC-026). Autosave.
