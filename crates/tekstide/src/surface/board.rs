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
//! - `CountDisplay` fields (`terminal_count`, `agent_run_count`,
//!   `approval_count`, `review_count`, `dirty_file_count`) are routed
//!   through the catalog via [`count_display_args`], never through
//!   `CountDisplay::label()`. This is response 130's explicit decision
//!   point: `label()` returns hardcoded English for three of four
//!   variants, and this surface is the one RFC-005/RFC-015 required to
//!   preserve "`Unavailable`/`NotImplemented` never render as `0`" --
//!   calling `label()` here would both hardcode English at the render
//!   layer and be the easiest way to quietly fail that requirement.
//!   `label()` keeps its existing role in
//!   `tekstide_core::shell::render_text` (the pre-GUI harness, kept --
//!   see `qa-evidence.md`); this surface does not call it at all.
//! - `branch_status` (`BranchDisplay`, RFC-030 PR-030-C) is the same
//!   shape as the `CountDisplay` fields above, routed through
//!   [`branch_display_args`] instead of `count_display_args` since its
//!   `Known` variant carries an untrusted branch name rather than a
//!   trusted count -- `quote_untrusted` first, the same as
//!   `display_name`/`root_path_hint` above, not `trusted_symbol`.
//! - `attention` (an `AttentionState` enum, not just `attention_label:
//!   String`) is also routed through the catalog, via
//!   [`attention_symbol`], for the same reason: the enum is available,
//!   so there is no reason to fall back to core's pre-baked English.
//! - `blocked_automation_labels` are shown by name (RFC-053 D7), after the
//!   count, through `project-board-blocked-automation-names`.
//! - `trust_label`, `security_mode_label`, `availability_label` have **no
//!   underlying enum exposed** in
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

use tekstide_core::project::ProjectId;
use tekstide_core::project_board::{
    AttentionState, BoardRowKind, BranchDisplay, CountDisplay, ProjectBoardRow,
    ProjectBoardViewModel,
};
use tekstide_core::text_safety;

use iced::widget::{column, container, scrollable, text};
use iced::{Element, Length};

use crate::i18n::{Catalog, CatalogArgs};
use crate::theme::Theme;

/// The id of the card list's scroll area, for `shell` to snap it to the
/// keyboard highlight.
pub(crate) const BOARD_SCROLL_ID: &str = "project-board-cards";

fn board_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new(BOARD_SCROLL_ID)
}

/// Where the scroll area should sit so that card `index` of `count` is in
/// view: the cards are close to equal height, so the fraction of the way down
/// the list is the fraction of the way down the scroll. `None` for a single
/// card (nothing to scroll).
pub(crate) fn scroll_fraction(index: usize, count: usize) -> Option<f32> {
    (count > 1).then(|| index.min(count - 1) as f32 / (count - 1) as f32)
}

