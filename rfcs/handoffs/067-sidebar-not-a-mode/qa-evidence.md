# RFC-067 QA evidence

## PR-067-A — the sidebar persists

### D1 — the explorer renders and is reachable in both modes

`sidebar_view` no longer takes a `mode` parameter at all: it renders the explorer whenever a
project is active, regardless of mode, and falls back to an empty element only for the
(structurally unreachable while routed to `ActiveProjectWorkspace`) no-active-project case.

**A second, non-obvious gate had to be found and removed too.** `handle_explorer_key` itself
returned early unless `project.mode() == ProjectMode::Content` — so even once the tree was
visually present in Terminal mode, `Up`/`Down`/`Enter` would have done nothing there. Found by
reading the function this RFC's own task breakdown pointed at, not by the handoff pack naming it.
`ensure_explorer_scanned` had the identical gate, which would have left a project opened straight
into Terminal mode showing the always-visible tree stuck on "Loading…" forever — found the same
way. Both removed.

**Ablated**: reverting `handle_explorer_key`'s own guard to the old `if project.mode() !=
ProjectMode::Content { return; }` fails `activating_a_file_in_the_explorer_while_in_terminal_mode_
switches_to_content_mode` with exactly the message naming the regression:
`"D1: the explorer must be reachable, not just visible, in terminal mode"`.

### D1 — the placeholder is deleted, not reworded

`sidebar-placeholder-title` removed from `en.ftl`; `sidebar_label` (its only caller) removed
entirely, since its whole job was composing that one string with a focus marker. The i18n
enforcement suite (`report_catalog_keys_unused_by_any_render_call`,
`every_source_locale_key_resolves_in_every_shipped_locale`) confirms nothing still references it.
Two tests that depended on `sidebar_label` updated: `sidebar_label_reflects_focus` deleted (its
own premise no longer exists), `the_sidebar_and_content_placeholders_name_no_rfc_and_say_
something_true` narrowed to `the_content_mode_placeholder_names_no_rfc_and_says_something_true`
(only the content-mode placeholder, which this RFC does not touch, remains to test).

### D7 — activating a file in terminal mode switches to Content mode and shows it

**A real, pre-existing bug found while implementing this, not invented by it.**
`ProjectSession::open_text_document` — the one function every document-open path in this crate
calls through — unconditionally called `self.set_mode(ProjectMode::Content)`. Before this RFC that
was harmless: the explorer was the only reachable caller, so "every open forces Content" and "the
one caller that should switch mode does" were the same fact. RFC-067 made the sidebar (and this
function) reachable from a second caller — RFC-027's recovery offer, which also opens documents
through this same path — so the two stopped being the same decision, and the unconditional switch
became D8's exact violation (confirmed live: my first version of the D8 test below failed with
`left: Some(Content), right: Some(TerminalImmersion)` before this was found).

**Fixed at the root, not patched at the symptom.** `set_mode` removed from `open_text_document`
entirely (see its own new doc comment). A new, symmetric `AppState::open_active_project_content_
workspace()` (mirrors the already-existing `open_active_project_terminal_workspace` exactly) is
called explicitly, once, from the one place D7 names: `handle_explorer_key`'s own `Action::Open`
arm, immediately after a successful `open_active_project_text_document`.

A pre-existing core-level test, `opening_text_document_from_terminal_mode_forces_content_mode`,
had encoded the old universal behaviour as its own name and assertion. **Inverted, not deleted**:
renamed to `opening_a_text_document_through_the_shared_path_does_not_force_content_mode`, same
scenario, assertion flipped to prove the shared path now holds the mode.

**Ablated**: removing the explicit `open_active_project_content_workspace()` call from the
explorer's own arm fails `activating_a_file_in_the_explorer_while_in_terminal_mode_switches_to_
content_mode` with `left: TerminalImmersion, right: Content` — the exact defect this call exists
to prevent.

Live-captured: `evidence/pr-067-a/activating-a-file-switches-to-content-mode.png` — a real
`main.rs` activated from the explorer while a real terminal was running in Terminal mode; the main
area shows the file's real content, and `Switch to Terminal` (not `Switch to Content`) confirms the
mode actually moved.

### D8 — a document opened by any path that is not the user's own activation does not change the mode

Driven through RFC-027's recovery offer specifically, per the task breakdown's own instruction
("it is the only one of the three that both opens a document and is reachable while a terminal is
on screen"): `accepting_a_recovery_offer_in_terminal_mode_does_not_switch_the_mode`
(`shell/tests.rs`) sets a project to `TerminalImmersion` *before* the offer modal is ever
constructed, writes a real recovery record, accepts it through the real `Message::ModalActivate`
path, and asserts both that the buffer became visible (the offer still works) and that the mode
never moved.

The third path D8 names, the background watch notice (`record_project_watch_notice`), was checked
by direct inspection rather than given its own test: it only ever marks documents already in the
open set as touched, and never calls `open_text_document`/`open_active_project_text_document` at
all, so it cannot reach the removed code path by construction — consistent with the task
breakdown's own account of it ("opens nothing new").

**Ablated, both layers** (the core-level test and the GUI-level test independently prove the same
property at two different seams): reverting `open_text_document`'s own removal of `set_mode` fails
both `accepting_a_recovery_offer_in_terminal_mode_does_not_switch_the_mode` (`tekstide`) and
`opening_a_text_document_through_the_shared_path_does_not_force_content_mode` (`tekstide-core`),
each with the exact `Content`-instead-of-`TerminalImmersion` symptom.

### D3 — no new focus zone, `Tab` cycle unchanged

`FocusZone` (`input.rs`) is untouched by this slice — no variant added, `next()`/`previous()`
unchanged. Every existing focus-cycling test (`tab_cycles_shell_focus_with_a_real_terminal_focused_
and_writes_nothing` and others) passes unchanged, which is the proof: a zone added or reordered
would have broken them.

### D9 — the six-terminal bound, session bar and two visible slots are untouched

No terminal-workspace code was touched by this slice. The live capture
(`evidence/pr-067-a/terminal-mode-running-terminal-and-tree.png`) shows a real running terminal,
its session bar entry (`Terminal 1 (Primary) — Running`), and the status bar's own `1 running`
alongside the now-persistent file tree — nothing about the terminal side changed shape.

### Live capture

`cargo build --release -p tekstide`, isolated `XDG_STATE_HOME` under `/dev/shm`, a throwaway
project with two real files (`main.rs`, `src/lib.rs`).

- `evidence/pr-067-a/terminal-mode-with-the-file-tree.png` -- **the required D2 proof, the direct
  replacement for the deleted placeholder**: Terminal mode, no terminal running yet, and the real
  file tree (`src`, `main.rs`) beside the "Nothing is running... Start a terminal" placeholder --
  not "Files are listed here in Content mode."
- `evidence/pr-067-a/terminal-mode-running-terminal-and-tree.png` -- a real terminal launched and
  running (`tekstide$` prompt, session bar, `1 running` in the status bar), the file tree still
  beside it.
- `evidence/pr-067-a/activating-a-file-switches-to-content-mode.png` -- D7's own proof, described
  above.

Processes terminated cleanly with `SIGTERM` after capture; throwaway state and project directory
removed from `/dev/shm` afterward, keeping only the three images copied into this RFC's own
`evidence/pr-067-a/` directory.

### Book, swept

The checklist's own Whole-RFC item names `what-works-today.md` as describing the sidebar as
mode-dependent. **Checked directly: it does not** -- its own "Projects, files, and editing"
section never claims the explorer is Content-mode-only. The real stale claim was in
`keyboard-reference.md`: *"With `Tab` focused on the sidebar in Content mode, `Up`/`Down` move the
explorer highlight..."* Fixed to say the tree is in both modes now, and that opening a file from
Terminal mode switches to Content mode. `configuration.md`'s two "file tree" mentions are about
font family, unrelated to mode. `getting-started.md`'s `Ctrl+Alt+M` keybinding row is still
accurate (D9: the toggle itself is untouched).

### Gate, PR-067-A

- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `i18n::enforcement` (23/23, one catalog key removed, confirmed unreferenced), `rfc_docs_
  invariants` (17/17, this slice touches no RFC document): clean.
- `cargo test --doc --workspace`: clean.
- `cargo test -p tekstide-core --lib`: 1118/1118. `cargo test -p tekstide --bin tekstide`:
  758/758.
- **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR` each run**
  (`/dev/shm/g067a{1,2,3}`): `758 + 17 + 1118`, 0 failed, 0 fixture entries left each time, clean
  on the first attempt.
