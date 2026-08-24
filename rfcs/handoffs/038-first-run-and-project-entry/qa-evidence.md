---
title: "RFC-038: QA evidence"
rfc: "RFC-038"
rfc_file: "../../accepted/038-first-run-and-project-entry.md"
source_rfc_status: "Accepted 2026-08-24 — M12, first"
target_milestone: "M12"
created: "2026-08-24"
---

# QA evidence

One section per PR. Cite the command that produced each result — per `ARCHITECTURE.md`, a claim
about behaviour names the command that established it, and that rule binds the implementer and
the reviewer equally.

Screenshots go in `evidence/pr-038-<letter>/`, each with the launch command recorded beside it.
The cold-start capture in `../first-run-correction/evidence/` is the reference for form.

## PR-038-A — the path field

**Build.** A path entry field on the Project Board's empty state (`board::empty_state_view`),
focused by construction -- see that function's own doc for why not `iced::widget::text_input`
(this project routes every keystroke through one reviewed router, `input::route_non_modal_input`;
a second, widget-internal capture path would bypass it). Wired through the existing,
already-reviewed `add_project_from_path` -- no canonicalisation, symlink logic, or root
validation added in `shell.rs`. Failure renders a bounded, escaped diagnostic and leaves the
application running; success clears the field and writes a real `project_added` audit record.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (326 tekstide +
714 tekstide-core, up from 318 tekstide before this slice; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings` -- all clean.

**The acceptance criterion.** Proven from a **real key event through production code**, not a
dispatched `Message`:

- Automated: `shell::tests::a_real_typed_path_and_enter_opens_a_project_from_a_cold_empty_board`
  builds a `KeyPress` per character, routes each through the real `route_non_modal_input`, and
  calls the real `update` -- exactly the shape
  `a_real_typed_key_inserts_into_the_active_document` already established for the editor.
- Live, against the release binary: `cargo build --release -p tekstide`, launched
  `env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d> ./target/release/tekstide` (no arguments;
  `WAYLAND_DISPLAY` unset to force the X11/XWayland backend `xdotool` requires, RFC-015's own
  precedent). Window focused via `xdotool windowfocus --sync <id>`, every keystroke sent
  individually via `xdotool key --window <id> --clearmodifiers <keysym>` -- never a pre-seeded
  string -- then a real `Return`. Screenshots via `niri msg action screenshot-window --id <id>`
  (this box's `screenshot-path null` again, saved from the clipboard with `wl-paste`):
  - `evidence/pr-038-a/before-empty-board-with-field.png` -- the empty board, field focused
    (blue `border_focused` outline), no text naming an action that does not exist.
  - `evidence/pr-038-a/after-project-opened.png` -- a real row (`tsd-pr038a-demo`,
    `/tmp/tsd-pr038a-demo`, `Restricted`), status bar reading `Project Board | 1 project`.
  - **A disclosed synthetic-input reliability finding, the same class RFC-015's own evidence
    recorded**: the first attempt's leading `/` keystroke, sent immediately after
    `xdotool search`, was dropped -- the field held `tmp/tsd-pr038a-demo` (no leading slash),
    which `add_project_from_path` correctly refused as `DoesNotExist`. This incidentally also
    proves the failure-diagnostic path live (see below). Retried with `windowfocus --sync` and a
    short delay before the first keystroke, which resolved it, matching PR-015-E's own finding
    about the keystroke immediately after a focus change.

**Security -- `what-a-path-field-must-not-trust.md`.**

- §1 (untrusted, escaped at render): `board::path_field_display_text` routes through
  `text_safety::quote_untrusted`, factored out of the widget tree so it is directly testable
  (the same shape `row_lines` uses). `shell::tests::a_directionality_override_in_the_typed_path_renders_as_a_visible_marker_not_obeyed`
  proves `U+202E` renders as `<U+202E>` and the real override character never reaches the
  rendered string, tested against `path_field_error_text` (the function
  `attempt_open_project_from_path_field` actually calls to build what a user sees on failure).
- §2 (failure renders, never exits):
  `shell::tests::a_bad_path_renders_a_notice_and_the_application_keeps_running` -- a nonexistent
  path renders `PathFieldError::DoesNotExist`, the field keeps exactly what was typed (so the
  user can correct it), and the same live `state` opens a real project immediately after,
  proving the process kept running. The live screenshot above shows the same diagnostic for
  real.
- §3 (diagnostic bounded, not just escaped): `path_field_error_text` truncates the raw path to
  `MAX_PATH_FIELD_ERROR_DISPLAY_CHARS` (128, RFC-023's own number) before escaping, marking
  truncation visibly -- truncate-then-escape, matching `bound_key_segment`'s order exactly, but
  through `quote_untrusted`'s single canonical whole-string API rather than a second, hand-rolled
  escaping call (see that function's own doc for why calling `escape_untrusted_chars` a second
  time would only have escaped the same text twice for no benefit). No second escaping routine
  was written anywhere in this slice.
- §4 (Restricted, agent run refused):
  `shell::tests::a_project_opened_through_the_field_refuses_an_agent_run_until_trust_is_granted`
  -- a project opened through the field, then a real `Ctrl+Alt+A`-shaped `LaunchAgentRun` dispatch,
  refused exactly the way the CLI path's own
  `agent_run_launch_shell_input_switches_to_terminal_immersion_and_shows_the_real_trust_refusal`
  already proves. `Restricted` itself needs no field-specific code: `ProjectSession::new` sets it
  unconditionally, inherited for free by reusing `add_project_from_path`.
- §6 (no reimplementation): `shell.rs` calls `state.app_shell.add_project_from_path(&path)`
  directly; no `canonicalize`, symlink policy, or root validation anywhere in this slice's diff.

**The audit guard -- §5.** `record_path_field_project_added` (`shell.rs`) mirrors
`apply_workspace_trust_grant`'s own direct `open_real_audit_store` +
`AuditCoordinator::new` + one producer-call shape, reached directly rather than reusing
`main.rs`'s `open_cli_project_path_and_record` (whose caller, `boot()`, exits on `Err` --
catastrophic from a text field, §2). Proven live:
`shell::tests::opening_a_project_through_the_real_field_writes_exactly_one_real_project_added_record`.
**Ablated** (single-variable: only the `record_path_field_project_added(state, project_id)` call
removed, nothing else touched): the test failed, `0` records instead of `1`; reverted, re-ran
green. `crates/tekstide/src/tests.rs`'s
`add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else` was widened from a
single allowed file to a `HashMap` of exact counts (`main.rs`: 1, `shell.rs`: 1), kept a count
rather than a presence check for the same reason response 264 gave for RFC-031 -- a *third*,
unreviewed call in either already-allowed file must still fail it. **Ablated**: temporarily added
a dead second call (`if false { ... add_project_from_path(&path) ... }`) inside `shell.rs`; the
test failed (`shell.rs calls add_project_from_path 2 time(s), expected 1`); reverted, re-ran
green.
`shell::tests::resubmitting_the_same_path_through_the_field_focuses_it_without_a_second_record`
proves re-submitting an already-open path focuses it and writes no second record, the field's own
analogue of `reopening_the_same_project_path_focuses_it_instead_of_writing_a_second_record`.

**Resource bound, disclosed but not required by the checklist.** `push_to_path_field` caps
`state.path_field` at `MAX_PATH_FIELD_CHARS` (4096, Linux's own `PATH_MAX` -- reasoned against the
real OS limit rather than an arbitrary UI number, so a genuine path is never rejected by this
bound). Shared by typing and `Ctrl+V`'s resolved paste (`Message::PathFieldPasteResolved`), so
neither can grow the field without bound; not itself an acceptance-checklist item, recorded
because a hostile or oversized paste is exactly the vector
`what-a-path-field-must-not-trust.md` §1 names. **Ablated**:
`shell::tests::the_path_field_stops_growing_at_its_bound_rather_than_unbounded` proves a 4596-char
paste stops at exactly 4096.

**Not `Ctrl+V` tested live** (deferred, not a gap in this checklist): `iced::clipboard::read()`
has no synchronous test seam in this suite (same limitation `attempt_paste_into_terminal`'s own
tests accept for terminal paste) -- `push_to_path_field`'s bound is proven directly instead, and
the message-arm wiring (`Message::PathFieldPasteResolved`) is a direct structural mirror of the
already-reviewed `TerminalPasteResolved` arm.

**Scope discipline.** Did not touch `boot()`'s CLI argument handling, `keyboard_help`, or the
populated-board arm, per the task breakdown -- PR-038-B (below) touches both of the latter two,
deliberately, as its own separately-reviewed slice.

## PR-038-B — `Ctrl+Alt+O`

**Build.** `NavigationAction::OpenProjectEntryField`, `Candidate` with `Some("Ctrl+Alt+O")`,
reveals and focuses the same `path_field_section` PR-038-A built, now also on the *populated*
board -- the second-project case the empty-board-only field could not serve. `Escape` dismisses
it without submitting (no-op on the permanently-shown empty-board field, which has nothing else
to reveal by dismissing).

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (330 tekstide +
715 tekstide-core, up from 326/714; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` --
all clean.

**Mechanically unclaimed, not by inspection.**
`navigation::tests::open_project_entry_field_shortcut_is_a_candidate_that_collides_with_no_other_rule`,
the same shape `open_current_agent_run_detail_shortcut_is_a_candidate_that_collides_with_no_other_rule`
already established. **Ablated**: changed the binding to `Ctrl+Alt+P` (colliding with
`OpenProjectBoard`) -- the test failed; reverted.

**The catalog key and the count.** `action_catalog_key`'s exhaustive match gained
`OpenProjectEntryField => Some("keyboard-help-open-project-entry-field")` --
`action_catalog_key_is_some_iff_the_action_is_live` (request 290's own biconditional) now covers
it automatically, no separate test needed. `every_live_binding_is_described_to_the_user` and
`the_empty_state_lists_every_live_keybinding` both updated from `9` to `10` **deliberately**, not
loosened to `>=`. Live: the cold-start screenshot below shows all ten, including
`Ctrl+Alt+O   Add a project by path`.

**A real bug found and fixed while wiring the dispatch.** The obvious mapping --
`app_command_for(OpenProjectEntryField) = Some(AppCommand::OpenProjectBoard)`, reusing the
existing route-change command -- compiles and looks right, but is wrong: routing through
`app_command_for`'s `Some` branch also runs `ensure_explorer_scanned`, which (via
`ApplicationShell::scan_active_project_explorer_directory`) unconditionally navigates a
freshly-scanned active project to `ActiveProjectWorkspace`, silently undoing the very route
change this action exists to make. Caught by
`ctrl_alt_o_opens_a_second_project_through_real_keys_on_a_populated_board` itself failing
(`route()` came back `ActiveProjectWorkspace`, not `ProjectBoard`) -- diagnosed with targeted
`eprintln!` at each stage (command chosen → route after `dispatch` → route after
`ensure_explorer_scanned`), which isolated the flip to the last of those three. Fixed by moving
this action out of `app_command_for`'s `Some` arm entirely (grouped with
`PasteIntoTerminal`/`SaveActiveDocument`'s "no core route/mode change through this path"
category) and dispatching `AppCommand::OpenProjectBoard` directly in `update`'s `Shell` arm,
which never reaches `ensure_explorer_scanned` -- correct, since this action never shows the
explorer. Recorded in both `app_command_for`'s and the dispatch site's own doc comments so the
"obvious" mapping is not silently reintroduced later.

