use std::path::{Path, PathBuf};

use tekstide_core::project::ProjectId;
use tekstide_core::project::root::{
    ExplorerDirectoryScan, ExplorerNode, ExplorerNodeKind, ExplorerNodeState,
    FileAccessContainmentStatus, FileAccessSymlinkStatus, FileAccessTarget,
};
use tekstide_core::project::{ExplorerTree, ExplorerTreeRowKind, ProjectExplorerStatus};
use tekstide_core::project::{FileGitStatus, ProjectGitSummary, ProjectProviderState};

use super::{
    DEFAULT_WINDOW_ROWS, RowWindow, detail_text, node_line, row_text, rows_that_fit, tree_lines,
    window_for,
};
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
        omitted_entries: 0,
        omitted_is_lower_bound: false,
    }
}

/// A tree whose root has been scanned into `scan`. RFC-052: the tree is
/// what the surface draws, and a test can fabricate one through the same
/// `set_scan` a synchronous caller uses -- no filesystem involved.
fn tree_with(scan: ExplorerDirectoryScan) -> ExplorerTree {
    let mut tree = ExplorerTree::default();
    tree.set_scan(Path::new(""), Ok(scan));
    tree
}

/// Every line of a tree, with the window wide open.
fn all_lines(catalog: &Catalog, tree: &ExplorerTree, highlight: usize) -> Vec<String> {
    tree_lines(
        catalog,
        tree,
        &ProjectExplorerStatus::Ready,
        highlight,
        0,
        1000,
        None,
    )
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

    let line = node_line(&catalog, &node, false, None);

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

    let line = node_line(&catalog, &node, false, None);

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
            let line = node_line(&catalog, &node, false, None);
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
    let file = node_line(
        &catalog,
        &plain_node("x", ExplorerNodeKind::File),
        false,
        None,
    );
    let dir = node_line(
        &catalog,
        &plain_node("x", ExplorerNodeKind::Directory),
        false,
        None,
    );
    let other = node_line(
        &catalog,
        &plain_node("x", ExplorerNodeKind::Other),
        false,
        None,
    );

    assert_ne!(file, dir);
    assert_ne!(file, other);
    assert_ne!(dir, other);
    assert!(file.contains("[FILE]"));
    assert!(dir.contains("[DIR]"));
    assert!(other.contains("[OTHER]"));
}

/// The row list of a tree is built from the scans it holds and nothing
/// else -- it never walks the filesystem -- and (RFC-052) there is **no
/// parent row**: folders expand in place, so there is nothing to walk back
/// out of. The row count is exactly the scan's node count, whatever
/// directory the scan is of.
#[test]
fn a_trees_rows_are_exactly_its_scans_nodes_and_there_is_no_parent_row() {
    let nodes = vec![
        plain_node("a.txt", ExplorerNodeKind::File),
        plain_node("b", ExplorerNodeKind::Directory),
    ];
    let tree = tree_with(scan_at_root(nodes.clone()));
    let rows = tree.rows();
    assert_eq!(rows.len(), nodes.len());
    assert!(
        rows.iter()
            .all(|row| matches!(row.kind, ExplorerTreeRowKind::Node { .. }))
    );

    // A scan of a nested directory is loaded, but it is not on screen
    // until that folder is expanded -- and it adds no "up" row either way.
    let mut tree = tree_with(scan_at_root(nodes.clone()));
    tree.set_scan(
        Path::new("b"),
        Ok(ExplorerDirectoryScan {
            directory: target_at("b"),
            ..scan_at_root(vec![plain_node("inner.txt", ExplorerNodeKind::File)])
        }),
    );
    assert_eq!(tree.rows().len(), nodes.len(), "collapsed: nothing added");
    tree.toggle(Path::new("b"));
    // toggling asks for a refresh; the cached rows show meanwhile.
    assert_eq!(tree.rows().len(), nodes.len() + 1);
}

