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

- [x] **A person who has read nothing, given only the built binary, can put a project on the
      board using what the window shows them.** Not "the field renders" — the whole journey.
      PR-038-A: `qa-evidence.md`'s live cold-start capture.
- [x] Proven from **real key events through production code**, not from dispatched messages.
      `shell::tests::a_real_typed_path_and_enter_opens_a_project_from_a_cold_empty_board` and the
      live `xdotool` capture, both driven through the real router.

## Cold start — the first evidence item, deliberately

- [x] Release binary, **no arguments**, fresh `XDG_STATE_HOME` (`$(mktemp -d)`), launch command
      recorded in full alongside the capture. `qa-evidence.md`'s PR-038-A section.
- [x] Screenshot before: the empty board with the field, focused.
      `evidence/pr-038-a/before-empty-board-with-field.png`.
- [x] Screenshot after: a real project on the board, opened through the field.
      `evidence/pr-038-a/after-project-opened.png`.
- [x] No text on either screen names an action that does not exist. Confirmed by viewing both
      captures directly.

Reference for form: `../first-run-correction/evidence/cold-start-empty-board.md`.

## Security — `what-a-path-field-must-not-trust.md`

- [x] Typed/pasted path routed through `text_safety::quote_untrusted` on render; never handed
      raw to `text(...)`. `board::path_field_display_text`.
- [x] A path containing a Unicode directionality override renders as a visible marker, proven
      by test, not by inspection.
      `shell::tests::a_directionality_override_in_the_typed_path_renders_as_a_visible_marker_not_obeyed`.
- [x] **A bad path leaves the application running.** Test proves no exit; the diagnostic is
      bounded and escaped per `bound_key_segment`'s shape, truncation marked visibly.
      `shell::tests::a_bad_path_renders_a_notice_and_the_application_keeps_running`; live in
      `after-project-opened.png`'s own first (retried) capture, disclosed in `qa-evidence.md`.
- [x] No second escaping routine was written. `path_field_error_text` calls
      `text_safety::quote_untrusted` exactly once; see its own doc comment for why not
      `escape_untrusted_chars` in addition.
- [x] A project added through the field is `Restricted`; an agent run in it is refused until
      trust is granted through `Ctrl+Alt+U`.
      `shell::tests::a_project_opened_through_the_field_refuses_an_agent_run_until_trust_is_granted`.
- [x] No `canonicalize`, symlink policy, or root validation added in `shell.rs`. Confirmed by
      diff: the only new call is `state.app_shell.add_project_from_path(&path)`.

## The audit guard

- [x] `project_added` is recorded on the new call site. `record_path_field_project_added`.
- [x] `add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else` updated to
      name both call sites with **exact counts**, still a count and not a presence check.
- [x] Ablated: remove the record from the new call site, watch that test fail, restore.
      Done twice: the record itself, and (separately) a duplicated call site to prove the guard
      test's own widened allow-list holds a count, not a presence check. Both in `qa-evidence.md`.

## Bindings and help

