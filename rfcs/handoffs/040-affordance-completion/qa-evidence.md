---
title: "RFC-040: QA evidence"
rfc: "RFC-040"
rfc_file: "../../accepted/040-affordance-completion.md"
source_rfc_status: "Accepted 2026-08-25 — M12, first of three"
target_milestone: "M12"
created: "2026-08-25"
---

# QA evidence

One section per PR. Cite the command that produced each result.

Screenshots in `evidence/pr-040-<letter>/` with the launch command beside them;
`../first-run-correction/evidence/cold-start-empty-board.md` is the reference for form.

## PR-040-A — the audit as a test

**Build.** `keyboard_help::control_coverage` (`#[cfg(test)]` -- this is audit infrastructure, not
production logic; nothing outside the test suite has a reason to ask "does this action have a
visible control"), exhaustive over `NavigationAction` the same way `action_catalog_key` already
is, mapping every live action to either `ControlCoverage::VisibleControl { description,
on_press_snippet }` (a real button's own literal `.on_press(Message::Variant` text) or
`ControlCoverage::KeyboardOnly(reason)`. Three actions have a real control today
(`OpenProjectBoard`, `SwitchActiveProject`, `OpenFolderBrowser` -- all three built by RFC-038/039);
two are permanent, reasoned allow-list entries (`PasteIntoTerminal`, D3's own convention;
`OpenProjectEntryField`, whose *workflow* the Browse button already serves even though the
*action* has none); the remaining eight carry `"no visible control yet -- tracked for RFC-040
PR-040-C"`, honest about what does not exist rather than a placeholder. This is the write-the-
allow-list-before-anything-depends-on-it step the README/task-breakdown both required.

**Two tests, matching the two required properties.**

- `no_click_mechanism_other_than_button_on_press_exists_anywhere_in_the_crate` -- the premise:
  `mouse_area`/`MouseArea`/`.on_click(` absent from every scannable source file, the same shape
  `no_raw_color_construction_anywhere_in_the_crate` already established for a different premise.
- `every_live_action_has_a_visible_control_or_a_reasoned_allow_list_entry` -- the coverage: every
  `Candidate` rule with a binding is looked up in `control_coverage`; `VisibleControl` entries are
  checked against the real, current source (their `on_press_snippet` must actually appear
  somewhere in the crate); `KeyboardOnly` entries are checked for a non-empty reason; anything
  missing from the match at all is a `None` the exhaustive match cannot produce for a live action
  without a compile error first.

**Security/correctness note, found by the ablation itself.** The first version of the coverage
test scanned every file `scannable_source_files()` returns, including `keyboard_help.rs` --
which is where every `on_press_snippet` string literal is *defined*. That made the check vacuous:
searching the whole crate for a string that is guaranteed to appear in its own definition site
always finds it, regardless of whether the real button it names still exists anywhere else. Caught
by running the required ablation (below), not by inspection -- the test passed even after the
snippet was replaced with one that does not exist. Fixed by excluding `keyboard_help.rs` from the
scan for this one check; the premise test above still scans it (a stray `mouse_area` there would
be just as real a violation).

**Ablations.**

- Replaced `SwitchActiveProject`'s own `on_press_snippet` with a string that exists nowhere in the
  crate, ran the coverage test: failed, naming `SwitchActiveProject` by variant, with the false
  snippet quoted in the panic message. This is the same run that first exposed the
  `keyboard_help.rs`-self-match bug above -- the *first* attempt at this ablation passed when it
  should have failed, which is what caught the bug; the version recorded here is the one that
  correctly fails, after the fix. Reverted.
- Appended a line containing the literal substring `mouse_area` to `surface/editor.rs`, ran the
  premise test: failed, naming that file. Reverted (`git checkout --`).

**Gates.** `cargo build`, `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings` -- clean.
`cargo test --workspace --all-targets --all-features` (385 tekstide + 734 tekstide-core, up from
383/734; two new tests, no others changed): three consecutive runs under default parallelism, per
the checklist's own explicit requirement -- run 1 failed
`command_approval_family_produces_real_durable_audit_records_through_the_pipeline`, runs 2 and 3
clean. **Disclosed, not investigated further**: this is the flake `test-process-leak.md`'s own
table already names (one of its original four) and RFC-039's own `qa-evidence.md` already
recorded a second, distinct cause for (a shared-`AuditStore` query-race under parallel load,
response 312) -- this occurrence is neither new nor caused by anything in this PR, since neither
new test in this slice touches the audit store at all. `git diff --check` clean.

## PR-040-B — modals get buttons

**Build.** All nine modals' own `button_line` closures now return a real `iced::widget::button`
instead of plain `text`, keeping the same `"> "`/`"  "` focus marker inside the label. `Message::
ModalActivate`'s old inline match was factored into a standalone `activate_current_modal(state)`
-- the one place a modal's own decision is made, called by `Enter` and by every new click message
alike, so a decision reached by mouse runs the identical code a decision reached by keyboard
already did, never a second, parallel copy. Per button:

- **The "safe"/non-destructive half of every two-button modal** (`Reject`/`Dismiss`/`Cancel`,
  seven buttons across six modals, plus both of the still-scaffolding LayerDemo's buttons and
  Help's new "Close") dispatches the literal `Message::ModalDismiss` -- the same message `Escape`
  already sends -- rather than a new click message, because `activate_current_modal`'s own guard
  clauses already treat any non-destructive focus identically to `ModalDismiss` (verified by
  reading every guarded arm, not assumed). This is the strongest possible form of "click and
  keystroke share one handler": they are not two paths converging on the same effect, they are
  the same `Message` value.
- **The destructive/decision-committing half** (`Accept`, `Reload`, `Grant`, `Purge`, `Close`, and
  *both* of the approval dialog's buttons -- RFC-022 PR-022-E's own dialog has no
  `ModalDismiss`-equivalent half, both `ApproveOnce` and `Reject` are real decisions) gets one new
  `Message` variant each (`PasteConfirmAcceptPressed`, `ExternalChangeReloadPressed`,
  `ApprovalApproveOncePressed`, `ApprovalRejectPressed`, `TrustGrantGrantPressed`,
  `TranscriptPurgePressed`, `ProjectCloseClosePressed`). Each handler sets the modal's own
  `focus` field to the button that was clicked, then calls `activate_current_modal` -- the exact
  function `Enter` calls, so the click's only addition is "move focus here first," not a
  redefinition of what activating means.
- **The folder browser** is the one modal that is not a two-button choice: each row is now a real
  button (`browse_view`'s new `on_row_click` closure parameter, mirroring
  `surface::board::row_view`'s own `open_message` split), dispatching `Message::
  FolderBrowserRowPressed(index)` -- sets `highlight` to the clicked row, then calls
  `activate_current_modal`, the same function `Enter` already uses for row navigation. The
  "commit this directory" button (`Space`'s own action) dispatches the literal existing `Message::
  FolderBrowserChooseCurrentDirectory` directly -- not a new message, since it already is one.

**Mouse exclusivity (§1's hardest requirement).** `opaque(center(...))` already makes every
background control unreachable by a real click while a modal is open (`view`'s own `stack!` puts
the scrim on top, full-window -- verified live below). That layout fact is not the only guard:
every one of the crate's ten background click handlers now also carries an explicit
`if state.modal.is_some() { return; }` guard -- four already had it (`attempt_close_project_tab`,
`open_approval_history_entry`, `open_trust_grant_dialog`, `open_transcript_purge_dialog`, all
pre-existing for other reasons), six did not and now do (`reopen_recent_project`,
`switch_to_project_tab`, `go_to_project_board`, `open_folder_browser`, `revoke_workspace_trust`,
`toggle_transcript_capture_declined`). This is deliberate defense in depth, not a second
interaction-capturing layer competing with `opaque` (§1's prohibition): a state-level guard at the
handler, alongside a layout-level capture at the view -- the same two-layer shape keystroke
suppression already has (`SubscriptionMode::for_modal` at the source, the terminal-write-site
`is_none()` check as a second, independent layer). "A property that holds by accident of layout is
one refactor from not holding" -- this guard does not depend on `opaque`'s bounds staying
full-window.

**Live GUI verification.** `cargo build --release`; launched against a real temp project
(`env -u WAYLAND_DISPLAY XDG_STATE_HOME=<mktemp -d> ./target/release/tekstide <tmp project dir>`).
Opened the Help modal with a real `Ctrl+Alt+K` keypress (after a real prior click on empty window
space -- this session's own known xdotool/niri synthetic-focus quirk, not an app defect, see
`affordance-audit.md`'s own note); screenshot confirms a real, styled `Close` button now renders
where PR-039-D's own audit found zero buttons in any of the nine modals
(`evidence/pr-040-b/help-modal-close-button-before-click.png`). A real `xdotool click 1` at the
button's own on-screen coordinates then closed the modal, returning to the Project Board
(`evidence/pr-040-b/help-modal-after-close-click.png`) -- the mouse half of "this control works,"
proven against real `iced` hit-testing, not only against `update()` dispatch. One modal
demonstrated live, not all nine: the other eight share the identical `button(...).on_press(...)`
construction this one uses (same crate-wide premise
`no_click_mechanism_other_than_button_on_press_exists_anywhere_in_the_crate` already asserts), so
this is representative rather than exhaustive live coverage -- disclosed, not hidden, and backed by
the exhaustive `update()`-level test for every other modal's own click path (below).

**Tests, one new fact each.**

- `a_control_behind_an_open_modal_cannot_be_clicked` -- §1's required proof. Two of the ten
  background controls, on two different surfaces (Trust Settings, the Project Board): dispatch
  their real click `Message` while a modal is open (no effect), then again once it closes (the
  ordinary effect happens) -- the same two-phase shape
  `modal_open_blocks_pty_write_and_closing_it_resumes_delivery` already uses for keystroke
  suppression. Not all ten: every handler now shares one identical one-line guard, so this proves
  the shared mechanism rather than repeating the same assertion ten times.
- `clicking_accept_writes_the_real_pasted_content_and_closes_the_dialog`,
  `clicking_reload_takes_the_disk_content_and_closes_the_dialog`,
  `clicking_approve_once_sends_a_real_decision_regardless_of_current_focus`,
  `clicking_reject_sends_a_real_decision_regardless_of_current_focus`,
  `clicking_grant_grants_trust_for_real_regardless_of_current_focus`,
  `clicking_purge_removes_the_real_file_regardless_of_current_focus`,
  `clicking_close_terminates_the_real_process_regardless_of_current_focus`,
  `clicking_a_row_navigates_into_it_regardless_of_current_highlight` -- one per new click message,
  each constructed with focus (or highlight) starting somewhere *other* than the button clicked, so
  passing proves the click itself moves focus and decides, not that it merely agreed with whatever
  was already focused. Each mirrors an existing `Enter`-path test's own real assertion (a real PTY
  write, a real disk reload, a real audited grant, a real file deleted, a real process terminated,
  a real re-scan) rather than a synthetic stand-in.
- `Cancel`'s own click is not separately tested: `project_close_dialog_view`'s own `button_line`
  wires it to the literal `Message::ModalDismiss` `Escape` already sends, so
  `escaping_the_close_confirmation_also_records_a_cancelled_decision` already proves the
  `Cancelled` audit record is identical by click or by key -- by construction (one `Message`
  value), not by two paths that merely happen to agree.

**Destructive-choice-never-default-focus, re-verified.** No default-focus code changed in this
slice; every dialog's own default-focus test (`trust_grant_dialog_defaults_focus_to_cancel_and_
activating_it_grants_nothing`, `activating_reject_dismisses_the_paste_dialog_without_writing`,
`cancelling_the_close_confirmation_leaves_everything_running_and_records_it`, and the destructive-
focus-defaults-to-`Reject` approval promotion tests) still passes unmodified across all three runs
below.

**Ablations.**

- Temporarily removed `toggle_transcript_capture_declined`'s new guard: `a_control_behind_an_
  open_modal_cannot_be_clicked` failed (`left: true, right: false` -- the toggle fired while a
  modal was open). Restored, re-ran, green.
- Temporarily removed the `activate_current_modal(state)` call from `Message::
  ProjectCloseClosePressed`'s own handler (kept the focus-setting line): `clicking_close_
  terminates_the_real_process_regardless_of_current_focus` failed
  (`assertion failed: state.modal.is_none()` -- clicking Close no longer did anything). Restored,
  re-ran, green.

**A stale claim corrected.** `escape_dismisses_the_paste_dialog_without_writing_even_with_accept_
focused`'s own doc comment stated "this shell has no mouse-click handling anywhere" as the reason
"click-away" could not be tested -- true when written (PR-018-C, before RFC-038's first real
button existed), false as of this slice. Corrected in place: "click-away" still has no reachable
trigger, but now because `opaque`'s own full-window capture leaves nothing behind the modal to
click, not because the crate has no click handling at all.

**Gates.** `cargo build`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings` -- clean. `cargo test --workspace --all-targets --all-features`
(394 tekstide + 734 tekstide-core, up from 385/734; nine new tests, no others changed except the
two doc-comment corrections above): three consecutive runs under default parallelism, all clean,
no flake hit this round. `git diff --check` clean.

## PR-040-C — visible controls

**Build.** Eight real, clickable buttons, one per `TrackedGap` action PR-040-A's own allow-list
named, each placed per D2 on the surface its own action applies to, not a toolbar or palette:

- `ToggleProjectMode` — `mode_toggle_row`, a small header above whichever mode's own content is
  showing in `main_area_view` (present in both Content and Terminal Immersion; absent with no
  active project, since there is no mode to switch with nothing open).
- `LaunchTerminal` — `launch_terminal_button`, "+ New Terminal," on Terminal Immersion's own
  session bar. Also required a new `empty_terminal_workspace_view` arm: `terminal_workspace_view`
  was never reachable with zero panes before this slice (its own caller guard excludes that case),
  so without a second arm the very first terminal in a project would have had no button at all --
  exactly the gap this RFC exists to close, reintroduced by a naive placement.
- `SaveActiveDocument` — the editor's own "Save" button (`surface::editor::view`'s new `on_save`
  parameter, the same "surface renders, `shell.rs` supplies the message" split
  `board::empty_state_view` already established), shown whenever a document is open.
- `LaunchAgentRun`, `OpenCurrentAgentRunDetail`, `OpenApprovalHistory` — three more buttons on
  `TrustSettings`, per the task breakdown's own "agent run where a trusted project's actions
  live." `LaunchAgentRun` is shown always, even Restricted -- it reuses the exact real-refusal
  path `Ctrl+Alt+A` already has (a typed notice, not a silent no-op), a decision stated rather
  than defaulted.
- `OpenTrustSettings`, `OpenHelp` — the top bar's own new `top_bar_actions_row`. `OpenHelp` is
  shown always; `OpenTrustSettings` only with an active project (nothing to configure trust *for*
  at the Project Board) -- factored into `top_bar_offers_trust_settings` for the same testability
  reason `main_area_label`/`sidebar_label` already are, since an `Element` is not directly
  inspectable.

Keyboard accelerators and the new buttons converge on the same functions throughout
(`toggle_active_project_mode`, `launch_terminal_in_active_project`,
`save_active_document_button_pressed`, `launch_agent_run_in_active_project`,
`open_current_agent_run_detail`, `open_approval_history`, `open_trust_settings`, `open_help`),
mirroring `open_folder_browser`'s own "one setup, two routes to it" precedent from PR-040-B. Every
one of the eight also carries the `state.modal.is_some()` guard PR-040-B's own review (response
317) required be exhaustive, not sampled -- see the click-message classification below.

**The count moves from 3 to 11.** `keyboard_help::control_coverage`'s own tally, PR-040-A's own
measurement: all eight `TrackedGap` entries move to `VisibleControl`, each `on_press_snippet`
checked against real source by the existing coverage test
(`every_live_action_has_a_visible_control_or_a_reasoned_allow_list_entry`, unmodified). 11 of 13
live actions now have a real control; the remaining 2 (`PasteIntoTerminal`, `OpenProjectEntryField`)
are PR-040-A's own permanent, reasoned allow-list entries, not a gap.

**Carried over from response 317's required follow-up.** `click_message_kind`
(`shell.rs`): exhaustive over every one of `Message`'s variants, the same discipline
`control_coverage` already established for `NavigationAction` -- classifies each as
`BackgroundControl` (must refuse while a modal is open), `ModalDecision` (a modal's own decision,
must act), or not click-originated at all. A new variant fails to *compile* until classified.
Ablated: temporarily added an unclassified variant, two separate matches (this one and `update`'s
own) failed to compile, confirmed, reverted. `click_message_kind_classifies_every_real_on_press_message`
exercises it against real constructed values (18 `BackgroundControl`, 10 `ModalDecision`, a sample
of `None`), matching every real `.on_press` site PR-040-A/B/C's own coverage tests already verify
exists.

**A real bug found live, not by a test.** The first version of `empty_terminal_workspace_view`
rendered no launch-refusal notice at all -- live-clicking "Launch AI CLI Run" on an untrusted,
terminal-less project landed silently in Terminal Immersion with nothing explaining why. Fixed by
factoring `terminal_launch_notice_rows` out of `terminal_workspace_view` so both views share the
identical three-notice logic (`terminal_launch_notice`/`terminal_paste_notice`/
`agent_run_launch_notice`), rather than the second view silently omitting them. Re-verified live:
the real refusal text ("This project isn't trusted yet — grant trust to start an agent run.") now
renders. This is the third time in RFC-038/039/040 that live verification found something the
test suite did not -- worth stating in those terms, not only as a bug count.

**Live GUI verification.** Built release, launched against a real temp project. A single
click-through covered most of the surface at once: clicked "Trust Settings" (top bar) -- landed on
the real surface, showing "Switch to Terminal" (mode toggle), "Grant Trust…", "Decline Future
Capture", "Purge Project Transcripts…", "Launch AI CLI Run", "AgentRun Report", "Approval
History", all six real buttons rendering together. Clicked "Launch AI CLI Run" -- landed in
Terminal Immersion's new empty-panes view, showing the real refusal notice (post-fix) plus
"Switch to Content" and "+ New Terminal". Clicked "+ New Terminal" -- a real, running shell
appeared ("Terminal 1 (Primary) — Running", a live `tekstide$` prompt), proving
`launch_terminal_in_active_project` works end to end through a real click from the empty state.
Screenshots in `evidence/pr-040-c/`. Not every one of the eight was individually clicked live
(disclosed, not hidden) -- each shares the identical `.on_press(Message::...)` construction the
crate-wide premise test already asserts is the only click mechanism, and each is verified at the
`update()` level by its own `clicking_*` test below.

**Tests, one new fact each**, mirroring an existing keyboard-path test's own real assertion rather
than a synthetic stand-in:

- `clicking_the_mode_toggle_switches_the_real_active_project_mode`
- `clicking_new_terminal_switches_modes_and_launches_a_real_session`
- `clicking_save_writes_the_real_edited_document_to_disk`
- `clicking_launch_ai_cli_run_shows_the_real_trust_refusal_when_untrusted`
- `clicking_the_agent_run_report_button_opens_the_real_report_surface`
- `clicking_approval_history_sets_the_open_surface_and_forces_content_mode`
- `clicking_trust_settings_routes_to_the_real_trust_settings_surface`
- `clicking_the_top_bar_help_button_opens_the_real_help_modal`
- `the_top_bar_trust_settings_button_is_hidden_without_an_active_project_and_shown_with_one` --
  context-dependent, D2's own required decision, tested directly against
  `top_bar_offers_trust_settings` rather than an uninspectable `Element`.

`a_control_behind_an_open_modal_cannot_be_clicked` (PR-040-B) extended with a third sample drawn
from this slice's own `OpenTrustSettingsButtonPressed`, ablated the same way (removed
`open_trust_settings`'s own guard, the test failed naming the leaked surface change, restored) --
proving the exclusivity pattern generalises to the new background controls, not only the original
ten.

**Gates.** `cargo build`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings` -- clean throughout. `cargo test --workspace --all-targets
--all-features`, three consecutive runs under default parallelism, all clean: 404 tekstide + 736
tekstide-core (tekstide-core's own count reflects the separate, already-reviewed
`runtime/terminal/launch.rs` leak fix, response 319 -- no test in this slice touches that file).
Gated only after that fix landed, per the architect's own required order (response 318): a leak
this slice did not cause was making its own three-run gate unmeasurable, so the gate waited for
the fix rather than absorbing the noise. Leaked-process count settled at a constant 32/run
throughout (the fix's own disclosed, bounded ~28/run from one unrelated benchmark test, plus 2
pre-existing) -- not climbing, not this slice's own concern. `git diff --check` clean.

## PR-040-D — closeout

_Pending._

## Known limitations (RFC-040-wide)

_Pending._
