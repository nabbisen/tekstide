---
title: "RFC-019: Editor and Explorer Surfaces - QA Evidence"
rfc: "RFC-019"
rfc_file: "../../proposed/019-editor-and-explorer-surfaces.md"
status: "PR-019-A/B accepted (response 180) — PR-019-C implemented 2026-08-11, not yet reviewed"
target_milestone: "M10"
created: "2026-08-10"
---

# QA Evidence

Record results here as each slice lands: gate output, ablations with the exact failure
they produced, findings, and limitations.

**This file is where results go. It is not where obligations go.** If a slice discovers
something a later slice must handle, put it in that slice's entry in
`task-breakdown-pr-plan.md` as well — that is what an implementer reads before starting.
This project has lost obligations to that gap four times.

## Recording conventions

- **Ablations name the exact failure**, not "the test failed." A specific wrong value is
  checkable; a green/red result is not.
- **One ablation per property.** An ablation that breaks two things proves neither.
- **A green ablation is a defect in the ablation**, not a pass.
- **Screenshots state what they prove and do not.**
- **Disclose rather than manufacture.** Declining to produce an artifact, with the
  reason, is worth more than a staged one.
- **Retire obligations explicitly.** When a carried-forward item stops applying, say so
  and why, in the place it was recorded. A list that only grows is one nobody reads.

## PR-019-A — Design and handoff acceptance

Granted by the human owner 2026-08-10 with RFC-019. Handoff pack authored the same day.

## PR-019-B — The explorer tree

Implemented 2026-08-10, not yet reviewed. Against `task-breakdown-pr-plan.md`'s review gate:

**Starting state confirmed by enumeration before writing any code.** `grep -rn` for `scan_active_project_explorer_directory`, `open_active_project_text_document`, `replace_active_project_text`, `save_active_project_text_document`, `refresh_active_project_text_document` across both crates matched only `#[cfg(test)]` call sites and the `AppState`/`ApplicationShell` definitions themselves — exactly the condition RFC-019's own text predicted. `scan_active_project_explorer_directory_has_exactly_the_two_named_production_call_sites` now pins the state that confirmation left behind: exactly two named call sites, `ensure_explorer_scanned` (the first scan, on entering Content mode) and `handle_explorer_key` (a rescan, on the user selecting a directory) — a third call site fails this test by name, the same shape `terminal_input_policy_evaluate_has_exactly_one_production_call_site` and `write_terminal_input_has_exactly_the_three_named_production_call_sites` use for the equivalent property in RFC-018.

**A new surface, `crates/tekstide/src/surface/explorer.rs`, mirrors `board.rs`'s own shape rather than inventing a new one.** `view()` takes read-only pieces (`Option<&ExplorerDirectoryScan>`, `&ProjectExplorerStatus`, the highlight index, `&Catalog`, `&Theme`), never `&mut State` or anything that could reach the modal layer — the same constraint `surface.rs`'s own module doc names. The actual line-building logic (`node_line`, `row_line`, `status_line`, `tree_lines`) is factored out into plain functions returning `String`/`Vec<String>`, directly testable without `iced`, the same split `board::row_lines` and `session_bar::entry_text` use.

**Every rendered name, path hint, and status escaped through `text_safety::quote_untrusted` — checked against both directions.** `node_line` escapes `node.name` before it ever reaches `CatalogArgs` (which structurally cannot accept a raw `&str` for untrusted text — `.untrusted()` only accepts `&DisplayText`, and the only constructor for that type is `quote_untrusted` itself). `ProjectExplorerStatus::Error`'s own message is escaped the same way — found while writing the catalog message, not assumed safe: `ExplorerScanError`'s `Display` impl embeds the target's relative path, which is exactly the same attacker-influenced class as a node name, and the first draft of the `.ftl` comment claimed otherwise before I checked the actual `Display` implementation.

**The bidi-override case tested specifically, and ablated in the direction that matters.** `a_bidi_override_node_name_renders_escaped_and_the_raw_character_is_absent` pastes a real `proj\u{202E}gpj.exe` name and asserts both the escaped `<U+202E>` marker is present and the raw override character is absent. **Ablated**: temporarily replaced `node_line`'s body with a raw `format!` bypassing `quote_untrusted` entirely (the catalog-argument API makes this the *only* way to bypass escaping — `trusted_symbol` requires `&'static str`, which a runtime name will not even compile against) — the test failed exactly as expected, with the raw `\u{202e}` character present in the panic's own printed value (`"expected the escaped marker in \"[FILE] proj\\u{202e}gpj.exe\""`), then reverted. `a_plain_node_name_renders_without_any_escape_marker` is the opposite-direction control: a non-hostile name renders with no escape marker at all, so the bidi test is exercising real escaping rather than a coincidence of that one fixture.

**No `*_label` free function called anywhere in this module.** `no_hardcoded_english_label_function_is_called_in_this_module` scans `explorer.rs`'s own source text for the literal call syntax of all four RFC-019 named (`explorer_node_kind_label(`, `explorer_node_state_label(`, `explorer_symlink_status_label(`, and the sibling `text_document_state_label(` PR-019-C owns) — a substring match a reviewer can verify by eye, the same shape `write_terminal_input_has_exactly_the_three_named_production_call_sites` uses for a different property. Every word instead resolves through one Fluent message, `explorer-node-entry`, with four selectors (`$kind`, `$name` untrusted, `$state`, `$symlink`) — one lookup per rendered node line, matching `session-bar-entry`'s own shape rather than concatenating separately-resolved strings.

**`NFR-UX-002`: every state/symlink combination renders a distinct line, checked exhaustively, not sampled.** `every_state_and_symlink_combination_renders_a_distinct_line` enumerates all 4×4 combinations of `ExplorerNodeState`/`FileAccessSymlinkStatus` and asserts each produces a line no other combination also produces — this module never applies colour to a node line at all, so text distinctness is the whole of the property, not a channel alongside colour. `every_kind_renders_a_distinct_marker` covers the third, independent axis (`ExplorerNodeKind`) the same way.

**Open question 2 answered: the explorer does not show a symlink's target, only that a symlink exists and its status.** Decided with escaping already in place (`explorer-node-entry`'s `$symlink` selector was written and tested before this decision), so the question was about usefulness rather than risk, per the RFC's own instruction. A symlink's target is itself attacker-influenced text pointing outside the project, and rendering it usefully (relative vs. absolute, resolved vs. broken) is real design work with no clear minimal shape — the indicator alone (`[symlink]` / `[broken symlink]` / `[symlink escapes root]`) answers "should I be suspicious of this entry" without that work. Revisit if a real user need for the target surfaces; the escaping path is already proven either way.

**No filesystem walking in the shell.** `visible_rows` is built directly from `scan.nodes`, in the order core's own scan returned them, plus a synthetic parent row computed from `scan.directory.selected_relative_path.parent()` — no `std::fs` call anywhere in `explorer.rs`. `visible_rows_never_exceeds_the_scans_own_node_count_plus_the_parent_entry` checks this as a structural property: the rendered row count is always exactly `nodes.len()` (project root) or `nodes.len() + 1` (elsewhere), never anything a directory read of its own could have produced.

**Selection drives `scan_content_explorer_directory`, wired as real keyboard interaction, not deferred.** `SurfaceInput` had no accessor for its own `key` field before this slice — `shell.rs`'s `RoutedInput::Surface` arm was a documented no-op since PR-015-D, and this is its first real consumer. Added `SurfaceInput::key()` (`pub(crate)`, `input.rs`). `handle_explorer_key` moves a shell-local highlight index (`State::explorer_highlight`, the direct analogue of `PasteConfirmButton`'s focus index — a keyboard-cursor concern, not a duplicate of any core state) with Up/Down, and Enter on a directory (or the synthetic parent row) triggers a real rescan via `ApplicationShell::scan_active_project_explorer_directory`. Enter on a file row is a deliberate no-op — opening a document is PR-019-C's job, not this slice's.

**Gates**: `fmt --check`, `clippy --workspace --all-targets --all-features -D warnings`, full test suite (505 `tekstide-core`, unchanged — this slice touches no core code, per the RFC's own "nothing here needs designing" instruction — + 166 `tekstide`, 12 net new: 11 in `surface/explorer/tests.rs` + 1 enumeration test in `shell/tests.rs`), `git diff --check`. All passed.

**Not done, correctly**: no editor (PR-019-C's job — a file row's Enter is a no-op here). No editing or save (PR-019-D). Screenshots deferred to PR-019-E's closeout, matching the RFC's own "Evidence Required" section shape (one shared evidence slice, not one per surface) rather than RFC-018's dedicated evidence slice, since RFC-019 has no PR-019-B-specific evidence gate item beyond the tests above.

**Approved 2026-08-10 (response 180).** No required items. One non-blocking note: nothing yet demonstrated the surface reaches the screen (every test operates on plain functions returning `String`). Not required before PR-019-C, but asked to be confirmed alongside that slice's own GUI work rather than left until PR-019-E with three slices resting on an unverified surface. Discharged in PR-019-C's own entry below — both surfaces confirmed together, since PR-019-C's GUI session answers it for free.

## PR-019-C — The editor, read-only

Implemented 2026-08-11, not yet reviewed. Against `task-breakdown-pr-plan.md`'s review gate:

**The text area renders raw — the opposite property from every other surface in this crate, and checked in the opposite direction.** `body_text(document: &TextDocument) -> String` is the one function in this crate that must never call `text_safety::quote_untrusted`; `view()` calls only this for the text area, `chrome_line` (below) for everything around it. `body_text_preserves_a_bidi_override_character_raw` opens a real file named with a `U+202E` sequence *in its content* and asserts the raw character survives and no `<U+202E>` marker appears. **Ablated in the opposite direction PR-019-B's own bidi test was**: temporarily wrapped `body_text`'s return in `quote_untrusted` and re-ran the raw-preservation test — it failed with the exact wrong value (`"\u{2068}echo proj<U+202E>gpj.exe\u{2069}"`, isolate marks and escape marker both present), then reverted. `asserting_the_escaped_form_would_fail_because_body_text_never_escapes` records the same property by construction: `body_text` has no code path that could produce an escape marker, so a test asserting one appears cannot pass without the function itself changing — the manual ablation is what demonstrated that, not asserted.

**Chrome around the editor escapes — including a category the RFC's own text did not name.** `chrome_line` escapes the document's own path (`chrome_line_escapes_a_bidi_override_in_the_path`) exactly like an explorer node name. Writing `editor-open-error`'s `.ftl` comment surfaced the same finding PR-019-B's review made: `TextDocumentOpenError`'s `Display` embeds the target's relative path in *every* variant, including the 4 MiB `TooLarge` refusal this RFC explicitly asks to be rendered — escaped via `open_error_line` before it reaches the catalog, checked against a real oversized-file open failure in `opening_a_file_over_the_policy_bound_is_refused_and_rendered`, not only a synthetic message string.

**Cursor and viewport are not duplicated.** `State` gained no cursor field — `body_text`/`chrome_line`/`view` all take `&TextDocument` and read `document.state()` directly; nothing in `crates/tekstide` stores a line/column pair of its own. This slice does not yet render a visible cursor indicator or wire cursor movement (read-only, per the RFC's own scope: no editing means nothing needs to know where an edit would land) — carried forward to PR-019-D, which is where cursor movement first has a reason to exist.

**The 4 MiB refusal is rendered, using the real policy's own bound.** `opening_a_file_over_the_policy_bound_is_refused_and_rendered` writes a file one byte over `TextDocumentOpenPolicy::default().max_editable_bytes`, opens it for real, and asserts the rendered line contains that real bound (`4194304`), not a second one this module could have introduced by accident.

**`text_document_state_label` (RFC-019's fourth and final named producer) is not called anywhere in this module** — checked by source-text scan, the same shape PR-019-B's own check uses for the other three. `TextDocumentState` renders through `editor-chrome`'s `$state` selector instead; `every_document_state_maps_to_a_distinct_symbol` checks all five variants map to distinct compile-time symbols directly (a pure function, no document needed for four of the five states this read-only slice cannot itself reach).

**Every user-facing word through `Catalog`.** `editor-chrome`, `editor-empty`, `editor-open-error` — no hardcoded English in `editor.rs` beyond the catalog keys themselves.

**Wired into the shell for real, and confirmed on screen — not left as untested plumbing.** `main_area_view` gained a `(Some(ProjectMode::Content), _)` arm rendering `surface::editor::view`, the same shape the `TerminalImmersion` arm already established. The explorer's Enter-on-a-file (a deliberate no-op in PR-019-B) now calls `open_active_project_text_document`. Response 180 asked, non-blocking, for confirmation that PR-019-B's explorer actually reaches the screen before PR-019-D builds further on it; since this slice's own GUI work answers that for free, both surfaces are confirmed together:

- `evidence/pr-019-c-01-file-opened-real-content.png` — a real scratch project (`/tmp/pr-019-c-scratch`, not committed), the explorer sidebar showing four real entries including a live `proj<U+202E>gpj.exe` file (escaped to `[FILE] proj<U+202E>gpj.exe`, the same bidi-override class already live elsewhere in this project's own state), and the editor's main area showing `README.md`'s real content (`# demo project`, `some content here.`) after Tab (focus to sidebar) then Enter. Proves: the explorer renders a real scan, the highlight/selection mechanism works, Enter opens a real file, the editor renders real raw content in the main area. Does not prove anything about the 4 MiB refusal or `ExternalChangeDecision` (PR-019-D's).
- `evidence/pr-019-c-02-bidi-filename-chrome-escaped.png` — the same session, Down then Enter to open the bidi-named file itself. The editor's own chrome header shows `proj<U+202E>gpj.exe`, escaped identically to the explorer's rendering of the same name, while the body renders the file's real content (`fake executable disguised as image`) raw. Proves: the editor's chrome escaping path is real, not only unit-tested, and is consistent with the explorer's. Does not prove the body would stay raw if *that* file's *content* (not name) carried a bidi character — `body_text_preserves_a_bidi_override_character_raw` covers that at the unit level; a third screenshot would have been the same shape as the first with no new information.

**A genuine navigation gap found and worked around, not fixed (out of scope).** There is currently no `NavigationAction` binding that maps to `AppCommand::OpenActiveProjectWorkspace` directly — `SwitchActiveProject`'s own binding is `None`/`Configurable`, disclosed as such in `navigation.rs` already. The route only changes to `ActiveProjectWorkspace` as a side effect of `ToggleActiveProjectMode` (`Ctrl+Alt+M`) or `LaunchTerminal` (`Ctrl+Alt+T`) succeeding. Screenshots above used `Ctrl+Alt+M` twice (Content → TerminalImmersion → Content) to reach the workspace route while landing back on the mode this slice needed. Not a PR-019-C defect — this is pre-existing `0.5.x` navigation shape, unrelated to what this slice renders — noted here so a future reader does not read the double-toggle in the capture method as this slice's own workaround for something broken.

**Gates**: `fmt --check`, `clippy --workspace --all-targets --all-features -D warnings`, full test suite (505 `tekstide-core`, unchanged + 175 `tekstide`, 9 net new in `surface/editor/tests.rs`), `git diff --check`. All passed.

**Not done, correctly**: no editing (`replace_active_text` untouched — PR-019-D). No `ExternalChangeDecision` rendering (PR-019-D). No cursor/viewport indicator or movement (carried forward, see above).

## PR-019-D — Editing and save

Pending implementation.

## PR-019-E — Closeout

Pending implementation.

## Known Limitations

Consolidated at closeout. Carried in from RFC-019's own text, to be restated with
evidence:

- **No syntax highlighting, language servers, multi-cursor, or search** — non-goals, each
  a product in its own right.
- **Files above 4 MiB are not editable.** `TextDocumentOpenPolicy`'s existing bound, not a
  new one introduced here.
- **No file-tree write surface** — no rename, delete, or create.
- **"Show invisibles", if built, is ordinary functionality and not a security control.**
  RFC-016 chose that framing deliberately; a marker the user can toggle off is not a
  boundary.
- **Nothing here changes terminal performance.** `NFR-PERF-004`, the three-terminal limit
  and the output ceiling are owned by readiness-driven terminal I/O.
