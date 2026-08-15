# Tekstide Architecture

Orientation for contributors. What the crates are, where the boundary between them
falls, and what the project's vocabulary means. For *why* any particular decision was
made, the RFCs in [`rfcs/`](./rfcs/) are the record; this file is the map.

## Two crates, one boundary

```
crates/
  tekstide-core/   state, domain, and policy.  Never renders.
  tekstide/        the GUI shell.  Renders, routes input, owns I/O resources.
```

The names do not say this, so it is written down here.

**`tekstide-core` decides. `tekstide` renders the decision and collects the answer.**

That is the whole boundary, and nearly every architectural review in this project has
turned on it. Concretely:

| | `tekstide-core` | `tekstide` |
| --- | --- | --- |
| Owns | domain models, security policy, the audit schema, project/session state | window, layout, widgets, input routing, clipboard, PTY panes |
| Decides | which ANSI sequences are accepted, whether a paste is allowed, what a limit is | nothing |
| Depends on | no GUI substrate, no `vte`, no `alacritty_terminal` | `iced`, and `tekstide-core` |
| Published as | `tekstide-core` on crates.io | `tekstide` (the installed binary) |

**If you find yourself writing a classification rule in `tekstide`, stop.** That rule
either already exists in core, or it belongs there, or it needs an RFC amendment. A
decision made in the shell is a decision that can drift from the one core makes, and the
project has paid for that twice — escaping duplicated into `approval::coordinator`
(consolidated by RFC-016 PR-016-C), and string-scan seams duplicated between RFC-015 and
RFC-016.

The reverse also holds: core does no I/O the shell should own. `RecentProjectStore` and
the audit store are opened by the shell and handed to core's coordinators, not resolved
inside core.

### Why `tekstide-core` has no GUI dependency

It is checkable, and it is checked:

```sh
cargo tree -p tekstide-core --edges normal | grep -ciE 'vte|alacritty'   # must be 0
```

RFC-017 interposes the terminal security filter at `vte`'s handler boundary. Implementing
that requires `vte`, so the *adapter* lives in `tekstide`. It holds **no policy of its
own** — every accept/reject decision is delegated to core. That is what keeps the two
from diverging: an adapter that cannot decide anything cannot decide differently.

## The properties that hold the whole thing up

Four invariants recur across RFCs. Breaking one is a security regression, not a
refactor.

**Single ingress.** Every byte from a PTY reaches the emulator through exactly one
filter entry point, and every byte written *to* a PTY leaves through exactly one gated
call site. Both are enumerated by tests that resolve each call site's enclosing function
by name — a new call site fails the test rather than being caught in review.

**Modal exclusivity.** While a dialog is open, terminal input is **not produced** — the
subscription that would produce it is not called at all. A second check at the write site
is defence in depth behind that, and both are independently tested. Neither substitutes
for the other.

**Untrusted text is escaped at render.** Project names, branch names, pasted content —
anything the user or a program supplies — goes through `text_safety::quote_untrusted`
before reaching trusted chrome. **The terminal grid is the one deliberate exception**,
because escaping terminal output would corrupt it. The exception is the grid, never the
chrome around it.

**The audit store records what happened, not what was convenient.** Producers write
through `AuditCoordinator`, never to the store directly, and an audit write failing must
never fail the operation it observes.

## Evidence conventions

This project treats evidence as a deliverable, not a formality.

**Ablation.** To claim a test proves a property: break the property, watch that specific
test fail, restore it. One ablation per property. **A green ablation is a defect in the
ablation, not a pass** — it means blocking and bypassing the thing were indistinguishable
by whatever the test observed.

The recurring failure mode is *a test that passes with the thing it tests deleted*. It
has occurred at least six times here. Positive controls are the general answer: assert
that your check reaches real data before asserting what it does not find.

**Screenshots** state what they prove **and what they do not**, and live under
`rfcs/handoffs/<rfc>/evidence/`.

**Claims** are checked against the RFC's own text at closeout, not only against the
evidence file — an RFC has twice asserted something its own results had falsified.

**Reachability comes before correctness.** Before a surface is scheduled, name the path a
user takes to reach it and the production code that populates what it renders. Not "which
RFC owns it" — the actual call site.

