# RFC-019: Editor and Explorer Surfaces - Developer Handoff Pack

Source RFC: [RFC-019](../../proposed/019-editor-and-explorer-surfaces.md)
Target milestone: **M10** (`0.6.x`)
Source RFC status: **Accepted by the human owner 2026-08-10**

**Start here.** Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-019](../../proposed/019-editor-and-explorer-surfaces.md) | The surfaces, the security core, the label trap. |
| 2 | This file | Orientation and what is binding. |
| 3 | [`the-escaping-asymmetry.md`](./the-escaping-asymmetry.md) | **Read before PR-019-B or PR-019-C.** The two halves are only correct together. |
| 4 | [`implementation-handoff.md`](./implementation-handoff.md) | What exists, the seams, what is genuinely missing. |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 7 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting — RFC-019 conforms to these rather than amending them:

- [RFC-006](../../done/006-projectsession-state-and-file-explorer-editor-basics.md) — the document, cursor, viewport and explorer models. **This RFC renders them. It does not amend them.**
- [RFC-015](../../done/015-application-shell-and-rendered-surface-model.md) — the surface contract and input routing.
- [RFC-016](../../done/016-internationalization-and-localization.md) — text safety, **and the editor's already-decided exception to it**.
- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md) — the core/shell boundary and the project's vocabulary, if you have not read it.

## Where to start

**PR-019-B or PR-019-C — they are independent.** PR-019-A is design acceptance, already granted with the RFC.

## The shape of this RFC, in one paragraph

**Nothing here needs designing.** RFC-006 defines the model and `tekstide-core`
implements it: `TextDocument` with `TextCursor`, `TextViewport`, `TextDocumentState`,
dirty tracking, `ExternalChangeDecision`, `SaveDecision`, a `TextDocumentOpenPolicy` with
a 4 MiB editable bound, and the explorer scan. **None of it has a production caller** —
verify that yourself before starting, the way PR-018-B did. This is the third RFC in a
row with that shape. Your job is call sites and rendering. **If you are writing a policy
rule, a bound, or a state machine, stop**: it exists in core, or it belongs there, or it
is an RFC-006 amendment.

## What is binding

1. **The escaping asymmetry.** Explorer escapes; editor does not. See document 3.
2. **Cursor and viewport are core's state.** `TextCursor`/`TextViewport` live on
   `TextDocument` and `set_cursor` already exists. A shell-local cursor is duplicated
   state and breaks the surface contract PR-017-C held for the terminal pane.
3. **One open policy.** `TextDocumentOpenPolicy` already refuses above 4 MiB. Render the
   refusal; do not add a second bound.
4. **Every user-facing word through `Catalog`** — including the four `*_label` free
   functions the existing scan does not catch.
5. **An external change is answered by the user, not resolved silently.** A save that
   overwrites someone else's edit without asking is the defect PR-019-D exists to prevent.

## Traps this project has already fallen into

**The test that passes with the thing it tests removed.** At least six occurrences.
Every ablation breaks the property and watches the *specific* test fail. A green ablation
is a defect in the ablation, not a pass.

**Free functions escaping a substring-matched scan.** `slot_label`/`status_label` shipped
ten hardcoded English strings into the session bar because the scan matches `.label()`
and they were not method calls. Four more of the same shape are waiting for you here.

**Recording an obligation where nobody reads it.** If a slice finds something a later
slice must handle, put it in that slice's entry in `task-breakdown-pr-plan.md`, not only
in `qa-evidence.md`. Evidence files hold results; scope entries are what implementers
read. This has cost the project four lost obligations.

**Prescriptions from review are not automatically right.** Two findings this cycle came
from an implementer testing a reviewer's instruction rather than applying it — the
`RoutedInput` shape in PR-018-B, and the WAL `drop` in RFC-021's sentinel fix. If a gate
in these documents looks wrong once you have the code in front of you, say so.
