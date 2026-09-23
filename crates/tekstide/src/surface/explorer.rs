//! RFC-019 PR-019-B: the explorer tree.
//!
//! Renders `tekstide_core::project::root::ExplorerDirectoryScan` -- the
//! second surface after `board.rs`, and the direct analogue of RFC-017's
//! grid-not-chrome boundary: **every rendered name, path hint, and
//! status is untrusted, and goes through `text_safety::quote_untrusted`
//! before it reaches the screen.** RFC-019 named, in advance, four
//! hardcoded-English free functions this slice would be tempted to call
//! directly (`explorer_node_kind_label`, `explorer_node_state_label`,
//! `explorer_symlink_status_label`, and the sibling
//! `text_document_state_label` PR-019-C owns). None of the four is
//! called anywhere in this module -- every word renders through
//! `Catalog` instead.
//!
//! No filesystem walking happens here. **RFC-052 PR-052-B: the explorer is a
//! tree.** `tekstide_core::project::ExplorerTree` holds the scans and which
//! folders are open; this module turns its rows into text (`row_text`,
//! assertable without `iced`) and draws **only the window of rows that fit**
//! (`window_for`, `rows_that_fit`) -- the total-row bound, decided by
//! measurement in PR-052-B: virtualisation, not a cap, so nothing is dropped
//! and a line says which rows are on screen. A row says what it *is* before
//! its name (a clipped line must lose the name, not the status), and a detail
//! area under the tree shows the highlighted row whole.
//!
//! **RFC-030 PR-030-C, REQ-GIT-003: per-file Git status, one exact-path
//! lookup per node, no rollup.** `node_line` looks up
//! `ProjectGitSummary::file_status(&node.relative_path)` for its own row
//! only -- a directory node's badge (if any) is exactly what `git status`
//! itself reported *for that literal path*, never an aggregate computed
//! by walking the map for every descendant. Decided explicitly, not left
//! implicit: a per-node lookup is the same shape `git_state_fields`
//! (PR-030-B's status bar) already uses, and walking the whole map for
//! every directory row on every render is a real cost this slice does
//! not need to pay. One case reads as a rollup without being one: a
//! directory that is *entirely* untracked is exactly what `git status`
//! collapses into a single `?? dir/` record (its default, unchanged
//! `--untracked-files` mode -- the same mode PR-030-B's status bar count
//! already depends on, deliberately left alone here so that count's
//! meaning does not silently shift), and a `PathBuf` built from
//! `"dir/"` compares equal to one built from `"dir"`, so that directory's
//! own row gets an `Untracked` badge for free. Once anything inside such
//! a directory is tracked or staged, `git` stops collapsing it and
//! enumerates its contents individually -- at that point the directory
//! row itself carries no badge again, and each file inside carries its
//! own, exactly as a plain per-node lookup would produce with no special
//! casing at all.

use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, text};
use iced::{Element, Length};

use tekstide_core::project::root::{
    BrowseNode, BrowseNodeState, DirectoryBrowseScan, ExplorerNode, ExplorerNodeKind,
    ExplorerNodeState, FileAccessSymlinkStatus,
};
use tekstide_core::project::{
    ExplorerTree, ExplorerTreeRow, ExplorerTreeRowKind, ProjectExplorerStatus,
};
use tekstide_core::project::{FileGitStatus, ProjectGitSummary};
use tekstide_core::text_safety;

use crate::i18n::{Catalog, CatalogArgs};
use crate::theme::Theme;

fn node_kind_symbol(kind: ExplorerNodeKind) -> &'static str {
    match kind {
        ExplorerNodeKind::Directory => "directory",
        ExplorerNodeKind::Other => "other",
        ExplorerNodeKind::File => "file",
    }
}

fn node_state_symbol(state: &ExplorerNodeState) -> &'static str {
    match state {
        ExplorerNodeState::Available => "available",
        ExplorerNodeState::Collapsed => "collapsed",
        ExplorerNodeState::Blocked(_) => "blocked",
        ExplorerNodeState::Unreadable => "unreadable",
    }
}

fn symlink_status_symbol(status: FileAccessSymlinkStatus) -> &'static str {
    match status {
        FileAccessSymlinkStatus::NoSymlink => "none",
        FileAccessSymlinkStatus::InRootSymlink => "in-root",
        FileAccessSymlinkStatus::UnresolvedSymlink => "unresolved",
        FileAccessSymlinkStatus::EscapesRoot => "escapes-root",
    }
}

