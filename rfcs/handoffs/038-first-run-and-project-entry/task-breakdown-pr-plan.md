---
title: "RFC-038: task breakdown and PR plan"
rfc: "RFC-038"
rfc_file: "../../accepted/038-first-run-and-project-entry.md"
source_rfc_status: "Accepted 2026-08-24 — M12, first"
target_milestone: "M12"
created: "2026-08-24"
---

# Slices, and the order they run in

**Execution order: A, B, C, G, D, F, I, E.** Letters are allocation order, not execution order —
C, G and F were added or re-scoped after A and B shipped. This project has already lost a day
to slice letters that implied an order they did not have (PR-020-B/C), so the order is stated
here once and the letters are never renamed.

Order matters and is not negotiable in one place: **PR-038-A lands the render arm before
PR-038-B binds a key to it.** That is the ordering rule PR-020-B established after this project
bound three keys to surfaces that did not exist yet — a binding must never exist in a state
where pressing it silently does the wrong thing.

## PR-038-A — the path field, and a project on the board

The slice that makes the product usable. Everything else is improvement.

**Build:** a text entry on the Project Board's empty state, **focused on arrival**, that accepts
a typed or pasted path and opens it on Enter.

- Wire through the existing `add_project_from_path`. Collect a string, hand it over. No
  canonicalisation, no symlink logic, no root validation in the surface — see
  `what-a-path-field-must-not-trust.md` §6.
- **Wire the `project_added` audit record on the new call site** and update
  `add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else` to name both
  files with exact counts, keeping it a count. See §5 of the same document.
- **Failure renders, never exits.** Bounded, escaped diagnostic per §3. The application stays
  running after a bad path, and a test proves it.
- The added project is `Restricted`; a test proves it (§4).

**Do not touch:** `boot()`'s CLI argument handling, `keyboard_help`, the populated-board arm.

**Evidence:** a cold start — no arguments, fresh `XDG_STATE_HOME` — in which a path is typed and
a project appears, proven from real key events through production code, not dispatched messages.
Plus a screenshot of the board before and after.

## PR-038-B — `Ctrl+Alt+O`

**Carried in from PR-038-A's review (request 297):** name `Ctrl+V` in the field's own hint,
following `transcript-purge-dialog-hint`'s precedent. `Ctrl+V` pastes into the field and is
named nowhere, while `Ctrl+Shift+V` — the gesture the keyboard list on that same screen teaches
— is intercepted as a global binding, reaches `attempt_paste_into_terminal`, finds no focused
terminal, and does nothing silently. **Do not retarget `Ctrl+Shift+V` to the field**: an action
that silently changes destination based on focus is the surprise RFC-018 exists to prevent.


**Build:** a `NavigationAction` for opening the field, `Candidate` with `Some("Ctrl+Alt+O")`,
reachable when a project is already open — the second-project case PR-038-A does not serve.

- **Prove the binding is unclaimed mechanically**, against `KeybindingPolicy`, not by reading
  the list. `open_current_agent_run_detail_shortcut_is_a_candidate_that_collides_with_no_other_rule`
  is the shape to copy.
- The new action gets a `keyboard_help` catalog key. `action_catalog_key_is_some_iff_the_action_
  is_live` will fail until it does, which is that test working.
- `every_live_binding_is_described_to_the_user` asserts exactly nine. It becomes ten. Update the
  count deliberately; do not loosen the assertion to `>=`.

## PR-038-C — a help surface that does not need the board

**Re-scoped 2026-08-24 by the human owner:** *"Short cuts should not be shown in the app main
pane. It should be in Help."* This slice now also **removes the keyboard list from the Project
Board** — both the empty-state and populated arms. `0.12.1` put it there because there was
nowhere else for it, which was reference material pushed onto the primary working surface. Help
is where it belongs.

`every_board_state_renders_the_keyboard_list` exists to stop that list disappearing; it must be
**replaced**, not deleted — by an equivalent guard that the Help surface renders every live
binding. The property was never "the board shows it", it was "somewhere a user can reach shows
all of it".

**Build:** a surface rendering `keyboard_help_lines`, on its own binding, reachable from
anywhere — including Terminal Immersion, which is the case `0.12.1` left unserved.

- Reuse `keyboard_help`; do not build a second list. The whole point of that module is that one
  derivation feeds every consumer.
- If it is modal, RFC-018's obligations apply in full: scrim, keystroke suppression, focus,
  Escape. If it is a route rather than a modal, say which and why in the evidence.

## PR-038-D — recent projects, one key each

**Build:** the empty board lists remembered projects and opens one without retyping its path.

`restore_recent_projects` already populates `Vec<RestoredRecentProject>` at boot and **no surface
reads it** — this consumes data already on disk. Note that the cache is user-writable and is a
display hint only: **the audit store remains authoritative for trust** (RFC-032). Rendering a
remembered project must not restore or imply any trust state the audit store does not confirm.

Project names and paths from the cache are untrusted and escaped, exactly as the board's
existing rows are.

**This is the droppable slice** if A–C run long — droppable by the human owner via the
architect, not by you. Escalate, do not descope.

## PR-038-G — the folder browser

**Added 2026-08-24 by the human owner's direction**, overturning RFC-038's D1: a typed path is
not an acceptable primary way to choose a folder.

