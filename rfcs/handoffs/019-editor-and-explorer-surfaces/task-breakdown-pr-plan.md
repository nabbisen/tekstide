---
title: "RFC-019: Editor and Explorer Surfaces - Task Breakdown / PR Plan"
rfc: "RFC-019"
rfc_file: "../../proposed/019-editor-and-explorer-surfaces.md"
status: "Accepted 2026-08-10 — ready for implementation"
target_milestone: "M10"
created: "2026-08-10"
---

# RFC-019 Task Breakdown

Five slices. **[`the-escaping-asymmetry.md`](./the-escaping-asymmetry.md) is required
reading before B or C** — the two halves are only correct together.

## PR-019-A — Design and handoff acceptance

Granted 2026-08-10 with the RFC. Nothing to implement.

## PR-019-B — The explorer tree

Scope: render `ExplorerDirectoryScan`; selection drives
`scan_content_explorer_directory`. No file mutation — no rename, delete, or create.

Review gate:

- **The starting state confirmed**: the content-model accessors had no production caller
  before this slice, shown by enumeration rather than asserted.
- **Every rendered name, path hint and status escaped** through `text_safety::quote_untrusted`.
- **A bidi-override case tested specifically** — a node named with `U+202E` renders
  escaped, and the raw character does not appear. **Ablate it**: a test that still passes
  with the escaping removed is not testing the property.
- **No `*_label` free function called.** All four (`explorer_node_kind_label`,
  `explorer_node_state_label`, `explorer_symlink_status_label`, `text_document_state_label`)
  are invisible to the existing `.label()` scan. Every word through `Catalog`.
- **`NFR-UX-002`**: symlink and access states distinguishable without colour.
- **Open question 2 answered**: does the explorer show a symlink's target? Decide with
  escaping already in place, and record which and why.
- No filesystem walking in the shell — render what core's scan provides.

## PR-019-C — The editor, read-only

Scope: open through `open_text_document`, render the document with cursor and viewport
from core, render the open-policy refusal for oversized files.

Review gate:

- **The text area renders raw.** A document containing `U+202E` shows the **raw**
  character; bidi reorders. **Ablate in the opposite direction**: a test asserting the
  escaped form appears must fail. This is the half that breaks files if inverted.
- **Chrome around the editor escapes** — path, dirty indicator, tab label.
- **Cursor and viewport read from core**, not duplicated in shell state. Enumerate: no
  shell-side cursor field.
- **The 4 MiB refusal is rendered**, not silently empty, and uses the existing policy —
  no second bound introduced.
- Every user-facing word through `Catalog`.

## PR-019-D — Editing and save

Scope: `replace_active_text`, `save_active_document`, dirty state, and
`ExternalChangeDecision` when the file changed underneath.

Review gate:

- **The external-change decision is rendered and answerable.** A save that overwrites
  someone else's change without asking is the defect this slice exists to prevent.
  Demonstrate the prompt with a real file changed underneath a real open buffer — not a
  synthesised decision value.
- **Every dismissal path defaults to not overwriting.** Test each exit individually, the
  way PR-018-C did for the paste dialog.
- If the prompt is a modal, it is a `ModalContent` **variant**, not a second `Option`
  field on `State`.
- **Dirty state comes from core**, not tracked in the shell.
- If core's edit surface proves insufficient for real editing, **stop and raise it** as an
  RFC-006 question rather than working around it in the shell.

## PR-019-E — Closeout

Scope: checklist, QA evidence, known limitations, answers to the three open questions,
and an explicit claim statement.

Review gate:

- The claim statement is checked **against RFC-019's own text**, not only the evidence
  file. RFC-017's closeout passed its own gate while its RFC still asserted two falsified
  things; that check exists because of it.
- **No claim that "show invisibles" is a security control**, if one was built. RFC-016
  chose that framing deliberately.
- **No claim of any terminal performance change.**
- Open question 1 — whether to widen the `.label()` scan to catch free functions —
  **raised, not absorbed.**
- Open question 3 — syntax highlighting — answered with the editor working, not before.

## Sequencing

**B and C are independent.** D needs C. E needs all.

```
A ─┬─→ B ─────────┬─→ E
   └─→ C ─→ D ────┘
```