/// `None` (no project Git summary, a project outside any repository, a
/// refused/branch-only repository, or simply no entry for this exact
/// path) all read as `"none"` -- the same "absent means nothing to
/// report" convention `git_state_fields` already uses for the status
/// bar's own fields.
fn git_status_symbol(status: Option<FileGitStatus>) -> &'static str {
    match status {
        Some(FileGitStatus::Modified) => "modified",
        Some(FileGitStatus::Added) => "added",
        Some(FileGitStatus::Deleted) => "deleted",
        Some(FileGitStatus::Renamed) => "renamed",
        Some(FileGitStatus::Untracked) => "untracked",
        Some(FileGitStatus::Unmerged) => "unmerged",
        None => "none",
    }
}

/// The one line a node renders as, factored out from [`view`] so the
/// escaping and catalog routing are directly testable without `iced` --
/// the same split `board.rs::row_lines` and `session_bar.rs::entry_text`
/// use. `node.name` is untrusted (a repository can name a file anything,
/// including a bidi-override sequence); escaped before it reaches the
/// catalog, never passed to `trusted_symbol` (which is `&'static str`
/// only, so a runtime name would not even compile there). `git_summary`
/// is the active project's own (`None` when there is no active project,
/// or -- same value, same rendering -- when a project simply has none
/// computed yet); the per-node lookup itself is
/// [`ProjectGitSummary::file_status`], this module's own doc comment
/// covers why it is exact-path with no rollup.
///
/// `expanded` is RFC-052's one addition: a directory on the collapse list
/// (`.git`, `node_modules`, `target`) reads `(collapsed)` while it is
/// closed, and stops saying so once the user has opened it -- the word
/// would otherwise contradict the rows under it.
pub(crate) fn node_line(
    catalog: &Catalog,
    node: &ExplorerNode,
    expanded: bool,
    git_summary: Option<&ProjectGitSummary>,
) -> String {
    let name = text_safety::quote_untrusted(&node.name);
    let git_status = git_summary.and_then(|summary| summary.file_status(&node.relative_path));
    let state = if expanded && node.state == ExplorerNodeState::Collapsed {
        "available"
    } else {
        node_state_symbol(&node.state)
    };
    catalog.get_with_args(
        "explorer-node-entry",
        &CatalogArgs::new()
            .trusted_symbol("kind", node_kind_symbol(node.kind))
            .untrusted("name", &name)
            .trusted_symbol("state", state)
            .trusted_symbol("symlink", symlink_status_symbol(node.symlink_status))
            .trusted_symbol("git", git_status_symbol(git_status)),
    )
}

/// Two spaces per level. Depth is also carried by where the row sits under
/// its parent, so this is a reinforcement, not the only channel.
const INDENT_PER_LEVEL: &str = "  ";

/// What comes before a row's text: `[+]` a closed folder, `[-]` an open
/// one, nothing for anything that cannot be toggled. **Characters, not an
/// icon** (D4: an icon never carries meaning alone -- RFC-052 PR-052-C
/// decides what may reinforce it), padded so names line up.
fn expansion_marker(expandable: bool, expanded: bool) -> &'static str {
    match (expandable, expanded) {
        (false, _) => "    ",
        (true, false) => "[+] ",
        (true, true) => "[-] ",
    }
}

/// The text of one tree row, **without** the keyboard-highlight marker:
/// indentation, expansion marker, then the escaped, catalog-routed row.
/// This is the one function that decides what a row says, so the
/// escaping and i18n tests hold the same properties they held when a row
/// was one Fluent string (RFC-052 §7): every name reaches the screen
/// through [`node_line`]'s `quote_untrusted`, whoever calls this.
pub(crate) fn row_text(
    catalog: &Catalog,
    row: &ExplorerTreeRow<'_>,
    git_summary: Option<&ProjectGitSummary>,
) -> String {
    let indent = INDENT_PER_LEVEL.repeat(row.depth);
    match row.kind {
        ExplorerTreeRowKind::Node {
            node,
            expanded,
            expandable,
        } => format!(
            "{indent}{}{}",
            expansion_marker(expandable, expanded),
            node_line(catalog, node, expanded, git_summary)
        ),
        ExplorerTreeRowKind::Loading => {
            format!("{indent}    {}", catalog.get("explorer-row-loading"))
        }
        ExplorerTreeRowKind::CannotRead => {
            format!("{indent}    {}", catalog.get("explorer-row-cannot-read"))
        }
        ExplorerTreeRowKind::Empty => format!("{indent}    {}", catalog.get("explorer-empty")),
        ExplorerTreeRowKind::Omitted { count, at_least } => format!(
            "{indent}    {}",
            catalog.get_with_args(
                "explorer-omitted-entries",
                &CatalogArgs::new()
                    .number("count", count)
                    .trusted_symbol("bound", if at_least { "at-least" } else { "exact" }),
            )
        ),
        ExplorerTreeRowKind::RowsNotShown { count } => catalog.get_with_args(
            "explorer-rows-not-shown",
            &CatalogArgs::new().number("count", count),
        ),
    }
}