// RFC-038 PR-038-D: nine parameters -- the path-field trio
// (PR-038-A/B/G) plus the row-highlight/reopen pair this slice adds.
// Matches this crate's own existing precedent for a real, already-wide
// parameter list over a grouping struct invented only to satisfy the
// lint (`audit/integration.rs`, `approval/coordinator.rs`).
#[allow(clippy::too_many_arguments)]
pub fn view<'a, Message: 'a + Clone>(
    view_model: &ProjectBoardViewModel,
    catalog: &'a Catalog,
    theme: &'a Theme,
    path_field: &'a str,
    path_field_notice: Option<String>,
    show_field_on_populated_board: bool,
    open_browser_message: Message,
    row_highlight: usize,
    reopen_project_message: impl Fn(ProjectId) -> Message + 'a,
) -> Element<'a, Message> {
    if let Some(_empty_state) = &view_model.empty_state {
        return empty_state_view(
            catalog,
            theme,
            path_field,
            path_field_notice,
            open_browser_message,
        );
    }

    let rows: Vec<Element<'a, Message>> = view_model
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            // RFC-038 PR-038-D: `Recent*`-kind rows get a real, clickable
            // "Open" button, the same `is_live`-gated shape
            // `shell::approval_history_entry_view` already uses for its
            // own list -- an `ActiveSession` row is already open, so it
            // gets none. `Message` is constructed once here, not inside
            // `row_view`, so that pure function stays free of any
            // `ProjectId`-shaped decision about *which* message to build.
            let open_message = (row.row_kind != BoardRowKind::ActiveSession)
                .then(|| reopen_project_message(row.project_id.clone()));
            row_view(row, catalog, theme, index == row_highlight, open_message)
        })
        .collect();

    // RFC-052 PR-052-C's live capture found this: with more cards than the
    // window is tall, the column ran off the bottom -- the last card's "Open"
    // button was cut through, and the board said "15 projects" while showing
    // three. The cards scroll; `shell` snaps the scroll to the keyboard
    // highlight (`BOARD_SCROLL_ID`). The path field stays pinned below.
    let mut sections: Vec<Element<'a, Message>> = vec![
        scrollable(container(column(rows).spacing(12)).padding(iced::Padding {
            right: 12.0,
            ..iced::Padding::ZERO
        }))
        .id(board_scroll_id())
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    ];

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
            open_browser_message,
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
fn empty_state_view<'a, Message: 'a + Clone>(
    catalog: &'a Catalog,
    theme: &'a Theme,
    path_field: &'a str,
    path_field_notice: Option<String>,
    open_browser_message: Message,
) -> Element<'a, Message> {
    // Release gate finding, 0.13.0 ("the first screen leads with the command
    // line"): the path field and Browse button -- what actually works
    // *inside* this window -- must render before the command-line mention,
    // not after it. Before this fix, a first-time user reading top-down hit
    // "start Tekstide with its path" (an instruction to leave the app) two
    // lines before reaching the field and button that make leaving
    // unnecessary. The strings were each individually true; the screen read
    // wrong. Demoted, not removed -- `tekstide /path/to/project` is still
    // genuinely useful and someone who learned it from the README should
    // find it confirmed here, just no longer first.
    let lines = column![
        text(catalog.get("project-board-empty-heading")).size(theme.font_size_heading()),
        path_field_section(
            catalog,
            theme,
            path_field,
            path_field_notice,
            open_browser_message
        ),
        text(catalog.get("project-board-empty-open-a-project")).size(theme.font_size_body()),
        text(catalog.get("project-board-empty-command-example")).size(theme.font_size_body()),
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
fn path_field_section<'a, Message: 'a + Clone>(
    catalog: &'a Catalog,
    theme: &'a Theme,
    path_field: &'a str,
    path_field_notice: Option<String>,
    open_browser_message: Message,
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

    // RFC-038 PR-038-G: the real, mouse-clickable control the owner's
    // D1 overturn asked for -- "a button, not only a key." Safe to be a
    // genuine `iced::widget::button` (unlike a keyboard-routed control,
    // this crate's "one reviewed router" principle is specifically
    // about *keyboard* input; mouse clicks were never part of that
    // threat model, so `iced`'s own native click dispatch is the
    // ordinary, unproblematic path here). `Ctrl+Alt+B` is the same
    // action's keyboard accelerator, not the only route to it --
    // `shell::open_folder_browser` is what both converge on.
    let browse_button = iced::widget::button(
        text(catalog.get("project-board-browse-button")).size(theme.font_size_body()),
    )
    .on_press(open_browser_message);

    let mut lines = column![
        text(catalog.get("project-board-path-field-label")).size(theme.font_size_body()),
        field_box,
        browse_button,
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

/// A card's text regrouped into the reading order a person scans in: **who**
/// (name, then where it lives), **what state** (trust, branch, attention), and
/// **how much** (the counts) -- instead of nine lines of the same size and
/// weight. Built from the very same catalog strings [`row_lines`] returns, so
/// regrouping cannot change what a card says (pinned by
/// `a_cards_sections_say_exactly_what_its_lines_say`).
pub(crate) struct RowSections {
    pub(crate) name: String,
    pub(crate) path: String,
    /// Trust, branch, attention: the state of the project, one badge each.
    pub(crate) state: Vec<String>,
    /// Terminals, agent runs, approvals, reviews, unsaved files, and the
    /// blocked-automation count when there is one: one badge each.
    pub(crate) counts: Vec<String>,
    /// "blocked: automatic LSP startup, ...", when there are any.
    pub(crate) blocked_names: Option<String>,
}

/// Whether a count is a fact. `Unavailable`, `NotImplemented` and `Unknown`
/// are each honest ("there is no session to count") but **carry no fact**, and
/// as badges they were the loudest thing on a card (review 424, B1).
fn count_is_known(count: CountDisplay) -> bool {
    matches!(count, CountDisplay::KnownCount(_))
}

/// `highlighted` decides whether the blocked-automation **names** are shown:
/// the list is the same for every Restricted project, which is every project
/// by default, so it was the same sentence repeated on every card (review 424,
/// B2). It is shown on the card the keyboard is on -- the count stays on every
/// card -- so what is blocked is always one key press away and never silently
/// absent (`REQ-NOTIFY-003`).
///
/// **A project that is not open says so once** instead of five "unknown"
/// badges: it has no session to count, so its counts, its attention word and
/// an unavailable branch are left out, and one badge says why.
pub(crate) fn row_sections(
    row: &ProjectBoardRow,
    catalog: &Catalog,
    highlighted: bool,
) -> RowSections {
    let mut lines = row_lines(row, catalog).into_iter();
    let name = lines.next().unwrap_or_default();
    let path = lines.next().unwrap_or_default();
    let trust = lines.next().unwrap_or_default();
    let branch = lines.next().unwrap_or_default();
    let counts: Vec<String> = lines.by_ref().take(5).collect();
    let attention = lines.next().unwrap_or_default();
    let rest: Vec<String> = lines.collect();
    // `row_lines` appends the blocked count, then (if any) the names.
    let (blocked, blocked_names) = match rest.len() {
        0 => (None, None),
        1 if row.blocked_automation_labels.is_empty() => (rest.first().cloned(), None),
        1 => (None, rest.first().cloned()),
        _ => (rest.first().cloned(), rest.get(1).cloned()),
    };
    let blocked_names = blocked_names.filter(|_| highlighted);

    if row.row_kind == BoardRowKind::ActiveSession {
        let mut counts = counts;
        counts.extend(blocked);
        return RowSections {
            name,
            path,
            state: vec![trust, branch, attention],
            counts,
            blocked_names,
        };
    }

    let fields = [
        row.terminal_count,
        row.agent_run_count,
        row.approval_count,
        row.review_count,
        row.dirty_file_count,
    ];
    let any_unknown = fields.iter().any(|count| !count_is_known(*count));
    let mut shown: Vec<String> = Vec::new();
    if any_unknown {
        shown.push(catalog.get("project-board-not-open"));
    }
    shown.extend(
        counts
            .into_iter()
            .zip(fields)
            .filter(|(_, count)| count_is_known(*count))
            .map(|(line, _)| line),
    );
    shown.extend(blocked);
    let mut state = vec![trust];
    if matches!(
        row.branch_status,
        BranchDisplay::Known(_) | BranchDisplay::Detached
    ) {
        state.push(branch);
    }
    RowSections {
        name,
        path,
        state,
        counts: shown,
        blocked_names,
    }
}

fn bold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Bold,
        ..iced::Font::DEFAULT
    }
}

/// Anything but `Calm` is worth a second look, so it gets a heavier border --
/// **in addition to its word** ("Risk", "Failed", "Approval needed"), never
/// instead of it (NFR-UX-002).
fn attention_stands_out(attention: AttentionState) -> bool {
    attention != AttentionState::Calm
}

/// One fact as a badge: its words in a small bordered pill. **A badge is a
/// container for a word, not a replacement for one** -- every badge's meaning
/// is its text, so the board stays readable with no colour at all (D4). They
/// wrap onto further lines rather than overflow a narrow window.
///
/// `emphasise_last` gives the final badge (attention, on the state row) a
/// heavier border when the project needs a second look.
fn badges<'a, Message: 'a>(
    facts: Vec<String>,
    theme: &'a Theme,
    emphasise_last: bool,
) -> Element<'a, Message> {
    let last = facts.len().saturating_sub(1);
    let pills: Vec<Element<'a, Message>> = facts
        .into_iter()
        .enumerate()
        .map(|(index, fact)| {
            let heavy = emphasise_last && index == last;
            container(text(fact).size(theme.font_size_status()))
                .padding(iced::Padding {
                    top: 3.0,
                    right: 9.0,
                    bottom: 3.0,
                    left: 9.0,
                })
                .style(
                    move |_base_theme: &iced::Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(theme.background())),
                        text_color: Some(theme.foreground()),
                        border: iced::Border {
                            color: if heavy {
                                theme.border_focused()
                            } else {
                                theme.border_default()
                            },
                            width: if heavy { 2.0 } else { 1.0 },
                            radius: 10.0.into(),
                        },
                        ..iced::widget::container::Style::default()
                    },
                )
                .into()
        })
        .collect();
    iced::widget::Row::with_children(pills)
        .spacing(6)
        .wrap()
        .vertical_spacing(6)
        .into()
}

