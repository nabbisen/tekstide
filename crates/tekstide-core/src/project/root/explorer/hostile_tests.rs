//! RFC-052 PR-052-A: the explorer's guards, judged against
//! [`super::hostile_fixture`] rather than against a README.
//!
//! The first test is the **falsifying ablation**: it runs the *same*
//! fixture through a deliberately guardless listing and asserts that the
//! hazards are real. If it ever stops passing, the fixture stopped being
//! hostile and every "the guard held" test below it proves nothing.

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::time::{Duration, Instant};

use super::hostile_fixture::{
    BIDI_NAME, BREADTH_ENTRIES, DEPTH_LEVELS, HostileFixture, NEWLINE_NAME, is_inside,
    non_utf8_name,
};
use super::{ExplorerNodeState, ExplorerScanError, FileExplorerScanPolicy, FileExplorerScanner};
use crate::project::root::{
    FileAccessBlockedReason, FileAccessSymlinkStatus, ProjectRootHandle, ProjectRootValidator,
    SymlinkPolicy,
};
use crate::project::{ProjectId, ProjectSession};
use crate::text_safety::quote_untrusted;

/// A small breadth for tests that need the other rows but not 100,000
/// files: over the 256 cap, so truncation is still exercised.
const SMALL_BREADTH: usize = 300;

fn handle(fixture: &HostileFixture) -> ProjectRootHandle {
    let root = ProjectRootValidator
        .validate(&fixture.project, SymlinkPolicy::FailClosed)
        .expect("the fixture's project root validates");
    let session = ProjectSession::new(
        ProjectId::for_test(1),
        root.display_name,
        root.selected_path,
        root.canonical_path,
    );
    ProjectRootHandle::from_project_session(&session)
}

fn scan(
    fixture: &HostileFixture,
    relative: &str,
) -> Result<super::ExplorerDirectoryScan, ExplorerScanError> {
    FileExplorerScanner.scan_directory(
        &handle(fixture),
        relative,
        &FileExplorerScanPolicy::linux_mvp(),
    )
}

/// The unreadable row needs a filesystem that actually refuses. As root
/// (or with `CAP_DAC_OVERRIDE`) mode 000 refuses nothing; a test that
/// went on to "prove the guard held" would prove a guard was never asked.
fn unreadable_directory_really_refuses(fixture: &HostileFixture) -> bool {
    fs::read_dir(fixture.unreadable_dir()).is_err()
}

// ---------------------------------------------------------------------
// The falsifying ablation: guards removed, the hazards are real.
// ---------------------------------------------------------------------

#[test]
fn with_the_guards_removed_the_hostile_fixture_actually_escapes_and_renders_raw() {
    let fixture = HostileFixture::build("ablation", 3);

    // (1) Escape: a naive walk that just follows the link lists content
    //     from *outside* the project root. `escape-dir` is spelled under
    //     the root and is not.
    let escape = fixture.escape_dir();
    assert!(
        !is_inside(&fixture.project, &escape),
        "the escape row must resolve outside the root, or it tests nothing"
    );
    let leaked: Vec<String> = fs::read_dir(&escape)
        .expect("a guardless read follows the link")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        leaked.iter().any(|name| name == "secret.txt"),
        "a guardless listing of the escape link shows outside content: {leaked:?}"
    );
    assert!(is_inside(&fixture.project, &fixture.in_root_link()));

    // (2) Raw names: the three the checklist names, drawn as-is.
    let names: Vec<_> = fs::read_dir(fixture.escaping_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 3);
    let lossy: Vec<String> = names
        .iter()
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    assert!(
        lossy.iter().any(|name| name.contains('\u{202E}')),
        "raw, the bidi-override name reaches a renderer intact: {lossy:?}"
    );
    assert!(
        lossy.iter().any(|name| name.contains('\n')),
        "raw, the newline name splits a row: {lossy:?}"
    );
    let non_utf8 = names
        .iter()
        .find(|name| name.as_bytes().starts_with(b"bad-"))
        .expect("the non-UTF-8 name exists");
    assert!(non_utf8.to_str().is_none(), "and one is not text at all");
    assert!(
        non_utf8.to_string_lossy().contains('\u{FFFD}'),
        "lossy conversion invents a replacement character the file does not have"
    );

    // (3) The other rows are real too.
    assert!(fs::read_dir(fixture.broken_link()).is_err());
    assert!(fs::symlink_metadata(fixture.broken_link()).is_ok());
    if unreadable_directory_really_refuses(&fixture) {
        assert!(fs::read_dir(fixture.unreadable_dir()).is_err());
    }
}

