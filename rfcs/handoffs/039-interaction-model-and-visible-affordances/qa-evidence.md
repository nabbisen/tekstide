---
title: "RFC-039: QA evidence"
rfc: "RFC-039"
rfc_file: "../../accepted/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Accepted 2026-08-24 — M12, after RFC-038"
target_milestone: "M12"
created: "2026-08-24"
---

# QA evidence

One section per PR. Cite the command that produced each result.

Screenshots in `evidence/pr-039-<letter>/`, each with its launch command recorded beside it —
`../first-run-correction/evidence/cold-start-empty-board.md` is the reference for form.

## PR-039-A — the strip exists

**Build.** A project tab strip composed inside the existing top-bar chrome (`top_bar`), one tab
per project in `AppState::projects()`'s own order -- **read-only this slice**: it shows, it does
not yet act (`Message` is not threaded through it at all; PR-039-B wires `on_press`). The active
project is distinguished through two independent channels, neither colour alone (RFC-015): a
border-style change (`zone_style`, the same focused/unfocused colour-and-width pair every other
zone in this crate already uses) and a textual marker prefix (`focus_marker`, the same `"> "`/
`"  "` convention already used throughout this crate). Renders nothing when no project is open
(`top_bar` simply omits the row) -- there is nothing yet to show a tab for.

