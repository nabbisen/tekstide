use std::path::{Path, PathBuf};

use tekstide_core::project::ProjectExplorerStatus;
use tekstide_core::project::ProjectId;
use tekstide_core::project::root::{
    ExplorerDirectoryScan, ExplorerNode, ExplorerNodeKind, ExplorerNodeState,
    FileAccessContainmentStatus, FileAccessSymlinkStatus, FileAccessTarget,
};

use super::{ExplorerRow, node_line, row_line, tree_lines, visible_rows};
use crate::i18n::{Catalog, LocalePreference};

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn real_catalog() -> Catalog {
    Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()))
}

fn target_at(relative_path: &str) -> FileAccessTarget {
    FileAccessTarget {
        project_id: ProjectId::new_uuid(),
        selected_relative_path: PathBuf::from(relative_path),
        selected_absolute_path: PathBuf::from(format!("/home/user/demo/{relative_path}")),
        canonical_path: PathBuf::from(format!("/home/user/demo/{relative_path}")),
        root_canonical_path: PathBuf::from("/home/user/demo"),
        symlink_status: FileAccessSymlinkStatus::NoSymlink,
        containment_status: FileAccessContainmentStatus::InsideRoot,
    }
}

fn plain_node(name: &str, kind: ExplorerNodeKind) -> ExplorerNode {
    ExplorerNode {
        name: name.to_string(),
        relative_path: PathBuf::from(name),
        kind,
        state: ExplorerNodeState::Available,
        symlink_status: FileAccessSymlinkStatus::NoSymlink,
    }
}

fn scan_at_root(nodes: Vec<ExplorerNode>) -> ExplorerDirectoryScan {
    ExplorerDirectoryScan {
        directory: target_at(""),
        nodes,
        truncated: false,
    }
}

/// **The bidi-override case, tested specifically** (RFC-019's gate for
/// this slice): a node named with `U+202E` must render with the escaped
/// `<U+202E>` marker present and the raw override character absent --
/// the explorer is trusted chrome, and a repository can name a file
/// anything.
#[test]
fn a_bidi_override_node_name_renders_escaped_and_the_raw_character_is_absent() {
    let catalog = real_catalog();
    let node = plain_node("proj\u{202E}gpj.exe", ExplorerNodeKind::File);

    let line = node_line(&catalog, &node);

    assert!(
        line.contains("<U+202E>"),
        "expected the escaped marker in {line:?}"
    );
    assert!(
        !line.contains('\u{202E}'),
        "the raw override character must never reach the rendered line, got {line:?}"
    );
}

/// The opposite-direction check response 175/176's convention across
/// this project asks for: confirms the escaped marker really is what
/// [`node_line`] depends on `quote_untrusted` for, by checking the
/// non-hostile baseline case renders the name verbatim (modulo the
/// isolate marks `quote_untrusted` always wraps content in) -- so the
/// bidi test above is exercising real escaping, not a coincidence of
/// this particular fixture.
#[test]
fn a_plain_node_name_renders_without_any_escape_marker() {
    let catalog = real_catalog();
    let node = plain_node("readme.md", ExplorerNodeKind::File);

    let line = node_line(&catalog, &node);

    assert!(line.contains("readme.md"));
    assert!(!line.contains("<U+"));
}

/// Enumerates every `ExplorerNodeState`/`FileAccessSymlinkStatus`
/// combination this module can render and asserts each produces a
/// **distinct** line -- `NFR-UX-002` requires distinguishability without
/// colour, and this module never applies colour to a node line at all,
/// so distinctness of the rendered *text* is the whole of the property.
#[test]
fn every_state_and_symlink_combination_renders_a_distinct_line() {
    let catalog = real_catalog();
    let states = [
        ExplorerNodeState::Available,
        ExplorerNodeState::Collapsed,
        ExplorerNodeState::Blocked(
            tekstide_core::project::root::FileAccessBlockedReason::PermissionDenied,
        ),
        ExplorerNodeState::Unreadable,
    ];
    let symlinks = [
        FileAccessSymlinkStatus::NoSymlink,
        FileAccessSymlinkStatus::InRootSymlink,
        FileAccessSymlinkStatus::UnresolvedSymlink,
        FileAccessSymlinkStatus::EscapesRoot,
    ];

    let mut rendered = std::collections::HashSet::new();
    for state in &states {
        for symlink in &symlinks {
            let node = ExplorerNode {
                name: "fixture".to_string(),
                relative_path: PathBuf::from("fixture"),
                kind: ExplorerNodeKind::File,
                state: state.clone(),
                symlink_status: *symlink,
            };
            let line = node_line(&catalog, &node);
            assert!(
                rendered.insert(line.clone()),
                "state {state:?} + symlink {symlink:?} rendered a line already produced by \
                 another combination: {line:?}"
            );
        }
    }
    assert_eq!(rendered.len(), states.len() * symlinks.len());
}

