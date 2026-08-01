# RFC-014: Desktop GUI Substrate and Terminal Rendering Strategy - Developer Handoff Pack

Source RFC: [RFC-014](../../done/014-desktop-gui-substrate-and-terminal-rendering.md)
Target milestone: **M8**
Source RFC status: **Proposed — criteria accepted 2026-07-28**

**Start here.** This file is the entry point for the RFC-014 spike. Everything you need is linked below; read in this order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-014](../../done/014-desktop-gui-substrate-and-terminal-rendering.md) | The decision, criteria C1-C14, and why a pure TUI was rejected. |
| 2 | This file | Orientation, resolved open questions, and what is binding on you. |
| 3 | [`implementation-handoff.md`](./implementation-handoff.md) | Quarantine rules, candidate selection, measurement methodology, evidence requirements. |
| 4 | [`pr-014-c-filter-interposition.md`](./pr-014-c-filter-interposition.md) | **Detailed instructions for PR-014-C, the security-boundary slice. Read before choosing a terminal-emulation crate, not after.** |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries, scope, and review gates. |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Acceptance traceability against C1-C14; what evidence is required. |
| 7 | [`qa-evidence.md`](./qa-evidence.md) | Where you record observed gates, measurements, findings, and limitations. |

## Where to start work

**Begin at PR-014-B.** PR-014-A (RFC and criteria acceptance) is complete — accepted 2026-07-28.

PR-014-B through PR-014-E are spike implementation and measurement work. PR-014-F is the decision record, authored by the architect after your evidence lands; it requires maintainer sign-off before M8 implementation begins.

Slice-by-slice scope and review gates are in [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).

## Three things that are binding

1. **The spike is disposable and quarantined.** `crates/tekstide-gui-spike`, `publish = false`, no product crate may depend on it. Precedent: `tekstide-pty-spike` from RFC-007.
2. **The spike's job is to falsify, not to demonstrate.** A spike that only shows things working has failed at its purpose. Record what does not work and what you could not evaluate.
3. **Do not modify `tekstide-core`.** If the spike reveals a change the product needs — and it will, see PR-014-C — record the requirement; do not make the change here.

This handoff inherits the source RFC lifecycle state.

## Source Summary

RFC-014 selects the desktop GUI substrate and the terminal-rendering strategy as one decision, because the terminal surface is the hardest constraint on substrate choice.

A pure TUI was rejected on accepted-requirements grounds, not preference: RFC-009 requires approval, trust, paste-confirmation, and destructive dialogs to render *outside terminal output*, and that property cannot hold when every surface is characters in one grid. i18n and accessibility requirements point the same way.

The spike is disposable and quarantined, following the `tekstide-pty-spike` precedent from RFC-007. It introduces no product dependency until the decision record is accepted.

## Decisions Resolved Since RFC Approval

RFC-014's open questions are resolved as follows and are binding on this handoff:

1. **TUI rejection** — accepted with RFC approval. Not evaluated by the spike.
2. **Second candidate** — selected by the spike author from a named shortlist using the selection rule in `implementation-handoff.md` §2. Rationale recorded in PR-014-B.
3. **Missed performance budget** — escalation policy defined in `implementation-handoff.md` §5. Requirements §8.1 already sanctions calibrating NFR values with evidence.
4. **Syntax highlighting** — remains deferred per RFC-006. The spike does *not* evaluate highlighting engines, but must confirm styled-span rendering, which the terminal renderer requires anyway.
