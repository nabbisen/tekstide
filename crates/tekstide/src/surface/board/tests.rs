use std::path::{Path, PathBuf};

use tekstide_core::project::ProjectId;
use tekstide_core::project_board::{AttentionState, BoardRowKind, CountDisplay, ProjectBoardRow};

use super::{highlighted_row_lines, row_lines};
use crate::i18n::{Catalog, LocalePreference};

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn real_catalog() -> Catalog {
    Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()))
}

/// A row with every `CountDisplay` field `Unavailable` and a plain,
/// non-malicious name -- the baseline every test below overrides from.
fn baseline_row() -> ProjectBoardRow {
    ProjectBoardRow {
        project_id: ProjectId::new_uuid(),
        display_name: "demo-project".to_string(),
        root_path_hint: "/home/user/demo-project".to_string(),
        secondary_path_hint: None,
        availability_label: None,
        trust_label: "Trusted".to_string(),
        security_mode_label: "Full Access".to_string(),
        restricted_mode: false,
        blocked_automation_count: 0,
        blocked_automation_labels: Vec::new(),
        branch_status: CountDisplay::Unavailable,
        terminal_count: CountDisplay::Unavailable,
        agent_run_count: CountDisplay::Unavailable,
        approval_count: CountDisplay::Unavailable,
        review_count: CountDisplay::Unavailable,
        dirty_file_count: CountDisplay::Unavailable,
        attention: AttentionState::Calm,
        attention_label: AttentionState::Calm.label().to_string(),
        row_kind: BoardRowKind::ActiveSession,
    }
}

/// **`CountDisplay` fidelity, the acceptance criterion RFC-005/RFC-015
/// both name explicitly**: `Unavailable` and `NotImplemented` must never
/// render as `0`, or as anything containing a bare `0` that could be
/// mistaken for a real zero count. Checked against the real rendered
/// strings, not `CountDisplay::label()`'s output (never called here --
/// see the module doc).
#[test]
fn unavailable_and_not_implemented_never_render_as_zero() {
    let catalog = real_catalog();

    let mut unavailable_row = baseline_row();
    unavailable_row.terminal_count = CountDisplay::Unavailable;
    unavailable_row.agent_run_count = CountDisplay::Unavailable;
    unavailable_row.approval_count = CountDisplay::Unavailable;
    unavailable_row.review_count = CountDisplay::Unavailable;
    unavailable_row.dirty_file_count = CountDisplay::Unavailable;

    let mut not_implemented_row = baseline_row();
    not_implemented_row.terminal_count = CountDisplay::NotImplemented;
    not_implemented_row.agent_run_count = CountDisplay::NotImplemented;
    not_implemented_row.approval_count = CountDisplay::NotImplemented;
    not_implemented_row.review_count = CountDisplay::NotImplemented;
    not_implemented_row.dirty_file_count = CountDisplay::NotImplemented;

    for row in [&unavailable_row, &not_implemented_row] {
        for line in row_lines(row, &catalog) {
            assert!(
                !line.contains('0'),
                "a CountDisplay::Unavailable/NotImplemented field must never render with a \
                 bare 0 anywhere in its text: {line:?}"
            );
        }
    }
}

/// The positive case, so the test above is not merely "nothing contains
/// 0 because nothing renders": a genuine `KnownCount(0)` -- a real,
/// known-zero terminal count -- legitimately does render `0`, through
/// real CLDR plural selection (`"0 terminals"`), not a hardcoded string.
#[test]
fn a_genuine_known_zero_count_does_render_as_zero() {
    let catalog = real_catalog();
    let mut row = baseline_row();
    row.terminal_count = CountDisplay::KnownCount(0);

    let rendered = row_lines(&row, &catalog);
    // Fluent's automatic bidi isolation wraps the interpolated `{$count}`
    // placeable (documented and asserted the same way throughout
    // `i18n::tests`/`shell::tests` since PR-016-D) -- the literal digit
    // is `\u{2068}0\u{2069}`, not a bare `0`.
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("\u{2068}0\u{2069} terminals")),
        "a real KnownCount(0) must render as a real zero, distinguishable from Unavailable/\
         NotImplemented by more than coincidence: {rendered:?}"
    );
}

/// A single non-numeric `CountDisplay` state renders through real
/// catalog selection, not `label()` -- proven by checking the exact
/// wording, which only exists in `en.ftl`, not in `CountDisplay::label()`
/// (whose `"not available"` differs from this key's `"not available"`
/// only by coincidence of wording choice, not by being the same code
/// path -- the two are deliberately kept independent).
#[test]
fn unavailable_terminal_count_uses_the_catalog_not_label() {
    let catalog = real_catalog();
    let mut row = baseline_row();
    row.terminal_count = CountDisplay::Unavailable;
    let rendered = row_lines(&row, &catalog);
    assert!(
        rendered
            .iter()
            .any(|line| line == "terminals: not available"),
        "expected the catalog's own wording for an unavailable terminal count: {rendered:?}"
    );
}

