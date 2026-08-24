//! The Project Board surface: renders `ApplicationShell::project_board()`
//! (an existing, already-tested `ProjectBoardViewModel`) into the
//! content area. No new state -- every value here is read from the view
//! model handed in; nothing is cached or duplicated.
//!
//! **Untrusted vs. trusted fields, and why each is handled the way it
//! is:**
//!
//! - `display_name`, `root_path_hint`, `secondary_path_hint` are
//!   filesystem-derived (a project's directory name, its path) --
//!   attacker-influenceable, exactly what RFC-016's Trojan Source threat
//!   model is about. Routed through `text_safety::quote_untrusted`
//!   before ever reaching a widget. **Never** passed to
//!   `CatalogArgs::trusted_symbol` (which requires `&'static str` and
//!   would not even compile for these -- but the discipline matters
//!   independent of what the type system happens to catch) and never
//!   handed to `text(...)` raw.
//! - `CountDisplay` fields (`branch_status`, `terminal_count`,
//!   `agent_run_count`, `approval_count`, `review_count`,
//!   `dirty_file_count`) are routed through the catalog via
//!   [`count_display_args`], never through `CountDisplay::label()`.
//!   This is response 130's explicit decision point: `label()` returns
//!   hardcoded English for three of four variants, and this surface is
//!   the one RFC-005/RFC-015 required to preserve "`Unavailable`/
//!   `NotImplemented` never render as `0`" -- calling `label()` here
//!   would both hardcode English at the render layer and be the
//!   easiest way to quietly fail that requirement. `label()` keeps its
//!   existing role in `tekstide_core::shell::render_text` (the
//!   pre-GUI harness, kept -- see `qa-evidence.md`); this surface does
//!   not call it at all.
//! - `attention` (an `AttentionState` enum, not just `attention_label:
//!   String`) is also routed through the catalog, via
//!   [`attention_symbol`], for the same reason: the enum is available,
//!   so there is no reason to fall back to core's pre-baked English.
//! - `trust_label`, `security_mode_label`, `availability_label`,
//!   `blocked_automation_labels` have **no underlying enum exposed** in
//!   `ProjectBoardRow` -- only a pre-rendered `String` from
//!   `tekstide-core`. These are trusted (fixed-set, not
//!   filesystem-derived) but rendered as-is, not yet catalog-driven.
//!   Recorded as a known limitation, not silently accepted: closing it
//!   would need `tekstide-core::ProjectBoardRow` to expose the
//!   underlying enums alongside (or instead of) the label strings,
//!   which is a `tekstide-core` API change out of this slice's scope
//!   (`implementation-handoff.md` §8: raise `tekstide-core` changes
//!   first, don't fold them into an unrelated slice).
//! - The empty-state heading/actions **are** catalog-driven, because
//!   the only signal this surface needs from core is `Option::is_some()`
//!   -- no enum or new core field required, unlike the label-string
//!   fields above.

use tekstide_core::project_board::{
    AttentionState, CountDisplay, ProjectBoardRow, ProjectBoardViewModel,
};
use tekstide_core::text_safety;

use iced::widget::{column, container, text};
use iced::{Element, Length};

use crate::i18n::{Catalog, CatalogArgs};
use crate::theme::Theme;

pub fn view<'a, Message: 'a>(
    view_model: &ProjectBoardViewModel,
    catalog: &'a Catalog,
    theme: &'a Theme,
    path_field: &'a str,
    path_field_notice: Option<String>,
    show_field_on_populated_board: bool,
) -> Element<'a, Message> {
    if let Some(_empty_state) = &view_model.empty_state {
        return empty_state_view(catalog, theme, path_field, path_field_notice);
    }

    let rows: Vec<Element<'a, Message>> = view_model
        .rows
        .iter()
        .map(|row| row_view(row, catalog, theme))
        .collect();

    let mut sections: Vec<Element<'a, Message>> = vec![column(rows).spacing(12).into()];

    // RFC-038 PR-038-B: `Ctrl+Alt+O`'s own render arm -- the
    // second-project case, since the empty state's own field
    // (`empty_state_view`) only shows while there are no rows at all.
    // `show_field_on_populated_board` is `shell::path_field_is_showing`,
    // computed once and threaded down, so this cannot independently
    // decide differently than `handle_project_board_path_field_key`
    // does about whether a keystroke should reach the field.
    if show_field_on_populated_board {
        sections.push(path_field_section(
            catalog,
            theme,
            path_field,
            path_field_notice,
        ));
    }

    container(column(sections).spacing(20))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(16)
        .into()
}