/// `ProjectExplorerStatus::Error`'s own message embeds the target's
/// relative path (`ExplorerScanError`'s `Display`) -- attacker-influenced,
/// the same class as a node name. Escaped exactly like one before it
/// reaches the catalog.
fn status_line(catalog: &Catalog, status: &ProjectExplorerStatus) -> Option<String> {
    match status {
        ProjectExplorerStatus::Error { message } => {
            let escaped = text_safety::quote_untrusted(message);
            Some(catalog.get_with_args(
                "explorer-status-error",
                &CatalogArgs::new().untrusted("message", &escaped),
            ))
        }
        ProjectExplorerStatus::Empty | ProjectExplorerStatus::Ready => None,
    }
}

/// The rows of a tree that fit a window, and where the window starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowWindow {
    /// Index of the first row drawn.
    pub(crate) top: usize,
    /// One past the last row drawn.
    pub(crate) end: usize,
}

/// Which rows to draw so that `highlight` is on screen and the window
/// moves as little as possible from where it was (`top`): moving the
/// highlight inside the window does not scroll it. Pure, so the scrolling
/// rule is asserted as a rule -- and the tree can be any length, because
/// **only the window is ever built into widgets** (RFC-052 D8: the total
/// row bound, decided by measurement, is virtualisation).
pub(crate) fn window_for(total: usize, highlight: usize, top: usize, capacity: usize) -> RowWindow {
    let capacity = capacity.max(1);
    if total <= capacity {
        return RowWindow { top: 0, end: total };
    }
    let highlight = highlight.min(total - 1);
    let mut top = top.min(total - capacity);
    if highlight < top {
        top = highlight;
    } else if highlight >= top + capacity {
        top = highlight + 1 - capacity;
    }
    RowWindow {
        top,
        end: top + capacity,
    }
}

/// Every line the explorer renders, in order: the status line (if any), the
/// windowed rows with their focus marker, and -- **whenever rows are not
/// all on screen -- a line saying which ones are** (RFC-052: nothing is
/// hidden silently). Factored out from [`view`] for the same testability
/// reason as [`node_line`].
pub(crate) fn tree_lines(
    catalog: &Catalog,
    tree: &ExplorerTree,
    status: &ProjectExplorerStatus,
    highlight: usize,
    top: usize,
    capacity: usize,
    git_summary: Option<&ProjectGitSummary>,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(message) = status_line(catalog, status) {
        lines.push(message);
    }
    let rows = tree.rows();
    if rows.is_empty() {
        lines.push(catalog.get("explorer-empty"));
        return lines;
    }
    let window = window_for(rows.len(), highlight, top, capacity);
    for (index, row) in rows.iter().enumerate().take(window.end).skip(window.top) {
        let marker = if index == highlight { "> " } else { "  " };
        lines.push(format!("{marker}{}", row_text(catalog, row, git_summary)));
    }
    if window.end - window.top < rows.len() {
        lines.push(
            catalog.get_with_args(
                "explorer-rows-position",
                &CatalogArgs::new()
                    .number("first", window.top + 1)
                    .number("last", window.end)
                    .number("total", rows.len()),
            ),
        );
    }
    lines
}

/// Where the keyboard highlight is and which rows fit: what `view` needs
/// to draw the window.
#[derive(Clone, Copy, Debug)]
pub struct ExplorerCursor {
    pub highlight: usize,
    pub top: usize,
    pub capacity: usize,
}