/// The security-critical case, mirroring response 130's own probe plan:
/// a project whose *name* carries a live bidi override must render with
/// the override escaped to its visible `<U+202E>` marker, never live --
/// exactly RFC-016's Trojan Source threat model, now with a real render
/// call site to defend for the first time (PR-015-D is the first
/// surface to render untrusted text at all).
#[test]
fn an_untrusted_project_name_with_a_bidi_override_is_escaped_not_live() {
    let catalog = real_catalog();
    let mut row = baseline_row();
    row.display_name = "proj\u{202E}gpj.exe".to_string();

    let rendered = row_lines(&row, &catalog);
    let name_line = rendered.first().expect("name is always the first line");

    assert!(
        !name_line.contains('\u{202E}'),
        "a live bidi override must never survive into the rendered row: {name_line:?}"
    );
    assert!(
        name_line.contains("<U+202E>"),
        "the override must be escaped to its visible marker by text_safety, not silently \
         dropped: {name_line:?}"
    );
}

/// The same property for `root_path_hint` -- a different untrusted
/// field, same requirement, proven separately rather than assumed to
/// follow from the name case.
#[test]
fn an_untrusted_root_path_with_a_bidi_override_is_escaped_not_live() {
    let catalog = real_catalog();
    let mut row = baseline_row();
    row.root_path_hint = "/home/user/proj\u{202E}gpj.exe".to_string();

    let rendered = row_lines(&row, &catalog);
    let path_line = rendered
        .get(1)
        .expect("root path is always the second line");

    assert!(!path_line.contains('\u{202E}'));
    assert!(path_line.contains("<U+202E>"));
}

/// Ordinary, non-malicious names and paths must render with no visible
/// `<U+XXXX>` marker -- `text_safety::quote_untrusted` always wraps an
/// untrusted span in bidi isolate marks unconditionally (a separate,
/// permanent property, already documented in `text_safety`'s own
/// module doc), but *escaping* to a visible marker is conditional on
/// content, not blanket mangling of every project name.
#[test]
fn an_ordinary_project_name_renders_unescaped() {
    let catalog = real_catalog();
    let row = baseline_row();
    let rendered = row_lines(&row, &catalog);
    assert_eq!(rendered[0], "\u{2068}demo-project\u{2069}");
    assert_eq!(rendered[1], "\u{2068}/home/user/demo-project\u{2069}");
    assert!(!rendered[0].contains("<U+"));
    assert!(!rendered[1].contains("<U+"));
}

/// `blocked_automation_count` reuses PR-016-D's own key -- proven with a
/// nonzero count (the zero case is deliberately omitted from the row
/// entirely, see `board::row_lines`, so there is nothing to render when
/// there is nothing blocked).
#[test]
fn a_nonzero_blocked_automation_count_reuses_the_pr_016_d_key() {
    let catalog = real_catalog();
    let mut row = baseline_row();
    row.blocked_automation_count = 3;
    let rendered = row_lines(&row, &catalog);
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("blocked automations")),
        "expected the shared blocked-automation-count key's plural wording: {rendered:?}"
    );
}

/// A zero blocked-automation count renders no line at all for it --
/// proven directly, since silence is the intended behaviour, not an
/// oversight.
#[test]
fn a_zero_blocked_automation_count_renders_no_line_for_it() {
    let catalog = real_catalog();
    let row = baseline_row();
    assert_eq!(row.blocked_automation_count, 0, "test precondition");
    let rendered = row_lines(&row, &catalog);
    assert!(
        !rendered
            .iter()
            .any(|line| line.contains("blocked automation"))
    );
}

/// Attention state renders through the catalog's own wording, using the
/// real `AttentionState` enum -- not `attention_label`'s pre-baked
/// string (which happens to read identically, by design, but is not the
/// code path under test).
#[test]
fn attention_state_renders_through_the_catalog() {
    let catalog = real_catalog();
    let mut row = baseline_row();
    row.attention = AttentionState::Risk;
    let rendered = row_lines(&row, &catalog);
    assert!(rendered.iter().any(|line| line == "Risk"));
}