**A second real finding, disclosed rather than asserted around**: opening a second project
through the field does not make it active -- `AppState::add_project_session`'s own
`if self.active_project_id.is_none()` guard is pre-existing, deliberate core behaviour (so
`boot()`'s multi-path CLI loop does not fight itself over which of several paths ends up
active), and the field correctly inherits it rather than special-casing around it. A user who
adds a second project through `Ctrl+Alt+O` sees it appear on the board but stays on whichever
project was already active; nothing in this RFC provides a way to switch
(`NavigationAction::SwitchActiveProject` remains `Configurable`/`None`, a pre-existing gap
`future-work.md` already names). Proven in
`shell::tests::ctrl_alt_o_opens_a_second_project_through_real_keys_on_a_populated_board`, which
asserts this explicitly rather than assuming the more surprising "switches to it" behaviour.

**`Escape`, ablated.**
`shell::tests::escape_dismisses_the_on_demand_field_without_submitting_or_touching_the_open_project`
and `shell::tests::escape_is_a_no_op_on_the_permanently_shown_empty_board_field`. **Ablated**
(single-variable: the `Escape` match arm's guard changed to `if false && state.path_field_requested`,
nothing else touched): the dismiss test failed (`"Escape must hide the on-demand field"`);
reverted, re-ran green.