/// Lines the sidebar keeps for things that are not tree rows (the status
/// line and the "rows N-M of T" line), so [`rows_that_fit`] leaves room.
const RESERVED_LINES: usize = 2;
/// The **detail** area under the tree: the highlighted row in full, wrapped
/// over this many lines. A row is drawn on one line and clipped at the
/// sidebar's edge, so a long name, or a chain of status words on a nested row
/// (`[OTHER] (blocked) [symlink escapes root]`), can be cut off; this is
/// where the row under the highlight is always readable whole. **Fixed
/// height**, so the window arithmetic in [`rows_that_fit`] stays exact
/// whatever the row says.
pub(crate) const DETAIL_LINES: usize = 3;
/// `iced`'s `text` line height is `1.3` times its size by default, and the
/// column below spaces lines by [`LINE_SPACING`]; the window arithmetic
/// uses the same two numbers the drawing does, or the last row would be
/// clipped (the RFC-053 lesson: one definition, two users).
const LINE_HEIGHT_FACTOR: f32 = 1.3;
const LINE_SPACING: f32 = 2.0;
/// The sidebar container's padding, top and bottom.
const SIDEBAR_VERTICAL_PADDING: f32 = 32.0;
/// What the window holds before the layout has been measured.
pub(crate) const DEFAULT_WINDOW_ROWS: usize = 20;

/// How many rows fit a sidebar of `height` pixels at `font_size`.
pub(crate) fn rows_that_fit(height: Option<f32>, font_size: f32) -> usize {
    let Some(height) = height else {
        return DEFAULT_WINDOW_ROWS;
    };
    let pitch = font_size * LINE_HEIGHT_FACTOR + LINE_SPACING;
    let usable = (height - SIDEBAR_VERTICAL_PADDING - detail_height(font_size)).max(0.0);
    ((usable / pitch).floor() as usize)
        .saturating_sub(RESERVED_LINES)
        .max(1)
}

/// The detail area's height: its lines at the text line height, plus the gap
/// above it.
fn detail_height(font_size: f32) -> f32 {
    DETAIL_LINES as f32 * font_size * LINE_HEIGHT_FACTOR + LINE_SPACING * 2.0
}

/// The highlighted row in full, or `None` when it is a row short enough to read
/// on the row itself. Leading indentation is dropped: the detail is for reading,
/// not for showing depth.
pub(crate) fn detail_text(
    catalog: &Catalog,
    tree: &ExplorerTree,
    highlight: usize,
    git_summary: Option<&ProjectGitSummary>,
) -> Option<String> {
    let rows = tree.rows();
    let row = rows.get(highlight)?;
    // The rows that carry a sentence -- a name and its status, or "N more
    // entries not shown" -- can be clipped; "Loading", "cannot be read" and
    // "empty" are short enough to read whole on the row itself.
    matches!(
        row.kind,
        ExplorerTreeRowKind::Node { .. }
            | ExplorerTreeRowKind::Omitted { .. }
            | ExplorerTreeRowKind::RowsNotShown { .. }
    )
    .then(|| row_text(catalog, row, git_summary).trim_start().to_owned())
}

/// No `Message` interest of its own -- selection is driven by keyboard
/// input the shell already routes here via `RoutedInput::Surface`
/// (`FocusZone::Sidebar`); this function only ever reads state, never
/// constructs a message, matching `board::view`'s own shape.
///
/// Rows are drawn **unwrapped**: a wrapped row is more than one line high
/// and [`rows_that_fit`] counts lines. A name too long for the sidebar is
/// clipped, not reflowed; nothing is lost, because the row is still one
/// row and the name is in the editor once opened.
pub fn view<'a, Message: 'a>(
    tree: &ExplorerTree,
    status: &ProjectExplorerStatus,
    cursor: ExplorerCursor,
    catalog: &'a Catalog,
    theme: &'a Theme,
    git_summary: Option<&ProjectGitSummary>,
) -> Element<'a, Message> {
    let lines = tree_lines(
        catalog,
        tree,
        status,
        cursor.highlight,
        cursor.top,
        cursor.capacity,
        git_summary,
    );
    let rows: Vec<Element<'a, Message>> = lines
        .into_iter()
        .map(|line| {
            text(line)
                .size(theme.font_size_body())
                .wrapping(Wrapping::None)
                .into()
        })
        .collect();
    let rows = container(column(rows).spacing(LINE_SPACING))
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true);
    let detail = container(
        text(detail_text(catalog, tree, cursor.highlight, git_summary).unwrap_or_default())
            .size(theme.font_size_body()),
    )
    .width(Length::Fill)
    .height(Length::Fixed(detail_height(theme.font_size_body())))
    .padding(iced::Padding {
        top: LINE_SPACING * 2.0,
        ..iced::Padding::ZERO
    })
    .clip(true);
    column![rows, detail].height(Length::Fill).into()
}

