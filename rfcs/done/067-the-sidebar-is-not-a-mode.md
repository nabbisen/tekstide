# RFC-067: The Sidebar Is Not A Mode

Status: **Implemented and closed 2026-10-09; released as `0.33.0` 2026-10-10.** See the
*Closed* section. **Accepted by the human owner 2026-10-08.** D1–D6 as written; **the open question decided: activating a file switches to Content mode** — see *Decided on acceptance*. Proposed 2026-10-08. `0.33.0`, M13. From the owner's question of 2026-10-08: can a
project tab show the file tree, a document and several terminals at once, while staying clean? No
requirement names this; it is a UX defect in what the project tab already offers.

## Summary

A project tab has one toggle, and it governs everything. Switching to terminals does not only change
the main area — **it takes the file tree away**, because `sidebar_view` matches on the mode and
renders the explorer only in Content mode.

What stands in its place is not another panel. It is a sentence:

> **Files are listed here in Content mode.**

The product already tells the user the file tree belongs there and asks them to leave their
terminals to get it. **The sidebar is also still focusable in that state** — `Tab` reaches a zone
whose entire content is an apology for being empty.

This RFC does not add a layout. It stops one panel from being two things.

## What already exists, and is not the problem

Terminal mode is better equipped than the toggle suggests: up to **six** terminals per project, a
session bar numbering them with their status, and **two visible at once** (`VisibleSlot::Primary`
and `Secondary`, only `Primary` taking keystrokes). *"Operate several terminals"* is largely built
and reachable. Nothing here changes it.

## Decisions

**D1 — The mode governs the main area, not the tab.** The explorer renders in both modes; the
sidebar stops matching on `ProjectMode`. `sidebar-placeholder-title` is deleted, not reworded — a
string whose only job is to explain an absence has no job once the absence is gone.

**D2 — This removes interface rather than adding it, and the slice must show that.** Nothing is
displaced: today's terminal-mode sidebar is one line of text. *Measurement 1: a live capture of
terminal mode with the file tree beside it, against the same throwaway fixture the current
placeholder appears in.*

**D3 — No new focus zone, and the `Tab` cycle keeps its order.** The `Sidebar` zone already exists
in both modes and is already reachable in terminal mode. This gives it something to hold; it must
not add a fourth zone, reorder the cycle, or change what `Tab` does.

**D4 — The cost of switching is measured, not assumed.** Switching is already lossless by
construction — the toggle sets a flag, documents keep text, cursor and viewport, terminals keep
running — but nobody has measured what the switch *costs to draw*. Reuse the paired-control harness
RFC-026 built and RFC-065 and RFC-027 reused. *Measurement 2: the render cost of a mode switch, with
the control carried inside the same run.*

**D5 — Whether anything more is needed is decided by that measurement, and "nothing" is a valid
answer.** The owner's own framing was "at a time **or a near real-time**". If the switch is below
perception, the remaining gap is only *watching* a terminal while editing, which is far narrower
than three surfaces at once. **A third slice that builds nothing, and records the number that made
it unnecessary, is a success.** Deciding this by taste is what this decision exists to prevent.

**D6 — No pane or slot vocabulary reaches the user in this RFC.** `Primary`/`Secondary` stay
internal. A user should not have to learn what a slot is to read their own screen.

## Non-goals

- **No splits, panes, or a third mode.** If D5's measurement says simultaneity is still wanted, that
  is a separate RFC with its own design, not a slice bolted onto this one.
- **No change to the terminal session bar or the six-terminal bound.**
- **No new `REQ-`.**

## Slices

- **PR-067-A — the sidebar persists.** D1, D2, D3, with the live capture.
- **PR-067-B — measure the switch.** D4.
- **PR-067-C — the decision.** D5. May build nothing; must record the number either way.

## Open question for the owner, to decide on acceptance

**In terminal mode, what happens when a file is activated in the now-visible tree?** Three shapes:

1. **It switches to Content mode and shows the file.** Predictable, conventional — but the
   sidebar's main action always ejects you from the terminals you were watching.
2. **It is refused in terminal mode.** A visible tree you cannot use is worse than no tree.
3. **It joins the open set without switching.** The active document changes where you cannot see it.

**My recommendation: 1.** Option 3 is a click whose effect is invisible, which is exactly the thing
this project's own standard forbids; option 2 puts a control on screen and then ignores it. Being
moved somewhere is a result a user can see and undo.

## Decided on acceptance (2026-10-08)

**D1–D6 as written. The open question is decided, and one consequence of deciding it is written
down here so the implementer does not have to find it.**