**Response 297's required follow-up.** The field's own hint now reads "Type a project path and
press Enter (Ctrl+V to paste):" -- catalog key renamed from `project-board-empty-path-field-label`
to `project-board-path-field-label` since `path_field_section` now renders it on the populated
board too, where "empty" no longer describes anything. `Ctrl+Shift+V` was **not** retargeted, per
the response's own explicit instruction (a paste action silently changing its destination based
on focus is exactly the surprise RFC-018 exists to prevent). Proven:
`shell::tests::the_path_field_hint_names_the_paste_gesture_that_actually_works_here`.

**Two stale claims found and corrected, beyond this PR's own diff, disclosed as scope beyond the
task breakdown's literal list**: `keyboard_help::usage_text`'s hardcoded English (`tekstide
--help`) and `README.md`'s Quick Start both still said "there is no in-app way to add a project"
-- true when written (0.12.1), false as of PR-038-A. Both corrected;
`keyboard_help::tests::usage_text_says_how_to_open_a_project` updated to assert the *current*
claim rather than the stale one. Not part of PR-038-B's own task list, but leaving one of two
copies of the same now-false sentence fixed and the other broken seemed worse than the small
scope addition -- flagged here for your judgement rather than silently expanded past.

**Live, against the release binary**: `cargo build --release -p tekstide`, launched with one
project already open via CLI argument (`env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d>
./target/release/tekstide /tmp/tsd-pr038b-first`), same `xdotool`/`niri` capture method as
PR-038-A.

- `evidence/pr-038-b/before-one-project-no-field.png` -- one project on the board, ten
  keybindings listed (`Ctrl+Alt+O` among them), no field.
- Real `Ctrl+Alt+O` (`xdotool key --window <id> --clearmodifiers ctrl+alt+o`): the field appears
  above the keyboard list, below the existing row, hint naming `Ctrl+V`.
- Real path typed one keystroke at a time, real `Return`:
  `evidence/pr-038-b/after-second-project-added.png` -- both projects now on the board (`1
  project` → `2 projects` in the status bar), field gone again.

**Scope discipline.** `boot()`'s CLI argument handling untouched. PR-038-C (below) touches
`keyboard_help` and the board's own render arms, deliberately, as its own separately-reviewed
slice.

## PR-038-C — the help surface

**Build.** A modal (`ModalContent::Help`, `Ctrl+Alt+K`), reachable from anywhere -- global
keybindings are matched before terminal focus or shell zone are even consulted, and the modal
composes in `view()` regardless of `AppRoute`/`ProjectMode`. Renders
`keyboard_help::keyboard_help_lines` directly (no second list), in a `modal_dialog_box`, no
buttons -- `ModalFocusNext`/`ModalFocusPrevious`/`ModalActivate` are all no-ops against it (the
first two truly no-op, `ModalActivate` closes it the same way every other modal's "no real
decision for this focus" arm already does); only `Escape` (`ModalDismiss`, already generic
across every `ModalContent` variant) does anything.

