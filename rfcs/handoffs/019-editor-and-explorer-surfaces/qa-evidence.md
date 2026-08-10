---
title: "RFC-019: Editor and Explorer Surfaces - QA Evidence"
rfc: "RFC-019"
rfc_file: "../../proposed/019-editor-and-explorer-surfaces.md"
status: "Accepted 2026-08-10 — not started"
target_milestone: "M10"
created: "2026-08-10"
---

# QA Evidence

Record results here as each slice lands: gate output, ablations with the exact failure
they produced, findings, and limitations.

**This file is where results go. It is not where obligations go.** If a slice discovers
something a later slice must handle, put it in that slice's entry in
`task-breakdown-pr-plan.md` as well — that is what an implementer reads before starting.
This project has lost obligations to that gap four times.

## Recording conventions

- **Ablations name the exact failure**, not "the test failed." A specific wrong value is
  checkable; a green/red result is not.
- **One ablation per property.** An ablation that breaks two things proves neither.
- **A green ablation is a defect in the ablation**, not a pass.
- **Screenshots state what they prove and do not.**
- **Disclose rather than manufacture.** Declining to produce an artifact, with the
  reason, is worth more than a staged one.
- **Retire obligations explicitly.** When a carried-forward item stops applying, say so
  and why, in the place it was recorded. A list that only grows is one nobody reads.

## PR-019-A — Design and handoff acceptance

Granted by the human owner 2026-08-10 with RFC-019. Handoff pack authored the same day.

## PR-019-B — The explorer tree

Pending implementation.

## PR-019-C — The editor, read-only

Pending implementation.

## PR-019-D — Editing and save

Pending implementation.

## PR-019-E — Closeout

Pending implementation.

## Known Limitations

Consolidated at closeout. Carried in from RFC-019's own text, to be restated with
evidence:

- **No syntax highlighting, language servers, multi-cursor, or search** — non-goals, each
  a product in its own right.
- **Files above 4 MiB are not editable.** `TextDocumentOpenPolicy`'s existing bound, not a
  new one introduced here.
- **No file-tree write surface** — no rename, delete, or create.
- **"Show invisibles", if built, is ordinary functionality and not a security control.**
  RFC-016 chose that framing deliberately; a marker the user can toggle off is not a
  boundary.
- **Nothing here changes terminal performance.** `NFR-PERF-004`, the three-terminal limit
  and the output ceiling are owned by readiness-driven terminal I/O.