/// Every row of a tree says what it is through the catalog, and the words
/// on the rows that only say something are the catalog's, not literals.
#[test]
fn the_rows_that_only_say_something_resolve_through_the_catalog() {
    let catalog = real_catalog();
    let mut tree = tree_with(scan_at_root(vec![plain_node(
        "d",
        ExplorerNodeKind::Directory,
    )]));
    tree.toggle(Path::new("d")); // pending: a Loading row
    let lines = all_lines(&catalog, &tree, 0);
    assert!(
        lines
            .iter()
            .any(|line| line.contains(&catalog.get("explorer-row-loading")))
    );

    tree.set_scan(
        Path::new("d"),
        Err(
            tekstide_core::project::root::ExplorerScanError::CannotReadDirectory {
                target: Box::new(target_at("d")),
            },
        ),
    );
    let lines = all_lines(&catalog, &tree, 0);
    assert!(
        lines
            .iter()
            .any(|line| line.contains(&catalog.get("explorer-row-cannot-read"))),
        "{lines:?}"
    );
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
    let lines = all_lines(&catalog, &tree_with(scan), 1);

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
    let lines = tree_lines(
        &catalog,
        &ExplorerTree::default(),
        &status,
        0,
        0,
        1000,
        None,
    );

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
    let lines = all_lines(&catalog, &tree_with(scan), 0);
    // The empty row is indented under its folder (here the root, depth 0).
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains(&catalog.get("explorer-empty")),
        "{lines:?}"
    );
}

/// A truncated scan says so, **and says how many** -- RFC-052: nothing is
/// hidden silently. `ExplorerDirectoryScan::truncated` exists so a bounded
/// listing does not look complete; the count is what makes the notice
/// checkable.
#[test]
fn a_truncated_scan_names_how_many_entries_it_left_out() {
    let catalog = real_catalog();
    let scan = ExplorerDirectoryScan {
        directory: target_at(""),
        nodes: vec![plain_node("a.txt", ExplorerNodeKind::File)],
        truncated: true,
        omitted_entries: 44,
        omitted_is_lower_bound: false,
    };
    let lines = all_lines(&catalog, &tree_with(scan), 0);
    let last = plain_words(lines.last().unwrap());
    assert!(last.contains("44 more entries are not shown"), "{last:?}");

    // One is grammatical, and a count the scanner stopped short of says so.
    let one = ExplorerDirectoryScan {
        omitted_entries: 1,
        ..scan_at_root(vec![plain_node("a.txt", ExplorerNodeKind::File)])
    };
    let one = ExplorerDirectoryScan {
        truncated: true,
        ..one
    };
    let lines = all_lines(&catalog, &tree_with(one), 0);
    assert!(plain_words(lines.last().unwrap()).contains("One more entry is not shown"));

    let bounded = ExplorerDirectoryScan {
        truncated: true,
        omitted_entries: 1_000_001,
        omitted_is_lower_bound: true,
        ..scan_at_root(vec![plain_node("a.txt", ExplorerNodeKind::File)])
    };
    let lines = all_lines(&catalog, &tree_with(bounded), 0);
    let last = plain_words(lines.last().unwrap());
    assert!(
        last.contains("At least 1000001 more entries are not shown"),
        "{last:?}"
    );
}

