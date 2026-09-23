---
title: "RFC-053: What The Window Says Is True — implementation handoff"
rfc: "RFC-053"
rfc_file: "../../accepted/053-what-the-window-says-is-true.md"
source_rfc_status: "Accepted 2026-09-24 — M12 remainder"
target_milestone: "M12 remainder"
created: "2026-09-24"
---

# Six things the window says that are not true

Source RFC: [RFC-053](../../accepted/053-what-the-window-says-is-true.md)

## What this is

Every item was found by **running the `0.22.0` release binary**, not by reading code. None is hard.
One of them — the status bar's one-line invariant — is stated in a doc comment, depended on to size
every PTY, and enforced nowhere.

This is `0.23.0`, the first release under the schedule authorised on 2026-09-24, and it ships ahead
of RFC-052 for one reason: **a thing the product says that is not true is fixed before a thing the
product does not yet do.**

## Read these first

1. [`what-a-surface-must-not-say.md`](./what-a-surface-must-not-say.md) — **required before writing
   code.**
2. The RFC's measured table — each row names the file.
3. RFC-052's pack is **not** this slice. The explorer is untouched here.

## The plan

[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md): A is the words, B is the layout. **B goes
last and alone** — it changes how every terminal is sized. Checklist:
[`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md).

## Not in this slice

The explorer (RFC-052), user configuration (RFC-054), the editor's gutter, caret and undo (RFC-057),
and any change to what a count *means* — only to what it is called and when it is known.
