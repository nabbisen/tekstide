---
title: "RFC-040: Affordance Completion — implementation handoff"
rfc: "RFC-040"
rfc_file: "../../done/040-affordance-completion.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-040 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-25"
---

# Finish the sentence RFC-039 started

Source RFC: [RFC-040](../../done/040-affordance-completion.md)

| # | Read | Why |
| --- | --- | --- |
| 1 | [RFC-040](../../done/040-affordance-completion.md) | Six findings, three principles, three decisions already made |
| 2 | [`what-a-clickable-modal-must-not-become.md`](./what-a-clickable-modal-must-not-become.md) | **Read before adding a button to any modal.** Modals are trusted chrome and carry destructive decisions |
| 3 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Four slices, order stated once |
| 4 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | What must be true and evidenced |
| 5 | [`qa-evidence.md`](./qa-evidence.md) | Where evidence goes |

## What this is

RFC-039's own audit counted: **three of thirteen** live actions have a visible control, and
**every one of the nine modals in this crate is keyboard-only for its own decision**. Several are
opened by a real button — trust granting, transcript purge, project close, the folder browser —
so a user arrives with a mouse and cannot finish or cancel without a keyboard.

That is the finding this RFC exists to close. Not "the product would be nicer with buttons": a
flow that begins with a click and cannot end with one is broken for the person in it.

## The measurement is the first slice, deliberately

PR-040-A makes the audit a test before anything is built. Two reasons, and the second matters
more:

1. Everything after it is then measured rather than asserted.
2. The keyboard-only allow-list gets written **before** anyone is tempted to add to it under
   deadline. An allow-list written while you are trying to make a count go green is not an
   allow-list, it is an excuse ledger.

RFC-039's audit was accurate on Monday and its own count was wrong by Tuesday. That is the
argument.

## What "done" means

Not "buttons exist." **No flow that a user can begin with a mouse requires a keyboard to finish
or abandon**, and every live action either has a visible control or is on a reasoned allow-list
that somebody chose. "Ten of thirteen" becomes a number decided rather than a number nobody
noticed.

## Scope boundaries

**In:** the audit-as-test and its allow-list; buttons for modal decisions; visible controls for
the actions D2 assigns to their surfaces.

**Out:** a visual redesign, an icon set, a toolbar, a command palette (`OpenCommandPalette` stays
`Reserved` — see D2). Removing keyboard operability from anything. Actions owned by RFCs that
have not built their surfaces yet — `OpenDiffReview` belongs to RFC-020's remaining slice, which
is scheduled next and must supply its own control.

**Escalate rather than descope.** If the allow-list starts growing to make a count pass, stop and
say so. That is the failure mode this RFC is most likely to produce.