/// The empty-state keys resolve to real text, not the key itself --
/// `Catalog::get`'s "missing key renders as the key" fallback would
/// fail this loudly if any key name were ever mistyped.
/// Deliberately does not check against `ProjectBoardEmptyState`'s own
/// pre-baked English (the module doc: those strings are never read).
#[test]
fn empty_state_keys_resolve_to_real_catalog_text() {
    let catalog = real_catalog();
    for key in [
        "project-board-empty-heading",
        "project-board-empty-open-a-project",
        "project-board-empty-command-example",
        // RFC-038 PR-038-B/C: the field's own label (shared with the
        // populated-board `Ctrl+Alt+O` case, so no longer "empty-"
        // prefixed); `project-board-empty-keyboard-heading` removed from
        // this list -- the keyboard list itself moved off this surface
        // entirely into `shell::help_modal_view` (PR-038-C).
        "project-board-path-field-label",
        // RFC-038 PR-038-G: the folder browser's own visible control.
        "project-board-browse-button",
    ] {
        let rendered = catalog.get(key);
        assert_ne!(
            rendered, key,
            "empty-state key {key:?} did not resolve to real text -- check the key name against en.ftl"
        );
    }
}

/// 0.12.1. The empty state rendered "Add Project" and "Open from path"
/// as inert `text()` widgets from the day it landed: no `button`, no
/// `on_press`, and no in-app route to add a project at all. A user who
/// started `tekstide` with no arguments -- which the published Quick
/// Start told them to do -- saw two action names and could activate
/// neither.
///
/// This asserts the keys are *gone from the catalogue*, not merely
/// unused by this module. An unused key would let any future surface
/// render the same lie again, and the advisory unused-key report is not
/// a gate.
#[test]
fn the_two_action_labels_that_named_nothing_are_gone_from_the_catalogue() {
    let catalog_source = std::fs::read_to_string(real_locales_dir().join("en.ftl"))
        .expect("source-locale catalog must be readable");

    for retired in [
        "project-board-empty-primary-action",
        "project-board-empty-secondary-action",
    ] {
        assert!(
            !catalog_source.contains(&format!("\n{retired} =")),
            "{retired} is defined again -- it names an action that does not exist.              There is still no in-app way to add a project (RFC-038); until there is,              the empty state must say how one is actually opened."
        );
    }
}

/// The empty state must still say how a project actually gets opened,
/// independent of the keyboard list's own move off this surface
/// entirely (RFC-038 PR-038-C, RFC-039's second principle).
#[test]
fn the_empty_state_shows_the_actual_open_command() {
    let catalog = real_catalog();
    assert!(
        catalog
            .get("project-board-empty-command-example")
            .contains("tekstide "),
        "the empty state must show the actual command that opens a project"
    );
}

/// The other half of `every_board_state_renders_the_keyboard_list`'s
/// replacement (RFC-038 PR-038-C's task breakdown: "must be replaced,
/// not deleted"). That test's own property -- every arm of `view` shows
/// the keyboard list -- no longer applies: no arm of `view` shows it
/// any more, deliberately (RFC-039's second principle: reference
/// material does not live on a working surface). This is the negative
/// half of the replacement: proves the move was real, not merely
/// unused-but-still-callable. The positive half -- the Help modal,
/// reachable from anywhere, lists every live binding -- lives in
/// `shell::tests` (`opening_help_through_a_real_key_event_shows_every_live_binding`),
/// next to the code that now owns it.
#[test]
fn this_surface_no_longer_references_the_keyboard_list_at_all() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/surface/board.rs"),
    )
    .expect("board.rs must be readable");

    assert!(
        !source.contains("keyboard_help_lines") && !source.contains("keyboard_help_view"),
        "board.rs must not render the keyboard list -- it moved to shell::help_modal_view \
         (RFC-038 PR-038-C)"
    );
}

/// RFC-038 PR-038-G: **the first genuine `iced::widget::button` on
/// *this* surface** -- `board.rs` had none before (only text and the
/// path field). **Correction, not a first for the crate**: `shell.rs`'s
/// `TrustSettings` (Grant/Revoke, capture toggle, purge) and
/// `ApprovalHistory` (`OpenApprovalHistoryEntry`) surfaces already use
/// real `button(...).on_press(...)` controls (`shell.rs:5691` onward) --
/// an earlier claim in this module's own doc comment and in this PR's
/// `qa-evidence.md`/review request that those were `"> Label"`-marker
/// text only was wrong, found and corrected while starting PR-038-D
/// (see `qa-evidence.md`'s own correction note). A source-level check,
/// the same shape `this_surface_no_longer_references_the_keyboard_list_at_all`
/// already uses, since `iced::Element` gives a test no way to
/// introspect whether a rendered tree is a real widget or an inert
/// label -- proves the control is a real `button(...).on_press(...)`,
/// not a repeat of the "named nothing" defect
/// `the_two_action_labels_that_named_nothing_are_gone_from_the_catalogue`
/// exists to prevent.
#[test]
fn the_browse_button_is_a_real_clickable_widget_not_an_inert_label() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/surface/board.rs"),
    )
    .expect("board.rs must be readable");

    assert!(
        source.contains("iced::widget::button("),
        "the Browse control must be a real iced::widget::button, not a marker-prefixed text()"
    );
    assert!(
        source.contains(".on_press(open_browser_message)"),
        "the Browse button must dispatch a real message on click"
    );
}

