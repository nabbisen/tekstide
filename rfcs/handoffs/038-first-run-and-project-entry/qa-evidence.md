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
populated-board arm, per the task breakdown. `Ctrl+Alt+O` (PR-038-B), the standalone help surface
(PR-038-C), recent-projects reopen (PR-038-D), and `ProjectBoardEmptyState`'s dead-field removal
(PR-038-E) are unstarted -- see those sections below.

## PR-038-B — `Ctrl+Alt+O`

_Pending._

## PR-038-C — the help surface

_Pending._

## PR-038-D — recent projects

_Pending._

## PR-038-E — closeout

_Pending._

## Known limitations (RFC-038-wide)

As of PR-038-A only:

- **No `Ctrl+Alt+O`.** A user with a project already open still has no keyboard route to open a
  second one -- only the empty board's field. PR-038-B.
- **No help surface independent of the board.** A user inside Terminal Immersion still needs
  `Ctrl+Alt+P` first, same as before this slice. PR-038-C.
- **No recent-projects reopen.** A previously-opened project not currently on the board must be
  retyped by path; `restore_recent_projects`'s data remains unread by any surface for this
  purpose. PR-038-D.
- **`ProjectBoardEmptyState::primary_action`/`secondary_action` still in the published API,**
  unread by anything, same as `0.12.1` left them. PR-038-E.
- **`Ctrl+V` paste is not exercised by a live/synthetic-input test**, only by its resource bound
  -- see PR-038-A's own section above.
- Unchanged from `0.12.1`: the configuration system loads nothing, no screen-reader support, no
  cross-platform evidence beyond Linux, the real Claude Code CLI never exercised by the test
  suite, `NFR-PERF-004` still unverified.
