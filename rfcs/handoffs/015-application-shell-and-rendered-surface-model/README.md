# RFC-015: Application Shell and Rendered Surface Model - Developer Handoff Pack

Source RFC: [RFC-015](../../proposed/015-application-shell-and-rendered-surface-model.md)
Target milestone: **M8**
Source RFC status: **Proposed**

**Start here.** This file is the entry point. Everything is linked below in reading order.

## Read in this order

| # | Document | Purpose |
| --- | --- | --- |
| 1 | [RFC-015](../../proposed/015-application-shell-and-rendered-surface-model.md) | Surface contract, layer model, input routing, seams. **Read "Input routing" before writing any code.** |
| 2 | This file | Orientation and what is binding. |
| 3 | [`implementation-handoff.md`](./implementation-handoff.md) | Module layout, the seams, `CountDisplay` fidelity, R1 measurement. |
| 4 | [`pr-015-c-input-routing.md`](./pr-015-c-input-routing.md) | **Detailed instructions for PR-015-C, the security-critical slice.** Read before designing the message enum. |
| 5 | [`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md) | Slice boundaries and review gates. |
| 6 | [`acceptance-qa-checklist.md`](./acceptance-qa-checklist.md) | Required evidence. |
| 7 | [`qa-evidence.md`](./qa-evidence.md) | Where you record gates, findings, and limitations. |

Read before starting — RFC-015 conforms to these rather than amending them:

- [RFC-009](../../done/009-terminal-security-boundary.md) — §212 requires trusted dialogs rendered outside terminal output. This is the property PR-015-C must make structurally true.
- [RFC-014](../../proposed/014-desktop-gui-substrate-and-terminal-rendering.md) — the substrate decision and its residual risks **R1** (latency unverified) and **R6** (focus-trap property does not transfer). RFC-015 discharges both.
- [RFC-005](../../done/005-application-shell-and-project-board.md) — the `ApplicationShell` and Project Board state you are rendering.

## Where to start work

**Begin at PR-015-B.** PR-015-A is design acceptance.

## Five things that are binding

1. **The shell is a view, not a model.** All state lives in `tekstide-core`. Render `shell.state()`, dispatch `AppCommand`. **Do not add state to the shell that mirrors core state** — that is the most likely architectural drift in this RFC.
2. **The modal layer is unreachable from surface code.** A surface may emit a message; only the shell may open, populate, or dismiss a modal. This is what makes RFC-009:212 structural rather than conventional.
3. **Exactly one input sink at a time.** Modal > terminal text focus > shell focus cycle. See `pr-015-c-input-routing.md`.
4. **No hardcoded user-facing strings, colours, or font sizes.** RFC-016 and RFC-023 fill these seams later; building without them now means retrofitting the whole UI.
5. **`CountDisplay` fidelity.** An unavailable or not-implemented count must never render as `0`. RFC-005 built that distinction deliberately; the rendered surface is where it is easiest to lose.

## Reuse from the RFC-014 spike

`crates/tekstide-gui-spike` is retained until this RFC closes out. Directly reusable as reference:

- the `stack`/`opaque` modal composition proven by C8;
- the `[focused]` text-prefix pattern that satisfies `NFR-UX-002`;
- `font_metrics.rs` (C7) for the layout work RFC-017 will need;
- the measurement harness shape — **with** the survivorship-bias caveat recorded in RFC-014 R9.

Reuse the patterns; do not copy spike code wholesale into product. The spike had no state discipline because it needed none.
