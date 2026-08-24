---
title: "RFC-039: Acceptance / QA Checklist"
rfc: "RFC-039"
rfc_file: "../../done/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-039 is in rfcs/done/"
target_milestone: "M12"
created: "2026-08-24"
---

# Acceptance / QA Checklist

Every unchecked line at closeout carries a stated reason.

## The acceptance criterion

- [x] **A person who has read nothing opens two projects, moves between them, closes one, and
      returns to the board — using only what the window shows them.** `qa-evidence.md`'s own
      "RFC-039's acceptance criterion, answered in its own words" -- every step in this sentence
      has a real, clicked-in-live-evidence control across PR-039-A/B/C.
- [x] Every workflow claimed as served names **the control the user sees**, not the keystroke
      that also works. True of everything PR-039-A/B/C themselves built. **Not** true of the wider
      application -- `affordance-audit.md`'s Finding 2 found nine live, pre-existing actions this
      RFC did not touch with no visible control at all. Answered for what shipped, not as a claim
      about the whole product; see `qa-evidence.md`'s own worded answer and the "Known
      limitations" section below.
- [x] Proven from real events through production code, with a cold-start capture. Every
      PR-039-A/B/C screenshot is a real `xdotool`/`niri`/`wl-paste` capture against the real
      release binary launched cold, never a description of intended behaviour -- including the two
      cases (PR-039-B, PR-039-C) where a capture needed correction, disclosed rather than silently
      replaced.

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

- [x] Every `NavigationAction` listed against the visible control that invokes it. All seventeen
      `linux_mvp()` rules, in `affordance-audit.md`'s Finding 2 table (thirteen live) plus
      Findings 3-5 (the four dead/reserved).
- [x] Every capability a user is expected to perform listed the same way. The application's
      *entire* mouse-clickable inventory (ten `.on_press` sites, the complete set — verified by
      grep, not sampled) cross-referenced against both `NavigationAction` and every modal's own
      decision (`affordance-audit.md`'s Finding 1).
- [x] Anything with no control **reported as a finding**, not given a keybinding. Nine live
      actions and all nine modals' own decisions -- reported in `affordance-audit.md`, nothing
      quietly wired.

## Closeout

- [x] Every ablation single-variable, unit being the design decision. Listed per-PR in
      `qa-evidence.md`: PR-039-A's marker/colour independence, PR-039-B's `Ctrl+Alt+N` collision
      and `tab_marker` independence, PR-039-C's `set_file_state` recovery symmetry.
- [x] Flakes checked against `test-process-leak.md` before reporting.
      `command_approval_family_produces_real_durable_audit_records_through_the_pipeline` is
      already one of that document's own four named tests; this session's own recurrence is
      recorded in PR-039-C's `qa-evidence.md` section as a *distinct, now-confirmed* mechanism
      (a shared-`AuditStore` query race, not the `Child::drop` leak), per response 312's own
      instruction not to fold the two together.
- [x] Statements this work falsifies — README, `--help` — corrected in the slice that falsified
      them. README: the "no safe-close dialog" / "safe-close dialog does not exist yet" claims,
      the safe-close audit-producer claim, the modal-layer enumeration (four → six, and the new
      keyboard-only-decision limitation), the keybinding table (missing `Ctrl+Alt+N`), and a new
      bullet for the tab strip itself, none of which existed in the README before this RFC.
      `--help` (`keyboard_help::usage_text`) is generated mechanically from `linux_mvp()`, already
      covered by `keyboard_help::tests::usage_text_lists_every_binding_the_gui_lists`; no manual
      edit needed or made.
- [x] Gates: `fmt`, `clippy -D warnings`, full suite, `git diff --check`. 383 tekstide + 734
      tekstide-core, three consecutive clean runs of the full default command (response 313's own
      verification), all clean.
- [x] Known limitations stated. `qa-evidence.md`'s own "Known limitations (RFC-039-wide)" section,
      citing `affordance-audit.md` throughout.

## Final Acceptance Decision

- [x] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Final Acceptance recorded 2026-08-25 (review request 314). Suite re-run by the reviewer under
full parallelism, three consecutive runs: 383 + 734 + 2, zero failed.

RFC-039 met its acceptance criterion. A user who has read nothing can see which projects are
open, move between them, close one, and get home, using controls in front of them -- proven
from real events with live evidence, including a tab focused-but-not-active frame that no
string-level test could supply.

Its own PR-039-D audit then reported that three of thirteen live actions have a visible
control, and that every modal in the crate is keyboard-only for its own decision. That is a
finding about the RFC's scope, not its delivery: the criterion was specific to moving between
projects and was satisfied. Closing this RFC without carrying that forward would have been
the dishonest move, so RFC-040 is filed in this same commit with all six findings as its
inputs.

Two defects fixed inside PR-039-C that predated this RFC and belonged to nobody:
close_project could never return SafeToClose for any real project (provider_state had no
upgrade path -- its only setter is #[cfg(test)]), and set_file_state's one-way downgrade
would have permanently poisoned any project whose file provider hiccuped. The first had gone
unnoticed because close_project had no production caller, the same shape as the for_test id
defect found a slice earlier: an unwired producer hides defects in everything it would have
exercised.

One correction carried into RFC-040 rather than left in the audit: Finding 2's heading said
nine and its own list named ten. Ten is right (13 live, 3 with controls).
```
