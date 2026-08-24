---
title: "RFC-039: task breakdown and PR plan"
rfc: "RFC-039"
rfc_file: "../../accepted/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Accepted 2026-08-24 — M12, after RFC-038"
target_milestone: "M12"
created: "2026-08-24"
---

# Four slices, executed A → B → C → D

Order is stated once, letters are never renamed. Same rule as RFC-038's pack, and for the same
reason: this project lost a day to slice letters that implied an order they did not have.

**Start after RFC-038 PR-038-C.** That slice moves the keyboard list off the Project Board into
Help; building a tab strip into the top bar while the board is still rendering a nine-line
reference list would mean fighting over the same screen twice.

## PR-039-A — the strip exists and shows what is open

**Build:** a project tab strip in the existing top bar, rendering the open projects, with the
active one visibly distinct. Read-only in this slice: it shows, it does not yet act.

- `view()` already composes `column![top_bar, content_area, status_bar]` in **every** mode,
  Terminal Immersion included — verified before this was decided. The strip goes in that
  existing chrome; no surface loses space it has today.
- Every project name is **untrusted text in trusted chrome**: escaped, and bounded so one long
  name cannot push the strip off-screen. Test with the bidi-override fixture already in this
  project's recent-projects state.
- The active project must be distinguishable **without relying on colour alone** — RFC-015's
  focus indicator established that rule and it applies here.

**Evidence:** two projects open, a screenshot of the strip in Content mode and in Terminal
Immersion, showing it survives both.

## PR-039-B — the strip acts: switch, and go home

**Carried in from PR-039-A's review (request 306): separate "active" from "focused".**
PR-039-A renders the active project with `zone_style` + `focus_marker` — this shell's **focus**
indicators, the same pair the sidebar and main-area zones use. Harmless while the strip is
read-only. The moment tabs are keyboard-operable, a user tabbing to a non-active project must
see where their keyboard is, and "focused tab" and "active project" would render identically —
on the first `Tab` press, not in some corner case.

**Focus keeps `zone_style` + `focus_marker` unchanged** — focus indication stays consistent
across the whole shell, because a user learns that border means "where my keyboard is".
**Active-project moves to a different channel**, your choice of which, non-colour-only per
RFC-015, and legible when a tab is both active and focused. **Evidence: a screenshot of a tab
focused but not active, beside the active one** — that frame is the proof, and string-level
tests cannot supply it.

**Build:** activating a tab switches to that project. A permanent leftmost **Projects** entry
returns to the board. Both mouse- and keyboard-operable.

- Switching needs `SwitchActiveProject`, today `Configurable` with **no binding** — one of the
  four dead actions RFC-036 tracks. Giving it a real route is part of this slice; say so in the
  evidence, because it takes RFC-036's dead-action count from four to three.
- Returning home is a **visible control**, not only `Ctrl+Alt+P`. The keystroke stays as an
  accelerator.
- A tab is not a spoofing risk on activation — it switches to the project it actually is — but
  the escaping from PR-039-A is unchanged, not relaxed.

**Evidence:** a cold start in which two projects are opened, switched between, and the board
returned to, using only visible controls. No keystroke a user would have had to learn.

## PR-039-C — close a project

**Read [`what-closing-a-project-must-not-lose.md`](./what-closing-a-project-must-not-lose.md)
first.** It is short and it is the only destructive action in this RFC.

**Build:** `×` on a tab closes that project, wiring core's `close_project` — reviewed, tested,
and never called by any GUI code until now.

- Idle project closes directly; live terminals or an active agent run raise a confirmation
  naming **counts**, not vague warnings.
- The confirmation identifies the project by **canonical path**, escaped and bounded.
- **Wire `safe_close_decision`**, both outcomes — closed and cancelled. Not at closeout.
- Closing is not purging: a test proves transcripts and audit records survive.
- Read `close_project`'s contract on child processes before assuming. If it does not stop them,
  **escalate** rather than compensating in the surface.

## PR-039-D — the affordance audit, and closeout

**Build nothing. Find things.**

Every `NavigationAction`, and every capability a user is expected to perform, listed against the
visible control that invokes it. Anything with no control is a **finding, reported** — not
quietly given a keybinding, which is the habit this RFC exists to break.

Expect it to surface real gaps; the ones already known are a starting point, not the answer:
`OpenDiffReview` and `OpenSafeCloseDialog` are dead, `OpenCommandPalette` is reserved with
nothing behind it, and `SwitchActiveProject` will have been resolved by PR-039-B.

Then the ordinary closeout: `qa-evidence.md` filled, checklist ticked with citations, known
limitations stated, RFC-039's acceptance criteria answered in its own words.

## Standing expectations

- **Name the control, not the keystroke.** A slice claiming a workflow is served must say what
  the user sees. This is the review gate for every PR here.
- **Single-variable ablations**, where the unit is the design decision rather than the line.
- **Disclose flakes** against `test-process-leak.md`'s table before reporting; four symptoms are
  known.
- **If your slice makes a shipped statement false, correcting it is part of your slice** — the
  README and `--help` both describe how to move around this product.