Removed the keyboard list from the Project Board entirely -- both the empty-state and populated
arms (`board.rs`'s own `keyboard_help_view` function and its two call sites deleted, along with
`project-board-empty-keyboard-heading`, now unused).

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (334 tekstide +
716 tekstide-core, up from 330/715; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` --
all clean.

**Mechanically unclaimed, not by inspection.**
`navigation::tests::open_help_shortcut_is_a_candidate_that_collides_with_no_other_rule`, same
shape as PR-038-B's own. **Ablated**: changed the binding to `Ctrl+Alt+H` (colliding with
`OpenApprovalHistory`) -- test failed; reverted.

**The catalog key and the count.** `action_catalog_key`'s exhaustive match gained
`OpenHelp => Some("keyboard-help-open-help")` -- `action_catalog_key_is_some_iff_the_action_is_live`
covers it automatically. `every_live_binding_is_described_to_the_user` and
`advertised_bindings_are_exactly_the_live_ones` both updated from ten to **eleven**
deliberately.

**The replacement guard -- task breakdown's own instruction: "must be replaced, not deleted."**
`board::tests::every_board_state_renders_the_keyboard_list`'s own property (every arm of
`board::view` shows the keyboard list) no longer applies: no arm shows it any more,
deliberately. Replaced with two halves:

- Negative, in `board/tests.rs`:
  `this_surface_no_longer_references_the_keyboard_list_at_all` -- board.rs's source text
  contains neither `keyboard_help_lines` nor `keyboard_help_view`. **Ablated**: temporarily
  reintroduced a dead call to `keyboard_help::keyboard_help_lines` in `board::view`'s first
  line -- test failed; reverted.
- Positive, in `shell/tests.rs`:
  `opening_help_through_a_real_key_event_shows_every_live_binding` -- a real `Ctrl+Alt+K` key
  event opens the modal, whose data source lists every live binding (the same count
  `every_live_binding_is_described_to_the_user` establishes for that source directly). **Ablated**
  (single-variable: only the `state.modal = Some(ModalContent::Help)` assignment guarded behind
  `if false &&`, nothing else touched): both this test and
  `ctrl_alt_k_opens_help_from_inside_terminal_immersion` failed (`None`, modal never opened);
  reverted, re-ran green.

Also: `help_modal_view_reuses_the_shared_keyboard_help_derivation_not_a_second_list` -- source-text
proof `help_modal_view` calls the shared `keyboard_help_lines`, satisfying the task breakdown's
"reuse `keyboard_help`; do not build a second list" directly, not only by review.

**Reachable from Terminal Immersion -- the case `0.12.1` left unserved, named explicitly in the
task breakdown.**
`shell::tests::ctrl_alt_k_opens_help_from_inside_terminal_immersion` -- a real project, real
`ToggleActiveProjectMode` dispatch into genuine Terminal Immersion mode, then a real `Ctrl+Alt+K`
-- the modal opens. Live, in the capture below: cold start, then `Ctrl+Alt+K`, then `Escape`
(Terminal Immersion itself not separately screenshotted this slice -- the unit test above is the
proof for that specific route; the live capture proves the modal renders correctly and
dismisses cleanly).

**Live, against the release binary**: `cargo build --release -p tekstide`, launched
`env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d> ./target/release/tekstide`, no arguments.

- `evidence/pr-038-c/before-cold-start-no-keyboard-list.png` -- the empty board; confirms the
  keyboard list is genuinely gone, not merely untested-for.
- Real `Ctrl+Alt+K` (`xdotool key --window <id> --clearmodifiers ctrl+alt+k`):
  `evidence/pr-038-c/after-help-modal-open.png` -- all eleven bindings, scrim visible, board
  dimmed beneath -- trusted chrome over untrusted-nothing (no project names in this capture, but
  the scrim/composition is the same `stack![base, opaque(scrim)]` every other modal uses).
- Real `Escape`: `evidence/pr-038-c/after-escape-closes-help.png` -- back to the plain empty
  board, nothing left behind.

**A significant environmental finding this slice's own testing surfaced, disclosed rather than
worked around.** Running the full suite repeatedly hit `PTY exhaustion (os error 28, "No space
left on device")`, cascading into ~70 unrelated test failures across the terminal/approval
subsystems. Traced to **~4023 orphaned, idle `/bin/sh` processes** (`PS1=tekstide$`, reparented
to `systemd --user`, ages from 72 seconds to ~2.5 hours), confirmed via `/proc/<pid>/environ`
and `/proc/<pid>/fd` before any cleanup. This is `test-process-leak.md`'s own documented
`Child::drop`-does-not-kill defect, but for a code path that fix's own evidence explicitly
scoped out: `runtime/terminal/launch.rs`, the *production* real-shell-spawn path
`shell/tests.rs`'s many real-terminal tests exercise directly. A panicking terminal test leaks
its spawned shell exactly as approval-family tests did before `KillOnDropChild`; this appears to
have accumulated silently across this session's many test runs until it exhausted the
system-wide PTY pool (a shared, hard-capped OS resource, unlike the approval leak's effect on
`bind_recovers_from_a_stale_socket_file` alone).

Cleaned up with the human owner's explicit authorization, after investigation and a first pass
at `SIGTERM` (silently ignored by these shells -- confirmed via `SigIgn`, bit 14 set --
`SIGKILL` used instead): PTY count recovered from 4096/4096 to single digits, all 334 + 716
tests then passed cleanly. **Not fixed at its source this slice** -- that is core work
(`runtime/terminal/launch.rs` needs the same `KillOnDropChild` treatment
`test-process-leak.md`'s evidence already gave `approval::tests`' two call sites), out of
PR-038-C's own scope, and is not this slice's to silently absorb. Flagged for the architect's
decision on where it belongs -- a new PR, or folded into `test-process-leak.md`'s own tracking.

## PR-038-G — the folder browser

**Added 2026-08-24 by the human owner's direction**, overturning RFC-038's own D1: a typed path
is not an acceptable *primary* way to choose a folder.

**Build.** A real, clickable `iced::widget::button` ("Browse...") on the Project Board's path
field section (`board::path_field_section`) -- the first genuine `button` on *this surface*
(`board.rs` had none before). `Ctrl+Alt+B` is the accelerator alongside it, not the only route (task breakdown:
"a button, not only a key"). Both converge on `open_folder_browser`, opening
`ModalContent::FolderBrowser` at `$HOME` (falling back to the filesystem root). `Enter`
navigates the highlighted row (a subdirectory, or `Parent`); `Space` commits the directory
currently *shown* through the exact same `add_project_from_path` entry point PR-038-A's field
uses -- same audit record (`record_new_project_added`, the field's own
`record_path_field_project_added` renamed to name both callers honestly), same `Restricted`
outcome, no second way to open a project. The path field remains as the secondary route,
untouched. Keyboard-operable throughout: `Tab`/`Shift+Tab` and `ArrowUp`/`ArrowDown` move the
highlight (clamped, not wrapping), `Enter` navigates, `Space` commits, `Escape` cancels --
`modal_subscription`'s pre-existing generic dispatch, extended with Arrow and Space.

**A disclosed deviation from the task breakdown's own instruction, first and most important
item in this report.** The task breakdown says: "Reuse RFC-019's explorer tree: it already
renders an `ExplorerDirectoryScan`... Do not write a second directory renderer, and do not walk
the filesystem in the surface." What shipped instead:

- **Core scanning**: a new, separate function, `project::root::browse_directory`, producing a
  new `DirectoryBrowseScan`/`BrowseNode`/`BrowseNodeState`, rather than reusing
  `FileExplorerScanner::scan_directory`. Judged unavoidable, not a preference: that scanner's
  only constructor path requires a `ProjectRootHandle` (in turn built from a live
  `ProjectSession`), and its output type, `ExplorerDirectoryScan`, carries a `FileAccessTarget`
  whose `selected_relative_path`/`canonical_path` are project-root-*relative* by construction --
  containment/symlink-escape semantics that only mean something once a project root exists. A
  folder browser exists specifically *to choose* that root; there is no project yet for a path to
  be relative to. Building a `FileAccessTarget` for it would mean inventing fields with no honest
  referent -- exactly the "text that asserts something false" this codebase's own convention
  (`ExplorerNode.relative_path`, if reused for an absolute path) already teaches against.
  `browse_directory` follows symlinks freely rather than escape-checking them, for the same
  reason: there is no root to escape from yet -- whatever is ultimately chosen is independently
  re-validated in full by `add_project_from_path`'s own `ProjectRootValidator` at commit time
  (`what-a-path-field-must-not-trust.md` applies unchanged, cited in the module's own doc
  comment).
- **Rendering**: this is the part that is honestly closer to "a second directory renderer" than
  the scanning split above. `surface/explorer.rs` gained `BrowseRow`/`visible_browse_rows`/
  `browse_node_line`/`browse_row_line`/`browse_tree_lines`/`browse_view` -- a near line-for-line
  parallel of this module's own pre-existing `ExplorerRow`/`visible_rows`/`node_line`/`row_line`/
  `tree_lines`/`view`, differing only in the narrower `BrowseNodeState` (no `Blocked`, since
  `browse_directory` never constructs it) and the absence of a symlink-status column (`BrowseNode`
  tracks none, for the reason above). No filesystem walking was added to the surface either way.
  Not reworked into a shared renderer this slice: doing so would mean either forcing
  `BrowseNode` through `ExplorerNode`'s shape (reintroducing the dishonest-field problem the core
  split exists to avoid) or extracting a smaller shared trait/helper over just the
  name/kind-symbol/state-symbol/line-assembly logic, which was judged a genuine design decision
  for the reviewer, not mine to make unilaterally mid-slice.

  **Flagged explicitly for the architect's decision**: accept the current split (a new,
  honestly-typed scanner is required either way; the renderer duplication is small,
  independently tested, and isolated), or require a follow-up PR extracting the shared
  render-layer logic. Not blocked on an answer before filing this review -- the code compiles,
  is tested, and gates clean either way; this is a request for the architect's judgment on
  in-tree duplication, not a defect report.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (346 tekstide +
724 tekstide-core, up from 334/716 before this slice; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` --
all clean.

**Mechanically unclaimed, not by inspection.**
`navigation::tests::open_folder_browser_shortcut_is_a_candidate_that_collides_with_no_other_rule`.
**Ablated**: changed the binding to `Ctrl+Alt+K` (colliding with `OpenHelp`) -- both that test
and `open_help_shortcut_is_a_candidate_that_collides_with_no_other_rule` failed; reverted.
`every_live_binding_is_described_to_the_user` and `advertised_bindings_are_exactly_the_live_ones`
both updated from eleven to **twelve** deliberately.

**The audit guard.** `add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else`'s
allow-list widened: `shell.rs`'s own count from one to two, naming both call sites
(`attempt_open_project_from_path_field`, `choose_current_browsed_directory`) explicitly rather
than the map simply reading `2`. **Ablated**: temporarily commented out
`choose_current_browsed_directory`'s call to `record_new_project_added` --
`shell::tests::choosing_a_directory_through_the_real_browser_writes_exactly_one_real_project_added_record`
failed (0 records, not 1); reverted.

**The acceptance criterion.** Proven from **real key events through production code**, both
mouse and keyboard, not from dispatched messages:

- Automated (`shell/tests.rs`): `ctrl_alt_b_opens_the_folder_browser_with_a_real_scan`,
  `the_real_browse_button_message_opens_the_same_modal_the_keyboard_shortcut_does`,
  `ctrl_alt_b_opens_the_folder_browser_from_inside_terminal_immersion`,
  `escape_closes_the_folder_browser_modal`,
  `enter_navigates_into_a_subdirectory_and_back_up_via_the_parent_row`,
  `a_failed_navigation_leaves_the_last_good_scan_untouched_and_sets_navigate_failed`,
  `modal_focus_next_and_previous_move_the_folder_browser_highlight_clamped_not_wrapping`,
  `space_commits_the_shown_directory_as_a_new_restricted_project_and_closes_the_modal`,
  `committing_an_already_open_project_a_second_time_focuses_it_without_a_second_record`,
  `a_commit_failure_renders_the_error_and_keeps_the_modal_open`. `board/tests.rs`:
  `the_browse_button_is_a_real_clickable_widget_not_an_inert_label` -- a source-level check
  (the same shape `this_surface_no_longer_references_the_keyboard_list_at_all` already uses,
  since `iced::Element` gives a test no way to introspect whether a rendered tree is a real
  widget or an inert label) that the control is a genuine `button(...).on_press(...)`, not a
  repeat of the "named nothing" defect PR-038-A's own evidence already named.
- Live, against the release binary: `cargo build --release -p tekstide`, launched
  `env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d> HOME=<mktemp -d> ./target/release/tekstide`,
  no arguments. `HOME` overridden to a synthetic temp directory (containing only
  `projects/tsd-pr038g-demo`) rather than the real one, so the captured screenshots show no real
  personal file or folder names -- same `xdotool`/`niri msg action screenshot-window`/`wl-paste`
  capture method PR-038-A/B/C already established, this time also exercising a **real mouse
  click** (`xdotool mousemove --sync <x> <y>` then `xdotool click 1`, at the real, on-screen
  "Browse..." button's coordinates) rather than only synthetic key events:
  - `evidence/pr-038-g/before-cold-start-empty-board.png` -- the empty board, the real "Browse..."
    button visible next to the field.
  - `evidence/pr-038-g/after-real-mouse-click-opens-browser.png` -- a real mouse click on the
    button opens the browser at the synthetic `$HOME`, scrim visible, board dimmed beneath.
  - `evidence/pr-038-g/after-arrow-down-highlights-projects.png` -- real `Down` (`xdotool key
    --clearmodifiers Down`) moves the highlight onto `projects`.
  - `evidence/pr-038-g/after-enter-navigates-into-projects.png` -- real `Return` navigates into
    it; `tsd-pr038g-demo` now listed.
  - `evidence/pr-038-g/after-navigated-into-demo-dir.png` -- real `Down` then `Return` navigates
    into `tsd-pr038g-demo` itself (empty; only the `Parent` row remains).
  - `evidence/pr-038-g/after-space-commits-project-without-typing.png` -- real `space` commits
    it: the board now shows `tsd-pr038g-demo`, `/tmp/.../projects/tsd-pr038g-demo`, `Restricted`,
    `Project Board | 1 project`. **No path was ever typed** -- every character of the chosen
    directory's name came from navigation, matching the task breakdown's own evidence
    requirement exactly.
  - Process terminated cleanly with `SIGTERM` after capture (no leaked shell -- no terminal was
    ever launched in this session, so `test-process-leak.md`'s defect class does not apply
    here); synthetic `$HOME`/`$XDG_STATE_HOME` temp directories left for the OS to reclaim.

**Security -- `what-a-path-field-must-not-trust.md`.** Applies unchanged, per the module's own
doc comment: a directory found by browsing is untrusted exactly as a typed one is.
`add_project_from_path` re-validates in full regardless of how the path arrived; no
canonicalisation, symlink policy, or root validation was added to `shell.rs` or to the surface --
confirmed by diff, the only new calls are `browse_directory` (bounded, read-only, core) and the
one, already-reviewed `add_project_from_path`. A commit failure (simulated: the chosen directory
removed between the scan and the commit, since a real race is not reliably reproducible) renders
`PathFieldError::DoesNotExist` and leaves the modal open rather than closing on nothing --
`shell::tests::a_commit_failure_renders_the_error_and_keeps_the_modal_open`, the same
"never a silent no-op" shape `a_bad_path_renders_a_notice_and_the_application_keeps_running`
already proves for the field.

**Correction, found while starting PR-038-D.** This PR's own review request (300) and this
section, as first written, claimed the "Browse..." button was "the first genuine
`iced::widget::button` in this crate," naming `TrustSettings` and `ApprovalHistory` as
counter-examples ("a `\"> Label\"`-marker-prefixed `text()`, keyboard-only"). That is wrong.
`shell.rs` already imports `button` from `iced::widget` at its top-level import list and uses
real `button(...).on_press(...)` five times before this PR: `trust_settings_view`'s Grant/Revoke
(`Message::RevokeWorkspaceTrust`/`OpenTrustGrantDialog`, `shell.rs:5691`/`5700`), its capture
toggle (`ToggleTranscriptCaptureDeclined`, `:5725`), its purge control
(`OpenTranscriptPurgeDialog`, `:5757`), and `approval_history_entry_view`'s open control
(`OpenApprovalHistoryEntry`, `:5869`). Confirmed by direct grep, not by re-reading a summary.