/// RFC-038 PR-038-D: `highlighted_row_lines` prefixes exactly the name
/// line, exactly one of the two rows, with the keyboard cursor's own
/// marker -- the same "> "/"  " convention already proven elsewhere
/// (`surface::explorer::tests`), tested here at the string level rather
/// than through `iced`'s `Element` tree.
#[test]
fn highlighted_row_lines_marks_only_the_name_line_of_the_highlighted_row() {
    let catalog = real_catalog();
    let row = baseline_row();

    let highlighted = highlighted_row_lines(&row, &catalog, true);
    let not_highlighted = highlighted_row_lines(&row, &catalog, false);
    let unmarked = row_lines(&row, &catalog);

    assert!(
        highlighted[0].starts_with("> "),
        "the highlighted row's name line must carry the marker: {:?}",
        highlighted[0]
    );
    assert!(
        not_highlighted[0].starts_with("  "),
        "a not-highlighted row's name line must carry the blank-space equivalent, not nothing: \
         {:?}",
        not_highlighted[0]
    );
    assert_eq!(
        &highlighted[0][2..],
        unmarked[0],
        "the marker must be a prefix, not a rewrite of the underlying name line"
    );
    assert_eq!(
        &highlighted[1..],
        &unmarked[1..],
        "only the name line carries the marker -- every other line must be untouched"
    );
}

/// The other half of the widget-vs-inert-label proof for the recent-row
/// "Open" control, mirroring `the_browse_button_is_a_real_clickable_widget_not_an_inert_label`'s
/// own source-level shape: a real button, gated on row kind, present
/// regardless of highlight (the highlight is a keyboard cursor, not a
/// precondition for the mouse).
#[test]
fn the_recent_row_open_button_is_real_and_gated_on_row_kind_not_highlight() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/surface/board.rs"),
    )
    .expect("board.rs must be readable");

    assert!(
        source.contains("project-board-recent-open-button"),
        "the recent row's Open control must render through the catalog"
    );
    assert!(
        source.contains("row.row_kind != BoardRowKind::ActiveSession"),
        "the Open control must be gated on row kind (absent for ActiveSession rows), not on \
         whether the row happens to be highlighted"
    );
}

/// RFC-038's own acceptance criterion, in its own words: "The board's
/// empty state contains no text naming an action that is not
/// activatable. A test asserts this by enumeration, not by inspection."
///
/// Enumerates every `catalog.get("...")` key literal this whole module
/// calls, in source order, and requires the exact list -- the same "a
/// second call site fails this test by name" discipline every
/// enumeration test in this crate already uses, so a new key added
/// anywhere in `board.rs` must be named here explicitly and reasoned
/// about below, not merely counted or left to a future reader to
/// notice by inspecting the rendered output.
///
/// Of the six: three are plain descriptive prose with no action-shaped
/// claim at all (`project-board-empty-heading`,
/// `project-board-empty-open-a-project`,
/// `project-board-empty-command-example`); one describes a real,
/// keystroke-routed field rather than naming a separate action
/// (`project-board-path-field-label`); and two are real button labels,
/// each proven to sit on a genuine, wired `iced::widget::button` by its
/// own dedicated test rather than re-proven here
/// (`project-board-browse-button` by
/// `the_browse_button_is_a_real_clickable_widget_not_an_inert_label`;
/// `project-board-recent-open-button` by
/// `the_recent_row_open_button_is_real_and_gated_on_row_kind_not_highlight`).
/// This is the exact shape the pre-`0.12.1` defect did not have: two
/// keys (`project-board-empty-primary-action`/`secondary-action`, gone
/// from the catalogue entirely -- `the_two_action_labels_that_named_
/// nothing_are_gone_from_the_catalogue`) named actions with nothing
/// behind them at all.
#[test]
fn every_catalog_key_this_module_renders_is_enumerated_and_none_names_a_dead_action() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/surface/board.rs"),
    )
    .expect("board.rs must be readable");

    let needle = "catalog.get(\"";
    let keys: Vec<&str> = source
        .match_indices(needle)
        .map(|(index, _)| {
            let rest = &source[index + needle.len()..];
            let end = rest
                .find('"')
                .expect("a catalog.get( call must be a plain string literal");
            &rest[..end]
        })
        .collect();

    assert_eq!(
        keys,
        vec![
            "project-board-empty-heading",
            "project-board-empty-open-a-project",
            "project-board-empty-command-example",
            "project-board-browse-button",
            "project-board-path-field-label",
            "project-board-recent-open-button",
        ],
        "board.rs's own catalog.get( keys, in source order -- a new one added here must be \
         named explicitly in this test's own doc comment and reasoned about, not merely \
         counted: {keys:?}"
    );
}
