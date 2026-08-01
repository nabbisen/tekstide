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

## Active work: PR-015-E — `0.4.1` (added 2026-08-01)

`0.4.0` shipped on 2026-08-01 with PR-015-B/C/D/F/G. **PR-015-E is next**, and its entry in `task-breakdown-pr-plan.md` lists two of its four obligations — the other two (C4 / `NFR-PERF-002`, and the chrome-level focus indicator) arrived through review responses and the `0.4.0`/`0.4.1` split.

- **[`pr-015-e-mode-switching.md`](./pr-015-e-mode-switching.md)** — read this before the task-breakdown entry.

The focus indicator is the one to read first: `0.4.0` shipped without it, defensibly, because `FocusZone` has a single variant so there is nothing to distinguish. **This slice adds the second zone, and the defence expires the moment it does.**

## Two things landed ahead of you (added 2026-07-30)

RFC-016 was implemented before RFC-015, so two seams this RFC was originally expected to *create* already exist. **Conform to them; do not redesign them.**

1. **The i18n call shape is fixed: `i18n::Catalog::resolve` and `Catalog::get`** (`crates/tekstide/src/i18n.rs`, PR-016-B). The original RFC-016 handoff said RFC-015 would create a placeholder that PR-016-B replaced, with an instruction not to change the call shape. That is now inverted — PR-016-B established the shape because RFC-015 had not landed, so **RFC-015 conforms to it.** If it proves awkward for a renderer, raise it before building surfaces rather than working around it locally.

2. **Untrusted text renders through `tekstide_core::text_safety`** (PR-016-C). Every untrusted span a surface displays — command text, project names, branch names, file paths, terminal-derived strings — goes through `quote_untrusted`, never raw. Trusted chrome (localized labels) does not. The escaping is already shared with `approval::coordinator`; **do not add a second escape path in the shell.** RFC-016 §Risks: *"escaping belongs to the shared untrusted-text render path, not to each surface."*

Also relevant to PR-015-D: `tekstide_core::shell::render_text()` is the pre-GUI text harness, and it holds roughly sixteen hardcoded user-facing strings in `tekstide-core` — outside the catalog's reach by design, since the shell crate owns rendering. **PR-015-D is expected to delete it**, and RFC-016 PR-016-E's no-hardcoded-strings scan is waiting on that. If you do not delete it, say so explicitly, because PR-016-E's scan scope depends on the answer.

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