/// What a first-time user sees, and until `0.12.1` the whole reason the
/// product looked broken: this rendered "Add Project" and "Open from
/// path" as inert `text()` widgets for two actions that do not exist,
/// while naming none of the nine live keybindings. `0.12.1` said how a
/// project actually gets opened and listed every binding, derived, on
/// this surface itself. RFC-038 PR-038-A added the missing action: a
/// path field, focused by construction (there is nothing else in
/// `MainArea` to route a keystroke to while the board is empty --
/// `shell::handle_project_board_path_field_key`'s own doc explains why
/// no separate "is this focused" state is needed). RFC-038 PR-038-C then
/// moved the keyboard list itself off this surface entirely, into
/// `shell::help_modal_view` (`Ctrl+Alt+K`, reachable from anywhere) --
/// RFC-039's own principle that reference material does not live on a
/// working surface. This board no longer renders any binding.
///
/// **Not `iced::widget::text_input`.** This project routes every
/// keystroke through one reviewed router (`input::route_non_modal_input`)
/// so global keybindings always win and a modal can suppress input
/// structurally, not by a guard a future surface could forget.
/// `text_input` maintains its own internal keyboard capture independent
/// of that router -- introducing it here would open a second,
/// unreviewed path for a keystroke to reach the application, exactly
/// what `input`'s module doc says this crate does not do anywhere else.
/// The field is instead a plain rendering of `path_field`, a `String`
/// `shell.rs` owns and appends to one `KeyPress` at a time, the same
/// shape [`crate::surface::editor::apply_edit_key`] already established
/// for the (multi-line) editor.
///
/// `path_field` is untyped, untrusted input -- routed through
/// `text_safety::quote_untrusted` here, same as every other
/// filesystem-derived string this module renders, never handed to
/// `text(...)` raw.
fn empty_state_view<'a, Message: 'a>(
    catalog: &'a Catalog,
    theme: &'a Theme,
    path_field: &'a str,
    path_field_notice: Option<String>,
) -> Element<'a, Message> {
    let lines = column![
        text(catalog.get("project-board-empty-heading")).size(theme.font_size_heading()),
        text(catalog.get("project-board-empty-open-a-project")).size(theme.font_size_body()),
        text(catalog.get("project-board-empty-command-example")).size(theme.font_size_body()),
        path_field_section(catalog, theme, path_field, path_field_notice),
    ]
    .spacing(6);

    container(lines)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(16)
        .into()
}

/// The path field itself -- label, box, and its optional failure notice
/// -- factored out so both [`empty_state_view`] and `view`'s own
/// populated-board branch (RFC-038 PR-038-B's `Ctrl+Alt+O` case) render
/// the exact same field, rather than two copies that could drift.
///
/// **Not `iced::widget::text_input`.** This project routes every
/// keystroke through one reviewed router (`input::route_non_modal_input`)
/// so global keybindings always win and a modal can suppress input
/// structurally, not by a guard a future surface could forget.
/// `text_input` maintains its own internal keyboard capture independent
/// of that router -- introducing it here would open a second,
/// unreviewed path for a keystroke to reach the application, exactly
/// what `input`'s module doc says this crate does not do anywhere else.
/// The field is instead a plain rendering of `path_field`, a `String`
/// `shell.rs` owns and appends to one `KeyPress` at a time, the same
/// shape [`crate::surface::editor::apply_edit_key`] already established
/// for the (multi-line) editor.
///
/// `path_field` is untyped, untrusted input -- routed through
/// `text_safety::quote_untrusted` here, same as every other
/// filesystem-derived string this module renders, never handed to
/// `text(...)` raw.
fn path_field_section<'a, Message: 'a>(
    catalog: &'a Catalog,
    theme: &'a Theme,
    path_field: &'a str,
    path_field_notice: Option<String>,
) -> Element<'a, Message> {
    let field_box =
        container(text(path_field_display_text(path_field)).size(theme.font_size_body()))
            .width(Length::Fill)
            .padding(8)
            .style(
                move |_base_theme: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(theme.surface_elevated())),
                    text_color: Some(theme.foreground()),
                    border: iced::Border {
                        color: theme.border_focused(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..iced::widget::container::Style::default()
                },
            );

    let mut lines = column![
        text(catalog.get("project-board-path-field-label")).size(theme.font_size_body()),
        field_box,
    ]
    .spacing(6);

    // `path_field_notice` is already a fully-resolved, catalog-rendered
    // string (`shell::path_field_error_text`) -- this module renders it
    // as-is, the same "notice computed by `shell.rs`, rendered by the
    // surface" division `terminal_launch_refusal_text` already uses for
    // the terminal workspace's own launch-refusal notice.
    if let Some(notice) = path_field_notice {
        lines = lines.push(text(notice).size(theme.font_size_body()));
    }

    lines.into()
}