/// RFC-038 PR-038-G: the folder browser's own rows -- the direct
/// analogue of [`ExplorerRow`], for
/// `tekstide_core::project::root::DirectoryBrowseScan` rather than
/// `ExplorerDirectoryScan` (see that type's own doc for why the two are
/// genuinely different scans, not one reused as two).
#[derive(Clone, Copy, Debug)]
pub(crate) enum BrowseRow<'a> {
    Parent,
    Node(&'a BrowseNode),
}

/// The direct analogue of [`visible_rows`]: `Parent` shows whenever
/// there is anywhere to go up to (`scan.parent_dir.is_some()`), unlike
/// the project explorer's "not yet at the project root" rule -- a
/// folder browser has no fixed root to stop climbing at short of the
/// filesystem root itself, which `DirectoryBrowseScan::parent_dir`
/// already encodes as `None`.
pub(crate) fn visible_browse_rows(scan: &DirectoryBrowseScan) -> Vec<BrowseRow<'_>> {
    let mut rows = Vec::with_capacity(scan.nodes.len() + 1);
    if scan.parent_dir.is_some() {
        rows.push(BrowseRow::Parent);
    }
    rows.extend(scan.nodes.iter().map(BrowseRow::Node));
    rows
}

fn browse_node_state_symbol(state: BrowseNodeState) -> &'static str {
    match state {
        BrowseNodeState::Available => "available",
        BrowseNodeState::Collapsed => "collapsed",
        BrowseNodeState::Unreadable => "unreadable",
    }
}

/// The direct analogue of [`node_line`]: `node.name` is untrusted
/// (a real directory can be named anything, including a bidi-override
/// sequence) and escaped before it reaches the catalog, same discipline,
/// same reason.
pub(crate) fn browse_node_line(catalog: &Catalog, node: &BrowseNode) -> String {
    let name = text_safety::quote_untrusted(&node.name);
    catalog.get_with_args(
        "browse-node-entry",
        &CatalogArgs::new()
            .untrusted("name", &name)
            .trusted_symbol("state", browse_node_state_symbol(node.state)),
    )
}

pub(crate) fn browse_row_line(catalog: &Catalog, row: BrowseRow<'_>) -> String {
    match row {
        BrowseRow::Parent => catalog.get("browse-parent-entry"),
        BrowseRow::Node(node) => browse_node_line(catalog, node),
    }
}

/// RFC-040 PR-040-B: each row is now a real, clickable button --
/// `on_row_click` is the same "surface renders the data, `shell.rs`
/// supplies the message" split [`crate::surface::board::row_view`]'s
/// own `open_message` parameter already established, generalized to a
/// closure since a row's message carries *which* row (its index),
/// unlike that single-button case. Navigation itself is still driven
/// by `Enter`/`ModalActivate` for the keyboard path; this closure's own
/// message goes through the identical modal-activation call (`shell.rs`'s
/// own `activate_current_modal`) after setting `highlight` to the
/// clicked row (see
/// `Message::FolderBrowserRowPressed`'s own doc in `shell.rs`), not a
/// second, parallel navigation path.
pub fn browse_view<'a, Message: 'a + Clone>(
    scan: &'a DirectoryBrowseScan,
    highlight: usize,
    catalog: &'a Catalog,
    theme: &'a Theme,
    on_row_click: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let current = text_safety::quote_untrusted(&scan.current_dir.display().to_string());
    let mut lines: Vec<Element<'a, Message>> = vec![
        text(catalog.get_with_args(
            "browse-dialog-current",
            &CatalogArgs::new().untrusted("path", &current),
        ))
        .size(theme.font_size_body())
        .into(),
    ];

    let rows = visible_browse_rows(scan);
    if rows.is_empty() {
        lines.push(
            text(catalog.get("browse-empty"))
                .size(theme.font_size_body())
                .into(),
        );
    }
    for (index, row) in rows.into_iter().enumerate() {
        let marker = if index == highlight { "> " } else { "  " };
        let label = format!("{marker}{}", browse_row_line(catalog, row));
        lines.push(
            button(text(label).size(theme.font_size_body()))
                .on_press(on_row_click(index))
                .into(),
        );
    }
    if scan.truncated {
        lines.push(
            text(catalog.get("browse-truncated-notice"))
                .size(theme.font_size_body())
                .into(),
        );
    }

    column(lines).spacing(4).into()
}

#[cfg(test)]
mod tests;
