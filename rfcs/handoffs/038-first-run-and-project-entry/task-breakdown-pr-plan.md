---
title: "RFC-038: task breakdown and PR plan"
rfc: "RFC-038"
rfc_file: "../../accepted/038-first-run-and-project-entry.md"
source_rfc_status: "Accepted 2026-08-24 — M12, first"
target_milestone: "M12"
created: "2026-08-24"
---

# Five slices, in order

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