**Escaping and bounding, per D3 and `what-closing-a-project-must-not-lose.md` §5.** The strip is
trusted chrome, not the RFC-016 terminal-grid exception. `tab_label` routes `display_name`
through `text_safety::quote_untrusted` before it ever reaches a widget, and truncates to
`MAX_TAB_NAME_DISPLAY_CHARS` (24, shorter than the path field's own 128-character notice bound,
since several tabs render side by side in one fixed-width row) with a trailing ellipsis marker --
truncate-then-escape, the same order `path_field_error_text` already establishes and for the
same reason (escaping expands text; truncating after would risk cutting a marker in half).

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (366 tekstide +
727 tekstide-core, up from 359/727; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` --
all clean.

**Tests, at the level this crate always tests rendering: the string, not the `Element` tree.**

- `shell::tests::tab_label_marks_the_active_project_and_not_the_inactive_one` -- the
  colour-independence rule (RFC-015) applied at the tab strip's own call site, the same shape
  `focus_marker_differs_and_is_not_colour_dependent` already establishes for the generic helper.
- `shell::tests::tab_label_escapes_a_bidi_override_in_the_display_name` -- the bidi-override
  fixture the task breakdown asked for, proving `<U+202E>` renders as a visible marker and the
  real override character never reaches the rendered label.
- `shell::tests::an_ordinary_tab_name_renders_without_any_escape_marker` -- the opposite-direction
  check (response 175/176's own convention): a plain name renders with no marker, so the bidi
  test above is exercising real escaping, not a coincidence of that fixture.
- `shell::tests::tab_label_truncates_a_long_display_name_with_an_ellipsis_marker` -- a
  200-character name is bounded and marked, never reaches the label whole.
- `shell::tests::the_project_tab_strip_shows_nothing_with_no_project_open` /
  `..._shows_something_once_a_project_is_open` -- the `Option` boundary, proven against a real
  `State`, not just `tab_label` in isolation.
- `shell::tests::the_project_tab_strip_survives_terminal_immersion` -- real `AppCommand::
  ToggleActiveProjectMode` dispatch into genuine Terminal Immersion, the strip still present --
  the automated half of this slice's own evidence requirement, live capture below is the other
  half.

**The acceptance criterion: two projects open, real screenshots, Content mode and Terminal
Immersion.** `cargo build --release -p tekstide`, launched
`env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d> ./target/release/tekstide <mktemp -d>/tsd-pr039a-alpha <mktemp -d>/tsd-pr039a-beta`
-- two real projects, both from CLI arguments, the first (`alpha`) auto-activated per
`AppState::add_project_session`'s own pre-existing first-project-only rule. The same
`xdotool`/`niri msg action screenshot-window`/`wl-paste` capture method every RFC-038 slice
already established:

- `evidence/pr-039-a/content-mode-two-tabs.png` -- cold start, `Project Board` route: both tabs
  visible (`tsd-pr039a-alpha`, `tsd-pr039a-beta`), `alpha` marked active with both the border and
  the `>` marker, `beta` with neither. Bonus beyond the slice's own requirement: proves the strip
  also survives the Project Board route, not only the active-project workspace.
- Real `Ctrl+Alt+M` (`xdotool key --clearmodifiers ctrl+alt+m`):
  `evidence/pr-039-a/terminal-immersion-two-tabs.png` -- `Project Workspace` route, Terminal /
  Agent Immersion Mode, both tabs still present and correctly marked.
- Real `Ctrl+Alt+M` again: `evidence/pr-039-a/content-mode-workspace-two-tabs.png` -- Content
  mode, both tabs still present and correctly marked.
- Process terminated cleanly with `SIGTERM` after capture; no terminal was ever launched this
  session, so `test-process-leak.md`'s defect class does not apply.

**Security.** No new I/O, no new call to `add_project_from_path` or any audit producer -- a pure
render addition over data `AppState::projects()` already exposed.

## PR-039-B — switch, and go home

**Build.** Real tabs: `project_tab_strip` now returns `Element` unconditionally (no longer
`Option`), each project tab an `iced::widget::button` dispatching
`Message::SwitchActiveProjectTabPressed(ProjectId)`, plus a permanent leftmost "Projects" tab
(`project-tab-strip-home`) dispatching `Message::GoToProjectBoardTabPressed` -- D1's two required
routes ("enter a project", "return to the entrance") both now real, both mouse- and
keyboard-operable. A third `FocusZone::TabStrip` makes the strip keyboard-focusable in the same
`Tab`/`Shift+Tab` cycle as `MainArea`/`Sidebar`; `ArrowLeft`/`ArrowRight` move a clamped
`tab_strip_highlight` index inside it, `Enter` activates whichever tab is highlighted
(`handle_tab_strip_key`). A global `Ctrl+Alt+N` (`NavigationAction::SwitchActiveProject`, now
`Candidate` with a real binding) cycles the active project forward with wraparound -- a coarser,
one-shot accelerator distinct from the strip's own precise per-tab selection, both converging on
`ApplicationShell::switch_active_project`.

**Response 306's required change, done first.** Focus and active are now two independent visual
channels, not one reused pair: focus stays exactly what every other zone already uses (border via
`zone_style`'s colour/width pair, `focus_marker`'s `"> "`/`"  "` textual prefix); active moves to a
distinct pair (`tab_active_style`'s background fill, `tab_marker`'s `"●"`/`"○"` symbol). Both
legible at once is the common case (a tab that is both focused and active) and the rare case (one
tab focused, a different tab active) alike -- see `focused-tab-distinct-from-active-tab.png`
below.

**Response 307's required correction and its own follow-on fix.** The home tab's `active` symbol
does not mean the same thing a project tab's does. `home_tab_label` no longer calls `tab_marker`
at all -- it carries only `focus_marker`, never `"●"`/`"○"`. `"●"` means one thing everywhere it
appears now: "this is `AppState::active_project_id()`", a fact about a project session, which the
home tab is not. "You are on the board" is still shown, honestly, through `tab_active_style`'s
background fill alone (`project_tab_strip` still passes `home_active` there unchanged) -- just not
through the symbol reserved for project identity. Response 307 also caught that
`focused-tab-distinct-from-active-tab.png` did not show what its own caption claimed (no tab in
that frame was actually focused) -- the same synthetic-focus quirk described below for the
Enter-activation capture, hit on a neighbouring capture and not re-checked there. All five
screenshots in this section were re-captured against the rebuilt binary after both fixes; the
descriptions below match what is actually in each file.

**`FocusZone::TabStrip` and the reviewed router.** `route_non_modal_input`'s precedence is
unchanged: global keybinding match, then `Tab`/`Shift+Tab` (shell focus-cycle, never reaches a
surface), then `terminal_focus` if set, then `RoutedInput::Surface`. `#[non_exhaustive]` has no
effect within this crate, so every exhaustive match on `FocusZone` had to be extended by hand;
`cargo build` confirmed none broke silently (the existing checks all compare with `==`, not
exhaustive matches). `handle_tab_strip_key` is reached only through this same router, as a new
arm alongside the pre-existing `Sidebar` branch -- no second, widget-internal capture path.

**Security.** `switch_to_project_tab` and `go_to_project_board` both call
`ensure_explorer_scanned` after dispatch, the same re-scan-on-entry discipline PR-038-F's
scan-only entry point established -- switching to a project never trusts a stale explorer listing
left over from whichever project was active before. No new call to `add_project_from_path` or any
audit producer; `cycle_to_next_active_project` and `switch_to_project_tab` only ever route to
`ProjectId`s already present in `AppState::projects()`.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (373 tekstide +
728 tekstide-core, up from 366/727; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` -- all
clean.

**Ablations.**

- `ctrl_alt_n_cycles_to_the_next_open_project_wrapping` / the collision-test family: rebinding
  `SwitchActiveProject` to `Ctrl+Alt+K` (already claimed by `OpenHelp`) made both
  `switch_active_project_shortcut_is_a_candidate_that_collides_with_no_other_rule` and
  `open_help_shortcut...` fail together, confirming the collision check is live, not vacuous.
  Reverted.
- `tab_marker_combines_focus_and_active_independently`: collapsing `active` into `focus_marker`
  alone (dropping the `active_symbol` half of `tab_marker`) produced `"   "` where the test
  expected `"  ○ "`, confirming the four focus×active combinations are actually independent, not
  coincidentally distinguishable. Reverted.

**Tests, at the level this crate always tests rendering: the string, not the `Element` tree.**

- `focus_cycles_through_all_three_zones_and_back` -- the full `MainArea → Sidebar → TabStrip →
  MainArea` cycle and its true reverse, both directions, through `update()` -- `previous()`'s own
  doc comment had anticipated this third zone before it existed.
- `tab_marker_combines_focus_and_active_independently` -- all four combinations, pairwise
  distinct (see ablation above).
- `project_tab_label_escapes_a_bidi_override_in_the_display_name`,
  `an_ordinary_project_tab_name_renders_without_any_escape_marker`,
  `project_tab_label_truncates_a_long_display_name_with_an_ellipsis_marker` -- PR-039-A's
  escaping/truncation proofs, carried forward against the renamed, now-focus-aware label
  functions.
- `home_tab_label_carries_the_catalog_text_and_only_the_focus_marker` -- response 307's fix:
  proves the home tab's label carries `focus_marker` and never `"●"`/`"○"`.
- `switch_active_project_tab_pressed_switches_and_enters_the_workspace` /
  `go_to_project_board_tab_pressed_returns_to_the_board` -- both `Message` arms proven directly
  against a real `State`: active project and route both change (or return), together.
- `arrow_keys_move_the_tab_strip_highlight_only_while_the_strip_is_focused` -- no-op outside
  `FocusZone::TabStrip`, clamped movement inside it.
- `enter_on_the_highlighted_home_tab_returns_to_the_board` /
  `enter_on_a_highlighted_project_tab_switches_to_that_project` -- a real routed `Enter`
  `SurfaceInput` dispatched through `update`, the same path a live keypress takes.
- `ctrl_alt_n_cycles_to_the_next_open_project_wrapping` /
  `ctrl_alt_n_is_a_no_op_with_fewer_than_two_projects_open` -- three real projects, three presses,
  wraps to the first; the coarser accelerator's own bound.
- `switching_tabs_works_from_inside_terminal_immersion` -- the strip's action-handling, not only
  its rendering, survives the same route PR-039-A already proved rendering survives.

**Live evidence.** `cargo build --release -p tekstide`, launched
`env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d> ./target/release/tekstide <mktemp -d>/tsd-pr039b-alpha <mktemp -d>/tsd-pr039b-beta`
-- two real projects, both from CLI arguments. The same `xdotool`/
`niri msg action screenshot-window`/`wl-paste` capture method every prior slice established.

- `evidence/pr-039-b/before-cold-start-two-tabs.png` -- cold start: `Project Board` route,
  `alpha` (the auto-activated first project) shows `●`, `beta` shows `○`. `Projects` carries no
  symbol at all -- only the background-fill "you are here" treatment (visibly lighter than
  `alpha`'s own background) -- since `"●"` means "active project" and the board is not a project.
- Real mouse click on the `beta` tab (`xdotool mousemove --sync` + `click 1`):
  `evidence/pr-039-b/after-clicking-beta-tab.png` -- `beta` now `●`, route becomes
  `ActiveProjectWorkspace` (status bar: "Project Workspace | 2 projects"), explorer/editor panes
  visible.
- Real mouse click on the `Projects` tab:
  `evidence/pr-039-b/after-clicking-projects-home-tab.png` -- route returns to `ProjectBoard`;
  `beta` still shows `●` (still the active project); `Projects` again shows no symbol, only the
  background fill, now carried by a real click rather than only the unit tests above.
- Real click on `beta` (making it active), a real click on empty window space (clearing native
  button focus -- see below), then `Tab`, `Tab` (`xdotool key --clearmodifiers Tab` twice:
  `MainArea → Sidebar → TabStrip`), then `ArrowRight` (highlight moves off the home tab onto
  `alpha`): `evidence/pr-039-b/focused-tab-distinct-from-active-tab.png` -- **response 306's
  required evidence**. `alpha` shows the focus border and `"> ○"` (focused, not active); `beta`
  shows `"●"` with no border (active, not focused); `Projects` shows neither symbol nor border.
  Both the focus and active channels legible at once, on different tabs, from a real keypress
  sequence.
- From that same highlighted state, real `Return`
  (`xdotool key --window "$WID" --clearmodifiers Return`):
  `evidence/pr-039-b/after-enter-switches-to-highlighted-tab.png` -- `alpha` becomes active (`●`
  and the focus border together) and the route switches to `ActiveProjectWorkspace`, proving the
  strip is keyboard-operable end to end through the real built binary, not only through
  `enter_on_a_highlighted_project_tab_switches_to_that_project`'s equivalent constructed scenario.

  Getting clean captures of this and the focused-not-active frame above took real correction,
  disclosed rather than smoothed over. Response 307 caught that my first
  `focused-tab-distinct-from-active-tab.png` did not show what its own caption claimed -- no tab
  in that frame was actually focused, because the `Tab, Tab, ArrowRight` sequence had silently had
  no effect. Read against the account already written for the Enter capture in this same session
  (a pixel-identical "no effect" result on the first two attempts, despite `xdotool windowfocus
  --sync` beforehand), this was the identical quirk hitting a neighbouring capture that had not
  been re-checked. Both screenshots above were re-taken with the same fix: a real mouse click on
  empty window space, establishing genuine compositor-level input focus, immediately before the
  `Tab, Tab, ArrowRight[, Return]` sequence. Both now show exactly what their captions describe.
  My read is still an `xdotool`/niri/XWayland synthetic-focus delivery quirk in this
  evidence-gathering harness, not an application defect -- the automated test
  (`enter_on_a_highlighted_project_tab_switches_to_that_project`) already proved the handler logic
  correct independent of any live capture -- but the root cause was not isolated further, since
  that is tooling, not product behaviour. The lesson carried forward: when a quirk is found and
  worked around in one capture, every capture taken in the same session needs the same check, not
  just the one where the symptom was first noticed.
- Process terminated cleanly with `SIGTERM` after capture; no terminal was ever launched this
  session, so `test-process-leak.md`'s defect class does not apply.

## PR-039-C — close a project

_Pending._

## PR-039-D — affordance audit and closeout

_Pending._

## Known limitations (RFC-039-wide)

_Pending._
