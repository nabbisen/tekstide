# RFC-039: Interaction Model and Visible Affordances

Status: **Proposed 2026-08-24.** Written after the human owner reviewed `0.12.1` and RFC-038's
first two slices and said: *"What is the most important is to design user workflow(s) and make
UI/UX to help them. Currently, no button or link to open project tab, close it, return to the
entrance etc."*
Target milestone: **M12, after RFC-038** — accepted by the human owner 2026-08-24, ahead of
RFC-020 and RFC-034.

Its three open questions were **decided by the architect on acceptance** and are recorded as
decisions below. Each is the owner's to overturn; none is the implementer's to inherit.
Date: 2026-08-24

Related RFCs:

- [RFC-003](../done/003-information-architecture-and-ui-mode-model.md) — the IA and mode model
  this RFC does not replace; it supplies the missing layer between that model and the user.
- [RFC-005](../done/005-application-shell-and-project-board.md) — owns the Project Board whose
  rows are, today, inert text.
- [RFC-038](../accepted/038-first-run-and-project-entry.md) — project entry. Overlaps
  deliberately and is bounded in §Relationship below.
- [RFC-036](../accepted/036-dormant-capability-closure.md) — `close_project` joins its list.

## Summary

**This product has no interaction design. It has a keybinding policy that has been mistaken for
one.**

Every capability shipped so far was built, reviewed, tested, and then made reachable by binding
a `Ctrl+Alt+<letter>` to it. Each slice asked, correctly, *can a user reach this?* — and answered
it with a keystroke. No slice asked *what is the user trying to do, and what do they see?*

The result is measurable, not a matter of taste:

| | |
| --- | --- |
| Buttons in the entire application | **5**, all inside Trust Settings and Approval History |
| Buttons on the Project Board — the surface a user arrives at | **0** |
| Project Board rows that can be clicked to enter a project | **0** — they are text |
| Visible way to return to the Project Board | none; `Ctrl+Alt+P` if you know it |
| Visible way to close a project | none — and `close_project` has **no production caller** |

The last row is the tell. Closing a project is not a missing button; it is a **dormant
capability**, built and reviewed in core and never reached from anywhere, the same orphan
pattern as `set_resource_limits`, `to_ai_cli_profile` and the `sensitive_config_changed`
producer. The reachability audit found seven of those and never noticed this one, because it
searched for functions with no callers and not for *actions a user cannot take*.

## The workflows, and what each is missing

This is the substance of the RFC: the product's work, described as a person experiences it.

| # | The user wants to | Today | Missing |
| --- | --- | --- | --- |
| 1 | **Arrive and understand what this is** | A wall of nine shortcuts on the main pane | One clear action; reference material moved to Help |
| 2 | **Open a project** | Type or paste a filesystem path | A folder browser (RFC-038, owner-directed) |
| 3 | **See which projects are open, and pick one** | Inert rows of text | Selectable, activatable rows |
| 4 | **Enter a project and work in it** | `Ctrl+Alt+M` toggles mode, if known | A visible route from a board row into the project |
| 5 | **Return to the entrance** | `Ctrl+Alt+P` | A visible, always-present way back |
| 6 | **Close a project** | Impossible from the GUI | A control, and `close_project` wired to it |
| 7 | **Find out what the app can do** | Read `navigation.rs`, or the board's shortcut list | A Help surface that owns this material |

## Principles

Three, and they are the RFC's real content — every decision below follows from them.

**1. Every action a user needs has a visible control.** A keyboard shortcut is an *accelerator
for an action that is already visible*, never the only route to it. This project has shipped
nine bindings and five buttons; that ratio is the defect.

**2. Reference material does not live on a working surface.** The keyboard list belongs in Help.
It was put on the Project Board in `0.12.1` because there was nowhere else for it — a real
discoverability gap papered over by pushing reference text onto the primary surface. Help is the
answer; the board is not.

