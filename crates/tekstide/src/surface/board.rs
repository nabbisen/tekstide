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
) -> Element<'a, Message> {
    if let Some(_empty_state) = &view_model.empty_state {
        return empty_state_view(catalog, theme);
    }

    let rows: Vec<Element<'a, Message>> = view_model
        .rows
        .iter()
        .map(|row| row_view(row, catalog, theme))
        .collect();

    container(column(rows).spacing(12))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(16)
        .into()
}

fn empty_state_view<'a, Message: 'a>(
    catalog: &'a Catalog,
    theme: &'a Theme,
) -> Element<'a, Message> {
    container(
        column![
            text(catalog.get("project-board-empty-heading")).size(theme.font_size_heading()),
            text(catalog.get("project-board-empty-primary-action")).size(theme.font_size_body()),
            text(catalog.get("project-board-empty-secondary-action")).size(theme.font_size_body()),
        ]
        .spacing(6),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(16)
    .into()
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