This is the newest convention and it exists because it was missing. RFC-020 was scheduled
as `0.8.0`'s spine, and its handoff pack written and accepted, before anyone checked
whether an `AgentRun` or a `ChangeSet` could be created at all. Neither can:
`launch_agent_run_with_runtime` and `add_detected_generated_change_set` have no production
caller, so both of RFC-020's surfaces would have rendered nothing, forever. RFC-021 and
RFC-024 had already shipped correct, reviewed, unreachable models for the same reason.

Every review gate in every handoff pack asked whether the rendering was **correct**. None
asked whether anything could **reach** it. A model with no producer and a surface with no
route are the same defect, and neither is visible from inside the slice that builds it.

## Glossary

Terms this codebase uses without explanation. Domain vocabulary first, then house terms.

| Term | Meaning |
| --- | --- |
| **PTY** | Pseudo-terminal. The kernel device pair that lets Tekstide run a real shell and read its output. Standard Unix vocabulary (`pty(7)`, `openpty`, `/dev/pts`), used throughout RFC-007/008/017. |
| **VTE** | The terminal-emulation crate parsing ANSI/VT escape sequences. The security filter interposes at its handler boundary. |
| **CSI / OSC** | Escape-sequence families. CSI is cursor/formatting control; OSC sets things like window titles and clipboard. Some are accepted, some rendered inert — RFC-009 decides which. |
| **Grid** | The terminal's cell matrix. Renders untrusted bytes unescaped, and is the *only* place that exception applies. |
| **Chrome** | Tekstide's own UI around the content — sidebar, top bar, status bar, dialogs. Never renders unescaped untrusted text. |
| **Surface** | A rendered region under RFC-015's contract: it may not duplicate core state, render trusted chrome, or reach modal state. |
| **Immersion mode** | The layout where a project's terminals fill the main area, as opposed to Content mode. |
| **Ingress** | A path by which bytes enter a boundary. "Single ingress" means exactly one such path exists, provable by enumeration. |
| **P1–P4** | The terminal filter's four properties: single ingress, no side channels, classification parity with the emulator, and stream-position independence (classification unchanged by how bytes are chunked across reads). |
| **Ablation** | Deliberately breaking a property to confirm the test that claims to detect it actually fails. See above. |
| **Spike** | A throwaway experiment answering one technical question, in a crate marked `publish = false`. Extreme Programming jargon, not Rust vocabulary. Both spike crates were deleted once their properties had product-code equivalents; RFC-014 records the conditions that had to hold first. |
| **Positive control** | An assertion that a check reaches real data, added so that "nothing found" cannot mean "nothing looked at." |
| **Handoff pack** | The `rfcs/handoffs/<rfc>/` document set an implementer reads before starting. `README.md` is always the entry point. |

## Where things live

| Path | What |
| --- | --- |
| `rfcs/proposed/`, `rfcs/done/`, `rfcs/archive/` | RFCs. **The folder is the source of truth for lifecycle state** (RFC-000); the Status field moves in the same commit as the file. |
| `rfcs/handoffs/<rfc>/` | Implementation instructions, review gates, and recorded evidence per RFC. |
| `rfcs/future-work.md` | The durable index of deferred work. If something is deferred and only recorded in a closed RFC's evidence, it is lost. |
| `rfcs/delivery-plan.md` | RFC queue, release-cycle tracking, standing constraints. |
| `ROADMAP.md` | Milestones and their versions. |

## Reading order for a new contributor

1. This file.
2. [`rfcs/delivery-plan.md`](./rfcs/delivery-plan.md) §Standing Constraints — six rules every RFC inherits.
3. [`rfcs/done/009-terminal-security-boundary.md`](./rfcs/done/009-terminal-security-boundary.md) — the security model most other decisions defer to.
4. [`rfcs/done/015-application-shell-and-rendered-surface-model.md`](./rfcs/done/015-application-shell-and-rendered-surface-model.md) — the surface contract and input model every rendered surface plugs into.
5. The handoff pack for whatever you are implementing, starting at its `README.md`.