- [x] `Ctrl+Alt+O` proven unclaimed **mechanically** against `KeybindingPolicy`.
      `navigation::tests::open_project_entry_field_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
      ablated (binding changed to a colliding one, test failed, reverted).
- [x] The new action has a `keyboard_help` catalog key; the live-binding count updated from
      nine to ten deliberately, assertion not loosened.
      `keyboard-help-open-project-entry-field`; both count sites updated (`keyboard_help/tests.rs`,
      `board/tests.rs`).
- [x] The help surface is reachable from Terminal Immersion, not only from the board.
      `shell::tests::ctrl_alt_k_opens_help_from_inside_terminal_immersion` -- real project, real
      mode dispatch into genuine Terminal Immersion, real `Ctrl+Alt+K`.
- [x] If modal: scrim, keystroke suppression, focus and Escape all met per RFC-018. Modal
      (`ModalContent::Help`, `Ctrl+Alt+K`). Scrim: same `stack![base, opaque(scrim)]`/
      `modal_scrim_style` every other modal uses, live in `after-help-modal-open.png`. Keystroke
      suppression: structural, via the same `SubscriptionMode`/`ModalAbsent` mechanism every
      modal already gets, nothing new required. Focus: nothing to focus (no buttons) --
      `ModalFocusNext`/`Previous` are no-ops against it, documented on `ModalContent::Help`
      itself. Escape: `escape_closes_the_help_modal`, live in `after-escape-closes-help.png`.

## Folder browser (PR-038-G)

- [x] Reached from a **visible control**, not only a key: a real `iced::widget::button`
      ("Browse..."), live in `evidence/pr-038-g/before-cold-start-empty-board.png` and clicked
      for real in `after-real-mouse-click-opens-browser.png`.
      `board/tests.rs::the_browse_button_is_a_real_clickable_widget_not_an_inert_label`.
- [x] The chosen folder goes through the same `add_project_from_path` entry point PR-038-A uses,
      with the same audit record and the same `Restricted` outcome.
      `shell::tests::space_commits_the_shown_directory_as_a_new_restricted_project_and_closes_the_modal`,
      `choosing_a_directory_through_the_real_browser_writes_exactly_one_real_project_added_record`
      (ablated).
- [x] `what-a-path-field-must-not-trust.md` applies unchanged. `qa-evidence.md`'s own
      PR-038-G "Security" paragraph.
- [x] The path field remains as a secondary route. Untouched; confirmed by diff.
- [x] Keyboard-operable throughout. `Tab`/`Shift+Tab`/`Arrow` keys move the highlight (clamped),
      `Enter` navigates, `Space` commits, `Escape` cancels -- all proven by real message
      dispatch in `shell/tests.rs`, and by real `xdotool` key events live.
- [x] Evidence: a cold start in which a project is opened **without typing a path** -- navigate
      and choose -- proven from real key events (and, additionally, a real mouse click on the
      button itself), plus a screenshot of the browser itself.
      `evidence/pr-038-g/after-real-mouse-click-opens-browser.png` through
      `after-space-commits-project-without-typing.png`.
- [ ] **Disclosed, not silently resolved**: does not reuse RFC-019's explorer renderer as the
      task breakdown asked ("do not write a second directory renderer") -- `browse_view` and its
      siblings are a near-duplicate of `view`/`node_line`/`tree_lines`. Core scanning could not
      honestly reuse `FileExplorerScanner`/`ExplorerDirectoryScan` (project-root-relative by
      construction; no project exists yet at browse time) -- see `qa-evidence.md`'s own PR-038-G
      section for the full reasoning. Left unchecked deliberately: this is a question for the
      architect's decision, not a completed item.

## Recent projects (PR-038-D)

- [x] Remembered names and paths escaped as untrusted. Pre-existing, unchanged this slice:
      `project_board.rs::recent_project_row` already fed the board's own escaping
      (`row_lines`/`highlighted_row_lines`); this PR added no new untrusted-text render path.
- [x] Rendering a remembered project restores or implies **no** trust the audit store does not
      confirm. **The real finding this slice exists to report**: this was *not already true* --
      `verify_restored_trust` only ran once, at boot, before this PR, so every non-CLI reopen
      path (including this PR's own new one) restored cached trust with zero confirmation.
      Fixed at all three call sites (`reopen_recent_project`, `attempt_open_project_from_path_field`,
      `choose_current_browsed_directory`), each separately ablated. `qa-evidence.md`'s own
      PR-038-D section.
- [x] One-key reopen, proven from real key events, plus a real mouse click on the row's own
      button. `qa-evidence.md`'s own PR-038-D section, live captures and automated tests both.
- [x] Not dropped -- built in full this slice; the human owner's own escalation clause never
      needed to trigger.

## Closeout

- [ ] `ProjectBoardEmptyState::primary_action` / `secondary_action` removed from
      `tekstide-core`'s public API. Pending PR-038-E — deliberately untouched this slice, per
      the task breakdown's scope boundary.
- [ ] Recorded in the changelog as a **breaking change**, with the version implication stated.
      Pending PR-038-E.
- [x] Every ablation in this slice was **single-variable**. Three in PR-038-A, three in PR-038-B,
      four in PR-038-C, two in PR-038-G (the collision test against a colliding binding, which
      also caught `open_help_shortcut...` failing as its own collision victim; the audit-record
      guard against the new call site's record write commented out), four in PR-038-D (the new
      call site's own audit-record guard; the new call site's own trust-confirmation fix; the
      same trust-confirmation fix retroactively ablated at both of the two pre-existing call
      sites it also closed). Each reverted after confirming the expected failure. See
      `qa-evidence.md`.
- [x] Flakes disclosed, not re-run past; any fifth symptom of the approval/socket flake reported
      as a disclosure rather than attributed to this work. PR-038-B's run hit three (all
      already-named). PR-038-C's own run separately hit a fourth already-named symptom
      (`command_approval_family_produces_real_durable_audit_records_through_the_pipeline`,
      confirmed passing in isolation) — and, distinctly, the PTY-exhaustion cascade recorded in
      `qa-evidence.md`'s own PR-038-C section, which is a different, newly-disclosed defect, not
      a fifth symptom of the approval/socket one. PR-038-G's own run hit none. PR-038-D's own run
      hit none either.
- [x] Gates: `fmt`, `clippy -D warnings`, full suite, `git diff --check`. All clean, PR-038-D's
      own state (357 tekstide + 724 tekstide-core, 0 failed).
- [x] Known limitations stated, including anything this slice leaves unreachable.
      `qa-evidence.md`'s "Known limitations" section, now "as of PR-038-A/B/C/D/G," including the
      render-layer duplication and the recent-cache staleness note both disclosed and flagged
      above.

## Final Acceptance Decision

- [ ] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Pending review.
```
