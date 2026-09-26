---
title: "RFC-057 — what the editor must not do"
rfc: "RFC-057"
rfc_file: "../../accepted/057-editor-essentials.md"
source_rfc_status: "Accepted 2026-09-26 — M10 remainder"
target_milestone: "M10 remainder"
created: "2026-09-26"
---

# What the editor must not do

This is the one surface in the product that holds work a user cannot get back by redrawing. Four
ways this slice goes wrong.

## 1. It must not put a caret into the text

A caret drawn as a **character** inside the rendered string is indistinguishable from that same
character occurring in the file. A user reading a line cannot tell which one they are looking at,
and neither can a test. That is RFC-053's defect class — the window saying something untrue — dressed
as a convenience.

The caret is an **element**: the line is composed as the text before it, the caret, and the text
after. The fixture contains a file whose text holds the caret's own character, and the test asserts
the two are distinguishable.

## 2. It must not have two ideas of where the cursor is

`document.cursor()` is the cursor. A position derived independently for drawing — from a character
count, a measured width, anything — is a second source of truth, and two sources of truth about a
position is exactly the defect RFC-053 D3 found in `content_area_height`, where a constant stood in
for a measured bar and every terminal was sized wrong.

One value, one reader. The ablation plants a second derivation and a test fails.

## 3. It must not let undo lie about what is saved

`TextDocumentState` already distinguishes `Clean`, `Dirty`, `ExternalChanged`, `Conflict` and
`SaveError`. Undo has to keep all five honest:

| Situation | What must be true |
| --- | --- |
| Typed, then undone back to the opened text | the document is `Clean` again — otherwise a user saves what they did not mean to keep |
| Undo depth reached | the product **says so**; it does not silently forget the oldest edit |
| The file changed on disk (`ExternalChanged` / `Conflict`) | undo does not reach back across the reload and write text over a file that moved underneath |
| Undone, then redone, then saved | what lands on disk is what the screen showed |

Undo is being added under `NFR-REL-005` — protection against losing work. An undo that resurrects
text over someone else's change is the same requirement failing in the other direction.

## 4. It must not become untestable

Today a row is one assertable string and the editor's tests do not need `iced`. If the gutter and
the caret can only be checked by looking at a picture, the surface has lost the property that has
caught most of this project's GUI defects — and RFC-052 named it as a risk for exactly this reason.

Each drawn line stays **one string plus the caret element**. Live captures are still required, but
they are the second check, not the only one.

## And the measurement discipline

**Slice A measures before anything changes** (D11). A number taken after the rewrite proves nothing:
there is no baseline, and "it got faster" cannot be falsified. If today's editor already misses
`NFR-PERF-003`'s p95 ≤ 16 ms, that is the release's finding and it is published — the number is not
allowed to become embarrassing enough to omit.
