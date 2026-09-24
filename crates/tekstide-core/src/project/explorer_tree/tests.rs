//! RFC-052 PR-052-B: the tree model against the hostile fixture.
//!
//! Every scan here goes through [`ExplorerScanRequest::run`] -- the same
//! function a worker thread runs -- and comes back through
//! [`ExplorerTree::apply`]. Nothing in the model scans by itself.

use std::path::Path;
use std::time::{Duration, Instant};

use super::*;
use crate::project::root::hostile_fixture::{HostileFixture, NEWLINE_NAME};
use crate::project::root::{ProjectRootValidator, SymlinkPolicy};
use crate::project::{ProjectId, ProjectSession};

const SMALL_BREADTH: usize = 300;

fn handle(project: &Path) -> ProjectRootHandle {
    let root = ProjectRootValidator
        .validate(project, SymlinkPolicy::FailClosed)
        .expect("the fixture's project root validates");
    let session = ProjectSession::new(
        ProjectId::for_test(1),
        root.display_name,
        root.selected_path,
        root.canonical_path,
    );
    ProjectRootHandle::from_project_session(&session)
}

/// Runs every pending scan and applies the results, as the shell's worker
/// threads and message handler do between two frames.
fn settle(tree: &mut ExplorerTree, root: &ProjectRootHandle) {
    for request in tree.scan_requests(root) {
        assert!(tree.apply(request.run()), "a fresh result is never stale");
    }
    assert_eq!(tree.pending().count(), 0);
}

fn open_root(fixture: &HostileFixture) -> (ExplorerTree, ProjectRootHandle) {
    let root = handle(&fixture.project);
    let mut tree = ExplorerTree::default();
    tree.request_scan(Path::new(""));
    settle(&mut tree, &root);
    (tree, root)
}

fn expand(tree: &mut ExplorerTree, root: &ProjectRootHandle, path: &str) {
    assert_eq!(tree.toggle(Path::new(path)), ExplorerToggle::Expanded);
    settle(tree, root);
}

/// The text of every node row, indented by depth, for readable assertions.
fn outline(tree: &ExplorerTree) -> Vec<String> {
    tree.rows()
        .iter()
        .map(|row| {
            let pad = "  ".repeat(row.depth);
            match row.kind {
                ExplorerTreeRowKind::Node {
                    node,
                    expanded,
                    expandable,
                } => {
                    let marker = match (expandable, expanded) {
                        (false, _) => "",
                        (true, true) => "-",
                        (true, false) => "+",
                    };
                    format!("{pad}{marker}{}", node.name)
                }
                ExplorerTreeRowKind::Loading => format!("{pad}<loading>"),
                ExplorerTreeRowKind::CannotRead => format!("{pad}<cannot read>"),
                ExplorerTreeRowKind::Empty => format!("{pad}<empty>"),
                ExplorerTreeRowKind::Omitted { count, at_least } => {
                    format!("{pad}<omitted {count}{}>", if at_least { "+" } else { "" })
                }
                ExplorerTreeRowKind::RowsNotShown { count } => {
                    format!("{pad}<rows not shown {count}>")
                }
                ExplorerTreeRowKind::IgnoredHidden { count } => {
                    format!("{pad}<ignored hidden {count}>")
                }
            }
        })
        .collect()
}

#[test]
fn the_root_is_not_requested_pending_then_loaded() {
    let fixture = HostileFixture::build("tree-root", 3);
    let root = handle(&fixture.project);
    let mut tree = ExplorerTree::default();
    assert_eq!(tree.root_state(), ExplorerRootState::NotRequested);
    assert!(tree.rows().is_empty());

    tree.request_scan(Path::new(""));
    assert_eq!(tree.root_state(), ExplorerRootState::Pending);
    assert_eq!(
        outline(&tree),
        ["<loading>"],
        "the wait is a row, not a blank"
    );

    settle(&mut tree, &root);
    assert_eq!(tree.root_state(), ExplorerRootState::Loaded);
    assert!(outline(&tree).iter().any(|row| row == "+control"));
}

