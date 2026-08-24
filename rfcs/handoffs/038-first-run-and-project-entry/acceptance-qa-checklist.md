---
title: "RFC-038: Acceptance / QA Checklist"
rfc: "RFC-038"
rfc_file: "../../done/038-first-run-and-project-entry.md"
source_rfc_status: "Implemented and closed 2026-08-24 — RFC-038 is in rfcs/done/"
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
- [x] **Disclosed, then decided, then corrected.** Does not reuse RFC-019's explorer renderer as
      the task breakdown asked ("do not write a second directory renderer") -- `browse_view` and
      its siblings are a near-duplicate of `view`/`node_line`/`tree_lines`. Core scanning could
      not honestly reuse `FileExplorerScanner`/`ExplorerDirectoryScan` (project-root-relative by
      construction; no project exists yet at browse time). **Decision (response 300)**: a shared
      render helper is not required -- forcing `BrowseNode` through `ExplorerNode`'s shape would
      reintroduce the dishonest-field problem the core split exists to avoid. **PR-038-I first
      built a runtime count-equality guard to protect the escaping property instead of sharing;
      PR-038-E then removed that guard** (response 304): `DisplayText`'s private field and single
      constructor already make the property a compile-time fact, so the runtime count added
      nothing and would have failed on correct code the moment either renderer legitimately
      escaped-then-rendered-directly rather than through `.untrusted(`. What actually protects
      the property is `DisplayText`'s own constructor/field guard (PR-038-E), not anything
      specific to this pair of renderers. `qa-evidence.md`'s own PR-038-G, PR-038-I, and PR-038-E
      sections.

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

- [x] `ProjectBoardEmptyState::primary_action` / `secondary_action` removed from
      `tekstide-core`'s public API. **A real finding while doing it**: the task breakdown's own
      "read by nothing" premise was false -- `tekstide-core::shell::render_project_board` (the
      pre-GUI `render_text()` harness) read both fields directly, and a real, passing core test
      asserted on the exact string they produced. Both fixed alongside the field removal; see
      `qa-evidence.md`'s own PR-038-E section.
- [x] Recorded in the changelog as a **breaking change**, with the version implication stated.
      `CHANGELOG.md`'s new `0.13.0` entry (bumped from `0.12.1`); `Cargo.toml`'s workspace
      version bumped to match.
- [x] RFC-038's acceptance criterion 2 ("no text naming an action that is not activatable...
      asserted by enumeration, not by inspection") answered by an actual enumeration test, not
      left implicit. `board::tests::every_catalog_key_this_module_renders_is_enumerated_and_none_names_a_dead_action`
      -- `qa-evidence.md`'s own PR-038-E section, item 4.
- [x] All four of RFC-038's acceptance criteria answered explicitly, in the RFC's own words.
      `qa-evidence.md`'s own PR-038-E section, closing paragraph.
- [x] Every ablation in this slice was **single-variable**. Three in PR-038-A, three in PR-038-B,
      four in PR-038-C, two in PR-038-G (the collision test against a colliding binding, which
      also caught `open_help_shortcut...` failing as its own collision victim; the audit-record
      guard against the new call site's record write commented out), four in PR-038-D (the new
      call site's own audit-record guard; the new call site's own trust-confirmation fix; the
      same trust-confirmation fix retroactively ablated at both of the two pre-existing call
      sites it also closed), two in PR-038-F (`ensure_explorer_scanned` pointed back at the
      navigating method, checked against both of the two pre-existing regression tests its own
      side effects used to break -- `open_surface`, response 233's own test; `route`, PR-038-B's
      own test, only meaningful once that slice's route workaround was itself reverted), two in
      PR-038-I (the new core contract test, ablated against the core method itself with no GUI
      touched; the new escaping-count guard, ablated with an added unmatched `.untrusted(` call
      -- **this guard was itself removed in PR-038-E**, see below), two in PR-038-E (the
      `DisplayText` constructor-count guard, ablated with a second constructor added; the
      `DisplayText` field-privacy guard, ablated by making the field `pub`). Each reverted after
      confirming the expected failure. See `qa-evidence.md`.
- [x] Flakes disclosed, not re-run past; any fifth symptom of the approval/socket flake reported
      as a disclosure rather than attributed to this work. PR-038-B's run hit three (all
      already-named). PR-038-C's own run separately hit a fourth already-named symptom
      (`command_approval_family_produces_real_durable_audit_records_through_the_pipeline`,
      confirmed passing in isolation) — and, distinctly, the PTY-exhaustion cascade recorded in
      `qa-evidence.md`'s own PR-038-C section, which is a different, newly-disclosed defect, not
      a fifth symptom of the approval/socket one. PR-038-G's own run hit none. PR-038-D's,
      PR-038-F's, PR-038-I's, and PR-038-E's own runs hit none either.