/// Fluent isolates every placeable in U+2066..U+2069 (ARCHITECTURE.md), and
/// a number is one; a test on the words a user reads strips them first.
fn plain_words(line: &str) -> String {
    line.chars()
        .filter(|c| !('\u{2066}'..='\u{2069}').contains(c))
        .collect()
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

// ---------------------------------------------------------------------
// RFC-030 PR-030-C, REQ-GIT-003: per-file Git status badges.
// ---------------------------------------------------------------------

fn git_summary_with(entries: &[(&str, FileGitStatus)]) -> ProjectGitSummary {
    ProjectGitSummary {
        provider_state: ProjectProviderState::Complete,
        branch_name: Some("main".to_string()),
        changed_file_count: Some(entries.len() as u32),
        ahead_count: None,
        behind_count: None,
        file_statuses: Some(
            entries
                .iter()
                .map(|(path, status)| (PathBuf::from(path), *status))
                .collect(),
        ),
    }
}

/// Each of `FileGitStatus`'s six categories renders its own, distinct
/// `[...]` badge -- `NFR-UX-002`'s distinguishability requirement, the
/// same property `every_state_and_symlink_combination_renders_a_distinct_line`
/// already proves for state/symlink.
#[test]
fn every_git_status_category_renders_a_distinct_badge() {
    let catalog = real_catalog();
    let categories = [
        FileGitStatus::Modified,
        FileGitStatus::Added,
        FileGitStatus::Deleted,
        FileGitStatus::Renamed,
        FileGitStatus::Untracked,
        FileGitStatus::Unmerged,
    ];

    let mut rendered = std::collections::HashSet::new();
    for status in categories {
        let summary = git_summary_with(&[("x", status)]);
        let line = node_line(
            &catalog,
            &plain_node("x", ExplorerNodeKind::File),
            false,
            Some(&summary),
        );
        assert!(
            rendered.insert(line.clone()),
            "{status:?} rendered a line already produced by another category: {line:?}"
        );
    }
    assert_eq!(rendered.len(), categories.len());
}

/// `[renamed]`'s own wording review 413 required to stay true for both
/// a real rename and a copy (porcelain v2's `R`/`C` both collapse to
/// `FileGitStatus::Renamed` upstream) -- checked here at the string a
/// user actually sees, not only that the enum variant collapses
/// correctly in `runtime::git`'s own tests.
#[test]
fn the_renamed_badge_names_both_rename_and_copy() {
    let catalog = real_catalog();
    let summary = git_summary_with(&[("x", FileGitStatus::Renamed)]);
    let line = node_line(
        &catalog,
        &plain_node("x", ExplorerNodeKind::File),
        false,
        Some(&summary),
    );
    assert!(line.contains("renamed"));
    assert!(line.contains("copied"));
}

/// A path absent from the summary's map renders exactly like no summary
/// at all -- absence is not itself a status, the same property
/// `ProjectGitSummary::file_status` documents at the data layer, checked
/// here at the rendered string.
#[test]
fn a_file_outside_the_summarys_map_renders_identically_to_no_summary_at_all() {
    let catalog = real_catalog();
    let summary = git_summary_with(&[("other.txt", FileGitStatus::Modified)]);
    let node = plain_node("untouched.txt", ExplorerNodeKind::File);

    let with_summary = node_line(&catalog, &node, false, Some(&summary));
    let without_summary = node_line(&catalog, &node, false, None);

    assert_eq!(
        with_summary, without_summary,
        "a path absent from the map must render exactly like no summary at all"
    );
}

/// `AcceptedBranchOnly`/`Refused` never reach `read_status_summary`, so
/// `file_statuses` is `None` there (`runtime::git`'s own doc comment) --
/// every node in that project must render exactly like no summary at
/// all, not a guessed badge.
#[test]
fn a_repository_with_no_file_statuses_computed_renders_no_badges() {
    let catalog = real_catalog();
    let branch_only = ProjectGitSummary {
        provider_state: ProjectProviderState::Complete,
        branch_name: Some("main".to_string()),
        changed_file_count: None,
        ahead_count: None,
        behind_count: None,
        file_statuses: None,
    };
    let node = plain_node("anything.txt", ExplorerNodeKind::File);

    let with_branch_only = node_line(&catalog, &node, false, Some(&branch_only));
    let without_summary = node_line(&catalog, &node, false, None);

    assert_eq!(with_branch_only, without_summary);
}

// ---------------------------------------------------------------------
// RFC-052 PR-052-B: the tree, drawn.
// ---------------------------------------------------------------------

/// Indentation and the expansion marker are part of what a row says, and
/// they are assertable as text -- no `iced` involved (RFC-052 §7).
#[test]
fn a_row_is_indented_by_depth_and_marked_open_or_closed_in_characters() {
    let catalog = real_catalog();
    let mut tree = tree_with(scan_at_root(vec![
        plain_node("src", ExplorerNodeKind::Directory),
        plain_node("docs", ExplorerNodeKind::Directory),
        plain_node("top.txt", ExplorerNodeKind::File),
    ]));
    tree.set_scan(
        Path::new("src"),
        Ok(ExplorerDirectoryScan {
            directory: target_at("src"),
            ..scan_at_root(vec![plain_node("lib.rs", ExplorerNodeKind::File)])
        }),
    );
    tree.toggle(Path::new("src"));

    let lines = all_lines(&catalog, &tree, 99);
    let text: Vec<String> = lines.iter().map(|line| plain_words(line)).collect();
    // `src` is open (`[-]`), `docs` closed (`[+]`), a file has neither, and
    // `lib.rs` sits two spaces further in than the folder that holds it.
    assert!(text[0].starts_with("  [-] [DIR] src"), "{text:?}");
    assert!(text[1].starts_with("        [FILE] lib.rs"), "{text:?}");
    assert!(text[2].starts_with("  [+] [DIR] docs"), "{text:?}");
    assert!(text[3].starts_with("      [FILE] top.txt"), "{text:?}");
    // The two-space highlight marker precedes all of it; only one row has it.
    assert_eq!(text.iter().filter(|l| l.starts_with("> ")).count(), 0);
}

/// **§1 through the tree: every name at every depth is escaped**, not just
/// the ones the old one-level view happened to draw. A newline in a name
/// must not split a row in two, and a bidi override must not reach the
/// screen.
#[test]
fn hostile_names_at_any_depth_never_reach_a_row_raw() {
    let catalog = real_catalog();
    let hostile = [
        "evil\u{202E}gpj.exe",
        "two\nlines.txt",
        "bad-\u{FFFD}-name.txt",
    ];
    let mut tree = tree_with(scan_at_root(vec![plain_node(
        "d",
        ExplorerNodeKind::Directory,
    )]));
    tree.set_scan(
        Path::new("d"),
        Ok(ExplorerDirectoryScan {
            directory: target_at("d"),
            ..scan_at_root(
                hostile
                    .iter()
                    .map(|name| ExplorerNode {
                        relative_path: PathBuf::from("d").join(name),
                        ..plain_node(name, ExplorerNodeKind::File)
                    })
                    .collect(),
            )
        }),
    );
    tree.toggle(Path::new("d"));

    let lines = all_lines(&catalog, &tree, 0);
    assert_eq!(
        lines.len(),
        1 + hostile.len(),
        "one line per row: {lines:?}"
    );
    for line in &lines {
        assert!(!line.contains('\u{202E}'), "raw override drawn: {line:?}");
        assert!(!line.contains('\n'), "a name split a row: {line:?}");
    }
    let joined = lines.join("\n");
    assert!(joined.contains("<U+202E>") && joined.contains("<U+000A>"));
}

/// The word `(collapsed)` is true while a folder is closed and would
/// contradict the rows under it once it is open.
#[test]
fn a_collapse_list_folder_says_collapsed_only_while_it_is_closed() {
    let catalog = real_catalog();
    let node = ExplorerNode {
        state: ExplorerNodeState::Collapsed,
        ..plain_node("target", ExplorerNodeKind::Directory)
    };
    assert!(node_line(&catalog, &node, false, None).contains("(collapsed)"));
    assert!(!node_line(&catalog, &node, true, None).contains("(collapsed)"));
}

/// The window rule, as a rule (RFC-052 D8: the total-row bound is
/// virtualisation, so only the rows that fit are ever built into widgets).
#[test]
fn the_window_shows_the_highlight_and_moves_as_little_as_it_can() {
    // Everything fits: no window at all.
    assert_eq!(window_for(5, 3, 0, 10), RowWindow { top: 0, end: 5 });
    // Moving inside the window does not scroll it.
    assert_eq!(window_for(100, 12, 10, 5), RowWindow { top: 10, end: 15 });
    // Past the bottom: scroll just enough to put the highlight last.
    assert_eq!(window_for(100, 15, 10, 5), RowWindow { top: 11, end: 16 });
    // Past the top: scroll just enough to put it first.
    assert_eq!(window_for(100, 9, 10, 5), RowWindow { top: 9, end: 14 });
    // A stale `top` (the tree shrank, or the window grew) is clamped so the
    // window never runs off the end.
    assert_eq!(window_for(20, 19, 500, 5), RowWindow { top: 15, end: 20 });
    // A highlight past the end is clamped to the last row.
    assert_eq!(window_for(20, 999, 0, 5), RowWindow { top: 15, end: 20 });
    // A zero-row window still shows one row: the highlight must be drawn.
    assert_eq!(window_for(20, 7, 0, 0), RowWindow { top: 7, end: 8 });
}

/// The capacity is measured from the sidebar's height with the numbers the
/// drawing uses; unknown until the first layout, then it follows the size.
#[test]
fn how_many_rows_fit_follows_the_measured_height() {
    assert_eq!(rows_that_fit(None, 14.0), DEFAULT_WINDOW_ROWS);
    let small = rows_that_fit(Some(300.0), 14.0);
    let large = rows_that_fit(Some(900.0), 14.0);
    assert!(small >= 1 && large > small, "{small} then {large}");
    // A larger font fits fewer rows in the same height.
    assert!(rows_that_fit(Some(900.0), 20.0) < large);
    // Degenerate heights still leave one row rather than none.
    assert_eq!(rows_that_fit(Some(0.0), 14.0), 1);
}

/// **Nothing is hidden silently.** A tree longer than its window draws only
/// the window and says which rows those are; one that fits says nothing
/// extra.
#[test]
fn a_window_narrower_than_the_tree_says_which_rows_it_is_showing() {
    let catalog = real_catalog();
    let nodes: Vec<ExplorerNode> = (0..50)
        .map(|index| plain_node(&format!("f{index:02}"), ExplorerNodeKind::File))
        .collect();
    let tree = tree_with(scan_at_root(nodes));

    let lines = tree_lines(
        &catalog,
        &tree,
        &ProjectExplorerStatus::Ready,
        30,
        0,
        10,
        None,
    );
    // 10 rows and the position line; the highlight (30) is inside them.
    assert_eq!(lines.len(), 11);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("> ") && line.contains("f30"))
    );
    assert_eq!(plain_words(lines.last().unwrap()), "Rows 22–31 of 50");

    let everything = tree_lines(
        &catalog,
        &tree,
        &ProjectExplorerStatus::Ready,
        0,
        0,
        100,
        None,
    );
    assert_eq!(everything.len(), 50, "no position line when all rows fit");
}

