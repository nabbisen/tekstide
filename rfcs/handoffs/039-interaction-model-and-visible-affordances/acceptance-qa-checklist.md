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

- [x] Idle project closes with no confirmation.
      `closing_an_idle_project_removes_it_with_no_confirmation`; live:
      `evidence/pr-039-c/after-clicking-close-on-idle-project.png`.
- [x] Live terminals or an active agent run raise a confirmation naming **counts**.
      `project_close_dialog_reasons_line_states_the_real_counts`,
      `closing_a_project_with_a_live_terminal_opens_a_confirmation_defaulted_to_cancel`; live:
      `evidence/pr-039-c/confirmation-names-path-and-counts.png` ("This will end: 1 running
      process", not vague text).
- [x] Confirmation identifies the project by **canonical path**, escaped and bounded.
      `project_close_dialog_escapes_a_bidi_override_in_the_canonical_path`,
      `project_close_dialog_body_names_the_canonical_path`; live: the same screenshot above shows
      the real path.
- [x] `close_project` wired — its first production caller. `attempt_close_project_tab` /
      `apply_project_close_confirmation`.
- [x] **`safe_close_decision` wired, both outcomes** (closed and cancelled). Unwired audit
      families noted as two → one.
      `confirming_the_close_terminates_the_real_process_and_removes_the_project` (`Authorized`
      then `Applied`, one shared `operation_id`),
      `cancelling_the_close_confirmation_leaves_everything_running_and_records_it` /
      `escaping_the_close_confirmation_also_records_a_cancelled_decision` (`Cancelled`, no
      `operation_id`).
- [x] Test proves closing leaves transcripts and audit records intact.
      `closing_a_project_leaves_its_transcripts_and_audit_records_intact`.
- [x] `close_project`'s child-process contract read, not assumed; escalated if it does not stop
      them. §6 already answers this (response 299) -- `close_project` never touches the runtime;
      `terminate_project_live_work` calls `TerminalPane::request_terminate` (its first production
      caller) before `close_project`, per the confirmed sequence. Escalation was not needed:
      `request_terminate` terminated the real `/bin/sh` correctly in every live capture
      (`evidence/pr-039-c/after-confirmed-close-real-termination.png`) and in
      `confirming_the_close_terminates_the_real_process_and_removes_the_project`. A separate,
      pre-existing defect **was** found and escalated along the way (response 311): `close_project`
      could never return `SafeToClose` for any real project, idle or not, because
      `CloseResourceSummary::provider_state` defaulted to `Unavailable` with no production upgrade
      path -- fixed at its source (`641a5ac`), not compensated for in this slice's own surface.

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