// ---------------------------------------------------------------------
// The guards, against the same fixture.
// ---------------------------------------------------------------------

#[test]
fn the_escape_rows_are_blocked_rows_and_the_escape_directory_cannot_be_scanned() {
    let fixture = HostileFixture::build("escape", SMALL_BREADTH);

    let links = scan(&fixture, "links").expect("the links directory itself is inside the root");
    let state = |name: &str| {
        links
            .nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("row {name} exists"))
    };

    for escaping in ["escape-dir", "escape-file"] {
        let node = state(escaping);
        assert_eq!(
            node.state,
            ExplorerNodeState::Blocked(FileAccessBlockedReason::SymlinkEscape),
            "{escaping}"
        );
        assert_eq!(
            node.symlink_status,
            FileAccessSymlinkStatus::EscapesRoot,
            "{escaping}"
        );
    }
    assert!(matches!(
        state("broken").state,
        ExplorerNodeState::Blocked(_)
    ));
    assert_eq!(state("in-root").state, ExplorerNodeState::Available);

    // Asking for the escape directory's contents is refused, not listed.
    let refused = scan(&fixture, "links/escape-dir");
    assert!(
        matches!(refused, Err(ExplorerScanError::Access(_))),
        "{refused:?}"
    );
    // ... and so is a `..` walk out of the root.
    assert!(scan(&fixture, "../outside").is_err());
}

#[test]
fn hostile_names_are_escaped_at_the_one_choke_point_and_the_raw_path_is_kept() {
    let fixture = HostileFixture::build("names", SMALL_BREADTH);
    let scan = scan(&fixture, "escaping").expect("scans");
    assert_eq!(scan.nodes.len(), 3, "every hostile name is still a row");

    for node in &scan.nodes {
        let shown = quote_untrusted(&node.name);
        let shown = shown.as_str();
        assert!(
            !shown.contains('\u{202E}'),
            "a directionality override reached display text: {shown:?}"
        );
        assert!(
            !shown.contains('\n'),
            "a newline reached display text: {shown:?}"
        );
    }

    let bidi = scan
        .nodes
        .iter()
        .find(|node| node.name == BIDI_NAME)
        .expect("bidi row");
    assert!(quote_untrusted(&bidi.name).as_str().contains("<U+202E>"));
    let newline = scan
        .nodes
        .iter()
        .find(|node| node.name == NEWLINE_NAME)
        .expect("newline row");
    assert!(quote_untrusted(&newline.name).as_str().contains("<U+000A>"));

    // The row's *path* stays byte-exact: opening the row must open the
    // file, not a lossy spelling of it.
    let raw = scan
        .nodes
        .iter()
        .find(|node| node.relative_path.file_name() == Some(non_utf8_name().as_os_str()))
        .expect("the non-UTF-8 row keeps its exact bytes in its path");
    assert!(raw.name.contains('\u{FFFD}'), "display name is lossy");
    assert!(
        fixture.project.join(&raw.relative_path).exists(),
        "and the path still opens the real file"
    );
}

#[test]
fn a_directory_of_many_entries_is_capped_and_says_it_was_truncated() {
    let fixture = HostileFixture::build("breadth-small", SMALL_BREADTH);
    let scan = scan(&fixture, "breadth").expect("scans");
    assert_eq!(scan.nodes.len(), 256);
    assert!(scan.truncated);
}