**D7 — Activating a file in terminal mode switches to Content mode and shows it.** The alternatives
were a control that is visible and ignored, or a click whose effect the user cannot see. Being moved
somewhere is a result a user can observe and reverse; the other two are not.

**D8 — The switch must be the user's own action, and must not happen for any other reason.** This
is the hazard D7 creates: once activating a file can change the mode, anything else that opens a
document could too. **RFC-027's own recovery offer opens documents** (`0.31.0`, shipping before
this), and a background refresh touches them. Neither may move the user out of a terminal they are
watching. **Only an explicit activation in the tree changes the mode** — not an open performed on
the user's behalf, not a refresh, not a recovery restore.

**D9 — Nothing is removed from terminal mode to make room.** The six-terminal bound, the session
bar and the two visible slots are untouched. This RFC gives the sidebar back its contents; it does
not renegotiate the main area.

## D5 answered, PR-067-C (2026-10-09): build nothing

**Measured, not assumed, per D4.** A mode switch's own view-build cost — Content mode on the
order of 20 microseconds, Terminal mode under 5, paired against each other in the same run,
`rfcs/handoffs/067-sidebar-not-a-mode/qa-evidence.md`'s own full account — against this project's
existing latency criterion for "a user would notice" (16 ms): roughly three orders of magnitude
of headroom, not a close call decided by which side of a line a noisy number landed on.

**Read against the owner's own framing** ("at a time or a near real-time," the Summary's own
question): a switch costing tens of microseconds to redraw is already near-real-time in every
sense that framing asks about. The remaining gap this RFC's own Summary named — *watching* a
terminal while editing, not operating one — is real and unclosed, but it is a narrower gap than
"three surfaces at once" was, and nothing measured here says it needs to be closed by this RFC.

**D5's own answer: build nothing.** No splits, no panes, no third mode. If that gap is ever worth
closing, it is a separate RFC with its own design, exactly as D5 and the non-goals already said —
this slice does not design it, and does not need to.

## Closed (2026-10-09)

All three slices and the Whole-RFC checklist are done. **released as `0.33.0` 2026-10-10** —
see `CHANGELOG.md`'s own status line, which does not claim "released" ahead of the actual publish.

**PR-067-A (the sidebar persists)** stopped `sidebar_view` from matching on `ProjectMode` at all:
it now renders the explorer whenever a project is active, in either mode. `sidebar_label` and the
placeholder string it composed (`sidebar-placeholder-title`) were deleted outright, not reworded —
a string whose only job was explaining an absence has no job once the absence is gone. Two further
Content-mode guards neither named by the handoff pack — in `handle_explorer_key` and
`ensure_explorer_scanned` — were found by reading the functions the task breakdown pointed at and
removed; without them the tree would have been visible but keyboard-inert, or stuck on "Loading…"
forever, in a project opened straight into Terminal mode.

**A real, pre-existing production bug was found while building the test D7/D8 required, not by
reading.** `ProjectSession::open_text_document` unconditionally forced Content mode for every
caller — harmless before this RFC, since the explorer was the only caller, but a live D8 violation
the instant RFC-027's recovery offer became a second caller of the same shared function. Fixed at
the root: the shared function no longer touches mode at all; a new, explicit
`AppState::open_active_project_content_workspace()` is called only from the explorer's own
`Action::Open` arm. D1, D7, and D8 were each independently ablated through `rfcs/handoffs/ablate.sh`
on a clean tree, not inferred from one passing test covering for another.

**PR-067-B (measure the switch)** reused RFC-026's own paired-control harness, each mode acting as
the other's control in the same run. The first version sat on the timing call's own resolution
floor — every figure an exact multiple of 4µs, medians equal to their own minimums — found at
review 509 and fixed by looping 200 view-builds inside the timed region and dividing in `f64`
nanosecond math, deliberately not `Duration::as_micros()` on an already-divided `Duration`, which
would have reintroduced the identical defect one step later. Final measured figures, release
build, two independent runs: Content mode ~18–23µs, Terminal mode ~4–5µs — roughly three orders of
magnitude of headroom against the ~16ms "a user would notice" yardstick this project already uses
for typing latency.

**PR-067-C (the decision)** answered D5 from that measurement, not from taste: build nothing. No
splits, panes, or third mode. See *D5 answered* above for the full reasoning against the owner's
own "at a time or a near real-time" framing.

**Live-captured** against a real project with both a running terminal and real output flowing,
isolated `XDG_STATE_HOME` under `/dev/shm`: `rfcs/handoffs/067-sidebar-not-a-mode/evidence/
pr-067-a/`. The book was swept for both a stale claim (`keyboard-reference.md`) and a real
capability it had never described at all (`what-works-today.md`'s own explorer section).
