---
title: "RFC-038: Acceptance / QA Checklist"
rfc: "RFC-038"
rfc_file: "../../accepted/038-first-run-and-project-entry.md"
source_rfc_status: "Accepted 2026-08-24 — M12, first"
target_milestone: "M12"
created: "2026-08-24"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The acceptance criterion that matters

- [ ] **A person who has read nothing, given only the built binary, can put a project on the
      board using what the window shows them.** Not "the field renders" — the whole journey.
- [ ] Proven from **real key events through production code**, not from dispatched messages.

## Cold start — the first evidence item, deliberately

- [ ] Release binary, **no arguments**, fresh `XDG_STATE_HOME` (`$(mktemp -d)`), launch command
      recorded in full alongside the capture.
- [ ] Screenshot before: the empty board with the field, focused.
- [ ] Screenshot after: a real project on the board, opened through the field.
- [ ] No text on either screen names an action that does not exist.

Reference for form: `../first-run-correction/evidence/cold-start-empty-board.md`.

## Security — `what-a-path-field-must-not-trust.md`

- [ ] Typed/pasted path routed through `text_safety::quote_untrusted` on render; never handed
      raw to `text(...)`.
- [ ] A path containing a Unicode directionality override renders as a visible marker, proven
      by test, not by inspection.
- [ ] **A bad path leaves the application running.** Test proves no exit; the diagnostic is
      bounded and escaped per `bound_key_segment`'s shape, truncation marked visibly.
- [ ] No second escaping routine was written.
- [ ] A project added through the field is `Restricted`; an agent run in it is refused until
      trust is granted through `Ctrl+Alt+U`.
- [ ] No `canonicalize`, symlink policy, or root validation added in `shell.rs`.

## The audit guard

- [ ] `project_added` is recorded on the new call site.
- [ ] `add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else` updated to
      name both call sites with **exact counts**, still a count and not a presence check.
- [ ] Ablated: remove the record from the new call site, watch that test fail, restore.

## Bindings and help

- [ ] `Ctrl+Alt+O` proven unclaimed **mechanically** against `KeybindingPolicy`.
- [ ] The new action has a `keyboard_help` catalog key; the live-binding count updated from
      nine to ten deliberately, assertion not loosened.
- [ ] The help surface is reachable from Terminal Immersion, not only from the board.
- [ ] If modal: scrim, keystroke suppression, focus and Escape all met per RFC-018.

## Recent projects (PR-038-D)

- [ ] Remembered names and paths escaped as untrusted.
- [ ] Rendering a remembered project restores or implies **no** trust the audit store does not
      confirm.
- [ ] If this slice was dropped, the drop was authorised by the human owner via the architect,
      and the authorisation is cited here.

## Closeout

- [ ] `ProjectBoardEmptyState::primary_action` / `secondary_action` removed from
      `tekstide-core`'s public API.
- [ ] Recorded in the changelog as a **breaking change**, with the version implication stated.
- [ ] Every ablation in this slice was **single-variable**.
- [ ] Flakes disclosed, not re-run past; any fifth symptom of the approval/socket flake reported
      as a disclosure rather than attributed to this work.
- [ ] Gates: `fmt`, `clippy -D warnings`, full suite, `git diff --check`.
- [ ] Known limitations stated, including anything this slice leaves unreachable.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