/// `ExplorerNodeKind::Directory`/`Other`/`File` each carry their own
/// symbol -- checked directly rather than only inferred from the
/// combination test above, since kind and state are independent axes.
#[test]
fn every_kind_renders_a_distinct_marker() {
    let catalog = real_catalog();
    let file = node_line(&catalog, &plain_node("x", ExplorerNodeKind::File));
    let dir = node_line(&catalog, &plain_node("x", ExplorerNodeKind::Directory));
    let other = node_line(&catalog, &plain_node("x", ExplorerNodeKind::Other));

    assert_ne!(file, dir);
    assert_ne!(file, other);
    assert_ne!(dir, other);
    assert!(file.contains("[FILE]"));
    assert!(dir.contains("[DIR]"));
    assert!(other.contains("[OTHER]"));
}

/// [`visible_rows`] must never walk the filesystem -- it is built
/// directly from `scan.nodes`, in order, plus the synthetic parent row
/// when the scan is not at the project root. This is the "no filesystem
/// walking in the shell" gate item, checked as a structural property:
/// the row count is exactly `nodes.len()` (root) or `nodes.len() + 1`
/// (non-root), never anything a directory read could have produced.
#[test]
fn visible_rows_never_exceeds_the_scans_own_node_count_plus_the_parent_entry() {
    let nodes = vec![
        plain_node("a.txt", ExplorerNodeKind::File),
        plain_node("b", ExplorerNodeKind::Directory),
    ];

    let root_scan = scan_at_root(nodes.clone());
    let root_rows = visible_rows(&root_scan);
    assert_eq!(
        root_rows.len(),
        nodes.len(),
        "no parent entry at the project root"
    );
    assert!(matches!(root_rows[0], ExplorerRow::Node(_)));

    let nested_scan = ExplorerDirectoryScan {
        directory: target_at("subdir"),
        nodes: nodes.clone(),
        truncated: false,
    };
    let nested_rows = visible_rows(&nested_scan);
    assert_eq!(
        nested_rows.len(),
        nodes.len() + 1,
        "a parent entry is added away from the project root"
    );
    assert!(matches!(nested_rows[0], ExplorerRow::Parent));
}

/// The synthetic parent row renders through the catalog too -- not a
/// hardcoded shell-local literal.
#[test]
fn the_parent_row_resolves_through_the_catalog() {
    let catalog = real_catalog();
    let line = row_line(&catalog, ExplorerRow::Parent);
    assert_eq!(line, catalog.get("explorer-parent-entry"));
}

/// The highlight marker moves with the index, and only the highlighted
/// row carries it -- the same textual-marker convention
/// (`"> "`/`"  "`) `shell.rs::focus_marker` and the paste dialog already
/// use, checked here at the plain-`String` level.
#[test]
fn the_highlight_marker_is_present_on_exactly_the_highlighted_row() {
    let catalog = real_catalog();
    let scan = scan_at_root(vec![
        plain_node("a.txt", ExplorerNodeKind::File),
        plain_node("b.txt", ExplorerNodeKind::File),
    ]);
    let lines = tree_lines(&catalog, Some(&scan), &ProjectExplorerStatus::Ready, 1);

    assert!(lines[0].starts_with("  "));
    assert!(lines[1].starts_with("> "));
}

/// `ProjectExplorerStatus::Error`'s message embeds an attacker-influenced
/// path (`ExplorerScanError`'s `Display`) -- escaped exactly like a node
/// name, checked with the same bidi fixture the node-name test above
/// uses so the two paths cannot silently diverge.
#[test]
fn the_error_status_message_is_escaped() {
    let catalog = real_catalog();
    let status = ProjectExplorerStatus::Error {
        message: "could not read directory: proj\u{202E}gpj.exe".to_string(),
    };
    let lines = tree_lines(&catalog, None, &status, 0);

    let status_line = lines
        .iter()
        .find(|line| line.contains("Explorer error"))
        .expect("an error status must render a status line");
    assert!(status_line.contains("<U+202E>"));
    assert!(!status_line.contains('\u{202E}'));
}