**Build:** a folder browser for choosing a project directory, reached from a **visible control**
on the Project Board — a button, not only a key. Reuse RFC-019's explorer tree: it already
renders an `ExplorerDirectoryScan`, has a parent entry for navigating up, and escapes every
untrusted name. Do not write a second directory renderer, and do not walk the filesystem in the
surface — `tekstide-core` owns scanning, bounded by `FileExplorerScanPolicy`.

- The chosen folder goes through the same `add_project_from_path` entry point PR-038-A uses,
  with the same audit record and the same `Restricted` outcome. The browser chooses a path; it
  does not gain a second way to open one.
- `what-a-path-field-must-not-trust.md` applies unchanged — a directory name from the
  filesystem is untrusted exactly as a typed one is.
- The path field from PR-038-A remains as a secondary route for pasting a known path.
- Keyboard-operable throughout, per RFC-015's focus model. A control that only responds to a
  mouse is not finished.

**Evidence:** a cold start in which a project is opened **without typing a path** — navigate and
choose — proven from real key events, plus a screenshot of the browser itself.

## PR-038-F — give core a scan-only entry point

**Added 2026-08-24 from PR-038-B's review.** Small, structural, and it closes a trap that has
now caught two slices.

`ApplicationShell::scan_active_project_explorer_directory` does two things: it scans, and it
**unconditionally sets `route = ActiveProjectWorkspace`**. `ensure_explorer_scanned` calls it
for background cache-priming, where only the scan is wanted — so every user-visible side effect
has to be undone afterwards. The code already does this once, saving and restoring
`open_surface` around the call, after response 233 found `OpenActiveProjectSurface` being
silently overwritten back to `TextEditor`. PR-038-B found the second: `route`, silently flipped
back one line after `dispatch` set it, worked around by routing the action out of
`app_command_for` entirely.

Two different pieces of state, two different workarounds, one conflation. A third caller will
hit a third.

**Build:** a scan-only entry point in `tekstide-core` — scanning without navigating — and have
`ensure_explorer_scanned` call it. The existing navigating method stays for `handle_explorer_key`,
where navigating on scan is genuinely correct. The `open_surface` save/restore dance should then
be deletable; if it is not, say why, because that is a third instance hiding.

**Ablate** by pointing `ensure_explorer_scanned` back at the navigating method and confirming
the PR-038-B route test fails.

Additive to core's public API, so no breaking change and no version implication.

## PR-038-I — one guard, so two renderers cannot diverge on escaping

**Added 2026-08-24 from PR-038-G's review, and it replaces a shared-render refactor rather than
deferring one.** PR-038-G shipped `browse_row_line`/`browse_node_line`/`browse_tree_lines` as a
near line-for-line parallel of `surface/explorer.rs`'s originals, disclosed by the implementer,
because the core scan types genuinely differ: `ExplorerDirectoryScan` carries project-root-
*relative* paths and a folder browser exists to choose that root.

The risk in two renderers is **not** volume, it is escaping divergence. Both render
filesystem-derived names; if one later gains a fix the other does not, that is a security
divergence. Both escape correctly today (`explorer.rs:89` and `:220`).

**Build:** a count-equality invariant over `surface/explorer.rs` — the number of `.untrusted(`
call sites equals the number of `quote_untrusted(` calls. Verified satisfiable before this was
assigned: both are **4** today. Per `ARCHITECTURE.md`'s enumeration-test unit rule the unit is
the call site, not the file, so a future renderer cannot pass an unescaped name and still pass.

`board.rs` reads 0 and 3 — it escapes and renders directly rather than through `.untrusted(`.
**Do not extend the invariant there** without deciding what it should mean; an invariant that is
false for a correct file is worse than none.

**Ablate** by removing one `quote_untrusted` call and confirming the count test fails.

A shared render helper is explicitly **not** required. Requiring an abstraction to enforce a
property a test enforces directly is the wrong trade, and forcing `BrowseNode` through
`ExplorerNode`'s shape is what PR-038-G's core split exists to avoid. If a third browser ever
appears, revisit then.

## PR-038-E — closeout

- **Remove `ProjectBoardEmptyState`'s `primary_action` and `secondary_action`** from
  `tekstide-core`'s public API. They hold pre-baked English for two actions that never existed
  and are read by nothing. **This is a breaking change to a published crate** — it needs a minor
  version bump and a changelog entry saying so plainly.
- Fill `qa-evidence.md`, tick `acceptance-qa-checklist.md` with citations, and state every
  known limitation.
- Answer RFC-038's acceptance criteria one by one, in its own words.

## Standing expectations

- **Single-variable ablations.** One change, watch the specific test fail, restore. A red result
  that needed two edits is as defective as a green one — `ARCHITECTURE.md` says so because the
  architect got it wrong twice in one week.
- **Disclose flakes, do not re-run past them.** The approval/socket flake now has four known
  symptoms (`test-process-leak.md`); a fifth is a disclosure, not a defect in your slice.
- **A premise that would surprise a user is a finding.** If you discover, mid-slice, something
  about this product that would startle a person who just installed it, that belongs at the top
  of a review request the day you find it — not as scaffolding inside a paragraph about
  something else.