fn row_view<'a, Message: 'a + Clone>(
    row: &ProjectBoardRow,
    catalog: &'a Catalog,
    theme: &'a Theme,
    highlighted: bool,
    open_message: Option<Message>,
) -> Element<'a, Message> {
    let sections = row_sections(row, catalog, highlighted);
    let marker = if highlighted { "> " } else { "  " };

    // The name is the heading: larger and bold. Everything under it is
    // smaller, so a card's first line is what a person's eye lands on.
    let mut column_items: Vec<Element<'a, Message>> = vec![
        text(sections.name)
            .size(theme.font_size_heading())
            .font(bold())
            .into(),
        text(sections.path).size(theme.font_size_status()).into(),
        badges(sections.state, theme, attention_stands_out(row.attention)),
        badges(sections.counts, theme, false),
    ];
    if let Some(names) = sections.blocked_names {
        column_items.push(text(names).size(theme.font_size_status()).into());
    }

    // RFC-038 PR-038-D: a real, clickable "Open" button on every
    // `Recent*`-kind row -- the same `is_live`-gated shape
    // `shell::approval_history_entry_view` already uses, present
    // regardless of which row is highlighted (the highlight is a
    // keyboard cursor, not a precondition for the mouse).
    //
    // **The open project's card says so instead of being silent.** It has no
    // button because it is already open, but a card with no button beside
    // cards that have one reads as broken; it now says "Open now".
    match open_message {
        Some(message) => column_items.push(
            iced::widget::button(
                text(catalog.get("project-board-recent-open-button")).size(theme.font_size_body()),
            )
            .on_press(message)
            .into(),
        ),
        None => column_items.push(
            text(catalog.get("project-board-active-marker"))
                .size(theme.font_size_body())
                .font(bold())
                .into(),
        ),
    }

    // The keyboard highlight is `> ` on the name (a word-shaped channel) and,
    // as a second channel, a thicker border in the focus colour.
    let border = if highlighted {
        (theme.border_focused(), 2.0)
    } else {
        (theme.border_default(), 1.0)
    };
    // The marker sits in its own fixed-width column so every card's text lines
    // up whether or not it carries the `> `.
    let body = iced::widget::row![
        text(marker)
            .width(Length::Fixed(24.0))
            .size(theme.font_size_heading()),
        column(column_items).spacing(6).width(Length::Fill),
    ];
    container(body)
        .width(Length::Fill)
        .padding(12)
        .style(
            move |_base_theme: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme.surface_elevated())),
                text_color: Some(theme.foreground()),
                border: iced::Border {
                    color: border.0,
                    width: border.1,
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
            &branch_display_args(CatalogArgs::new(), "status", &row.branch_status),
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
    // RFC-053 D7: name what is blocked, from the labels the model already
    // carries (REQ-NOTIFY-003 -- a bare count is not actionable). The
    // labels are a fixed, trusted set, but `CatalogArgs` has no
    // runtime-string trusted argument (`trusted_symbol` is `&'static str`
    // only), so they go through `untrusted` -- `quote_untrusted` on plain
    // ASCII adds only invisible isolate marks, and this keeps the string
    // in the catalog rather than concatenated in Rust.
    if !row.blocked_automation_labels.is_empty() {
        let names = text_safety::quote_untrusted(&row.blocked_automation_labels.join(", "));
        lines.push(catalog.get_with_args(
            "project-board-blocked-automation-names",
            &CatalogArgs::new().untrusted("names", &names),
        ));
    }

    lines
}

