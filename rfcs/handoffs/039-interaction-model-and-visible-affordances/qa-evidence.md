---
title: "RFC-039: QA evidence"
rfc: "RFC-039"
rfc_file: "../../done/039-interaction-model-and-visible-affordances.md"
source_rfc_status: "Implemented and closed 2026-08-25 — RFC-039 is in rfcs/done/"
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
at all -- it never renders `"●"`/`"○"`. `"●"` means one thing everywhere it appears now: "this is
`AppState::active_project_id()`", a fact about a project session, which the home tab is not.
Response 307 also caught that `focused-tab-distinct-from-active-tab.png` did not show what its own
caption claimed (no tab in that frame was actually focused) -- the same synthetic-focus quirk
described below for the Enter-activation capture, hit on a neighbouring capture and not re-checked
there.

**Response 308's own follow-on finding.** Dropping the symbol left "you are on the board" carried
by `tab_active_style`'s background-fill channel alone -- colour-only (its `border` field responds
to `focused`, not `active`), and measured well under WCAG 2.1 SC 1.4.11's 3:1 floor for a non-text
indicator: `background` (`rgb(0.08, 0.08, 0.09)`) against `surface_elevated`
(`rgb(0.12, 0.12, 0.12)`) is 1.107:1 (response 309's own correction of the figure first cited in
response 308), the same class of defect this project already found and fixed twice (`0.11.0`'s
unfocused-pane border at 2.63:1, `0.12.0`'s modal scrim at 2.40:1). Fixed by
wrapping the home tab's name in square brackets when active (`home_tab_label`'s new `active`
parameter) -- a shape distinct from `tab_marker`'s own circle vocabulary, so it cannot be misread
as a second active project, and legible with no colour at all, the same property `focus_marker`
already has. The project tabs' own background fill is not a parallel defect: `tab_marker`'s
`"●"`/`"○"` already carries RFC-015 compliance for `active` there, independent of the fill: the
fill is redundant reinforcement, not the sole channel, for those tabs specifically.

All five screenshots in this section were re-captured against the rebuilt binary after every fix
in this slice's own review history; the descriptions below match what is actually in each file.

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
- `home_tab_label_marks_being_on_the_board_with_brackets_not_colour_or_the_project_symbol` --
  response 308's fix: all four `active`×`focused` combinations render distinctly, active never
  equals inactive at the same focus state, and none of the four ever contains `"●"`/`"○"`.
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
  `alpha` (the auto-activated first project) shows `●`, `beta` shows `○`. `Projects` renders as
  `[Projects]` -- brackets, not `"●"`, since that symbol means "active project" and the board is
  not a project. Legible with no colour involved, unlike the background fill alone.
- Real mouse click on the `beta` tab (`xdotool mousemove --sync` + `click 1`):
  `evidence/pr-039-b/after-clicking-beta-tab.png` -- `beta` now `●`, route becomes
  `ActiveProjectWorkspace` (status bar: "Project Workspace | 2 projects"), explorer/editor panes
  visible.
- Real mouse click on the `Projects` tab:
  `evidence/pr-039-b/after-clicking-projects-home-tab.png` -- route returns to `ProjectBoard`;
  `beta` still shows `●` (still the active project); `Projects` again renders as `[Projects]`,
  now carried by a real click rather than only the unit test above.
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

**Build.** `ProjectCloseModal`/`ProjectCloseButton`, mirroring `TranscriptPurgeModal`'s existing
shape -- with one deliberate departure: `Cancel` is a real decision here (`safe_close_decision`'s
`Cancelled` outcome), not a silent close, so both `ModalActivate` and, uniquely among this crate's
modals, `ModalDismiss`/Escape both record it. `×` on every project tab (never the home tab) --
`iced` does not nest interactive widgets, so it is a sibling `button` next to the switch button,
both wrapped in one `container` that now carries the shared border/fill (`tab_active_style`
renamed `tab_container_style`, moved from `button::Style` to `container::Style`; the two inner
buttons get a new transparent `tab_inner_button_style`).