/// The tree's own row bound ends in a row that says how many rows were not
/// kept -- and reads correctly for one.
#[test]
fn passing_the_row_bound_ends_in_a_row_naming_how_many_are_not_shown() {
    let catalog = real_catalog();
    let bound = tekstide_core::project::MAX_TREE_ROWS;
    let nodes = |count: usize| {
        tree_with(scan_at_root(
            (0..count)
                .map(|index| plain_node(&format!("f{index}"), ExplorerNodeKind::File))
                .collect(),
        ))
    };
    for (count, expected) in [
        (bound + 1, "One more row is not shown"),
        (bound + 5, "5 more rows are not shown"),
    ] {
        let tree = nodes(count);
        let rows = tree.rows();
        assert_eq!(rows.len(), bound + 1);
        let last = plain_words(&row_text(&catalog, rows.last().unwrap(), None));
        assert!(last.contains(expected), "{last:?}");
    }
}

/// **A clipped line must lose the name, not the status.** The sidebar draws
/// one row per line and clips at its edge; a status word after a long name is
/// what the clip removes (RFC-052 PR-052-B's live capture showed
/// `[FILE] new.md [untracke`). Kind, state, symlink status and Git status all
/// come before the name, so the name is the only part that can be cut.
#[test]
fn every_status_word_comes_before_the_name_so_only_the_name_can_be_clipped() {
    let catalog = real_catalog();
    let node = ExplorerNode {
        name: "a-very-long-file-name-that-will-be-clipped.txt".to_string(),
        relative_path: PathBuf::from("a-very-long-file-name-that-will-be-clipped.txt"),
        kind: ExplorerNodeKind::File,
        state: ExplorerNodeState::Blocked(
            tekstide_core::project::root::FileAccessBlockedReason::SymlinkEscape,
        ),
        symlink_status: FileAccessSymlinkStatus::EscapesRoot,
    };
    let summary = git_summary_with(&[(&node.name, FileGitStatus::Modified)]);
    let line = plain_words(&node_line(&catalog, &node, false, Some(&summary)));

    let name_at = line.find(&node.name).expect("the name is drawn");
    for word in [
        "[FILE]",
        "(blocked)",
        "[symlink escapes root]",
        "[modified]",
    ] {
        let at = line
            .find(word)
            .unwrap_or_else(|| panic!("{word} missing from {line:?}"));
        assert!(at < name_at, "{word} comes after the name in {line:?}");
    }
    assert!(
        line.ends_with(&node.name),
        "the name is the last thing on the row: {line:?}"
    );
}