/// The path field's own rendered content -- factored out of
/// [`empty_state_view`] so it is directly testable without going
/// through `iced`'s `Element` tree, the same shape [`row_lines`] and
/// `shell::status_bar_summary` already use. `U+2588 FULL BLOCK` is a
/// static trailing cursor marker (this crate has no live text-editing
/// caret today -- see `empty_state_view`'s own doc for why not
/// `iced::widget::text_input`), appended *after* escaping so it can
/// never be mistaken for part of the typed value.
pub(crate) fn path_field_display_text(path_field: &str) -> String {
    format!(
        "{}\u{2588}",
        text_safety::quote_untrusted(path_field).as_str()
    )
}

fn row_view<'a, Message: 'a>(
    row: &ProjectBoardRow,
    catalog: &'a Catalog,
    theme: &'a Theme,
) -> Element<'a, Message> {
    let lines: Vec<Element<'a, Message>> = row_lines(row, catalog)
        .into_iter()
        .map(|line| text(line).size(theme.font_size_status()).into())
        .collect();

    container(column(lines).spacing(2))
        .width(Length::Fill)
        .padding(8)
        .style(
            move |_base_theme: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme.surface_elevated())),
                text_color: Some(theme.foreground()),
                border: iced::Border {
                    color: theme.border_default(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..iced::widget::container::Style::default()
            },
        )
        .into()
}

/// Every string a row displays, in order -- factored out from
/// [`row_view`] so the actual rendered content (escaping, catalog
/// selection) is directly testable without going through `iced`'s
/// `Element` tree, the same shape as `shell::status_bar_summary`.
pub(crate) fn row_lines(row: &ProjectBoardRow, catalog: &Catalog) -> Vec<String> {
    let name = text_safety::quote_untrusted(&row.display_name);
    let root_path = text_safety::quote_untrusted(&row.root_path_hint);

    let mut lines = vec![
        name.as_str().to_string(),
        root_path.as_str().to_string(),
        row.trust_label.clone(),
        catalog.get_with_args(
            "project-board-branch-status",
            &count_display_args(CatalogArgs::new(), "status", row.branch_status),
        ),
        catalog.get_with_args(
            "project-board-terminal-count",
            &count_display_args(CatalogArgs::new(), "count", row.terminal_count),
        ),
        catalog.get_with_args(
            "project-board-agent-run-count",
            &count_display_args(CatalogArgs::new(), "count", row.agent_run_count),
        ),
        catalog.get_with_args(
            "project-board-approval-count",
            &count_display_args(CatalogArgs::new(), "count", row.approval_count),
        ),
        catalog.get_with_args(
            "project-board-review-count",
            &count_display_args(CatalogArgs::new(), "count", row.review_count),
        ),
        catalog.get_with_args(
            "project-board-dirty-file-count",
            &count_display_args(CatalogArgs::new(), "count", row.dirty_file_count),
        ),
        catalog.get_with_args(
            "project-board-attention",
            &CatalogArgs::new().trusted_symbol("attention", attention_symbol(row.attention)),
        ),
    ];

    if row.blocked_automation_count > 0 {
        lines.push(catalog.get_with_args(
            "blocked-automation-count",
            &CatalogArgs::new().number("count", row.blocked_automation_count),
        ));
    }

    lines
}

/// The symbolic literal-variant names every `CountDisplay`-shaped
/// catalog key selects on -- shared across `blocked-automation-count`
/// (PR-016-D) and every `project-board-*-count` key here, so a reader
/// learns the vocabulary once.
fn count_display_args<'a>(
    args: CatalogArgs<'a>,
    name: &'a str,
    value: CountDisplay,
) -> CatalogArgs<'a> {
    match value {
        CountDisplay::KnownCount(count) => args.number(name, count),
        CountDisplay::Unavailable => args.trusted_symbol(name, "unavailable"),
        CountDisplay::NotImplemented => args.trusted_symbol(name, "not_implemented"),
        CountDisplay::Unknown => args.trusted_symbol(name, "unknown"),
    }
}

fn attention_symbol(attention: AttentionState) -> &'static str {
    match attention {
        AttentionState::Risk => "risk",
        AttentionState::ApprovalNeeded => "approval_needed",
        AttentionState::Review => "review",
        AttentionState::Failed => "failed",
        AttentionState::Running => "running",
        AttentionState::Dirty => "dirty",
        AttentionState::Calm => "calm",
    }
}

#[cfg(test)]
mod tests;