#[test]
fn a_folder_expands_in_place_and_a_nested_one_expands_inside_it() {
    let fixture = HostileFixture::build("tree-expand", 3);
    let (mut tree, root) = open_root(&fixture);

    expand(&mut tree, &root, "control");
    expand(&mut tree, &root, "control/src");

    let outline = outline(&tree);
    let at = |name: &str| outline.iter().position(|row| row.trim() == name).unwrap();
    // Children sit directly under their parent, one level deeper: nothing
    // was stepped into.
    assert!(at("-control") < at("README.md"));
    assert!(outline.contains(&"  README.md".to_owned()));
    assert!(outline.contains(&"  -src".to_owned()));
    assert!(outline.contains(&"    lib.rs".to_owned()));
    assert!(outline.contains(&"    +nested".to_owned()));
    assert!(
        at("+breadth") > at("lib.rs") || at("+breadth") < at("-control"),
        "siblings of the expanded folder stay where they were"
    );
}

#[test]
fn collapsing_hides_the_children_and_expanding_again_shows_the_cached_rows_at_once() {
    let fixture = HostileFixture::build("tree-collapse", 3);
    let (mut tree, root) = open_root(&fixture);
    expand(&mut tree, &root, "control");
    assert!(outline(&tree).contains(&"  README.md".to_owned()));

    assert_eq!(tree.toggle(Path::new("control")), ExplorerToggle::Collapsed);
    assert!(!outline(&tree).contains(&"  README.md".to_owned()));

    // Expanding again requests a refresh, but the cached children are shown
    // while it runs -- no flash of "loading".
    assert_eq!(tree.toggle(Path::new("control")), ExplorerToggle::Expanded);
    assert_eq!(tree.pending().count(), 1);
    assert!(outline(&tree).contains(&"  README.md".to_owned()));
    assert!(!outline(&tree).iter().any(|row| row.contains("<loading>")));
}

#[test]
fn an_expanded_directory_whose_scan_is_pending_is_a_loading_row() {
    let fixture = HostileFixture::build("tree-loading", 3);
    let (mut tree, _root) = open_root(&fixture);
    tree.toggle(Path::new("control"));
    let outline = outline(&tree);
    let at = outline.iter().position(|row| row == "-control").unwrap();
    assert_eq!(outline[at + 1], "  <loading>");
}

#[test]
fn a_stale_result_is_dropped_and_a_second_request_does_not_duplicate_work() {
    let fixture = HostileFixture::build("tree-stale", 3);
    let root = handle(&fixture.project);
    let mut tree = ExplorerTree::default();
    tree.request_scan(Path::new(""));
    let first = tree.scan_requests(&root).remove(0);
    tree.request_scan(Path::new(""));
    assert_eq!(
        tree.pending().count(),
        1,
        "one scan in flight per directory"
    );

    // A result for a generation that is no longer pending changes nothing.
    let mut stale = first.clone().run();
    stale.generation += 1;
    assert!(!tree.apply(stale));
    assert_eq!(tree.root_state(), ExplorerRootState::Pending);
    assert!(tree.apply(first.run()));
    assert_eq!(tree.root_state(), ExplorerRootState::Loaded);
    // And once applied, replaying it is stale.
    let again = tree.scan_requests(&root);
    assert!(again.is_empty());
}

#[test]
fn the_escape_rows_are_reported_and_cannot_be_expanded() {
    let fixture = HostileFixture::build("tree-escape", SMALL_BREADTH);
    let (mut tree, root) = open_root(&fixture);
    expand(&mut tree, &root, "links");

    let rows = tree.rows();
    let expandable = |name: &str| {
        rows.iter().find_map(|row| match row.kind {
            ExplorerTreeRowKind::Node {
                node, expandable, ..
            } if node.name == name => Some((expandable, node.state.clone())),
            _ => None,
        })
    };
    let (escape_dir_expandable, escape_state) = expandable("escape-dir").expect("row exists");
    assert!(!escape_dir_expandable, "an escaping link is not a folder");
    assert!(matches!(escape_state, ExplorerNodeState::Blocked(_)));
    assert!(!expandable("escape-file").unwrap().0);
    assert!(!expandable("broken").unwrap().0);
    assert!(
        expandable("in-root").unwrap().0,
        "a link that stays inside the root still opens"
    );

    // Even asked for directly -- as a bug or a crafted message could --
    // the scan of the escaping directory is refused, and nothing outside
    // the root is listed.
    tree.request_scan(Path::new("links/escape-dir"));
    settle(&mut tree, &root);
    let all = format!("{:?}", outline(&tree));
    assert!(!all.contains("secret.txt"), "outside content leaked: {all}");
}

