---
title: "RFC-065 — what losing a buffer looks like"
rfc: "RFC-065"
rfc_file: "../../accepted/065-the-multi-document-model.md"
source_rfc_status: "Accepted 2026-10-07 — M13"
target_milestone: "M13"
created: "2026-10-07"
---

# What losing a buffer looks like

This RFC starts from a defect that is shipping. Everything below is about not replacing it with a
different one.

## 1. The repair must be shown against the defect

`Action::Open(path)` → `open_active_project_text_document` → `app.rs:466` → `session.rs:1472` →
`content.rs:190`, which is `self.active_document = Some(document)`. **`dirty`, `unsaved` and
`confirm` appear in none of those four functions** — zero hits, checked one at a time.

So: write the test first, run it against today's code, and **see it fail**. Then fix it. A repair
whose test never saw the defect is a claim, and this is the one place in the product where the claim
being wrong costs a user work they cannot get back — the undo history goes with the document that was
replaced.

That is the shape RFC-026 PR-026-C used when it planted the third file *before* any code that
deletes existed. Same discipline, higher stakes.

## 2. The cure must not be worse than the defect

A confirmation on every second file would be worse than losing edits for a user who never edits before
browsing — most file-opening is navigation, not editing. **The set removes the question**: there is
nothing to replace, so there is nothing to ask about.

Prefer the set. The prompt is the interim, and only if the set is not ready.

## 3. A count that is wrong still compiles

`open_buffer_count` and `dirty_file_count` are each `u32::from(<one Option>)`. When the set is plural
they must count **all of it**, and nothing in the type system will notice if they do not.

**The test that matters fails when either returns one while two are open.** And it covers the close
dialog for free: `session.rs:1706` is `close_resources.dirty_files = file_state.dirty_file_count`, the
same count — so a dialog that undercounts unsaved work is the same bug wearing a different face.

## 4. N documents multiply three things

| | |
| --- | --- |
| **Bytes** | up to 4 MiB each |
| **Undo history** | two stacks, 500 operations each |
| **The refresh** | RFC-026 measured it at about **4 ms per document**, reading the whole file |

The third is the one that is easy to miss, because with one document it is invisible. A watched burst
touching N open documents is **N whole-file reads in one drain window**. Measure it on RFC-026's own
paired-control harness — which already carries its control in the same run — rather than building a
new one.

## And the one that is not about the set

**`REQ-EDIT-004` says "dirty *buffers*", and has since it was written.** The product has only ever had
one. The coverage row lists `004` as covered and qualifies the row *"single active document"*. When
the plural becomes true, say that the row was half met — the way RFC-057 corrected `REQ-EDIT-002`'s —
rather than quietly letting it start being right.