/// The detail area is where a row that the sidebar clipped is still readable
/// whole -- and it is the *escaped* row, the same text, so it opens no new
/// way for a hostile name to reach the screen.
#[test]
fn the_detail_shows_the_highlighted_row_in_full_and_escaped() {
    let catalog = real_catalog();
    let long = ExplorerNode {
        name: "evil\u{202E}gpj-with-a-name-far-too-long-for-a-narrow-sidebar.exe".to_string(),
        relative_path: PathBuf::from("evil"),
        state: ExplorerNodeState::Blocked(
            tekstide_core::project::root::FileAccessBlockedReason::SymlinkEscape,
        ),
        symlink_status: FileAccessSymlinkStatus::EscapesRoot,
        ..plain_node("x", ExplorerNodeKind::Other)
    };
    let tree = tree_with(scan_at_root(vec![
        long,
        plain_node("ok.txt", ExplorerNodeKind::File),
    ]));

    let detail = detail_text(&catalog, &tree, 0, None).expect("a node row has detail");
    let plain = plain_words(&detail);
    assert!(
        plain.starts_with("[OTHER] (blocked) [symlink escapes root]"),
        "{plain:?}"
    );
    assert!(plain.contains("<U+202E>") && !detail.contains('\u{202E}'));
    assert!(
        plain.ends_with("too-long-for-a-narrow-sidebar.exe"),
        "whole name: {plain:?}"
    );
    assert!(!detail.starts_with(' '), "no indentation in the detail");

    // A row that only says something has no detail: it already says it all.
    let mut loading = tree_with(scan_at_root(vec![plain_node(
        "d",
        ExplorerNodeKind::Directory,
    )]));
    loading.toggle(Path::new("d"));
    assert!(detail_text(&catalog, &loading, 1, None).is_none());
    // And a highlight past the end has none rather than panicking.
    assert!(detail_text(&catalog, &tree, 99, None).is_none());
}