#[test]
fn hostile_names_are_still_raw_in_the_model_and_every_one_is_a_row() {
    let fixture = HostileFixture::build("tree-names", SMALL_BREADTH);
    let (mut tree, root) = open_root(&fixture);
    expand(&mut tree, &root, "escaping");
    let names: Vec<String> = tree
        .rows()
        .iter()
        .filter_map(|row| match row.kind {
            ExplorerTreeRowKind::Node { node, .. } if row.depth == 1 => Some(node.name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 3, "{names:?}");
    // Escaping is the surface's job, at the point of drawing: the model
    // keeps the name a row's path was built from.
    assert!(names.iter().any(|name| name.contains(NEWLINE_NAME)));
    assert!(names.iter().any(|name| name.contains('\u{202E}')));
}

#[test]
fn an_unreadable_directory_is_a_row_not_a_missing_child() {
    let fixture = HostileFixture::build("tree-unreadable", 3);
    if std::fs::read_dir(fixture.unreadable_dir()).is_ok() {
        eprintln!("skipped: this process can read a mode-000 directory (running as root?)");
        return;
    }
    let (mut tree, root) = open_root(&fixture);
    expand(&mut tree, &root, "unreadable");
    let outline = outline(&tree);
    let at = outline.iter().position(|row| row == "-unreadable").unwrap();
    assert_eq!(outline[at + 1], "  <cannot read>");
}

#[test]
fn a_capped_directory_names_how_many_entries_it_left_out() {
    let fixture = HostileFixture::build("tree-omitted", SMALL_BREADTH);
    let (mut tree, root) = open_root(&fixture);
    expand(&mut tree, &root, "breadth");
    let outline = outline(&tree);
    let at = outline.iter().position(|row| row == "-breadth").unwrap();
    let shown = outline[at + 1..]
        .iter()
        .take_while(|row| row.starts_with("  "))
        .count();
    assert_eq!(
        shown,
        256 + 1,
        "256 entries and the row saying the rest were left out"
    );
    assert!(
        outline.contains(&format!("  <omitted {}>", SMALL_BREADTH - 256)),
        "{:?}",
        &outline[at..at + 3]
    );
}

#[test]
fn a_hundred_thousand_entries_are_capped_counted_and_never_reach_the_render_thread_as_rows() {
    let fixture = HostileFixture::build(
        "tree-100k",
        crate::project::root::hostile_fixture::BREADTH_ENTRIES,
    );
    let root = handle(&fixture.project);
    let mut tree = ExplorerTree::default();
    tree.request_scan(Path::new(""));
    settle(&mut tree, &root);

    // The scan -- what a worker thread does. Measured, not asserted tightly:
    // it is not on the render thread, and D8's budget is about the part that is.
    tree.toggle(Path::new("breadth"));
    let started = Instant::now();
    let requests = tree.scan_requests(&root);
    let completed: Vec<_> = requests.into_iter().map(ExplorerScanRequest::run).collect();
    let worker = started.elapsed();

    // What the render thread does: apply the result and flatten the rows.
    let started = Instant::now();
    for completed in completed {
        assert!(tree.apply(completed));
    }
    let rows = tree.rows();
    let render_thread = started.elapsed();

    let breadth_rows = rows.iter().filter(|row| row.depth == 1).count();
    assert_eq!(breadth_rows, 256 + 1);
    assert!(matches!(
        rows.iter().rev().find(|row| row.depth == 1).unwrap().kind,
        ExplorerTreeRowKind::Omitted {
            count: 99_744,
            at_least: false
        }
    ));
    eprintln!(
        "PR-052-B measurement: 100 000-entry expansion: worker (scan + count) {worker:?}; \
         render thread (apply + flatten) {render_thread:?}"
    );
    assert!(
        render_thread < Duration::from_millis(16),
        "applying and flattening a 100 000-entry directory took {render_thread:?} on the \
         render thread -- D8's frame budget"
    );
}

/// The row bound. Forty directories of 300 entries, all open, is 40 x 257
/// rows -- over [`MAX_TREE_ROWS`].
fn wide_tree() -> (std::path::PathBuf, ExplorerTree, ProjectRootHandle) {
    let base = crate::test_support::remove_when_this_test_ends(std::env::temp_dir().join(format!(
            "tekstide-wide-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    let project = base.join("project");
    for dir in 0..40 {
        let dir = project.join(format!("d{dir:02}"));
        std::fs::create_dir_all(&dir).unwrap();
        for file in 0..300 {
            std::fs::File::create(dir.join(format!("f{file:03}"))).unwrap();
        }
    }
    let root = handle(&project);
    let mut tree = ExplorerTree::default();
    tree.request_scan(Path::new(""));
    settle(&mut tree, &root);
    for dir in 0..40 {
        tree.toggle(Path::new(&format!("d{dir:02}")));
    }
    settle(&mut tree, &root);
    (base, tree, root)
}

#[test]
fn the_tree_is_bounded_and_the_last_row_says_how_many_rows_it_did_not_keep() {
    let (_base, tree, _root) = wide_tree();
    let rows = tree.rows();
    assert_eq!(
        rows.len(),
        MAX_TREE_ROWS + 1,
        "the bound, plus the row that says so"
    );
    let last = rows.last().unwrap();
    let ExplorerTreeRowKind::RowsNotShown { count } = last.kind else {
        panic!(
            "the last row must say how many rows are not shown: {:?}",
            last.kind
        );
    };
    // 40 dirs x (256 entries + 1 omitted row) + 40 folder rows + the root's
    // own rows, minus what was kept.
    let total = 40 + 40 * (256 + 1);
    assert_eq!(MAX_TREE_ROWS + count, total);
}

#[test]
fn flattening_the_row_bound_is_cheap() {
    let (_base, tree, _root) = wide_tree();
    let started = Instant::now();
    let mut rows = 0;
    for _ in 0..20 {
        rows = tree.rows().len();
    }
    let per_call = started.elapsed() / 20;
    eprintln!("PR-052-B measurement: flattening {rows} rows takes {per_call:?}");
    assert!(
        per_call < Duration::from_millis(8),
        "flattening the bounded tree took {per_call:?} -- half a frame"
    );
}

// --- RFC-055 PR-055-C: `explorer.show_ignored` --------------------------------

use crate::project::root::{
    ExplorerFloorReason, ExplorerIgnoreRule, ExplorerIgnoreState, ExplorerNode, ExplorerNodeKind,
    ExplorerNodeState, FileAccessSymlinkStatus,
};

fn scanned_node(name: &str, kind: ExplorerNodeKind, ignore: ExplorerIgnoreState) -> ExplorerNode {
    ExplorerNode {
        name: name.to_owned(),
        relative_path: std::path::PathBuf::from(name),
        kind,
        state: ExplorerNodeState::Available,
        symlink_status: FileAccessSymlinkStatus::NoSymlink,
        ignore,
    }
}

/// A tree whose root scan holds `nodes`, built through `set_scan` -- the model
/// alone, no git and no filesystem.
fn tree_of(nodes: Vec<ExplorerNode>, truncated: bool) -> ExplorerTree {
    let fixture = HostileFixture::build("show-ignored-model", 1);
    let root = handle(&fixture.project);
    let mut scan = FileExplorerScanner
        .scan_directory(&root, "", &FileExplorerScanPolicy::linux_mvp())
        .expect("scans");
    scan.nodes = nodes;
    scan.truncated = truncated;
    scan.omitted_entries = if truncated { 7 } else { 0 };
    scan.ignore_rule = ExplorerIgnoreRule::Floor(ExplorerFloorReason::NotAsked);
    let mut tree = ExplorerTree::default();
    tree.set_scan(Path::new(""), Ok(scan));
    tree
}

/// D7, both ways. **Off (the default)**: entries git called ignored are not drawn,
/// and the directory says how many it left out -- nothing is hidden silently.
/// **On**: they are drawn. Dotfiles and everything git did not call ignored are
/// the same either way.
#[test]
fn ignored_entries_are_hidden_and_counted_unless_the_setting_shows_them() {
    let nodes = || {
        vec![
            scanned_node(
                "target",
                ExplorerNodeKind::Directory,
                ExplorerIgnoreState::Ignored,
            ),
            scanned_node(
                "src",
                ExplorerNodeKind::Directory,
                ExplorerIgnoreState::NotIgnored,
            ),
            scanned_node(
                ".env",
                ExplorerNodeKind::File,
                ExplorerIgnoreState::NotIgnored,
            ),
            scanned_node(
                ".gitignore",
                ExplorerNodeKind::File,
                ExplorerIgnoreState::NotIgnored,
            ),
            scanned_node(
                "debug.log",
                ExplorerNodeKind::File,
                ExplorerIgnoreState::Ignored,
            ),
            scanned_node(
                "unasked",
                ExplorerNodeKind::File,
                ExplorerIgnoreState::Unknown,
            ),
        ]
    };
    let mut tree = tree_of(nodes(), false);
    assert!(!tree.show_ignored(), "the default is not to show them");
    assert_eq!(
        outline(&tree),
        [
            "+src",
            ".env",
            ".gitignore",
            "unasked",
            "<ignored hidden 2>"
        ],
        "hidden and counted; dotfiles and unknown entries stay"
    );

    tree.set_show_ignored(true);
    let shown = outline(&tree);
    assert!(
        shown.contains(&"+target".to_owned()) && shown.contains(&"debug.log".to_owned()),
        "{shown:?}"
    );
    assert!(
        !shown.iter().any(|row| row.contains("ignored hidden")),
        "{shown:?}"
    );
    assert_eq!(shown.len(), 6);
}

/// The omitted tail (the per-directory cap) has **unknown** ignore state: it is
/// counted by its own row and this setting neither adds to nor hides it.
#[test]
fn the_setting_leaves_the_omitted_tail_alone() {
    let tree = tree_of(
        vec![
            scanned_node(
                "a.log",
                ExplorerNodeKind::File,
                ExplorerIgnoreState::Ignored,
            ),
            scanned_node(
                "b.txt",
                ExplorerNodeKind::File,
                ExplorerIgnoreState::NotIgnored,
            ),
        ],
        true,
    );
    assert_eq!(
        outline(&tree),
        ["b.txt", "<ignored hidden 1>", "<omitted 7>"]
    );
}

/// A directory whose every entry is ignored is not "empty" -- it is a directory
/// that says how many entries it hid.
#[test]
fn a_directory_of_only_ignored_entries_says_so_and_is_not_empty() {
    let tree = tree_of(
        vec![scanned_node(
            "a.log",
            ExplorerNodeKind::File,
            ExplorerIgnoreState::Ignored,
        )],
        false,
    );
    assert_eq!(outline(&tree), ["<ignored hidden 1>"]);
}

/// A hidden directory is not drawn, so it cannot be expanded even if it was
/// before: the setting governs what is drawn, and turning it on again puts the
/// expansion back.
#[test]
fn a_hidden_directorys_expansion_comes_back_when_the_setting_is_turned_on() {
    let mut tree = tree_of(
        vec![scanned_node(
            "target",
            ExplorerNodeKind::Directory,
            ExplorerIgnoreState::Ignored,
        )],
        false,
    );
    tree.set_show_ignored(true);
    assert_eq!(tree.toggle(Path::new("target")), ExplorerToggle::Expanded);
    tree.set_show_ignored(false);
    assert_eq!(outline(&tree), ["<ignored hidden 1>"]);
    tree.set_show_ignored(true);
    assert!(
        outline(&tree)[0].starts_with("-target"),
        "{:?}",
        outline(&tree)
    );
}
