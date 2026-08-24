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

## PR-038-F — a scan-only core entry point

**Added 2026-08-24 from PR-038-B's own review.** Small and structural: closed a trap that had
already caught two slices.

**Build.** `ApplicationShell::scan_active_project_explorer_directory` did two things: scan, and
unconditionally set `route = ActiveProjectWorkspace`. One layer down,
`ProjectSession::scan_content_explorer_directory` did two things of its own: scan, and
unconditionally set `open_surface = TextEditor` and `mode = Content`. `ensure_explorer_scanned`
(background cache-priming, called after every command that could have changed which project or
mode is active) wanted only the scan -- every one of those side effects had to be undone
afterwards, or worked around structurally. Response 233 found the first (`open_surface` silently
overwriting `OpenActiveProjectSurface(surface)`), worked around by saving and restoring it.
PR-038-B found the second (`route` silently undoing `OpenProjectEntryField`'s own route change),
worked around by routing that action out of `app_command_for` entirely.

Added `scan_content_explorer_directory_without_navigating` (`ProjectSession`),
`scan_active_project_explorer_directory_without_navigating` (`AppState`, `ApplicationShell`) --
each the exact scan, none of the navigation. `ContentWorkspace::scan_explorer_directory` itself
was already scan-only (confirmed by reading it: it touches only `selected_explorer_path`/
`explorer_scan`/`explorer_status`); the conflation was introduced one layer up
(`ProjectSession`) and two layers up (`ApplicationShell`), so the new methods are additive at
every layer -- no breaking change, no version implication, matching the task breakdown's own
note. `ensure_explorer_scanned` now calls the new entry point.

**Both workarounds removed, not just documented as removable.**

- The `open_surface` save/restore dance in `ensure_explorer_scanned` is gone outright -- the new
  method never touches `open_surface` (or `mode`) in the first place, so there is nothing to
  save or restore.
- `app_command_for`'s `NavigationAction::OpenProjectEntryField` arm moved back into the normal
  `Some(AppCommand::OpenProjectBoard)` group it would have been in from the start, out of the
  `None`-returning group PR-038-B had to place it in. `update`'s `Shell` arm keeps a much smaller
  special case: only `state.path_field_requested = true` (shell-local UI state with no
  `AppCommand` to express it), not the route dispatch itself anymore.

**A design question resolved by checking, not assuming.** The task breakdown's own ablation
instruction ("point `ensure_explorer_scanned` back at the navigating method and confirm the
PR-038-B route test fails") does not hold against the tree as PR-038-B left it: that slice's own
workaround routes `OpenProjectEntryField` *around* `ensure_explorer_scanned` entirely, so
pointing the scan back at the navigating method changes nothing the existing route test can see
-- confirmed empirically (ablated, test still passed) before concluding this. That is what
mid-implementation testing is for: the instruction implicitly assumed the workaround itself would
also be reverted, which is what makes the ablation meaningful again. Reverted it, on the reasoning
that leaving the workaround in place while its own root cause is fixed elsewhere is exactly the
"stale text about a mechanism" defect this project keeps having to correct after the fact, this
time as stale *behaviour* rather than a stale comment. Flagging this rather than silently
resolving it, since it is a judgement call about how literally to read the task's own build note,
not a fact check.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (358 tekstide +
724 tekstide-core, unchanged tekstide-core count -- no new core tests; the properties this slice
protects were already covered by two pre-existing GUI-level regression tests, re-verified below --
0 failed), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings`, `git diff --check` -- all clean.

**Ablated, both dances, both against pre-existing tests, not new ones written to fit the fix.**

- `open_surface`: `opening_approval_history_from_navigation_sets_the_open_surface_and_forces_content_mode`
  (response 233's own regression test). Ablated: pointed `ensure_explorer_scanned` back at the
  navigating method -- failed (`open_surface` read back as `TextEditor`, not `ApprovalHistory`);
  reverted.
- `route`: `ctrl_alt_o_opens_a_second_project_through_real_keys_on_a_populated_board` (PR-038-B's
  own regression test), **only meaningful after** the `OpenProjectEntryField` workaround was
  itself reverted (see above). Ablated the same way -- failed (`route()` read back
  `ActiveProjectWorkspace`, not `ProjectBoard`); reverted.

**The call-site enumeration, narrowed and split.**
`scan_active_project_explorer_directory_has_exactly_the_two_named_production_call_sites` (formerly
naming `ensure_explorer_scanned` and `handle_explorer_key`) is now
`scan_active_project_explorer_directory_has_exactly_one_named_production_call_site`, naming only
`handle_explorer_key` -- the one caller where navigating on scan is genuinely correct (browsing
the file tree legitimately means "show me the editor"). A new, parallel test,
`scan_active_project_explorer_directory_without_navigating_has_exactly_one_named_production_call_site`,
names `ensure_explorer_scanned` as the scan-only method's own one caller -- the same "named
explicitly, not merely counted" shape every enumeration test in this crate already uses.

**Security.** No new untrusted-text render path, no new I/O, no new call to
`add_project_from_path` or any audit producer -- purely a structural split of an existing,
already-reviewed scan into two entry points that do less than the one they were carved from.

## PR-038-I — the guards slice: two, both before closeout

**Guard 2, added 2026-08-24 from PR-038-F's own review response.**
`scan_active_project_explorer_directory_without_navigating` is new public core API whose entire
reason for existing is a *negative* property -- it scans and does not navigate. Before this
slice that property was asserted only by two GUI-level tests, two layers up, through
`ensure_explorer_scanned` (PR-038-F's own qa-evidence.md section). Same shape as the
`action_catalog_key` gap response 290 corrected: a function's contract tested only through its
one caller's composed behaviour, so a later refactor that stops `ensure_explorer_scanned` calling
it would leave the contract untested and able to silently regain navigation with nothing failing.

**Build.** A direct core test, `shell::tests::scan_active_project_explorer_directory_without_navigating_touches_nothing_but_the_scan`
(`crates/tekstide-core/src/shell/tests.rs`) -- adds a project (which, by
`ApplicationShell::new`/`ProjectSession::new`'s own defaults, already leaves the shell on
`ProjectBoard` with the project in `Content` mode, so no extra setup is needed to reach the exact
state the scan-only method exists to leave undisturbed), calls the scan-only method directly,
and asserts `route()`, `open_surface()`, and `mode()` are all unchanged, plus that
`content_workspace().explorer_scan()` is genuinely `Some` -- the scan itself still happened. All
three accessors were confirmed public before this was assigned (`shell.rs:26`, `session.rs:173`,
`:177`).

**Ablated**: made `ApplicationShell::scan_active_project_explorer_directory_without_navigating`
delegate to the navigating method instead of `AppState`'s own scan-only one -- the **core** test
failed (`route()` read back `ActiveProjectWorkspace`, not `ProjectBoard`), with no GUI code
touched, so the failure is attributable to the core contract itself, not to a caller; reverted.

**Guard 1, added 2026-08-24 from PR-038-G's own review response** -- replaces a shared-render
refactor rather than deferring one (that decision was already made in PR-038-G's own response;
this slice only builds the guard). The risk in `surface/explorer.rs`'s two renderers
(`view`/`node_line`/`tree_lines` and PR-038-G's `browse_view`/`browse_node_line`/`browse_tree_lines`)
is not volume, it is escaping divergence -- both render filesystem-derived names, and if one
later gains a fix the other does not, that is a security divergence, not a tidiness problem.

**Build.**
`surface::explorer::tests::every_untrusted_value_this_module_hands_the_catalog_was_escaped_first`
-- a count-equality invariant over `surface/explorer.rs`'s own source text: the number of
`.untrusted(` call sites equals the number of `quote_untrusted(` calls (both **4** today, matching
what response 300 verified before assigning this). Per `ARCHITECTURE.md`'s enumeration-test unit
rule the unit is the call site, not the file, so a future renderer that passes an unescaped name
to `.untrusted(` changes one count but not the other and fails. Deliberately scoped to this module
only -- `board.rs` reads a different shape (0 `.untrusted(`, escapes and renders through a
different path) and extending the same invariant there without first deciding what it should mean
would produce a count-equality that is false for correct code, per the task's own caution.

**Ablated**: added a second, unmatched `.untrusted(` call inside `browse_node_line` (reusing the
already-escaped `name` local, so the ablation is purely about the *count*, not a real unescaped
value reaching a real render) -- failed (6 `.untrusted(` sites, 4 `quote_untrusted(` calls);
reverted. Not the exact mechanic the task breakdown named ("removing one `quote_untrusted` call")
but the identical property: an unbalanced count fails the same assertion either direction.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (359 tekstide +
725 tekstide-core, up from 358/724; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` -- all
clean.

**Security.** Both guards are pure test additions -- no production code changed, no new render
path, no new I/O.

**Correction (response 304, PR-038-E): Guard 1 was removed.** It was specified on a mistaken
premise -- I did not check that `CatalogArgs::untrusted` takes `&DisplayText`, a type with a
**private** field and exactly one constructor in the whole crate (`quote_untrusted`,
`text_safety.rs`). "Every untrusted value handed to the catalog was escaped first" has therefore
been unrepresentable otherwise since RFC-016; there was never a runtime discipline for the count
test to guard, only a compile-time one already in force. Worse: the guard was a count over two
quantities only *incidentally* equal today. The moment `surface/explorer.rs` legitimately escapes
a value and renders it directly rather than through `.untrusted(` -- exactly the shape `board.rs`
already uses, 0 against 3, which this same section already named as a reason not to extend the
invariant there -- the counts diverge and the test fails on correct code. Removed in PR-038-E,
along with its own test
(`surface::explorer::tests::every_untrusted_value_this_module_hands_the_catalog_was_escaped_first`).
Replaced by the actual invariant worth guarding: see PR-038-E's own section below.

## PR-038-E — closeout

The last slice. Carries three items response 302/304 folded in here, plus the task breakdown's
own original scope.

**1. `State::new`'s stale `verify_restored_trust` comment (response 302's finding).** Said
"currently nothing does yet (RFC-032's own dialog and restricted-mode gates are still ahead)" --
true when written, false since those shipped. Worse, the comment implied this was the *only*
place trust gets verified; PR-038-D added three more mid-session call sites
(`reopen_recent_project`, `attempt_open_project_from_path_field`,
`choose_current_browsed_directory`), each calling `verify_restored_trust` itself right after its
own `add_project_from_path` succeeds. Corrected to state what this call site actually covers
(CLI-argument projects already live when `State::new` runs) and to point at the other three
rather than imply there are none.

**2. The `DisplayText` constructor guard (response 304, replacing the deleted Guard 1).** Two
tests in `crates/tekstide-core/src/text_safety/tests.rs`:

- `exactly_one_function_in_the_crate_returns_displaytext` -- scans every non-test `.rs` file
  under this crate's own `src/` for the literal `-> DisplayText`, expecting exactly one match
  (`quote_untrusted`, `text_safety.rs`). Test files are excluded from the scan, the same
  convention `app::tests::only_one_production_call_site_ever_restores_a_projects_trust_state`
  already uses -- not because a test-only constructor would be safe, but because this guard's
  own doc comments and assertion messages necessarily contain the literal text `-> DisplayText`
  in prose, which a raw source-text scan cannot distinguish from a real signature.
  **Ablated**: added a second function returning `DisplayText` (`text_safety.rs`, delegating to
  `quote_untrusted`) -- failed (two sites, not one); reverted.
- `displaytexts_field_is_declared_private_in_source` -- not mechanically enforceable as a
  property of the language itself (Rust has no "assert this field is private" reflection), so
  asserted the same way structural properties are asserted elsewhere in this codebase: a direct
  source-text check that `text_safety.rs` still declares `struct DisplayText(String)`, not
  `pub String`. **Ablated**: changed the field to `pub String` -- failed; reverted.

Together these guard the property that actually matters: `DisplayText` keeps exactly one
constructor and a private field, which is what makes "untrusted text cannot reach a
`DisplayText`-typed parameter unescaped" a compile-time fact rather than a convention. Add a
second constructor, or make the field `pub`, and every existing `.untrusted(` call site across
the crate becomes silently unproven -- these two tests are what would catch that.

**3. `ProjectBoardEmptyState::primary_action`/`secondary_action` removed** from
`tekstide-core`'s public API -- pre-baked English for two actions (`"Add Project"`, `"Open from
path"`) that were never reachable from anywhere, unread by `board.rs` since `0.12.1`. **A real
finding while removing them, disclosed rather than silently worked around**: the task breakdown's
own premise ("read by nothing") was false. `tekstide-core::shell::render_project_board` -- the
pre-GUI text harness `render_text()` calls, kept from before the real GUI existed -- read both
fields directly to print `[Add Project] [Open from path]`, and a real, passing core test
(`shell::tests::first_run_project_board_renders_empty_state`) asserted on that exact string.
Removing the fields meant also fixing that harness (now prints only the heading -- it has no
concept of what actions the real frontend offers, so it says only what it can honestly know) and
that test (now asserts the opposite: neither literal appears). The `tekstide::i18n::enforcement`
exemption list (`CORE_EXEMPT_LITERALS`) also named both strings as dormant exemptions for
`project_board.rs`; both entries removed, since the literals they exempted no longer exist.
**This is the same "read by nothing" pattern response 300/303/304 already found three times this
RFC** (reusing the explorer tree, the route ablation, Guard 1) -- checked before executing, this
time by grepping for the fields' own readers rather than trusting the task breakdown's claim.

Recorded in `CHANGELOG.md` as a breaking change under a new `0.13.0` entry (workspace version
bumped from `0.12.1`), alongside the rest of RFC-038's user-facing changes -- the path field, the
folder browser button, the help modal, one-key recent-project reopen, and the trust-confirmation
fix PR-038-D found and closed across three call sites. `README.md`'s own Quick Start and Keyboard
Reference sections were also out of date (missing `Ctrl+Alt+O`/`Ctrl+Alt+B`/`Ctrl+Alt+K`/
`Ctrl+Shift+V`/`Ctrl+S` from the table entirely, and no mention of the browser or one-key
reopen in the prose) -- updated to match what the application actually does today, not just what
`0.12.1` added.

**4. RFC-038's acceptance criterion 2, answered by enumeration, not left implicit.** "The board's
empty state contains no text naming an action that is not activatable. A test asserts this by
enumeration, not by inspection." No test previously asserted this positively and on an ongoing
basis -- only negative checks that the two specific `0.12.1`-era dead keys are gone from the
catalogue. Added
`board::tests::every_catalog_key_this_module_renders_is_enumerated_and_none_names_a_dead_action`:
scans `board.rs`'s own source for every `catalog.get("...")` key literal, asserts the exact list
(six today), and its own doc comment reasons about each -- three plain descriptive strings, one
field label, and two real button labels each already proven wired to a genuine
`iced::widget::button` by its own dedicated test. A new key added to `board.rs` later must be
named in this test's own list and doc comment or the test fails, the same "named explicitly, not
merely counted" discipline every enumeration test in this crate uses.

**Gates.** `cargo build`, `cargo test --workspace --all-targets --all-features` (359 tekstide +
727 tekstide-core, up from 359/725; 0 failed), `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check` -- all
clean.

**RFC-038's acceptance criteria, answered one by one, in its own words.**

- *"A user launching `tekstide` with no arguments, who has never read any documentation, can put
  a project on the board using only what the window shows them. Proven from a real key event
  through production code -- not from a dispatched message."* **Met.** Three independent ways,
  each proven live against the release binary with real `xdotool` key/click events, not
  dispatched `Message`s: typing a path into the field and pressing Enter (PR-038-A,
  `evidence/pr-038-a/`), browsing to a folder with the keyboard or a real mouse click (PR-038-G,
  `evidence/pr-038-g/`), and, for a project opened before, one-key reopen from the board's own
  remembered-projects row (PR-038-D, `evidence/pr-038-d/`).
- *"The board's empty state contains no text naming an action that is not activatable. A test
  asserts this by enumeration, not by inspection."* **Met**, per item 4 above.
- *"`ProjectBoardEmptyState`'s dead fields are gone from the published API, and the breaking
  change is recorded in the changelog."* **Met**, per item 3 above.
- *"Every live keybinding is reachable from a help surface that does not require the Project
  Board to be the visible route."* **Met.** `Ctrl+Alt+K` opens a modal reachable from any route
  or mode, including from inside a project's own Terminal Immersion
  (`shell::tests::ctrl_alt_k_opens_help_from_inside_terminal_immersion`), listing every live
  binding derived directly from `KeybindingPolicy::advertised_bindings()` -- twelve today, up
  from the nine `0.12.1` first listed, unable to drift from what the input layer actually
  dispatches on since nothing here is hand-written (PR-038-C, `qa-evidence.md`'s own section).

## Known limitations (RFC-038-wide)

As of PR-038-A/B/C/D/E/F/G/I:

- **The folder browser's render layer duplicates the project explorer's own** (`browse_view` and
  siblings, alongside `view`/`node_line`/`tree_lines`), rather than sharing it -- disclosed and
  flagged for the architect's decision in PR-038-G's own qa-evidence.md section above. **Resolved,
  corrected in PR-038-E**: a shared render helper was decided against (forcing `BrowseNode`
  through `ExplorerNode`'s shape would reintroduce the dishonest-field problem the core split
  exists to avoid). PR-038-I first built a runtime count-equality guard over
  `.untrusted(`/`quote_untrusted(` call sites to protect the escaping property instead of a
  shared helper -- **that guard was itself redundant and removed in PR-038-E** (response 304):
  `DisplayText`'s private field and single constructor already make "untrusted text cannot reach
  the catalog unescaped" a compile-time fact, not a runtime discipline a count could add anything
  to. The two renderers stay unshared; what actually protects the escaping property is the type
  system, guarded now by `exactly_one_function_in_the_crate_returns_displaytext`/
  `displaytexts_field_is_declared_private_in_source` (PR-038-E's own section above) rather than by
  anything specific to `surface/explorer.rs`. Revisit sharing only if a third browser ever
  appears.
- **A recent project reopened mid-session does not update the on-disk `recent-projects.json`
  cache's own timestamps.** Only `boot()` writes that file, once, at startup -- disclosed in
  PR-038-D's own qa-evidence.md section above, pre-existing behaviour this slice did not
  introduce and is not scoped to fix.
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