/// The window arithmetic subtracts the detail area, or the last row would sit
/// under it.
#[test]
fn the_window_leaves_room_for_the_detail_area() {
    let with_detail = rows_that_fit(Some(600.0), 14.0);
    let pitch = 14.0 * 1.3 + 2.0;
    let without = (((600.0f32 - 32.0) / pitch).floor() as usize) - 2;
    assert!(
        with_detail + 2 <= without,
        "the detail area (about {} lines) must cost rows: {with_detail} vs {without}",
        super::DETAIL_LINES
    );
}

// ---------------------------------------------------------------------
// D8, measured: what drawing a big tree costs, and why only a window is built.
// ---------------------------------------------------------------------

fn headless_renderer() -> iced::Renderer {
    use iced::advanced::renderer::Headless;
    use std::task::{Context, Poll, Waker};
    let future = <iced::Renderer as Headless>::new(
        iced::Font::DEFAULT,
        iced::Pixels(16.0),
        Some("tiny-skia"),
    );
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(renderer) = future.as_mut().poll(&mut context) {
            return renderer.expect("a headless tiny-skia renderer");
        }
        std::thread::yield_now();
    }
}

/// Builds `element` and lays it out at the sidebar's width; returns how long
/// that took. Real text measurement (a headless renderer), unlike the null
/// renderer the rest of this crate's layout tests use.
fn build_and_layout(
    renderer: &iced::Renderer,
    make: impl FnOnce() -> iced::Element<'static, ()>,
) -> std::time::Duration {
    use iced::advanced::layout::Limits;
    use iced::advanced::widget::Tree;
    let started = std::time::Instant::now();
    let mut element = make();
    let mut tree = Tree::new(element.as_widget());
    let limits = Limits::new(iced::Size::ZERO, iced::Size::new(188.0, 700.0));
    let _ = element.as_widget_mut().layout(&mut tree, renderer, &limits);
    started.elapsed()
}