**3. A capability with no visible affordance is not shipped.** The existing reachability
doctrine says *name the path a user takes to reach it*. That was satisfied, repeatedly, by
naming a keystroke. It is hereby not sufficient: name the **control the user sees**. A binding
alone means the capability is reachable by someone who has read the source.

## Scope

- **Project Board rows become interactive**: selectable, and activating one enters that project.
- **A visible route back to the board** from any project surface.
- **Close a project**, with `close_project` wired to a real control — including what happens to
  its running terminals and agent runs, which is a real question this RFC must answer and not
  assume.
- **A Help surface** owning the keyboard reference, and its removal from the Project Board.
- **An affordance audit**: every `NavigationAction`, and every capability a user is expected to
  perform, listed against the visible control that invokes it. Actions with none are findings.

## Non-goals

- A theme, icon set, or visual redesign. This is about *what exists to interact with*, not how
  it looks.
- Mouse-only interaction. Everything added must remain keyboard-operable; RFC-015's focus model
  and RFC-018's trusted-UI rules apply unchanged to every new control.
- Replacing RFC-003's information architecture. That model is sound; nothing was built on top
  of it.
- Tabs specifically. The owner's phrase was "project tab"; whether the answer is tabs, a
  sidebar, or board-plus-back is a design question this RFC owns and does not prejudge.

## Relationship to RFC-038

RFC-038 keeps **project entry**: the folder browser, the path field, `Ctrl+Alt+O`, recent
projects, and the Help surface (which it was already building, and which now also removes the
board's shortcut list).

RFC-039 takes **everything after entry**: rows that respond, entering and leaving a project,
closing one, and the affordance audit. RFC-038 must not grow into this; a slice quietly becoming
a redesign is how scope stops being reviewable.

## Decisions (were open questions; settled on acceptance)

**D1 — the form is a project tab strip in the existing top bar.**

Checked before deciding rather than assumed: `view()` composes
`column![top_bar, content_area, status_bar]` in **every** mode, Terminal Immersion included. So
a tab strip in the top bar has no conflict with immersion — the chrome a strip would live in is
already always present, and no existing surface loses space it currently has.

One persistent affordance then serves four of the seven workflows at once:

| Workflow | The control |
| --- | --- |
| 3. See which projects are open, pick one | the strip itself |
| 4. Enter a project | click its tab |
| 5. Return to the entrance | a permanent leftmost **Projects** tab — the board |
| 6. Close a project | `×` on the tab |

A `+` at the end of the strip opens RFC-038's folder browser, which makes "open another
project" a visible action from anywhere rather than a second keybinding.

Chosen over board-plus-back (which serves 5 but leaves 3 and 4 unserved, and gives close no
home) and over a sidebar (which costs horizontal space permanently, in a product whose main
surfaces are a terminal grid and an editor). The owner's word was "project tab"; this is that,
arrived at against the constraints rather than adopted from the phrasing.

**D2 — closing a project with live work confirms; closing an idle one does not.**

A project with no running terminal and no active agent run closes directly. One that has either
raises a confirmation naming what will be lost. A confirmation on every close trains people to
dismiss confirmations, which is the failure mode RFC-018's paste model exists to avoid.

**This unblocks `safe_close_decision` and this RFC wires it.** That audit family has never had a
producer — RFC-031 scoped it out explicitly, "blocked on a dialog that does not exist". The
dialog now exists, so the blocker is gone, and wiring it takes the unwired families from two to
one. Completing a gap at the moment its blocker is removed is the same discipline as correcting
a statement at the moment your work falsifies it.

**D3 — escaping is necessary and not sufficient for the close control.**

Project names and paths in the strip are untrusted and escaped, as everywhere else — and per
RFC-018 the grid exception does not apply here, because a tab strip is **trusted chrome**.

For *switching*, a misleading label is a wrong belief and not a wrong action: activating a tab
switches to the project that tab actually is. For *closing*, it is a wrong action with real
consequences — terminals killed, an agent run interrupted. So **the close confirmation must
identify the project by its canonical path, escaped, not by its display name alone.** A user
must be able to tell which project they are about to close even if its name was chosen to look
like another's.