**What is still true**: this PR's button is the first on `board.rs` *specifically* -- that
surface genuinely had none before. **What is now known to be untrue**: that mouse clicks and
modals had never coexisted in this crate before this PR, and (in the accepted response to
review request 300) that "there was nothing to click" before this button existed. The
independently-checked conclusion in that response -- that `opaque(center(...))` already blocks
clicks reaching anything beneath an open modal, `shell.rs:3824` -- is unaffected: it was verified
directly against the render code, not inferred from the false "first button" premise, and the
five pre-existing buttons above sit on `active_project_workspace_view`'s own `MainArea` content,
under the exact same `stack![base, opaque(scrim)]` composition, so they were already covered by
the same mechanism. Nothing here changes PR-038-G's acceptance; this is a correction to the
written record, not a reopened question. `board/tests.rs`'s own doc comment for
`the_browse_button_is_a_real_clickable_widget_not_an_inert_label` corrected to match.

## PR-038-D — recent projects, one key each

**Build.** `restore_recent_projects` already populated a passive `Vec<RestoredRecentProject>`
since RFC-032/033, and `project_board.rs::recent_project_row` already rendered it as board rows
before this PR -- what was missing, per RFC-038's own OQ1 ("recent projects on the empty board:
yes... offering it as one-key reopen"), was the action. Added: `project_board_row_highlight`
(the board's own keyboard cursor, the same shape `approval_history_highlight` already is for its
list), moved by `Up`/`Down`, clamped not wrapping; `Enter`, or a real click on a `Recent*`-kind
row's own "Open" button, reopens it through `reopen_recent_project` -- the exact same
`add_project_from_path` entry point PR-038-A's field and PR-038-G's browser already use, so the
same audit record, the same `Restricted`-by-default outcome, and the same live re-validation
(`what-a-path-field-must-not-trust.md` applies unchanged: a remembered path is untrusted exactly
as a typed or browsed one is). `ActiveSession` rows are inert to `Enter`/the button -- already
open, nothing to reopen; switching to one is `NavigationAction::SwitchActiveProject`, still out
of scope (PR-038-B's own known-limitations note).

**A security finding, disclosed and fixed in this same slice, not left for review to catch.**
`AppState::add_project_session` optimistically restores a recent project's *cached* trust label
(keyed by canonical root) with nothing to confirm it against the durable audit store --
`project_board.rs`'s own doc already calls the cache "a display hint only." `verify_restored_
trust` exists to close exactly this gap, but until this slice it only ever ran once, at
`State::new()`, over whatever `ProjectSession`s CLI arguments had already created at boot. Every
non-CLI `add_project_from_path` call site added since PR-038-A -- the path field, the browser,
and now the board's own reopen -- restores cached trust the same optimistic way and **never
called `verify_restored_trust` afterward**. A user who once granted trust to a project, closed
it, and later retyped, browsed to, or (after this PR) one-key-reopened that exact path got
`Trusted` back with zero confirmation against the audit store, for the rest of that session.

Fixed at all three call sites in this slice: `reopen_recent_project` (this PR's own new one),
`attempt_open_project_from_path_field` (PR-038-A), and `choose_current_browsed_directory`
(PR-038-G) -- each now calls `verify_restored_trust(&mut state.app_shell)` immediately after a
successful `Added` outcome, the same demotion pass `State::new` already runs once at boot,
reused rather than reimplemented. Each is proven, and separately ablated, below.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (357 tekstide +
724 tekstide-core, up from 346/724; 0 failed -- this slice is GUI-layer only, no `tekstide-core`
change at all: every primitive it needed already existed), `cargo fmt --all --check`, `cargo
clippy --workspace --all-targets --all-features -- -D warnings` (one `#[allow(clippy::
too_many_arguments)]` on `board::view`, matching this crate's own existing precedent in
`audit/integration.rs`/`approval/coordinator.rs` over inventing a grouping struct for the lint
alone), `git diff --check` -- all clean.

**The acceptance criterion, both keyboard and mouse, both automated and live.**

- Automated (`shell/tests.rs`): `enter_on_a_highlighted_recent_row_reopens_it_without_retyping_the_path`
  (real `Up`/`Down`/`Enter` through `send_main_area_key`, the same real-routing shape
  `a_typed_key_edits_the_real_active_document_through_real_routing` already establishes),
  `up_and_down_move_the_project_board_row_highlight_clamped_not_wrapping`,
  `enter_on_a_highlighted_active_session_row_does_nothing`,
  `the_real_open_button_message_reopens_the_same_project_the_keyboard_does`,
  `a_reopen_of_a_no_longer_existing_recent_project_renders_a_notice_and_keeps_running`. `board/tests.rs`:
  `highlighted_row_lines_marks_only_the_name_line_of_the_highlighted_row` (the rendered string,
  not the `Element` tree, the same `row_lines`/`tree_lines` shape this crate always uses),
  `the_recent_row_open_button_is_real_and_gated_on_row_kind_not_highlight`.
- Live, against the release binary: `cargo build --release -p tekstide`. Two launches against the
  same `XDG_STATE_HOME` -- `env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d>
  ./target/release/tekstide <mktemp -d>/tsd-pr038d-demo` first (registers the project and saves
  the recent-projects cache at boot, per `boot()`'s own `store.save(...)` call, then killed), then
  a genuinely cold `env -u WAYLAND_DISPLAY XDG_STATE_HOME=<same dir> ./target/release/tekstide`
  with **no arguments at all** -- the same `xdotool`/`niri msg action screenshot-window`/
  `wl-paste` capture method PR-038-A/B/C/G already established:
  - `evidence/pr-038-d/before-cold-start-recent-row.png` -- cold start, one recent row, the
    keyboard cursor's own `>` marker already on it (highlight defaults to the only row), a real
    "Open" button.
  - `evidence/pr-038-d/after-enter-reopens-without-retyping.png` -- real `Return` (`xdotool key
    --clearmodifiers Return`, no other key ever pressed): the row is now a live session (real
    `0 pending approvals`/`0 reviews`/`0 dirty files`, not `unknown`), the "Open" button gone.
    **No path was ever typed.**
  - `evidence/pr-038-d/after-real-mouse-click-on-open-button.png` -- a third, separate cold-start
    cycle against the same cache, this time a real `xdotool mousemove`/`click 1` on the button's
    own on-screen coordinates rather than `Return`: identical result, proving the button and the
    key converge on the same `reopen_recent_project`.
  - Every process terminated cleanly with `SIGTERM` after its capture; no terminal was ever
    launched in this session, so `test-process-leak.md`'s defect class does not apply.

**The audit guard, widened and ablated.**
`add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else`'s allow-list:
`shell.rs`'s count from two to three, naming all three call sites explicitly. Ablated: commented
out `reopen_recent_project`'s call to `record_new_project_added` --
`reopening_a_recent_project_writes_exactly_one_real_project_added_record` failed (0 records, not
1); reverted.

**The trust-confirmation fix, ablated at all three call sites separately -- this is the
security-relevant part, not a formality.**

- `reopen_recent_project`: `reopening_a_project_cached_trusted_but_unconfirmed_by_the_audit_store_demotes_to_restricted`.
  Ablated: removed the new `verify_restored_trust` call -- failed (`Trusted`, not `Restricted`,
  the exact defect this fix exists to prevent); reverted.
- `attempt_open_project_from_path_field` (the PR-038-A retroactive fix):
  `typing_a_path_matching_a_cached_trusted_but_unconfirmed_recent_project_demotes_to_restricted`.
  **A real finding while writing this test, disclosed rather than silently corrected**: the
  board is not "empty" once a recent row exists (`path_field_is_showing`'s own condition), so
  without first pressing a real `Ctrl+Alt+O`, `Enter` reaches this slice's own
  `handle_project_board_row_key` instead of the field -- which, for a path matching the same
  cached project, reopens it through the *already-fixed* `reopen_recent_project` and passes the
  test for the wrong reason. Fixed by making the test press `Ctrl+Alt+O` first, which is also
  what correctly makes `handle_project_board_row_key` stand down (its own mutual-exclusion
  guard), so `Enter` can only reach the field. Ablated afterward exactly like the other two --
  failed (`Trusted`, not `Restricted`) with the corrected test; reverted.
- `choose_current_browsed_directory` (the PR-038-G retroactive fix):
  `browsing_to_a_cached_trusted_but_unconfirmed_recent_project_demotes_to_restricted` (dispatches
  `Message::FolderBrowserChooseCurrentDirectory` directly, so it was never exposed to the same
  interception risk as the field's own test). Ablated: removed the new `verify_restored_trust`
  call -- failed (`Trusted`, not `Restricted`); reverted.

**Security -- `what-a-path-field-must-not-trust.md`.** Applies unchanged: a remembered path is
untrusted exactly as a typed or browsed one is, however long it has sat in the cache.
`reopen_recent_project` adds no canonicalisation, symlink policy, or root validation of its own
-- confirmed by diff, the only new calls are the cache lookup (read-only) and the two,
already-reviewed primitives (`add_project_from_path`, `verify_restored_trust`). Project names and
paths from the cache render through the board's own existing, already-tested escaping
(`row_lines`/`highlighted_row_lines`) -- no new untrusted-text render path was added.

**Known limitation, disclosed, not silently accepted.** The `recent-projects.json` cache file
itself is not re-saved mid-session -- only `boot()` writes it, once, after processing CLI
arguments. A project reopened through this PR's own new action updates the live `AppState` (and,
via `verify_restored_trust`, the live session's trust) but **not** the on-disk cache's
`last_opened_at`/`last_activity` timestamps until the *next* process boot happens to write them
again incidentally. Pre-existing behaviour (`upsert_open_project_recent` already updates the
in-memory list; only the file write is boot-only), not introduced by this slice, and out of its
scope to fix -- named here so it is not mistaken for something this PR verified and got wrong.

## PR-038-E — closeout

_Pending._

## Known limitations (RFC-038-wide)

As of PR-038-A/B/C/D/G:

- **The folder browser's render layer duplicates the project explorer's own** (`browse_view` and
  siblings, alongside `view`/`node_line`/`tree_lines`), rather than sharing it -- disclosed and
  flagged for the architect's decision in PR-038-G's own qa-evidence.md section above, not
  silently absorbed. Possible follow-up: extract a shared render helper once the reviewer decides
  the shape it should take.
- **A recent project reopened mid-session does not update the on-disk `recent-projects.json`
  cache's own timestamps.** Only `boot()` writes that file, once, at startup -- disclosed in
  PR-038-D's own qa-evidence.md section above, pre-existing behaviour this slice did not
  introduce and is not scoped to fix.
- **`ProjectBoardEmptyState::primary_action`/`secondary_action` still in the published API,**
  unread by anything, same as `0.12.1` left them. PR-038-E.
- **`Ctrl+V` paste is not exercised by a live/synthetic-input test**, only by its resource bound
  -- see PR-038-A's own section above.
- **Adding a second project through `Ctrl+Alt+O` does not switch to it.** It lands on the board,
  active project unchanged -- pre-existing, deliberate core behaviour
  (`AppState::add_project_session`), inherited rather than special-cased around. Switching
  requires `NavigationAction::SwitchActiveProject`, still `Configurable`/`None` and unrelated to
  this RFC's own scope (see PR-038-B's own qa-evidence.md section).
- **`runtime/terminal/launch.rs`'s real shell-spawn path still leaks a process when a caller
  panics** -- the `Child::drop` defect `test-process-leak.md` documents for two `approval::tests`
  call sites, present here too and out of scope for either fix. Discovered and disclosed this
  slice (PR-038-C's own qa-evidence.md section); not fixed at its source.
- Unchanged from `0.12.1`: the configuration system loads nothing, no screen-reader support, no
  cross-platform evidence beyond Linux, the real Claude Code CLI never exercised by the test
  suite, `NFR-PERF-004` still unverified.
