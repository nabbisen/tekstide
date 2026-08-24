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

**Latency criteria stop the clock at state change, not at pixels.** Every `NFR-PERF-*`
latency figure in this project is measured from input arriving at the application to the
application's own state being updated. **Compositor and GPU present time are excluded.**

This is not a convenience. The only in-process way to timestamp presentation,
`iced::window::frames()`, *forces continuous redraw once subscribed* (RFC-015 §R1) — so it
does not merely add load, it replaces redraw-on-demand with redraw-always and changes the
mechanism under measurement. A number obtained that way would not describe the shipping
product.

State the boundary whenever you report one of these figures. It was left implicit for
three criteria and cost real work: `NFR-PERF-002` and `NFR-PERF-003` were quietly measured
this way and read as though they covered pixels, while `NFR-PERF-004` was measured to
pixels and declared unverifiable — the same silence, failing in opposite directions.
Corrected 2026-08-15; see RFC-017 Amendment 1.

Also state the **load condition**, and do not let it drift. `NFR-PERF-004` names "bounded
output"; the script used for it became a *saturating* producer (~250,000-500,000 wakes/sec)
when readiness-driven I/O removed the tick that had been throttling it, and nobody chose
that. A test condition whose meaning depends on a defect elsewhere will change silently
when the defect is fixed.

**Synthetic input for GUI evidence: use `wtype`, not `xdotool`.** RFC-015's evidence
established an XWayland route — `env -u WAYLAND_DISPLAY`, `xdotool search --name`,
`windowfocus`, `key --clearmodifiers` — and it **no longer works here**: `xdotool search` finds
no window at all, because the app runs as a native Wayland client and there is no XWayland
surface for the title to match. Confirmed as a negative result during PR-020-B (2026-08-18),
not assumed: a capture taken that way showed the unchanged prior screen, proving the keystroke
never landed. `wtype -M ctrl -M alt r -m alt -m ctrl` (a native Wayland virtual keyboard)
works. Screenshots remain `niri msg action screenshot-window`. Recorded here rather than in
RFC-015's pack, because closed evidence documents are not edited to match a later state — and
a convention nobody can execute is worse than none, since it costs the next slice the same
hour to rediscover.

**Enumeration tests: pick the unit that matches the property.** A source-scanning test that
guards "only X may do Y" comes in two shapes, and choosing wrongly silently weakens it.

- **A file-level allow-list is right when the allowed file *is* the reviewed implementation.**
  `only_this_module_opens_a_transcript_file_for_reading` allows `transcript/reader.rs`; a
  second read inside the reviewed reader is the same reviewed code, so the file is the right
  unit.
- **An occurrence count is required when the property is "every call site must also do Z."**
  There, a second call *inside an allowed file* is a new, unreviewed site — and a boolean
  `source.contains(..)` against a file allow-list passes it silently. Assert the exact count.

Both shapes look identical when written, which is why this is worth stating: the difference is
in the property, not the code. `only_boot_calls_add_project_from_path_...` (RFC-031) was
written in the first shape against a second-shape property — every caller of
`add_project_from_path` must also write an audit record — so a second call added to `main.rs`
would have passed a test whose own name promised it could not. Corrected to a count.

Related and older: a needle must not match its own definition line, and a scan that matches a
bare identifier will match doc comments mentioning it. Require the call syntax, not the name.

**Reachability comes before correctness.** Before a surface is scheduled, name the path a
user takes to reach it and the production code that populates what it renders. Not "which
RFC owns it" — the actual call site.

This convention exists because it was missing. RFC-020 was scheduled
as `0.8.0`'s spine, and its handoff pack written and accepted, before anyone checked
whether an `AgentRun` or a `ChangeSet` could be created at all. Neither can:
`launch_agent_run_with_runtime` and `add_detected_generated_change_set` have no production
caller, so both of RFC-020's surfaces would have rendered nothing, forever. RFC-021 and
RFC-024 had already shipped correct, reviewed, unreachable models for the same reason.

Every review gate in every handoff pack asked whether the rendering was **correct**. None
asked whether anything could **reach** it. A model with no producer and a surface with no
route are the same defect, and neither is visible from inside the slice that builds it.