#[test]
fn a_hundred_thousand_entry_directory_is_a_bounded_scan_not_a_walk_of_all_of_it() {
    let built = Instant::now();
    let fixture = HostileFixture::build("breadth-full", BREADTH_ENTRIES);
    let build_time = built.elapsed();

    let mut worst = Duration::ZERO;
    let mut last = None;
    for _ in 0..5 {
        let start = Instant::now();
        let result = scan(&fixture, "breadth").expect("scans");
        worst = worst.max(start.elapsed());
        last = Some(result);
    }
    let result = last.unwrap();
    assert_eq!(result.nodes.len(), 256, "the per-level cap bounds the rows");
    assert!(result.truncated);
    eprintln!(
        "PR-052-A measurement: fixture of {BREADTH_ENTRIES} built in {build_time:?}; \
         capped scan of it: worst of 5 = {worst:?}"
    );
    // Deliberately loose: this pins "bounded by the cap, not by the
    // directory", not a machine's speed. D8's real budget is measured in
    // qa-evidence.md.
    assert!(
        worst < Duration::from_secs(2),
        "a capped scan of a huge directory took {worst:?}"
    );
}

#[test]
fn a_deeply_nested_tree_is_walked_one_level_per_scan_so_nothing_recurses() {
    let fixture = HostileFixture::build("depth", 3);
    // The scanner has no recursion to overflow: one directory per call.
    // Every level of the depth is *sampled*, not walked: resolving a path
    // is linear in its depth, so a full walk of 1500 levels is quadratic
    // (about 30 s here, measured) -- which is a fact about the cost of
    // depth, recorded in qa-evidence.md, not something to pay on every run.
    let mut relative = String::from("depth");
    let mut timings = Vec::new();
    for level in 0..=DEPTH_LEVELS {
        if [0, 1, 10, 100, 500, 1000, DEPTH_LEVELS].contains(&level) {
            let start = Instant::now();
            let scanned = scan(&fixture, &relative).unwrap_or_else(|error| {
                panic!("level {level} ({} bytes): {error}", relative.len())
            });
            timings.push((level, start.elapsed()));
            if level == DEPTH_LEVELS {
                assert_eq!(scanned.nodes.len(), 1);
                assert_eq!(scanned.nodes[0].name, "leaf.txt");
            } else {
                assert_eq!(scanned.nodes[0].name, "d", "level {level} has its child");
                assert_eq!(scanned.nodes[0].state, ExplorerNodeState::Available);
            }
        }
        relative.push_str("/d");
    }
    eprintln!("PR-052-A measurement: one scan at depth N: {timings:?}");
}

#[test]
fn an_unreadable_directory_is_a_refusal_the_caller_can_show_not_a_panic() {
    let fixture = HostileFixture::build("unreadable", 3);
    if !unreadable_directory_really_refuses(&fixture) {
        eprintln!(
            "skipped: this process can read a mode-000 directory (running as root?), \
             so the row cannot be exercised here"
        );
        return;
    }

    // The parent lists it (as a directory row)...
    let project = scan(&fixture, "").expect("the project root scans");
    assert!(project.nodes.iter().any(|node| node.name == "unreadable"));
    // ... and opening it is a typed error.
    assert!(matches!(
        scan(&fixture, "unreadable"),
        Err(ExplorerScanError::CannotReadDirectory { .. })
    ));
}

#[test]
fn the_control_tree_scans_as_an_ordinary_project() {
    let fixture = HostileFixture::build("control", 3);
    let control = scan(&fixture, "control").expect("scans");
    let names: Vec<&str> = control.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["README.md", "src"]);
    assert!(
        control
            .nodes
            .iter()
            .all(|node| node.state == ExplorerNodeState::Available)
    );
}
