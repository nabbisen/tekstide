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
    assert_eq!(
        names.len(),
        3 - fixture.escaping_refused.len(),
        "escaping/ holds every name the filesystem accepted, no fewer, no more: {names:?}"
    );
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
    if fixture.escaping_refused.is_empty() {
        let non_utf8 = names
            .iter()
            .find(|name| name.as_bytes().starts_with(b"bad-"))
            .expect("the non-UTF-8 name exists");
        assert!(non_utf8.to_str().is_none(), "and one is not text at all");
        assert!(
            non_utf8.to_string_lossy().contains('\u{FFFD}'),
            "lossy conversion invents a replacement character the file does not have"
        );
    } else {
        eprintln!(
            "skipped: the filesystem refused the non-UTF-8 name: {:?}",
            fixture.escaping_refused
        );
    }

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
    assert_eq!(
        scan.nodes.len(),
        3 - fixture.escaping_refused.len(),
        "every hostile name the filesystem accepted is still a row"
    );

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
    if fixture.escaping_refused.is_empty() {
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
    } else {
        eprintln!(
            "skipped: the filesystem refused the non-UTF-8 name: {:?}",
            fixture.escaping_refused
        );
    }
}

/// RFC-052's checklist expects three hostile names in `escaping/`; APFS
/// (unlike ext4) refuses to create a file whose name is not valid UTF-8, so
/// macOS's fixture holds two. This exists so that gap is a named, permanent
/// fact in the suite -- `cargo test -- --list` shows it whether or not
/// anyone ever reads the two assertions above that quietly shrink around it.
#[test]
fn the_non_utf8_escaping_row_is_recorded_as_refused_on_a_filesystem_that_cannot_hold_it() {
    let fixture = HostileFixture::build("non-utf8-coverage", 3);
    if cfg!(target_os = "macos") {
        assert_eq!(
            fixture.escaping_refused.len(),
            1,
            "APFS is expected to refuse exactly the non-UTF-8 name: {:?}",
            fixture.escaping_refused
        );
        assert_eq!(fixture.escaping_refused[0].0, non_utf8_name());
    } else {
        assert!(
            fixture.escaping_refused.is_empty(),
            "this filesystem unexpectedly refused a hostile name: {:?}",
            fixture.escaping_refused
        );
    }
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
    // Review 612: a flat wall-clock budget missed a 24x regression entirely
    // (`OMITTED_COUNT_LIMIT` bounded the stats, not the walk -- counting
    // every one of a huge directory's omitted entries cost ~24 ms against a
    // small directory's ~0.5 ms, and a 2 s budget could not see either
    // side). Machine-independent instead: a small and a huge directory are
    // scanned in the *same run*, and it is the ratio between them that is
    // asserted. "Bounded by the cap, not by the directory" means that ratio
    // stays flat regardless of which machine runs it.
    let small_fixture = HostileFixture::build("breadth-small-ratio", SMALL_BREADTH);
    let big_fixture = HostileFixture::build("breadth-big-ratio", BREADTH_ENTRIES);

    // Best-of-5, not worst-of-5: a ratio is a *scaling* assertion, not a
    // *budget* one (the budget lives in qa-evidence.md and the measurement
    // harness, which do use worst-of-N -- what a user can suffer). Worst-of-N
    // amplifies noise into the ratio in both directions: one scheduler spike
    // on the big side trips the assertion on its own, and a spike on the
    // small side hides a real regression by inflating the denominator.
    // Best-of-N discards both.
    let best_of_5 = |fixture: &HostileFixture| -> Duration {
        let mut best = Duration::MAX;
        for _ in 0..5 {
            let start = Instant::now();
            let result = scan(fixture, "breadth").expect("scans");
            best = best.min(start.elapsed());
            assert_eq!(result.nodes.len(), 256, "the per-level cap bounds the rows");
            assert!(result.truncated);
        }
        best
    };

    let small_best = best_of_5(&small_fixture);
    let big_best = best_of_5(&big_fixture);
    eprintln!(
        "PR-052-A/review 612 measurement: best of 5, {SMALL_BREADTH} entries = {small_best:?}; \
         {BREADTH_ENTRIES} entries = {big_best:?}"
    );

    // A floor under the small side: near-zero on a very fast machine would
    // make the ratio meaningless. A capped scan's real floor is a few
    // hundred microseconds; 1 ms is a generous margin above that. This
    // loosens the ratio on a fast machine (a larger denominator shrinks it),
    // which is the safe direction for a floor to err in.
    let small_best = small_best.max(Duration::from_millis(1));
    let ratio = big_best.as_secs_f64() / small_best.as_secs_f64();
    assert!(
        ratio < 8.0,
        "a capped scan's cost must not scale with the real directory's size: \
         {BREADTH_ENTRIES} entries took {ratio:.1}x as long as {SMALL_BREADTH} entries \
         ({big_best:?} vs {small_best:?})"
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
    //
    // Review 618: on macOS, `DEPTH_LEVELS` is 200, not 1500 (see its own
    // doc comment in `hostile_fixture.rs`) -- a decision, named here so it
    // is not mistaken for silently-equal coverage: 1500 was chosen to
    // guarantee a *recursive* walker would overflow its stack; 200 does
    // not stress any stack. On macOS this test still proves "one level per
    // scan, nothing recurses" -- the property its own name states -- but
    // it is no longer the stack canary the same test is on Linux.
    // Review 617: sample points above `DEPTH_LEVELS` would sample past
    // the tree's own bottom on a platform where that constant is smaller
    // (macOS) -- filtered, not hardcoded, so the curve this measures
    // narrows with the constant instead of needing its own platform split.
    let samples: Vec<usize> = [0, 1, 10, 100, 500, 1000, DEPTH_LEVELS]
        .into_iter()
        .filter(|level| *level <= DEPTH_LEVELS)
        .collect();
    let mut relative = String::from("depth");
    let mut timings = Vec::new();
    for level in 0..=DEPTH_LEVELS {
        if samples.contains(&level) {
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
    assert_eq!(names, ["src", "README.md"], "folders first");
    assert!(
        control
            .nodes
            .iter()
            .all(|node| node.state == ExplorerNodeState::Available)
    );
}

#[test]
fn a_capped_scan_counts_what_it_left_out_and_says_when_it_stopped_counting() {
    let fixture = HostileFixture::build("omitted-count", SMALL_BREADTH);
    let root = handle(&fixture);

    // 300 entries, cap 256: 44 left out, counted exactly.
    let exact = FileExplorerScanner
        .scan_directory(&root, "breadth", &FileExplorerScanPolicy::linux_mvp())
        .expect("scans");
    assert!(exact.truncated);
    assert_eq!(exact.omitted_entries, SMALL_BREADTH - 256);
    assert!(!exact.omitted_is_lower_bound);

    // A count limit below the truth stops counting and says so: the row
    // must not state a number the scanner did not finish.
    let policy = FileExplorerScanPolicy {
        omitted_count_limit: 10,
        ..FileExplorerScanPolicy::linux_mvp()
    };
    let stopped = FileExplorerScanner
        .scan_directory(&root, "breadth", &policy)
        .expect("scans");
    assert_eq!(stopped.omitted_entries, 1 + 10);
    assert!(stopped.omitted_is_lower_bound);

    // A directory that fits is not truncated and counts nothing.
    let fits = FileExplorerScanner
        .scan_directory(&root, "control", &FileExplorerScanPolicy::linux_mvp())
        .expect("scans");
    assert!(!fits.truncated);
    assert_eq!(fits.omitted_entries, 0);
}
