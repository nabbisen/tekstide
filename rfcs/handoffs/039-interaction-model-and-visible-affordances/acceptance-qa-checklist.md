---
title: "RFC-039: Acceptance / QA Checklist"
rfc: "RFC-039"
rfc_file: "../../accepted/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Accepted 2026-08-24 — M12, after RFC-038"
target_milestone: "M12"
created: "2026-08-24"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The acceptance criterion

- [ ] **A person who has read nothing opens two projects, moves between them, closes one, and
      returns to the board — using only what the window shows them.**
- [ ] Every workflow claimed as served names **the control the user sees**, not the keystroke
      that also works.
- [ ] Proven from real events through production code, with a cold-start capture.

## The strip

- [x] Renders in Content mode **and** Terminal Immersion; screenshots of both.
      `evidence/pr-039-a/content-mode-workspace-two-tabs.png`,
      `evidence/pr-039-a/terminal-immersion-two-tabs.png` -- `qa-evidence.md`'s PR-039-A section.
- [x] Project names escaped and bounded; bidi-override fixture test passes.
      `shell::tests::tab_label_escapes_a_bidi_override_in_the_display_name`,
      `tab_label_truncates_a_long_display_name_with_an_ellipsis_marker`.
- [x] Active project distinguishable **without relying on colour alone**.
      `shell::tests::tab_label_marks_the_active_project_and_not_the_inactive_one` -- a textual
      marker (`focus_marker`) independent of the border-colour channel (`zone_style`).
- [x] Every control keyboard-operable as well as clickable. PR-039-B: `FocusZone::TabStrip` +
      `ArrowLeft`/`ArrowRight`/`Enter` (`handle_tab_strip_key`), proven live via
      `evidence/pr-039-b/focused-tab-distinct-from-active-tab.png` and
      `evidence/pr-039-b/after-enter-switches-to-highlighted-tab.png`, and by unit test
      (`enter_on_a_highlighted_project_tab_switches_to_that_project`).

## Switching and going home

- [x] Activating a tab switches projects. `switch_active_project_tab_pressed_switches_and_enters_the_workspace`;
      live: `evidence/pr-039-b/after-clicking-beta-tab.png`.
- [x] A permanent, visible route back to the board — not only `Ctrl+Alt+P`. The leftmost
      `Projects` tab, real click and real `Enter` both proven;
      `evidence/pr-039-b/after-clicking-projects-home-tab.png`.
- [x] `SwitchActiveProject` has a real route; RFC-036's dead-action count noted as four → three.
      `Ctrl+Alt+N`, `ctrl_alt_n_cycles_to_the_next_open_project_wrapping`;
      `keyboard_help::tests::no_action_without_a_working_binding_is_advertised`'s three-action list.

## Closing — `what-closing-a-project-must-not-lose.md`

- [ ] Idle project closes with no confirmation.
- [ ] Live terminals or an active agent run raise a confirmation naming **counts**.
- [ ] Confirmation identifies the project by **canonical path**, escaped and bounded.
- [ ] `close_project` wired — its first production caller.
- [ ] **`safe_close_decision` wired, both outcomes** (closed and cancelled). Unwired audit
      families noted as two → one.
- [ ] Test proves closing leaves transcripts and audit records intact.
- [ ] `close_project`'s child-process contract read, not assumed; escalated if it does not stop
      them.

## The affordance audit

- [ ] Every `NavigationAction` listed against the visible control that invokes it.
- [ ] Every capability a user is expected to perform listed the same way.
- [ ] Anything with no control **reported as a finding**, not given a keybinding.

## Closeout

- [ ] Every ablation single-variable, unit being the design decision.
- [ ] Flakes checked against `test-process-leak.md` before reporting.
- [ ] Statements this work falsifies — README, `--help` — corrected in the slice that falsified
      them.
- [ ] Gates: `fmt`, `clippy -D warnings`, full suite, `git diff --check`.
- [ ] Known limitations stated.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