- [x] Gates: `fmt`, `clippy -D warnings`, full suite, `git diff --check`. All clean, PR-038-E's
      own state (359 tekstide + 727 tekstide-core, 0 failed).
- [x] Known limitations stated, including anything this slice leaves unreachable.
      `qa-evidence.md`'s "Known limitations" section, now "as of PR-038-A/B/C/D/E/F/G/I." The
      render-layer duplication note is corrected to its final resolution (the type system, not a
      runtime guard, per PR-038-E). PR-038-F and PR-038-I both left no new limitation --
      structural/test-only; PR-038-I's own guard was itself removed by PR-038-E, disclosed above,
      not silently dropped.

## Final Acceptance Decision

- [x] Accepted.
- [ ] Accepted with required follow-up.
- [ ] Requires re-review after changes.

Reviewer notes:

```text
Final Acceptance recorded 2026-08-24 (review request 305). Suite re-run by the reviewer:
359 + 727 + 2, zero failed; fmt clean; clippy -D warnings clean.

RFC-038 is closed. The product has a door: a person who has read nothing can open a project
from the window, by typing a path, by browsing to a folder, or by reopening a remembered one
with a single key -- and can find out what else exists from a Help modal reachable anywhere.

Verified at closeout rather than accepted from the request:
  - The DisplayText constructor guard, ablated by the reviewer with a second constructor
    added: exactly_one_function_in_the_crate_returns_displaytext failed, restored, green.
  - 0.13.0 prepared and deliberately NOT shipped -- version bumped, changelog written, no
    tag at HEAD, nothing published. Correct: that decision is the owner's.
  - README's keyboard table now carries all five bindings it was missing.

Four instructions in this RFC's own pack rested on unchecked claims about the code, and the
implementer caught every one by checking before executing rather than after something broke:
reuse the explorer tree (impossible -- the scanner needs a ProjectRootHandle), the PR-038-F
route ablation (inert against the tree as PR-038-B left it), Guard 1 (redundant with a
compile-time guarantee), and now "ProjectBoardEmptyState's fields are read by nothing"
(false -- core's render_project_board read both, with a test asserting the exact string).

That last one is worth naming precisely, because it is the same shape as this project's worst
published defect. The locale comment said the fields "exist but are not read", meaning the
GUI renders catalog text instead. I widened a scoped observation into an unscoped claim
without checking core -- exactly how "no transcript is ever written" reached two releases
from a grep of one crate. A true premise, a false conclusion, and the widening invisible in
the sentence that carries it.

Also delivered beyond the pack: a positive enumeration for acceptance criterion 2 (every
catalog key board.rs renders, listed and reasoned about by name) after finding only negative
checks existed; the trust-restoration gap found and fixed retroactively across PR-038-A and
PR-038-G (request 302), which the reviewer had accepted without catching.

Known limitations at closeout are in qa-evidence.md and are real: recent-projects.json is
still only rewritten at boot, --help's framing sentences remain English-only, and the
render duplication between the explorer and browse renderers stands, deliberately -- the
type system, not a test, is what keeps it safe.
```
