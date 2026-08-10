---
title: "RFC-019: Editor and Explorer Surfaces - Acceptance / QA Checklist"
rfc: "RFC-019"
rfc_file: "../../proposed/019-editor-and-explorer-surfaces.md"
status: "Accepted 2026-08-10 — not started"
target_milestone: "M10"
created: "2026-08-10"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason. An absence with a reason is
evidence; an absence without one is a gap.

## Explorer Checklist (PR-019-B)

- [ ] Starting state confirmed: the content-model accessors had **no production caller**, shown by enumeration.
- [ ] Every node name, path hint and status escaped through `text_safety::quote_untrusted`.
- [ ] **Bidi-override case tested specifically** — escaped form present, raw character absent.
- [ ] That test **ablated**: removing the escaping makes it fail.
- [ ] No `*_label` free function called — all four named in the RFC.
- [ ] Every user-facing word resolves through `Catalog`, with distinctness asserted over resolved values, not key names.
- [ ] `NFR-UX-002`: symlink and access states distinguishable without colour.
- [ ] Open question 2 answered — symlink target shown or not, with reasoning.
- [ ] No filesystem walking in the shell; core's scan is the only source.

## Editor Checklist (PR-019-C)

- [ ] Text area renders **raw** — a `U+202E` document shows the raw character.
- [ ] **Ablated in the opposite direction**: a test asserting the escaped form appears must fail.
- [ ] Bidi reordering visible and correct.
- [ ] Chrome around the editor — path, dirty indicator, tab label — **escaped**.
- [ ] Cursor and viewport read from core; **no shell-side cursor field**, shown by enumeration.
- [ ] The 4 MiB open-policy refusal is rendered, using the existing policy — no second bound.
- [ ] Every user-facing word through `Catalog`.

## Editing Checklist (PR-019-D)

- [ ] `ExternalChangeDecision` **rendered and answerable**, demonstrated against a real file changed underneath a real open buffer.
- [ ] Every dismissal path defaults to **not overwriting**, each tested individually.
- [ ] Any modal is a `ModalContent` variant, not a second `Option` on `State`.
- [ ] Dirty state read from core, not tracked in the shell.
- [ ] Save goes through `save_active_document`; no shell-side write path.

## Honesty Checklist (PR-019-E)

- [ ] Claim statement checked **against RFC-019's own text**, not only the evidence file.
- [ ] **No claim that "show invisibles" is a security control**, if built.
- [ ] **No claim of any terminal performance change.**
- [ ] No claim of syntax highlighting, LSP, or search unless actually built.
- [ ] Open question 1 (widening the `.label()` scan) **raised, not absorbed**.
- [ ] Every unchecked line above carries a stated reason.

## Evidence Required

- [ ] Commit/PR list.
- [ ] Gate command output.
- [ ] The two opposite-direction escaping ablations, with the exact failures they produced.
- [ ] Screenshots: explorer tree, editor with a real file, external-change prompt — each stating what it proves **and does not**.
- [ ] Known limitations, consolidated.
- [ ] Answers to the RFC's three open questions.

## Final Acceptance Decision

- [ ] Accepted
- [ ] Accepted with required follow-up
- [ ] Rejected

Reviewer notes:
