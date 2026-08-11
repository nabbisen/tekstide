---
title: "RFC-019: Editor and Explorer Surfaces - QA Evidence"
rfc: "RFC-019"
rfc_file: "../../done/019-editor-and-explorer-surfaces.md"
status: "PR-019-A/B/C/D/E accepted with one required follow-up (responses 180, 181, 182, 183, 184) — required follow-up (core-side status-mapping defect recorded) implemented 2026-08-11, not yet reviewed"
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
- `evidence/pr-019-c-02-bidi-filename-chrome-escaped.png` — the same session, Down then Enter to open the bidi-named file itself. **Proves the chrome half of the asymmetry only**: the editor's own chrome header shows `proj<U+202E>gpj.exe` escaped, identically to the explorer's rendering of the same name, confirming the editor's chrome-escaping path is real, not only unit-tested, and consistent with the explorer's. **Does not prove the body renders raw** — corrected per response 181: the body shows `fake executable disguised as image`, plain ASCII with nothing escapable in it, so the image cannot distinguish "renders raw" from "had nothing to render." That property is proven by the ablation above instead (stronger evidence than a screenshot could give), not by this artifact. A fixture whose name **and** content both carry `U+202E` would show both halves in one frame — deferred to PR-019-E as the strongest single image this RFC can produce, per the reviewer's suggestion.

**A genuine navigation gap found and worked around, not fixed (out of scope).** There is currently no `NavigationAction` binding that maps to `AppCommand::OpenActiveProjectWorkspace` directly — `SwitchActiveProject`'s own binding is `None`/`Configurable`, disclosed as such in `navigation.rs` already. The route only changes to `ActiveProjectWorkspace` as a side effect of `ToggleActiveProjectMode` (`Ctrl+Alt+M`) or `LaunchTerminal` (`Ctrl+Alt+T`) succeeding. Screenshots above used `Ctrl+Alt+M` twice (Content → TerminalImmersion → Content) to reach the workspace route while landing back on the mode this slice needed. Not a PR-019-C defect — this is pre-existing `0.5.x` navigation shape, unrelated to what this slice renders — noted here so a future reader does not read the double-toggle in the capture method as this slice's own workaround for something broken.

**Gates**: `fmt --check`, `clippy --workspace --all-targets --all-features -D warnings`, full test suite (505 `tekstide-core`, unchanged + 175 `tekstide`, 9 net new in `surface/editor/tests.rs`), `git diff --check`. All passed.

**Not done, correctly**: no editing (`replace_active_text` untouched — PR-019-D). No `ExternalChangeDecision` rendering (PR-019-D). No cursor/viewport indicator or movement (carried forward, see above).

**Approved 2026-08-11 (response 181).** No required items. The opposite-direction ablation (escaping added, not removed — the inversion of every other surface's own bidi check) was confirmed as the one that matters, run correctly, with the exact wrong value recorded. The `TextDocumentOpenError` finding was called out as sharper than PR-019-B's `ExplorerScanError` one: the review gate's own text asked for the 4 MiB refusal to be rendered, and following that instruction without checking the `Display` impl would have shipped the unescaped path *because of* the instruction, not despite it.

**The generalisation for PR-019-D, stated once here rather than rediscovered a third time**: this is now two slices in a row where a core error type's `Display` impl turned out to interpolate attacker-influenced text, found both times by reading the impl rather than trusting an already-written comment. PR-019-D renders `ProjectContentError` and `ExternalChangeDecision` — **assume any core error type's `Display` interpolates a path until its impl says otherwise**, and check before writing the `.ftl` comment, not after.

**Caption correction, applied above**: the `02` screenshot's original caption implied it demonstrated the body rendering raw. It does not — the file's content is plain ASCII with nothing escapable in it, so the image cannot distinguish "renders raw" from "had nothing to render." The raw-body property is proven by the ablation, not by that screenshot; the caption above now says so explicitly. A fixture whose name *and* content both carry `U+202E` — showing the header escaped and the body raw in one frame — is deferred to PR-019-E as the single strongest image this RFC can produce.

**Carried into `rfcs/future-work.md`** (Desktop GUI Runtime theme), not left only in this evidence file: no `NavigationAction` reaches `AppCommand::OpenActiveProjectWorkspace` directly today; the workspace route is only reachable as a side effect of `Ctrl+Alt+M` or `Ctrl+Alt+T` succeeding.

## PR-019-D — Editing and save

Implemented 2026-08-11, not yet reviewed. Against `task-breakdown-pr-plan.md`'s review gate:

**A real core-API gap, raised rather than worked around — the RFC's own escape hatch, triggered.** `ProjectContentWorkspace::active_document()` returns `Option<&TextDocument>` only; there is no mutable accessor anywhere on the type, and therefore no way for this crate to reach `TextDocument::set_cursor()` even though that method exists. RFC-019's own text says: "If core's edit surface turns out insufficient for real editing, stop and raise it as an RFC-006 question. Do not work around it in the shell — that is the shape that produced two consolidations already." This is that situation. **What shipped instead, disclosed rather than hidden**: `apply_edit_key(text: &str, key: &iced::keyboard::Key) -> Option<String>` (`surface/editor.rs`) is deliberately append-only — every typed character appends to the end, Backspace removes the last character, Enter appends a newline — built only on `replace_active_text`'s whole-buffer-replace API, with no shell-local cursor state invented to fake cursor-aware insertion. `document.cursor()` still reads a real value (always `(0, 0)`, since nothing here ever calls `set_cursor`). **This is a real, honest, but limited editor** — it can open, edit-by-appending, and save a file correctly, but cannot insert or delete in the middle of content. Raising this as this slice's primary open question for the architect, not burying it in a comment: does adding `ProjectContentWorkspace::active_document_mut()` (or an equivalent cursor-forwarding method) belong in an RFC-006 amendment before PR-019-E's closeout, or is append-only an acceptable `0.6.0` limitation to disclose and defer?

**Append-only editing, tested against real key input, not the pure function alone.** Unit-level: `a_typed_character_appends_to_the_end`, `enter_appends_a_real_newline`, `backspace_removes_the_last_character_by_char_not_by_byte` (checked against `"café"`, a multi-byte character, so a naive byte-truncation bug would corrupt rather than cleanly remove it), `backspace_on_empty_content_is_a_no_op`, `a_non_edit_key_produces_no_edit`, `a_multi_byte_typed_character_appends_whole`. Shell-level, through the real router (`route_non_modal_input`, not a hand-built `SurfaceInput` — that type has no test constructor by design): `a_typed_key_edits_the_real_active_document_through_real_routing` opens a real file, routes a real `!` keypress with `FocusZone::MainArea` focused, and asserts the real `TextDocument`'s content changed. `SurfaceInput` gained a `#[cfg(test)]`-only constructor (`input::surface_input_for_test`) for this, mirroring `shell_input_for_test`/`terminal_stream_for_test`'s existing shape.

**`Ctrl+S` is a real global keybinding, not a shell-local shortcut.** `NavigationAction::SaveActiveDocument` added to `tekstide-core::navigation`, bound `Ctrl+S`/`Candidate` in `linux_mvp()`, checked mechanically against every other rule for collisions (`save_active_document_shortcut_is_a_candidate_that_collides_with_no_other_rule`, the same shape `paste_into_terminal_shortcut_...` already uses) — none. `app_command_for` maps it to `None` (like `PasteIntoTerminal`): no core route/mode change, real I/O handled directly in `update`'s `Shell(shell_input)` arm via the new `attempt_save_active_document`. `ctrl_s_saves_the_real_edited_document_to_disk` proves the real binding reaches real disk I/O: edits a real document, presses the real `Ctrl+S` combination through `route_non_modal_input`, and reads the file back from disk — not merely that `ProjectContentStatus` reports success.

**The conflict modal is a real `ModalContent` variant, not a second `Option` field — and every dismissal path defaults to not overwriting, each tested individually (PR-018-C's own convention).** `ExternalChangeModal { relative_path, focus: ExternalChangeButton }` / `ExternalChangeButton { Reload, Dismiss }` mirror `PasteConfirmationModal`/`PasteConfirmButton`'s exact shape (`next()`/`previous()`/`ORDER`, default focus on the non-destructive button — `Dismiss`, the same "less destructive-sounding target" convention `ModalContent::default()`'s own comment already states). `attempt_save_active_document` reads `ProjectContentStatus::Conflict` back from `workspace.status()` after a failed save (that mapping is `project::content::save_active_document`'s own job, already in place — not re-derived from the `Result` here) and opens the modal with the real `active_path_hint()`. **Correction, added at closeout (response 184): this sentence was written before the mapping's own coarseness was found.** `project::content::save_active_document` maps `SaveDecision::BlockedExternalChange` to `ProjectContentStatus::Conflict` *unconditionally* — it does not distinguish a genuine conflict from a clean document that merely changed on disk, unlike `refresh_active_document` in the same file, which does. That coarseness is what produced the PR-019-E defect (the modal claiming discarded changes with none to discard); the shell-side fix reads the more authoritative `document.state()` instead of trusting this mapping, and the mapping itself remains a real, disclosed `tekstide-core` defect — not fixed here, recorded in `rfcs/future-work.md`. Read "already in place" above as "already in place, and later found imprecise," not as an endorsement. **The review gate's own required proof — a real file changed underneath a real open buffer, not a synthesised `SaveDecision`:**

- `saving_over_a_real_external_change_opens_the_conflict_modal_and_reload_takes_the_disk_content`: opens a real file, edits it via `replace_active_project_text` (genuinely dirty), overwrites the same path on disk directly (`std::fs::write`, simulating another process), then routes a real `Ctrl+S` through `update`. Asserts the modal actually opened, defaulting to `Dismiss`; that the refused save never touched disk (`std::fs::read_to_string` still reads the external write, not the local edit); then cycles focus to `Reload` and activates it, asserting the document's real text now equals the external content (local edit discarded) and the modal closed.
- `dismissing_the_conflict_modal_never_writes_the_local_edit_to_disk`: the same real-conflict setup, but `Message::ModalDismiss` (Escape) instead of Reload — asserts the file on disk is **exactly** what the external write left it as, an ablation-shaped check proving dismissal never overwrites rather than merely that the modal closes.

This is on top of, not instead of, `tekstide-core`'s own `external_dirty_conflict_is_visible_without_overwriting_disk` (`crates/tekstide-core/src/shell/tests.rs`), which already proves `TextDocument::save()` has no force-overwrite bypass at the core layer — `save()` unconditionally returns `Err(ExternalChange)` once the disk snapshot has diverged, regardless of local dirty state, so "overwriting someone else's change" is structurally impossible through this API, not merely discouraged. These two new tests prove the *shell*/modal wiring surfaces that real core property correctly, against a real failed save, not a stub.

**A third instance of the `Display`-interpolates-a-path pattern, checked before writing the comment this time — the PR-019-C generalisation applied, not rediscovered the hard way a third time.** `TextDocumentSaveError`'s `Display` (`content::save`) embeds `target.selected_relative_path.display()` in every path-carrying variant, including `ExternalChange`. `external_change_dialog_body(catalog, relative_path)` was factored out of `external_change_modal_view` (mirroring `chrome_line`'s own shape) specifically so the escaping property is directly testable, and escapes the path via `text_safety::quote_untrusted` before it reaches `external-change-dialog-body`'s `$path`. **Ablated for real, not merely asserted**: temporarily replaced the function body with `format!("{} changed on disk", relative_path.display())` (bypassing `quote_untrusted` entirely — the catalog-argument API structurally cannot accept a raw string, so the bypass has to happen before the catalog call, same as PR-019-B/C's own ablations), re-ran `external_change_dialog_body_escapes_a_bidi_override_in_the_path`, and it failed with the exact wrong value `"proj\u{202e}gpj.exe changed on disk"` (the raw override character present, no `<U+202E>` marker) — then reverted from a backup and re-confirmed green.

