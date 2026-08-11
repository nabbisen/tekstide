# RFC-024: Diff Preview Policy - Developer Handoff Pack

Source RFC: [RFC-024](../../done/024-diff-preview-policy.md)
Target milestone: **M10** (`0.6.x`) — prerequisite for RFC-020
Source RFC status: **Accepted by the human owner 2026-08-11**

**Start here.** Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-024](../../done/024-diff-preview-policy.md) | The four decisions and why each was made. |
| 2 | This file | Orientation and what is binding. |
| 3 | [`the-four-decisions.md`](./the-four-decisions.md) | **How to implement each decision, and how each is proven.** Read before writing code. |
| 4 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 5 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 6 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting:

- [RFC-012](../../done/012-generated-change-review-foundations.md) — the detection model, and §Detection scope, which is what authorises this RFC to exist.
- [RFC-006](../../done/006-projectsession-state-and-file-explorer-editor-basics.md) — `FileSnapshot` / `ExternalChangeDecision`, the machinery Decision 3 requires you to reuse.
- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md) — the core/shell boundary and the project's vocabulary.

## This is `tekstide-core` work, not shell work

Unlike RFC-017/018/019, **nothing here renders anything.** RFC-024 produces a model; RFC-020 renders it later. If you find yourself in `crates/tekstide`, you are in the wrong crate.

That also means the usual "confirm no production caller exists" opener does not apply — **there is no existing model to confirm the state of.** You are building one. The equivalent starting-state check is the opposite: confirm that nothing in `tekstide-core` currently reads generated-change file content, so that whatever you add is the only path.

## What is binding

1. **Content is read only on demand, only for already-detected paths, and never retained beyond the request.** Decision 1. The third clause is the one that matters and it should be **structural** — see document 3.
2. **Refuse above the bound; never truncate.** Decision 2. This project has shipped truncation-before-classification once and still carries an unfixed silent-truncation defect in the terminal. Do not add a third.
3. **Reuse `FileSnapshot`/`ExternalChangeDecision` for staleness.** Decision 3. A second staleness mechanism is a second source of truth about the same question.
4. **Classify binary before reading as text.** Decision 4. Not by attempting a UTF-8 read and handling failure.
5. **Do not pre-escape.** Escaping is RFC-020's rendering concern. A model returning escaped text stops any non-rendering consumer seeing what the file contains.
6. **This is not a diff engine.** Producing a difference between two texts is solved; the value here is the policy around it. If you start designing an algorithm, the scope has drifted — say so.

## Traps this project has already fallen into

**The test that passes with the thing it tests removed.** At least six occurrences. Every ablation breaks the property and watches the *specific* test fail. A green ablation is a defect in the ablation, not a pass.

**A bound chosen by estimate.** Twice now an estimated figure was wrong once measured — the terminal's per-pane poll cost, and the flood script's actual throughput. RFC-024 §Open questions 1 asks for the bound's number to be decided **with the memory profile measured**.

**Recording an obligation where nobody reads it.** If a slice finds something a later slice must handle, put it in that slice's entry in `task-breakdown-pr-plan.md`, not only in `qa-evidence.md`. Four obligations have been lost that way.

**Prescriptions from review are not automatically right.** Three findings this cycle came from an implementer testing a reviewer's instruction rather than applying it. If a gate here looks wrong once you have the code in front of you, say so.
