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
//! No filesystem walking happens here. `ExplorerDirectoryScan` is
//! `tekstide-core`'s, already bounded by `FileExplorerScanPolicy`; this
//! module only renders what it is given.

use iced::widget::{column, container, text};
use iced::{Element, Length};

use tekstide_core::project::ProjectExplorerStatus;
use tekstide_core::project::root::{
    ExplorerDirectoryScan, ExplorerNode, ExplorerNodeKind, ExplorerNodeState,
    FileAccessSymlinkStatus,
};
use tekstide_core::text_safety;

use crate::i18n::{Catalog, CatalogArgs};
use crate::theme::Theme;

/// One rendered row: the synthetic "go up" entry (shown whenever the
/// current directory is not the project root), or a real node from
/// core's scan. `rows` (below) is the one place this list is built, so
/// a keyboard-navigation index and what the screen actually shows can
/// never disagree about what row N means.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ExplorerRow<'a> {
    Parent,
    Node(&'a ExplorerNode),
}

/// Every row currently rendered, in display order. Never walks the
/// filesystem -- built directly from `scan.nodes`, in the order core's
/// own scan returned them.
pub(crate) fn visible_rows(scan: &ExplorerDirectoryScan) -> Vec<ExplorerRow<'_>> {
    let mut rows = Vec::with_capacity(scan.nodes.len() + 1);
    if !scan.directory.selected_relative_path.as_os_str().is_empty() {
        rows.push(ExplorerRow::Parent);
    }
    rows.extend(scan.nodes.iter().map(ExplorerRow::Node));
    rows
}

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

/// The one line a node renders as, factored out from [`view`] so the
/// escaping and catalog routing are directly testable without `iced` --
/// the same split `board.rs::row_lines` and `session_bar.rs::entry_text`
/// use. `node.name` is untrusted (a repository can name a file anything,
/// including a bidi-override sequence); escaped before it reaches the
/// catalog, never passed to `trusted_symbol` (which is `&'static str`
/// only, so a runtime name would not even compile there).
pub(crate) fn node_line(catalog: &Catalog, node: &ExplorerNode) -> String {
    let name = text_safety::quote_untrusted(&node.name);
    catalog.get_with_args(
        "explorer-node-entry",
        &CatalogArgs::new()
            .trusted_symbol("kind", node_kind_symbol(node.kind))
            .untrusted("name", &name)
            .trusted_symbol("state", node_state_symbol(&node.state))
            .trusted_symbol("symlink", symlink_status_symbol(node.symlink_status)),
    )
}

pub(crate) fn row_line(catalog: &Catalog, row: ExplorerRow<'_>) -> String {
    match row {
        ExplorerRow::Parent => catalog.get("explorer-parent-entry"),
        ExplorerRow::Node(node) => node_line(catalog, node),
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

/// Every line the explorer tree renders, in order: the status line (if
/// any), then every row with its focus marker, then a truncation notice
/// if core's scan was bounded. Factored out from [`view`] for the same
/// testability reason as [`node_line`] -- the marker/highlight logic is
/// exactly the kind of off-by-one that deserves a plain-value test, not
/// only an `Element` tree nobody can assert against directly.
pub(crate) fn tree_lines(
    catalog: &Catalog,
    scan: Option<&ExplorerDirectoryScan>,
    status: &ProjectExplorerStatus,
    highlight: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(message) = status_line(catalog, status) {
        lines.push(message);
    }
    match scan {
        None => lines.push(catalog.get("explorer-empty")),
        Some(scan) => {
            let rows = visible_rows(scan);
            if rows.is_empty() {
                lines.push(catalog.get("explorer-empty"));
            }
            for (index, row) in rows.into_iter().enumerate() {
                let marker = if index == highlight { "> " } else { "  " };
                lines.push(format!("{marker}{}", row_line(catalog, row)));
            }
            if scan.truncated {
                lines.push(catalog.get("explorer-truncated-notice"));
            }
        }
    }
    lines
}

/// No `Message` interest of its own -- selection is driven by keyboard
/// input the shell already routes here via `RoutedInput::Surface`
/// (`FocusZone::Sidebar`); this function only ever reads state, never
/// constructs a message, matching `board::view`'s own shape.
pub fn view<'a, Message: 'a>(
    scan: Option<&ExplorerDirectoryScan>,
    status: &ProjectExplorerStatus,
    highlight: usize,
    catalog: &'a Catalog,
    theme: &'a Theme,
) -> Element<'a, Message> {
    let lines = tree_lines(catalog, scan, status, highlight);
    let rows: Vec<Element<'a, Message>> = lines
        .into_iter()
        .map(|line| text(line).size(theme.font_size_body()).into())
        .collect();
    container(column(rows).spacing(2))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests;
