# RFC-038: First-Run and Project Entry

Status: **Accepted by the human owner 2026-08-24.** Written the day the owner ran the `0.12.0`
executable and reported it "wholly useless. No help for me to operate. No action available."
Target milestone: **M12, first** — ahead of RFC-020 PR-020-C, which adds a surface to a product
a new user still cannot get into.

The three design questions below were **decided by the architect on acceptance** and are
recorded as decisions, not questions: an implementer must not inherit an unresolved
architecture decision. Each records the recommendation the owner scheduled and the reasoning,
and any of them is the owner's to overturn.
Date: 2026-08-22

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`

Related RFCs:

- [RFC-005](../done/005-application-shell-and-project-board.md) — owns the Project Board and
  `ProjectBoardEmptyState`, whose `primary_action` / `secondary_action` fields this RFC removes.
- [RFC-015](../done/015-application-shell-and-rendered-surface-model.md) — PR-015-D built the
  empty state that rendered those two fields' names as inert text.
- [RFC-036](../accepted/036-dormant-capability-closure.md) — the wire/delete/document pass over
  capabilities with no production caller. This RFC is the same defect one layer up: a
  *product* with no user-reachable entry point.

## Summary

**Tekstide has no in-app way to open a project.** The only production caller of
`add_project_from_path` is `std::env::args_os().skip(1)` in `main.rs`. Launched with no
argument — which the published Quick Start told users to do until `0.12.1` — the application
shows an empty Project Board, and nothing the user can do from inside the window will ever
put a project on it.

Until `0.12.1` that empty board also rendered the strings **"Add Project"** and **"Open from
path"** as plain `text()` widgets: the names of two actions that do not exist, with no button
and no handler behind either. `0.12.1` replaced them with the truth and added the keyboard
list and `--help`. **That was a correction, not a fix.** This RFC is the fix.

## The failure this RFC exists to record

This project has an explicit doctrine, in `ARCHITECTURE.md`, that predates this RFC by weeks:

> **Reachability comes before correctness.** Before a surface is scheduled, name the path a
> user takes to reach it and the production code that populates what it renders.

It was applied to individual capabilities at least seven times — the reachability audit found
seven orphaned functions, and RFC-031, RFC-033 and RFC-036 exist to close them. It was never
once applied to the application itself. The audit asked "can a user reach this capability?" of
every capability in the model, and never asked **"can a user reach the application?"**

The tell was in plain sight and was read as a feature list rather than as evidence: `0.12.0`'s
release notes describe every capability by keyboard shortcut, because shortcuts are the only
route to any of them. Nobody asked how a user learns a shortcut. The answer was that they
could not — the string `Ctrl` appeared **zero times** in the entire user-facing catalogue
while nine bindings were live.

Recorded here rather than in a post-mortem because the same shape will recur: a rule applied
diligently at one altitude and never at the altitude above it.

## Goals

1. **A user who runs `tekstide` with no arguments can open a project**, without a terminal,
   without documentation, and without knowing a keybinding beforehand.
2. **The empty state offers actions that exist.** Whatever it names must be activatable by
   both keyboard and pointer, per RFC-018's trusted-UI rules.
3. **`ProjectBoardEmptyState`'s `primary_action` / `secondary_action` fields are removed** from
   `tekstide-core`'s public API, or given real meaning. They currently hold pre-baked English
   naming actions that do not exist, in a published crate, read by nothing.
4. **A help surface that does not depend on the Project Board being visible.** `0.12.1` lists
   the bindings on the board, which is the best that could be done without new surface; a user
   inside Terminal Immersion still has to know `Ctrl+Alt+P` to get back to it.

## Non-goals

- A file-manager-grade directory browser. The explorer tree RFC-019 built is enough of a
  precedent to reuse or narrow; this RFC does not add a second one.
- Project *creation* (`git init`, scaffolding). Opening an existing directory only.
- Removing the CLI path argument. It works, it is scriptable, and it stays.

## Decisions (were design questions; settled on acceptance)

**D1. A path entry field, not a directory picker.** A text field is
small, keyboard-first, testable without a portal dependency, and consistent with the rest of
the product; it also asks a user to type a path, which is exactly the friction that made the
CLI-only route feel broken. A native picker means an XDG desktop portal dependency and a
capability this project has so far avoided entirely.

**Decided: the text field, this milestone.** It closes the goal completely and adds no new
external surface. A portal picker would introduce an XDG desktop portal dependency — the first
in this project — and blocking the fix for an unusable product on a dependency decision is the
wrong trade. A picker remains a separable later improvement and is explicitly not foreclosed.

**D2. Focus the field on an empty board, and bind `Ctrl+Alt+O` as well.** Every existing action is
`Ctrl+Alt+<letter>`, and `Ctrl+Alt+O` (Open) is unclaimed. The alternative is that the field
is simply focused when the board is empty, so typing works immediately with no binding to
learn — which is strictly better for the first-run case this RFC is about.

**Decided: both.** Focusing costs nothing and serves the user who has never read anything —
they type and it works, with no binding to learn, which is the entire first-run case. The
binding serves the user who already has a project open and wants a second one. `Ctrl+Alt+O` is
unclaimed; the implementer must prove that mechanically against `KeybindingPolicy`, not by
inspection, exactly as `Ctrl+Alt+R` was proven in PR-020-B.

**D3. The help surface is in this RFC, as its own PR.** Goal 4 can be met by a modal listing
`keyboard_help_lines` on a new binding. It is small, but it is a new surface with the usual
modal obligations (RFC-018's scrim and keystroke suppression, focus, escape).

**Decided: same RFC, separate PR.** The two share `keyboard_help`, and splitting them across
RFCs would repeat the mistake of scheduling a surface without asking who reaches it.

## Acceptance criteria

- A user launching `tekstide` with no arguments, who has never read any documentation, can put
  a project on the board using only what the window shows them. **Proven from a real key event
  through production code**, per this project's standing evidence rule — not from a dispatched
  message.
- The board's empty state contains no text naming an action that is not activatable. A test
  asserts this by enumeration, not by inspection.
- `ProjectBoardEmptyState`'s dead fields are gone from the published API, and the breaking
  change is recorded in the changelog.
- Every live keybinding is reachable from a help surface that does not require the Project
  Board to be the visible route.

## Open questions, also decided

- **OQ1 — recent projects on the empty board: yes, and it is the last slice.** The
  recent-projects list already persists paths across sessions and is repopulated at boot by
  `restore_recent_projects`, which builds a passive `Vec<RestoredRecentProject>` that **no
  surface reads**. Offering it as one-key reopen makes the second run of the product
  categorically better than the first and consumes data that is already on disk and currently
  dormant.

  Scheduled as the final PR so it cannot delay the slice that makes the product usable at all.
  **Dropping it is the owner's decision, not the implementer's** — if the earlier PRs run long,
  escalate rather than silently descoping.

- **OQ2 — the failure path renders a bounded, escaped diagnostic; it does not exit.** Today
  `add_project_from_path`'s `FailClosed` symlink refusal reaches `eprintln!` and
  `std::process::exit(1)` in `boot()`, before any window exists. That is correct for a CLI
  argument and **catastrophic for a text field**: a typo would close the application.

  Decided: the field's failure path renders a diagnostic in the surface and leaves the
  application running, and the CLI path keeps its existing exit behaviour unchanged. The
  rendered text is **untrusted** — a user-typed path can carry Trojan Source characters and is
  echoed back — so it follows RFC-023's `bound_key_segment` discipline exactly:
  `escape_untrusted_chars`, truncated to a bounded length with an explicit ellipsis. This is a
  security decision and is not the implementer's to re-open; see the pack's
  `what-a-path-field-must-not-trust.md`.