/// A scan with no nodes at all still renders something -- an explorer
/// that goes blank on an empty directory is indistinguishable from one
/// that failed to scan at all.
#[test]
fn an_empty_scan_renders_the_empty_notice_not_a_blank_view() {
    let catalog = real_catalog();
    let scan = scan_at_root(Vec::new());
    let lines = tree_lines(&catalog, Some(&scan), &ProjectExplorerStatus::Ready, 0);
    assert_eq!(lines, vec![catalog.get("explorer-empty")]);
}

/// A truncated scan says so -- `ExplorerDirectoryScan::truncated` exists
/// specifically so a bounded listing does not silently look complete.
#[test]
fn a_truncated_scan_renders_the_truncation_notice() {
    let catalog = real_catalog();
    let scan = ExplorerDirectoryScan {
        directory: target_at(""),
        nodes: vec![plain_node("a.txt", ExplorerNodeKind::File)],
        truncated: true,
    };
    let lines = tree_lines(&catalog, Some(&scan), &ProjectExplorerStatus::Ready, 0);
    assert!(
        lines
            .last()
            .unwrap()
            .contains(&catalog.get("explorer-truncated-notice"))
    );
}

/// **No `*_label` free function is called anywhere in this module.**
/// `explorer_node_kind_label`, `explorer_node_state_label`, and
/// `explorer_symlink_status_label` are the three of RFC-019's four named
/// hardcoded-English producers this module could reach (the fourth,
/// `text_document_state_label`, is PR-019-C's). Checked by scanning this
/// module's own source text for the literal call syntax, the same shape
/// `write_terminal_input_has_exactly_the_three_named_production_call_sites`
/// uses for a different property -- a substring match a reviewer can
/// verify by eye, not a claim resting on this test file's own honesty.
#[test]
fn no_hardcoded_english_label_function_is_called_in_this_module() {
    let source = include_str!("../explorer.rs");
    for forbidden in [
        "explorer_node_kind_label(",
        "explorer_node_state_label(",
        "explorer_symlink_status_label(",
        "text_document_state_label(",
    ] {
        assert!(
            !source.contains(forbidden),
            "{forbidden} must not be called in surface/explorer.rs -- route through Catalog instead"
        );
    }
}

/// RFC-038 PR-038-I (response 300's own follow-up): a mechanical guard
/// that every untrusted value this module passes to the catalog was
/// escaped first -- the property is "**every** untrusted value reaching
/// the catalog is escaped", so per `ARCHITECTURE.md`'s enumeration-test
/// unit rule the unit is the call site, not the file. Count-equality
/// rather than a per-call-site trace: this module's own convention is
/// "escape immediately into a local, then pass that local to
/// `.untrusted(`" (`node_line`, `status_line`, `browse_node_line`), so
/// one `quote_untrusted(` feeding exactly one `.untrusted(` is what
/// "escaped before it reaches the catalog" means here. A `browse_*`-style
/// renderer added later that
/// passes a raw, unescaped name to `.untrusted(` changes only the
/// `.untrusted(` count, not the `quote_untrusted(` count, and fails
/// this test.
///
/// **Scoped to this module only, deliberately.** `board.rs` reads a
/// different count (0 `.untrusted(`, escapes and renders through a
/// different path) -- extending this exact invariant there without
/// first deciding what it should mean for that module would produce a
/// count-equality that is false for correct code, worse than no
/// invariant at all (response 300's own caution).
#[test]
fn every_untrusted_value_this_module_hands_the_catalog_was_escaped_first() {
    let source = include_str!("../explorer.rs");
    let untrusted_call_sites = source.matches(".untrusted(").count();
    let escape_calls = source.matches("quote_untrusted(").count();

    assert_eq!(
        untrusted_call_sites, escape_calls,
        "surface/explorer.rs calls .untrusted( {untrusted_call_sites} time(s) and \
         quote_untrusted( {escape_calls} time(s) -- every untrusted value reaching the catalog \
         must be escaped first, so these must match"
    );
}