`attempt_close_project_tab` -- §1's split: `SafeToClose` closes directly (no modal, ever);
`NeedsConfirmation` opens the dialog, defaulted to `Cancel` (§4a: closing is irreversible, the
safe default is not closing); `UnsupportedOrUnknown` leaves the project untouched. `apply_project_close_confirmation`/
`terminate_project_live_work` implement §6's confirmed sequence exactly: `record_safe_close_authorized`,
then `request_terminate` on every live terminal (`TerminalPane::request_terminate`, its first
production caller, via each pane's own runtime -- each `TerminalPane` owns one), then
`close_project`, then `record_safe_close_decision`. Each terminated terminal is additionally
recorded through the pre-existing `record_plain_terminal_terminated` (best-effort, the same
`OrphanedUnknown`/`Failed` → `NotRequired` handling it already has). Agent-run ownership is
checked per terminal and, when present, routed through `apply_agent_terminal_outcome_for_project`
(response 310's foundation) rather than the plain-terminal path, so a project's own agent run
status is retired correctly even when that project is not the active one. `finish_project_close_navigation`:
if closing removed the active project and none remains, routes back to the board; otherwise
leaves the route alone, so closing a background tab never disturbs whatever the user was doing.

**A pre-existing core defect found and fixed along the way (response 311).**
`CloseResourceSummary::provider_state` defaulted to `Unavailable` for every `ProjectSession`, and
nothing in production code ever upgraded it -- `set_runtime_summary`, the only setter that could,
is `#[cfg(test)]`. `close_project` had zero production callers before this slice, so the defect
was never exercised end to end: `assess_project_close` could never return `SafeToClose` for any
real project, idle or not, ever. Confirmed directly, not inferred: four of this slice's own tests
hit it, each landing on `UnsupportedOrUnknown { reason: "active-resource state is unavailable" }`
where `SafeToClose` was expected. Fixed by giving `CloseResourceSummary` a real `Default` (provider
`Complete` -- every count it holds except `dirty_files` is tracked in-memory and incrementally
from construction, and `dirty_files` starts at 0 honestly too, since a fresh project has no open
buffers); `provider_missing()` keeps its own role for the genuinely exceptional case. A second
defect this exposed: `set_file_state`'s downgrade to `Unavailable` was one-way, never restoring
`Complete` even when handed a complete file state -- invisible while the default was already
`Unavailable` forever, a real latch once the default became `Complete`. Made symmetric in the same
change. Three existing core tests updated to the corrected behaviour; one new test
(`recovering_file_provider_completeness_restores_the_close_assessment`) proves the recovery path,
ablated to confirm it exercises the fix. Committed separately from this slice's own GUI work
(`641a5ac`), after the foundation commit (`ca2245f`) both responses 310 and 311 reviewed.

**Security.** `switch_to_project_tab`'s own `ensure_explorer_scanned` discipline does not apply
here (there is nothing to scan after a project disappears); instead, `finish_project_close_navigation`
only re-scans when a *different* project became active, the same "never trust a stale listing"
principle applied to the one case where it's reachable. No new call to `add_project_from_path` or
any audit producer beyond the two explicitly required (`record_safe_close_authorized`/
`record_safe_close_decision`, plus the pre-existing `record_plain_terminal_terminated`). `×` and
the confirmation both operate only on `ProjectId`s already present in `AppState::projects()`.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (383 tekstide + 734
tekstide-core, up from 373/728; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` -- all
clean.

**Ablations.**

- `recovering_file_provider_completeness_restores_the_close_assessment` (core): the one-way
  downgrade briefly restored (removing the upgrade branch of `set_file_state`) -- the test failed
  with `provider_state` stuck `Unavailable` after a recovered `Complete` file state, confirming the
  test actually exercises the symmetric fix. Reverted.
- The confirmed-close flow's own correctness is proven by construction, not a single ablatable
  line: `confirming_the_close_terminates_the_real_process_and_removes_the_project` drives a real
  `/bin/sh` through the full sequence and checks the pane is gone, the project is gone, and both
  audit phases share one `operation_id` -- any one of the three orchestration steps landing out of
  order or being skipped fails a specific assertion in that test, which is the same proof-by-
  construction shape `granting_trust_through_the_real_route_records_both_audit_records` already
  established for a comparable two-phase flow.

**Tests, at the level this crate always tests rendering: the string, not the `Element` tree, plus
real end-to-end flows against a real process.**

- `project_close_dialog_escapes_a_bidi_override_in_the_canonical_path` -- §2's own falsifiable
  claim, the bidi-override fixture this project always uses for it.
- `project_close_dialog_body_names_the_canonical_path` -- the path, not only the display name,
  reaches the confirmation text.
- `project_close_dialog_reasons_line_states_the_real_counts` -- §1's "counts, not vague warning
  text," proven against real `CloseReason` messages, with an explicit check that generic warning
  text ("unsaved work") never appears.
- `closing_an_idle_project_removes_it_with_no_confirmation` -- §1's idle half.
- `closing_a_project_with_a_live_terminal_opens_a_confirmation_defaulted_to_cancel` -- §1's
  confirmation half, plus §4a's safe default.
- `cancelling_the_close_confirmation_leaves_everything_running_and_records_it` /
  `escaping_the_close_confirmation_also_records_a_cancelled_decision` -- §4's declined outcome,
  reached both ways RFC-039 treats as the same decision; both prove the project and its real
  terminal are untouched and exactly one single-phase `Cancelled` record (`operation_id: None`)
  is written.
- `confirming_the_close_terminates_the_real_process_and_removes_the_project` -- §6's full sequence
  against a real `/bin/sh`: the pane is gone (not orphaned), the project is gone, and both audit
  phases (`Authorized` then `Applied`, sharing one `operation_id`) are persisted.
- `closing_a_project_leaves_its_transcripts_and_audit_records_intact` -- §3, required verbatim: a
  real transcript file on disk and a pre-existing audit record for the project both survive an
  idle close, byte-for-byte for the transcript.
- `closing_a_background_project_does_not_disturb_the_active_one` -- response 310's own point,
  proven live rather than only at the core layer: `×` works on a project that is not the active
  one, and closing it does not change which project is active.

**A test-isolation hazard, mechanism now confirmed rather than suspected (response 312).**
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline` (a pre-existing
test, already one of the four names in `test-process-leak.md`'s own table) first surfaced this
session as a flake under full parallel `cargo test -p tekstide`, passing reliably alone and under
`--test-threads=1` -- a **different cause** from the `Child::drop`-leak mechanism that document
tracks, even though the symptom landed on the same test name (recorded here as its own hypothesis,
per response 311's instruction, rather than folded into that document).

Response 312 caught a **second, deterministic instance of the identical mechanism**, in this
slice's own `confirming_the_close_terminates_the_real_process_and_removes_the_project`: `cargo
test --workspace --all-targets --all-features`, run three times in a row, failed the same
assertion every time (`a confirmed close writes exactly two phases: ... left: 1 right: 2`), while
`--test-threads=1` passed 383/383 all three times. Diagnosis: the test queried
`AuditQuery::latest(50)` and filtered the results client-side by `project_id` -- a window over the
*entire* store, shared by the whole test binary. Under real parallel execution, other tests wrote
more than fifty records to that same store between this test's own `Authorized` write and its
query, pushing `Authorized` out of the 50-record window while the later `Applied` stayed in. Not
a flake in the close logic itself -- `AuditStore` structurally cannot persist an `Applied` record
without a matching `Authorized` one already present (response 310's own two-phase enforcement), so
`Applied` being visible already proved `Authorized` existed; the test just wasn't looking for it
correctly.

**Fixed by querying server-side instead of windowing-then-filtering**: `AuditQuery`'s own
`project_id`/`family`/`outcome` fields apply as SQL `WHERE` clauses before the `LIMIT`
(`store.rs`'s own `query`), so passing this project's real id (and, where useful, the family/
outcome) means the fifty-record window only ever has to hold *this project's own* records,
regardless of how much unrelated traffic the shared store carries concurrently. Fixed in all five
call sites this slice added that used the vulnerable `latest(50)`-plus-client-filter shape.
Verified against the exact command and cadence that found it: `cargo test --workspace
--all-targets --all-features`, three consecutive runs, all clean (383 tekstide + 734
tekstide-core, 0 failed).

**This generalizes past both instances**: any `latest(N)`-plus-client-filter query against this
shared `AuditStore` is unreliable by construction the moment a concurrent test writes more than
`N` records of its own -- not particular to close, or to this file. The fix here is local (the
five call sites this slice touched); the general shape is worth remembering the next time an
audit-store-backed test is added anywhere in this workspace.

**Live evidence.** `cargo build --release -p tekstide`, launched
`env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d> ./target/release/tekstide <mktemp -d>/tsd-pr039c-alpha <mktemp -d>/tsd-pr039c-beta`
-- two real projects, both from CLI arguments. The same `xdotool`/`niri msg action
screenshot-window`/`wl-paste` capture method every prior slice established.

- `evidence/pr-039-c/before-cold-start-two-tabs.png` -- cold start: both project tabs show a real
  `×`, never the `[Projects]` home tab.
- Real click on `alpha`'s `×` (idle, nothing running):
  `evidence/pr-039-c/after-clicking-close-on-idle-project.png` -- `alpha` gone immediately, no
  modal, the Project Board's own recent-projects row for it now shows an "Open" button (RFC-038's
  own reopen affordance correctly picking it up).
- Real `Ctrl+Alt+T` in `beta`, then a real click on `beta`'s `×`:
  `evidence/pr-039-c/confirmation-names-path-and-counts.png` -- **the required proof for §1/§2**:
  "Close this project?", the real canonical path (`/tmp/tmp.xxxxxxxxxx/tsd-pr039c-beta`), "This
  will end: 1 running process" (a real count, not vague text), focus defaulted to `Cancel`.
- From that dialog, real `Tab` then `Return` (`xdotool key --clearmodifiers Tab` then `Return`):
  `evidence/pr-039-c/after-confirmed-close-real-termination.png` -- the real terminal pane is gone,
  `beta` is gone, route returned to the Project Board (no project remained active). Getting the
  `Return` press to register took a second attempt with an explicit `xdotool windowactivate --sync`
  first -- the same synthetic-focus delivery quirk in this evidence-gathering harness already
  disclosed for PR-039-B's own live-Enter captures, not a new one. The mouse click on the "Close"/
  "Cancel" text lines themselves does nothing at all, by design: unlike the tab strip's own
  buttons, `ProjectCloseModal`'s two lines are plain `text`, not `iced::widget::button` -- this
  modal is keyboard-only, the same convention every other modal in this crate already follows
  (`TranscriptPurgeModal` included), stated explicitly in its own on-screen hint
  ("Tab/Shift+Tab moves focus; Enter activates; Escape always cancels") which never mentions
  clicking.
- A fresh single-project launch, real `Ctrl+Alt+T`, real click on `×`, then real `Escape`:
  `evidence/pr-039-c/escape-cancels-terminal-still-running.png` -- the project and its real,
  still-`Running` terminal both untouched, proving §4's declined outcome live, not only through
  the unit tests above.
- Processes terminated cleanly with `SIGTERM` after each capture session; the one real terminal
  genuinely still running at the end of the last session was the one the test itself was
  demonstrating survives cancellation, so `test-process-leak.md`'s defect class does not apply to
  the evidence-gathering process itself either way.

## PR-039-D — affordance audit and closeout

**Build nothing. Find things** — per the task breakdown, no code changes in this section. Full
findings, with the exhaustive method behind each, in
[`affordance-audit.md`](./affordance-audit.md). Summary:

- **Every one of this crate's nine modals has zero mouse-clickable controls for its own
  decision** -- `Approve`/`Reject`, `Grant`/`Cancel`, `Purge`/`Cancel`, `Reload`/`Dismiss`,
  `Close`/`Cancel`, the folder browser's own row navigation, all plain `text`, keyboard-only.
  Confirmed by direct inspection of every modal's own view function (`button(` count: 0, in all
  nine, no exception). Generalizes response 312's own question about `ProjectCloseModal`
  specifically to the whole crate.
- **Ten of thirteen live global actions have no visible control anywhere in the
  application** -- `ToggleProjectMode`, `LaunchTerminal`, `PasteIntoTerminal`,
  `SaveActiveDocument`, `LaunchAgentRun`, `OpenCurrentAgentRunDetail`, `OpenApprovalHistory`,
  `OpenTrustSettings`, `OpenHelp`, and (more defensibly, since `OpenFolderBrowser`'s own button
  already serves the underlying workflow) `OpenProjectEntryField`. Confirmed against the
  application's *entire* mouse-clickable inventory -- ten `.on_press` call sites, full stop,
  verified by grep across every file in the crate. `OpenProjectBoard`, `SwitchActiveProject`, and
  `OpenFolderBrowser` are the only three of thirteen with one.
- **`OpenSafeCloseDialog` stays dead, now for a sharper reason**: PR-039-C built the real
  capability its name promises, wired to `×` instead of to this action.
- **`CycleVisibleTerminalSession`/`OpenDiffReview`** remain dead, unchanged, exactly the task
  breakdown's own starting point.
- **`OpenCommandPalette`** stays `Reserved`, nothing behind it, unchanged.
- **Nine tests share the exact query-race shape response 312 found and fixed five instances
  of** (`AuditQuery::latest(50)` plus a client-side project filter, against the crate's one real,
  shared `AuditStore`) -- named by test, not only by line number, in the audit document per
  response 312's own instruction. Not converted here: the choice between converting all nine,
  recording the risk, or making the store per-test is a real decision, not a mechanical follow-up.

None of the above is new code; nothing in this section changes gates, tests, or live evidence
already recorded for PR-039-A through PR-039-C.

## RFC-039's acceptance criterion, answered in its own words

*"A person who has read nothing opens two projects, moves between them, closes one, and returns
to the board — using only what the window shows them."* Yes, for exactly this scenario: the
Project Board's own "Browse..." button opens a project (PR-038-G); the tab strip's own per-project
tabs move between open projects (PR-039-B); `×` on a tab closes it, with a real, visible
confirmation naming counts and the canonical path when there is live work to lose (PR-039-C); the
permanent leftmost "Projects" tab returns to the board (PR-039-A/B). Every step in this specific
sentence has a real, clicked-in-live-evidence control — `before-cold-start-two-tabs.png` through
`after-clicking-close-on-idle-project.png`/`after-confirmed-close-real-termination.png` across
PR-039-A/B/C's own evidence directories.

*"Every workflow claimed as served names the control the user sees, not the keystroke that also
works."* True of every workflow this RFC's own three build slices (A/B/C) actually built. **Not**
true of the wider application -- the affordance audit above found ten live, pre-existing actions
this RFC did not touch that have no visible control at all. This criterion is answered for what
RFC-039 shipped, not as a claim about the whole product; the audit is what keeps that distinction
honest rather than implicit.

*"Proven from real events through production code, with a cold-start capture."* Every PR-039-A/B/C
screenshot is a real `xdotool`/`niri`/`wl-paste` capture against the real release binary, launched
cold with real CLI-argument projects -- never a description of intended behaviour. Each PR's own
`qa-evidence.md` section states its capture command and, where a capture needed correction (the
two synthetic-focus-quirk cases in PR-039-B/C's own sections), the correction rather than a
silently replaced file.

## Known limitations (RFC-039-wide)

- **Ten of the application's thirteen live global actions have no visible control** --
  `affordance-audit.md`'s Finding 2, table and both notes. Out of this RFC's own fix-scope (it
  built the tab strip and the close flow, not a whole-application affordance pass); recorded so it
  is a stated limitation, not a silent gap.
- **Every modal in the crate is keyboard-only for its own decision, including the three this RFC
  built or touched** (`TranscriptPurge`'s pre-existing shape, and the new `ProjectClose`) --
  `affordance-audit.md`'s Finding 1. A user who reaches a modal by mouse (clicking `×`, "Browse...",
  the Trust Settings buttons) cannot complete or cancel it without a keyboard. Disclosed rather
  than fixed quietly in PR-039-C, per response 312's own instruction that this is an audit finding,
  not a cleanup commit.
- **Nine tests share an unaddressed query-race shape** against the crate's one real, shared
  `AuditStore` -- `affordance-audit.md`'s Finding 6, named by test. PR-039-C's own five instances
  of the identical shape are fixed; these nine pass today but are not proven robust against the
  same failure under sufficient concurrent audit-store traffic.
- **`OpenSafeCloseDialog` remains a dead action** even though the capability it names now exists,
  reachable through `×` rather than through this `NavigationAction` -- `affordance-audit.md`'s
  Finding 3. Whether it should gain a binding as a coarser accelerator (the same shape
  `SwitchActiveProject` has for the tab strip) is an open design question, not decided here.
- **`CycleVisibleTerminalSession`/`OpenDiffReview` remain dead**, and **`OpenCommandPalette`
  remains `Reserved` with nothing behind it** -- unchanged by this RFC, restated for completeness
  rather than re-discovered.
