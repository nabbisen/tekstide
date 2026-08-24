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

_Pending._

## PR-039-C — close a project

_Pending._

## PR-039-D — affordance audit and closeout

_Pending._

## Known limitations (RFC-039-wide)

_Pending._