**Catalog additions**: `external-change-dialog-title`, `external-change-dialog-body` (`$path`, untrusted — reuses `generic_args()`'s existing `path` entry, no new completeness-test entry needed), `external-change-dialog-reload`, `external-change-dialog-dismiss`, `external-change-dialog-hint`. `every_source_locale_key_resolves_in_every_shipped_locale` passes with no changes to `generic_args()` beyond a one-line comment noting the reuse.

**Every affected exhaustive `match` extended, not left to panic on the new variant** — this is what actually blocked progress mid-slice: adding `NavigationAction::SaveActiveDocument` broke `app_command_for`'s match (the interrupting compile error), and adding `ModalContent::ExternalChange` broke `ModalFocusNext`/`ModalFocusPrevious`/`ModalActivate`'s matches in `update`, the modal-dispatch match in `view`, and `trusted_ui_state`'s match (`TerminalTrustedUiState`, defined in `tekstide-core::runtime::terminal::security::paste`). The external-change dialog is not a terminal-paste concern at all, so it maps to `SecurityDialogActive` — the same generic, "no real terminal-paste dialog kind of its own" bucket `LayerDemo` already occupies, documented as such at the call site rather than left to look like an oversight.

**`open_active_project_text_document` gained a third production call site** (`ModalActivate`'s `Reload` arm, alongside `handle_explorer_key`'s pre-existing `Action::Open`) — no dedicated "exactly N named call sites" test exists for this specific function (unlike `scan_active_project_explorer_directory`'s own enumeration test), so nothing pins this count; noted here for completeness rather than silently grown.

**Live GUI verification, not left as untested plumbing** (`.git-exclude/tools/launch-scratch-gui.sh`, a scratch `XDG_STATE_HOME`, real release binary, real project directory with one file):

- `evidence/pr-019-d-00-workspace-entered.png` — the workspace route entered (the pre-existing `Ctrl+Alt+M` double-toggle from PR-019-C's own capture method), explorer showing `notes.txt` already highlighted, main area showing the real empty-state notice.
- `evidence/pr-019-d-01-file-opened.png` — Tab to Sidebar, Enter opens the real file; the main area shows its real content, `hello world`.
- `evidence/pr-019-d-02-typed-edit-dirty.png` — Tab back to MainArea, real keystrokes typed. The chrome header shows `notes.txt (unsaved changes)`, proving `ProjectContentStatus::Edited`/`TextDocumentState::Dirty` render live, and the body shows the real append landing after the file's own trailing newline. (The three characters sent were `!`; this test environment's `xdotool type` mapped them to `1` without Shift for reasons unrelated to this crate — a keyboard-layout artifact of the capture environment, not an application defect. What the image actually proves — real keys reaching a real document via `handle_editor_key`, appended, dirty state rendered — holds regardless of which characters arrived; the unit/shell-level tests above pin the exact character behaviour instead of relying on this capture.)
- `evidence/pr-019-d-03-saved-clean.png` — real `Ctrl+S`. The chrome's `(unsaved changes)` marker is gone, and the scratch project's real file on disk was independently confirmed (`cat`) to contain the typed edit.
- `evidence/pr-019-d-04-conflict-modal.png` — a fresh local edit typed, then the same path overwritten on disk directly (simulating another process) while the buffer was still open, then real `Ctrl+S`. The real "File Changed On Disk" modal is shown, defaulting to `Dismiss`, with the chrome behind it correctly reading `notes.txt (conflict)`.
- `evidence/pr-019-d-05-reloaded.png` — Tab (Dismiss → Reload) then Enter. The document now shows the real external content (`external process wrote this`), the modal closed, chrome clean — Reload proven live, not only in the headless test.

The Dismiss-path property (never overwrites) was not re-captured as a second live screenshot; `dismissing_the_conflict_modal_never_writes_the_local_edit_to_disk` already proves it against real disk I/O, and a screenshot of "the modal closed and nothing changed" would show less than that test already demonstrates.

**Gates**: `fmt --all --check`, `clippy --workspace --all-targets --all-features -D warnings`, full test suite (506 `tekstide-core`, +1 net new — the `Ctrl+S` collision test — over PR-019-C's 505; 186 `tekstide`, +11 net new: 6 in `surface/editor/tests.rs`, 5 in `shell/tests.rs`), `git diff --check`. All passed.

**Not done, correctly**: no visible cursor indicator or cursor movement (blocked on the core-API gap above — `document.cursor()` is always `(0, 0)`, nothing here calls `set_cursor`). No mid-buffer insertion or deletion (the append-only limitation, disclosed above, not a bug). No `refresh_active_document`/`ExternalChangeDecision::ExternalChanged` (a non-conflicting external change, distinct from the `Conflict` case this slice handles) surfaced anywhere yet — `ProjectContentStatus::ExternalChanged` renders no chrome text of its own beyond `TextDocumentState::ExternalChanged`'s existing `editor-chrome` selector arm (already in place since PR-019-C); no UI prompts the user to refresh. Carrying this forward to PR-019-E's own review, not silently dropping it: is a non-conflicting external change (file changed on disk, but no local edits to lose) worth its own notice, or is the existing passive chrome indicator sufficient for `0.6.0`?

### PR-019-D addendum — response 182: cursor-aware editing replaces append-only

**Response 182's ruling**: the save/conflict work above is approved as-is and does not need re-review. **Append-only editing must not ship** — a working save plus text landing silently at the end regardless of where the user is looking is a real defect (file corruption reachable by ordinary use), not a limitation a disclosure can make safe. The architect decided the cursor question as design authority rather than deferring it: `active_document_mut()` was rejected (it would let a caller bypass `replace_active_text`'s `self.status` bookkeeping and reopen the exact invariant the read-only accessor protects); a **narrow cursor-only forwarding method** was authorised instead, since cursor position participates in no dirty/save/conflict computation anywhere in `content::document` or `project::content`.

**RFC-006 Amendment 1, recorded in that RFC's own text** (`rfcs/done/006-projectsession-state-and-file-explorer-editor-basics.md`), authorising `ProjectContentWorkspace::set_active_cursor(cursor: TextCursor) -> Result<(), ProjectContentError>`, threaded through `ProjectSession::set_active_cursor` (forwards + `record_activity()`, no `sync_file_state_from_content_workspace` — nothing in `ProjectFileState` derives from cursor) and `AppState`/`ApplicationShell::set_active_project_cursor` (forward-only, matching `replace_active_project_text`'s existing shape exactly). **Checked directly, not only asserted**: `cursor_move_never_changes_project_content_status` (`tekstide-core::shell::tests`) opens a real document, moves the cursor, and asserts `shell.render_text()` is byte-identical before and after — the exact property the amendment's rationale claims.

**`apply_edit_key` rewritten as cursor-aware, `navigate_cursor` added.** Insert/Enter/Backspace now operate at `TextCursor`'s real `(line, column)` position (char-indexed, multi-line aware via `text.split('\n')` — the same line convention `TextCursor.line` has to agree with) rather than always at the end. `EditResult { text, cursor }` replaces the old `Option<String>` return, so text and cursor are computed together and never drift from a stale second read. `navigate_cursor` handles ArrowLeft/Right/Up/Down independently of any edit — Up/Down clamp the target line's column to its real length (the standard plain-text-editor convention), checked specifically (`arrow_up_clamps_the_column_to_a_shorter_previous_line`, `arrow_down_clamps_the_column_to_a_shorter_next_line`) since a naive carry-the-column implementation would produce an out-of-range column. `body_text` is untouched — the RFC-016 raw-rendering property this crate's other ablations protect was not disturbed by this rewrite.

**Ablated for real, not only asserted.** Temporarily reverted `insert_at` to its old append-only body (ignoring the cursor argument entirely) and re-ran both `a_typed_character_inserts_at_the_cursor` (unit level) and `arrow_navigation_then_typing_inserts_in_the_middle_through_real_routing` (shell level, through the real key router). Both failed with the exact wrong values — `EditResult { text: "held!", cursor: (0, 0) }` instead of `EditResult { text: "he!ld", cursor: (0, 3) }` at the unit level; `"helloX"` instead of `"hXello"` at the shell level, after a real `ArrowRight` press moved the real cursor and a real typed key inserted there. Confirming the defect is caught at *both* layers — the pure function and the real routing path — is what proves the wiring, not only the algorithm, is correct. Reverted before committing.

**A rendered cursor indicator, the second half response 182 required.** `cursor_line(catalog, document)` (`surface/editor.rs`), factored out the same way `chrome_line` is, renders `document.cursor()` 1-indexed (`editor-cursor = Line {$line}, Column {$column}`) — trusted output only, no escaping needed (nothing here is attacker-influenced, unlike `chrome_line`'s path). `cursor_line_renders_the_real_one_indexed_position` and `cursor_line_renders_line_one_column_one_for_a_freshly_opened_document` check the 1-indexed conversion directly (a freshly opened document's real `(0, 0)` cursor must render "Line 1, Column 1", not "Line 0, Column 0").

**Live GUI verification, mid-buffer insertion demonstrated on screen, not only in the headless suite:**

- `evidence/pr-019-d-06-cursor-file-opened.png` — a freshly opened file, the real cursor indicator reading "Line 1, Column 1".
- `evidence/pr-019-d-07-cursor-moved.png` — five real ArrowRight presses; the indicator reads "Line 1, Column 6" (real cursor state, not a rendering-only counter).
- `evidence/pr-019-d-08-mid-buffer-insert.png` — one real typed key inserted exactly between `"hello"` and `" world"` (not at the end), the indicator advancing to "Line 1, Column 7", dirty chrome shown. (Same capture-environment artifact as `-02`: `xdotool key X` landed as lowercase `x` in this sandbox — a keyboard-layout quirk, not an application defect. What the image proves — insertion at the indicated position, not the end — holds regardless of the exact character; `arrow_navigation_then_typing_inserts_in_the_middle_through_real_routing` pins the precise character behaviour.)
- `evidence/pr-019-d-09-saved-after-mid-buffer-insert.png` — real `Ctrl+S`; chrome clean, and the scratch file on disk (independently `cat`-confirmed) reads `hellox world`.
- `evidence/pr-019-d-10-backspace-at-cursor.png` — one real Backspace; removes the just-inserted character exactly, back to `hello world`, cursor returning to column 6 — proving deletion also targets the real cursor position, not the end.

**Gates, re-run after the rewrite**: `fmt --all --check`, `clippy --workspace --all-targets --all-features -D warnings`, full test suite (508 `tekstide-core`, +2 over PR-019-D's original 506 — the two `set_active_cursor` tests; 201 `tekstide`, +15 over the original 186 — 2 new/rewritten test groups in `surface/editor/tests.rs` covering insert/backspace/enter/navigate/cursor-render, plus the real-routing mid-buffer test in `shell/tests.rs`), `git diff --check`. All passed.

**Not done, correctly, still**: no `refresh_active_document`/`ExternalChangeDecision::ExternalChanged` surfacing (carried forward to PR-019-E as before — unchanged by this addendum). Undo/redo remains out of this slice's scope (RFC-006 §3 lists it as "if feasible," not required).

## PR-019-E — Closeout

Implemented 2026-08-11. Against `task-breakdown-pr-plan.md`'s review gate.

### Commits

- `2d09d5e` — Accept RFC-019 and author its handoff pack.
- `ac7106e` — PR-019-B: the explorer tree.
- `03f685b` — PR-019-C: the editor, read-only.
- `6293390` — PR-019-C response 181: correct the 02 caption, carry two follow-ups.
- `90b7e3a` — PR-019-D: editing and save.
- `23acb30` — PR-019-D response 182: cursor-aware editing (RFC-006 Amendment 1).
- (this closeout) — PR-019-E: closeout, including the `ExternalChanged` wording fix and
  the single-frame asymmetry artifact.

### A defect found during closeout, fixed rather than only disclosed

Closing out PR-019-D required answering response 182's deferred question directly:
**does `ProjectContentStatus::ExternalChanged` need more than the passive chrome
indicator?** Checked against real code, not reasoned about in the abstract:
`TextDocument::save()`'s own `block_external_change` sets the document's real state to
`Conflict` only `if self.is_dirty()`, and to `ExternalChanged` otherwise — but
`ProjectContentWorkspace::save_active_document`'s error mapping turns *either* case into
the same `ProjectContentStatus::Conflict`, which is what `attempt_save_active_document`
gates the conflict modal on. Proven with a real scratch test before writing any fix:
open a real document, make **no** local edit, overwrite the file externally, save for
real —

```
content status: conflict | document: external changed | dirty files: 0
```

The modal PR-019-D built would have told this user **"your local changes will be
discarded"** with zero local changes to discard — a real, misleading claim, not a
hypothetical one. **Fixed, not merely disclosed**: `ExternalChangeModal` gained
`had_local_edits: bool`, read from `document.state() == TextDocumentState::Conflict`
(the same real distinction `save()` already computed, not re-derived); `$reason` selects
between two real messages in `external-change-dialog-body`. Ablated: temporarily forced
`had_local_edits = true` unconditionally — `saving_a_clean_document_over_a_real_external_change_does_not_claim_discarded_changes`
(real key routing, real `Ctrl+S`) failed on its own precondition assertion (the document
was never edited, yet the modal reported edits existed), confirming the test actually
exercises the real distinction. Reverted before committing.

Confirmed live: `evidence/pr-019-e-02-corrected-non-conflict-wording.png` — a clean
document, externally overwritten, real `Ctrl+S`; the dialog reads "Reload to see the new
content, or dismiss to keep your current view without saving," no discard claim, chrome
correctly reading `clean.txt (changed on disk)` rather than `(conflict)`.

**This is the answer to the deferred `ExternalChanged` question**: yes, it needed more
than the passive chrome indicator alone — specifically, the *save flow's* own modal
needed to stop conflating it with a genuine conflict. The chrome indicator itself
(`editor-chrome`'s `[external-changed]` arm, live since PR-019-C) was already correct
and needed no change. No new standalone notice was added beyond correcting the one that
already exists, since the modal only appears at a real trigger point (an attempted save)
and the chrome is always live regardless of whether one is attempted.

### The single-frame asymmetry artifact, deferred from PR-019-C, produced here

`evidence/pr-019-e-01-single-frame-asymmetry.png` — a fixture whose **name and content**
both carry `U+202E`: `proj<0x202E>gpj.exe` containing the literal bytes
`echo proj<0x202E>gpj.exe`. One frame shows both halves of RFC-019's central security
property at once: the chrome header renders `proj<U+202E>gpj.exe` **escaped** (the
`<U+202E>` marker, not the raw override), while the body renders the file's real content
**raw and bidi-reordered** — visually `echo projexe.jpg`, the Trojan Source class RFC-016
names, reordered correctly by the substrate exactly because this module never escapes it.
Response 181 called this the strongest single image this RFC could produce; response 183
carried it forward as a PR-019-E obligation. `evidence/pr-019-e-00-explorer-listing.png`
shows the same fixture escaped in the explorer tree first, for context.

### Open questions, answered

1. **Should the `.label()` scan be broadened to catch free functions?** **Raised, not
   absorbed** — recorded in `rfcs/future-work.md` (Desktop GUI Runtime theme) rather than
   decided here: the scan is `i18n::enforcement`'s territory, and a scan widened under a
   rendering RFC is a scan nobody owns, exactly as RFC-019's own text anticipated.
2. **Does the explorer render symlink targets?** Answered in PR-019-B: indicator only
   (`explorer_symlink_status_label`'s four states — none, in-root, unresolved, escapes
   root), never the target path itself. A target is attacker-influenced text pointing
   outside the project; showing the *fact* of an unsafe symlink is useful, showing *where*
   it points was judged not worth the added escaping surface for a first cut. Restated
   here since PR-019-B's own qa-evidence entry did not name this as an open-question
   answer explicitly.
3. **Syntax highlighting: at all, and if so when?** Answered now, with the editor
   working, per the gate's own instruction to decide this at closeout rather than before.
   **No.** It remains RFC-019's own non-goal (large dependency surface, no security
   content) and nothing in this RFC's implementation changes that calculus — a working,
   cursor-aware, save-capable editor does not need syntax colour to be correct or safe.
   If it is ever built, it is its own slice, evaluated on its own merits, not implied by
   this closeout.

### Claim statement, checked against RFC-019's own text

**What this RFC closes, matching its own "What this closes" section exactly**: RFC-006's
deferral of rendered content surfaces; Content mode being a placeholder since `0.4.0`.
Both surfaces (`ExplorerDirectoryScan`, `TextDocument`) render, with real editing, save,
and conflict handling. **What it does not close, also unchanged from the RFC's own
text**: diff review and the AgentRun report (RFC-020); anything terminal-related
(`NFR-PERF-004`, the three-terminal limit, the output ceiling — untouched by this RFC).

**No claim that "show invisibles" is a security control.** Nothing of the kind was
built — this RFC's editor renders raw text only, with no invisibles-marking affordance
of any kind. The claim RFC-016 forbade was never at risk of being made because the
feature it would describe does not exist yet.

**No claim of any terminal performance change.** This RFC touches no terminal code path;
`NFR-PERF-004` and its measurement remain exactly where RFC-017 left them.

**No claim of syntax highlighting, LSP, multi-cursor, or search-and-replace** — all
remain non-goals per RFC-019's own text, unchanged by this closeout (see open question 3
above).

**Editing is real but bounded, and the closeout must not imply otherwise (response 183's
own instruction)**: cursor-aware insert/delete/navigate at the real position, a real
save with real, distinguishing conflict handling. **No undo history beyond what RFC-006
models** — RFC-019 §Non-goals names this explicitly, and RFC-006 itself only ever listed
undo/redo as "if feasible," never delivered. **A mid-buffer edit is therefore
unrecoverable within the session**: Backspace removes what was just typed, but there is
no history to step back through past that. Reload (the conflict dialog's own recovery
path) discards *all* local edits in favour of disk, not a step-back to an earlier local
state. This is a real, user-facing limitation, stated plainly rather than left implicit
in "undo is a non-goal."

### Gates

`fmt --all --check`, `clippy --workspace --all-targets --all-features -D warnings`, full
test suite (508 `tekstide-core`, unchanged from the PR-019-D response-182 addendum; 203
`tekstide`, +2 over that addendum's 201 — the two conflict-wording tests above), `git diff
--check`. All passed.

## Known Limitations

Consolidated at closeout, restated with evidence rather than left as the RFC's own
carried-in text:

- **No syntax highlighting, language servers, multi-cursor, or search** — non-goals, each
  a product in its own right. Answered explicitly above (open question 3): no plan to
  build syntax highlighting, evaluated at closeout with the editor actually working.
- **No undo history beyond what RFC-006 models.** A mid-buffer edit is unrecoverable
  within the session once typed past what Backspace can still reach; Reload discards all
  local edits rather than stepping back to an earlier one. Stated plainly per response
  183's own instruction, not left implicit.
- **Editing is cursor-aware but not feature-complete**: no multi-cursor, no
  search-and-replace, no bracket matching or auto-indent — ordinary editor conveniences,
  each its own scope, none blocking correctness or safety.
- **Files above 4 MiB are not editable.** `TextDocumentOpenPolicy`'s existing bound, not a
  new one introduced here.
- **No file-tree write surface** — no rename, delete, or create.
- **No symlink target shown in the explorer** — indicator only (open question 2, answered
  above); a target is attacker-influenced text this first cut chose not to add escaping
  surface for.
- **"Show invisibles" was not built at all**, and if it is later, RFC-016's framing
  applies unchanged: ordinary functionality, not a security control. A marker the user
  can toggle off is not a boundary.
- **Nothing here changes terminal performance.** `NFR-PERF-004`, the three-terminal limit
  and the output ceiling are owned by readiness-driven terminal I/O, untouched by this RFC.
- **No `NavigationAction` reaches `AppCommand::OpenActiveProjectWorkspace` directly** —
  found during PR-019-C's GUI evidence work, not this RFC's to fix; recorded in
  `rfcs/future-work.md` (Desktop GUI Runtime theme) for RFC-023's keybinding pass.
- **The `.label()` completeness scan cannot catch hardcoded-English free functions** —
  open question 1, raised to `rfcs/future-work.md` rather than absorbed into this RFC,
  per its own text's instruction.