**A field that asserts a state is not a reference, and a sweep for references will not
find it.** When a rename or a move lands, the obvious sweep is for paths and links — they
break loudly, so they get fixed. Three other kinds of text name the same state and stay
silent when it changes: a *description of structure* (a folder list in prose), a *status
field* (`Status:`, `source_rfc_status:`), and a *count* ("the last M11 item", "all four
presets"). None of them contains the old path, so a grep for the path cannot reach them.

The RFC-037 five-folder migration is the worked example, and it took three separate people
to finish. The migration itself swept `proposed/0` and repointed every `rfc_file:` — all
references, all correct. It missed `ARCHITECTURE.md`'s own authoritative folder list, caught
by the owner asking whether the README was stale. It missed `source_rfc_status:` in RFC-023's
pack, caught by the dev team while working in that pack. Generalizing that second catch found
nine more in four other packs, stale since those RFCs closed — some for weeks.

So the rule is not "sweep harder." It is: **after a move, grep for the old *state word*, not
just the old path** — `Proposed`, `Scheduled`, `Ready for implementation` — and for handoff
packs specifically, the check is mechanical, because the folder is the source of truth:

    for each rfcs/handoffs/NNN-*/*.md with a source_rfc_status field,
    the value must agree with which of rfcs/{proposed,accepted,done,archive}/ holds RFC-NNN.

That invariant is checkable in a dozen lines and would have caught all thirteen files without
anyone noticing anything.

**An ablation changes exactly one thing.** Break the property, watch the *specific* test
fail, restore. If reaching the property requires also breaking something else, **stop**: what
you have proven is that the something-else is guarded, and the property you set out to test
has no test at all.

`0.12.1` recorded five ablations and had four. The fifth gave a `Reserved` navigation action a
help description *and* widened the policy filter that would otherwise keep it out of the help
builder — because the description is unreachable without that second change. Three tests
failed and were counted as evidence. They were the filter's tests, already run as the first
ablation. The reason was written in a comment on the ablation itself — *"also break the core
filter so the reserved rule reaches the GUI"* — which is the finding, stated and then read as
setup. Found by the dev team auditing the commit.

So the older rule (*a green ablation is a defect in the ablation*) has a twin: **a red
ablation that needed two edits is a defect in the ablation too.** Only single-variable results
are evidence.

**The unit is the design decision, not the line.** The mirror failure showed up reviewing
RFC-038 PR-038-B: the slice deliberately dispatches one action outside `app_command_for`'s
`Some` arm, so the reviewer's ablation added the "obvious" mapping back — one line, one
variable — and the suite stayed green. It stayed green because the special case was still
present and repaired the route immediately afterwards. The ablation had changed a line without
changing the *decision*. Re-run as "use the obvious mapping **instead of** the special case" —
two mechanical edits expressing one design choice — it failed on exactly the right test.

Both halves of the rule are about the same thing: an ablation must express the alternative a
future maintainer would actually write. Two edits that are one decision are fine. One edit that
leaves an existing repair standing is not.

**If your slice makes a shipped statement false, correcting it is part of your slice.** Not
scope creep, and not something to file for later. RFC-038 PR-038-A added the in-app way to open
a project, which falsified the same sentence in two shipped places — `tekstide --help`'s usage
text and the README's Quick Start, both saying "there is no in-app way to add a project." The
implementer fixed both and asked whether that was the right amount of scope. It was.

The reasoning is this project's own history: a false claim in user-facing text survived twelve
releases because correcting it was always somebody else's slice. A statement your work
falsifies has a known author, a known location and a known fix, at exactly the moment you
falsify it, and never again so cheaply. Adding capability outside your slice is scope creep;
correcting a claim your slice just made untrue is finishing it.

**A premise that would surprise a user is a finding.** When narrow reasoning reaches for a
fact about the product to support itself, ask whether that fact would startle someone who had
just installed it. If so it is not scaffolding, it is a finding, and it belongs at the top of
its own review request the day it is written.

The worked example cost twelve releases. On 2026-08-19 the dev team wrote, correctly, in
RFC-031's acceptance checklist: *"there is no interactive 'Add Project' GUI flow yet."* It was
load-bearing for a decision about one audit producer, and as that reasoning it was flawless.
As a fact about the product — *a user cannot add a project* — it went nowhere; the architect
read it, quoted the surrounding paragraph, and closed the RFC on it. Neither party missed the
fact. Both had it, and it had no route to become a finding. The owner found it by running the
program three days later.

**A claim about behaviour cites the command that produced it.** Not "verified", not "confirmed"
— the command, in the document. Every false claim this project has published came from
reasoning carefully about code instead of running it: "no transcript is ever written" (one
crate grepped, false in two releases), "0.12.0 is ready" (every gate green, the binary never
launched, twelve releases of a UI naming actions that do not exist), "five ablations" (above).

The rule is asymmetric on purpose and applies hardest to whoever reviews. **Anyone may reject
a behavioural claim that does not say how it was checked**, including the dev team rejecting
the architect's, and doing so needs no further justification than its absence.

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
| `rfcs/proposed/`, `rfcs/accepted/`, `rfcs/done/`, `rfcs/archive/` | RFCs. **The folder is the source of truth for lifecycle state** (RFC-000, 5-folder variant adopted by RFC-037); the Status field moves in the same commit as the file. `accepted/` is where startable work lives — reviewed, not yet shipped — and an RFC stays there while it is being implemented, because `done/` means shipped. An empty `proposed/` is correct, not a missing folder. |
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
