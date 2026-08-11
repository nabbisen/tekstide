---
title: "RFC-019: Editor and Explorer Surfaces - Acceptance / QA Checklist"
rfc: "RFC-019"
rfc_file: "../../done/019-editor-and-explorer-surfaces.md"
status: "Accepted 2026-08-10 — PR-019-B/C/D accepted (responses 180, 181, 182, 183); PR-019-E closeout implemented 2026-08-11, not yet reviewed"
target_milestone: "M10"
created: "2026-08-10"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason. An absence with a reason is
evidence; an absence without one is a gap.

## Explorer Checklist (PR-019-B)

- [x] Starting state confirmed: the content-model accessors had **no production caller**, shown by enumeration.
- [x] Every node name, path hint and status escaped through `text_safety::quote_untrusted`.
- [x] **Bidi-override case tested specifically** — escaped form present, raw character absent.
- [x] That test **ablated**: removing the escaping makes it fail.
- [x] No `*_label` free function called — all four named in the RFC.
- [x] Every user-facing word resolves through `Catalog`, with distinctness asserted over resolved values, not key names.
- [x] `NFR-UX-002`: symlink and access states distinguishable without colour.
- [x] Open question 2 answered — symlink target shown or not, with reasoning.
- [x] No filesystem walking in the shell; core's scan is the only source.

## Editor Checklist (PR-019-C)

- [x] Text area renders **raw** — a `U+202E` document shows the raw character.
- [x] **Ablated in the opposite direction**: a test asserting the escaped form appears must fail.
- [x] Bidi reordering visible and correct.
- [x] Chrome around the editor — path, dirty indicator, tab label — **escaped**.
- [x] Cursor and viewport read from core; **no shell-side cursor field**, shown by enumeration.
- [x] The 4 MiB open-policy refusal is rendered, using the existing policy — no second bound.
- [x] Every user-facing word through `Catalog`.

## Editing Checklist (PR-019-D)

- [x] `ExternalChangeDecision` **rendered and answerable**, demonstrated against a real file changed underneath a real open buffer.
- [x] Every dismissal path defaults to **not overwriting**, each tested individually.
- [x] Any modal is a `ModalContent` variant, not a second `Option` on `State`.
- [x] Dirty state read from core, not tracked in the shell.
- [x] Save goes through `save_active_document`; no shell-side write path.

## Honesty Checklist (PR-019-E)

- [x] Claim statement checked **against RFC-019's own text**, not only the evidence file.
- [x] **No claim that "show invisibles" is a security control**, if built.
- [x] **No claim of any terminal performance change.**
- [x] No claim of syntax highlighting, LSP, or search unless actually built.
- [x] Open question 1 (widening the `.label()` scan) **raised, not absorbed**.
- [x] Every unchecked line above carries a stated reason. (None unchecked above — no gap
      to explain.)

## Evidence Required

- [x] Commit/PR list.
- [x] Gate command output.
- [x] The two opposite-direction escaping ablations, with the exact failures they produced.
- [x] Screenshots: explorer tree, editor with a real file, external-change prompt — each stating what it proves **and does not**.
- [x] Known limitations, consolidated.
- [x] Answers to the RFC's three open questions.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