/// [`row_lines`] plus the keyboard cursor's own marker on the name
/// line, exactly the "> "/"  " convention `surface/explorer.rs`'s own
/// `tree_lines`/`browse_view` already use (not `shell::
/// focus_marker`: that helper is private to `shell.rs`, and every
/// existing render module here keeps its own copy of the same literal
/// rather than reaching across the surface/shell boundary for it).
/// Factored out
/// from [`row_view`] for the same testability reason `row_lines` itself
/// already is: the rendered string, not the `Element` tree.
#[cfg(test)]
pub(crate) fn highlighted_row_lines(
    row: &ProjectBoardRow,
    catalog: &Catalog,
    highlighted: bool,
) -> Vec<String> {
    let mut lines = row_lines(row, catalog);
    if let Some(first) = lines.first_mut() {
        *first = format!("{}{first}", if highlighted { "> " } else { "  " });
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

/// RFC-030 PR-030-C, review 411 R-c: `branch_status`'s own args builder,
/// not `count_display_args` -- `BranchDisplay::Known` carries an
/// untrusted branch name, not a trusted count. `project-board-branch-status`'s
/// own `*[other]` arm interpolates whatever `$status` resolves to when it
/// matches none of the fixed symbols, which is exactly how a
/// `quote_untrusted`-wrapped branch name reaches the rendered line: it is
/// never going to equal `not_implemented`/`unavailable`/`detached`/
/// `unknown`, so it always falls through to that arm.
fn branch_display_args<'a>(
    args: CatalogArgs<'a>,
    name: &'a str,
    value: &BranchDisplay,
) -> CatalogArgs<'a> {
    match value {
        BranchDisplay::Known(branch_name) => {
            args.untrusted(name, &text_safety::quote_untrusted(branch_name))
        }
        BranchDisplay::Detached => args.trusted_symbol(name, "detached"),
        BranchDisplay::Unavailable => args.trusted_symbol(name, "unavailable"),
        BranchDisplay::NotImplemented => args.trusted_symbol(name, "not_implemented"),
        BranchDisplay::Unknown => args.trusted_symbol(name, "unknown"),
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