/// **The total-row bound, decided by measurement (RFC-052 review 421):**
/// virtualisation. PR-052-A measured ~4 µs a row to build and lay out, so a
/// tree of 10 000 rows built whole costs tens of milliseconds every time it
/// changes; the sidebar builds only the window (about 30 rows) and the cost
/// does not depend on the tree. This test prints both numbers, and pins the
/// property that matters -- the windowed view of the *largest* tree the model
/// allows fits well inside a frame -- with a deliberately loose bound (half a
/// frame) rather than a machine's speed.
#[test]
fn drawing_the_largest_tree_builds_only_the_window_and_fits_inside_a_frame() {
    let renderer = headless_renderer();
    let catalog: &'static Catalog = Box::leak(Box::new(real_catalog()));
    let theme: &'static crate::theme::Theme = Box::leak(Box::new(crate::theme::Theme::default()));
    let rows = tekstide_core::project::MAX_TREE_ROWS;
    let tree = tree_with(scan_at_root(
        (0..rows)
            .map(|index| plain_node(&format!("file-{index:05}.rs"), ExplorerNodeKind::File))
            .collect(),
    ));
    let status = ProjectExplorerStatus::Ready;
    let capacity = rows_that_fit(Some(700.0), theme.font_size_body());

    // Warm the font system once: the first layout in a process loads fonts.
    let _ = build_and_layout(&renderer, || {
        super::view::<()>(
            &tree,
            &status,
            super::ExplorerCursor {
                highlight: 0,
                top: 0,
                capacity,
            },
            catalog,
            theme,
            None,
        )
    });

    let mut windowed = std::time::Duration::MAX;
    for _ in 0..5 {
        windowed = windowed.min(build_and_layout(&renderer, || {
            super::view::<()>(
                &tree,
                &status,
                super::ExplorerCursor {
                    highlight: 5_000,
                    top: 4_990,
                    capacity,
                },
                catalog,
                theme,
                None,
            )
        }));
    }

    // What building every row would cost: the alternative this rules out.
    let lines = tree_lines(catalog, &tree, &status, 0, 0, rows, None);
    let whole = build_and_layout(&renderer, || {
        iced::widget::column(
            lines
                .into_iter()
                .map(|line| {
                    iced::widget::text(line)
                        .size(theme.font_size_body())
                        .wrapping(iced::widget::text::Wrapping::None)
                        .into()
                })
                .collect::<Vec<iced::Element<'static, ()>>>(),
        )
        .spacing(2)
        .into()
    });

    eprintln!(
        "PR-052-B measurement: {rows} rows; window of {capacity}: build + layout {windowed:?}; \
         building all {rows}: {whole:?}"
    );
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    assert!(
        windowed < std::time::Duration::from_millis(8),
        "the windowed view of a {rows}-row tree took {windowed:?} (best of 5). A number just \
         over the bound while the machine's load average ({load:?}) is well above its core count \
         points at load, not a regression: re-run when it is idle."
    );
    assert!(
        windowed * 10 < whole,
        "virtualisation must be worth it: window {windowed:?} vs whole tree {whole:?}"
    );
}
