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
use crate::project::ProjectId;
use crate::project::root::{
    FileAccessBlockedReason, FileAccessSymlinkStatus, ProjectRootHandle, ProjectRootValidator,
    SymlinkPolicy,
};
use crate::text_safety::quote_untrusted;

/// A small breadth for tests that need the other rows but not 100,000
/// files: over the 256 cap, so truncation is still exercised.
const SMALL_BREADTH: usize = 300;

/// RFC-052 review 689: a second breadth for
/// `a_hundred_thousand_entry_directory_is_a_bounded_scan_not_a_walk_of_all_of_it`'s
/// own ratio -- see that test's own comment for why comparing a
/// sub-limit directory (`SMALL_BREADTH`) against a supra-limit one
/// (`BREADTH_ENTRIES`) could never be machine-independent. Derived from
/// `OMITTED_COUNT_LIMIT` itself, not a literal, so it cannot silently
/// fall below the limit if that constant moves.
const SUPRA_LIMIT_BREADTH: usize = super::OMITTED_COUNT_LIMIT * 2;

/// RFC-052 review 693: the breadth ratio's own bound, used by both
/// `a_hundred_thousand_entry_directory_is_a_bounded_scan_not_a_walk_of_all_of_it`
/// (`ratio < BREADTH_RATIO_BOUND`) and its signal-side ablation,
/// `an_unbounded_policy_makes_the_breadth_ratio_cross_its_bound`
/// (`ratio > BREADTH_RATIO_BOUND`) -- one derived constant rather than
/// `2.5` written by hand in three places, two of them opposite
/// assertions. A future re-derivation that updates one bound but misses
/// the ablation would have the ablation assert a crossing of a number
/// that is no longer the real bound, which is exactly the failure this
/// constant exists to make impossible: change it once, and both
/// assertions and both failure messages move together. Derivation and
/// measured history: this test's own comment, `052-file-explorer/
/// qa-evidence.md`.
const BREADTH_RATIO_BOUND: f64 = 2.5;

fn handle(fixture: &HostileFixture) -> ProjectRootHandle {
    let root = ProjectRootValidator
        .validate(&fixture.project, SymlinkPolicy::FailClosed)
        .expect("the fixture's project root validates");
    ProjectRootHandle::new(ProjectId::for_test(1), root)
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
    // side).
    //
    // Review 689: the original comparison here (`SMALL_BREADTH` -- 300,
    // under `OMITTED_COUNT_LIMIT` -- against `BREADTH_ENTRIES` -- 100 000,
    // far over it) could never be machine-independent, structurally, not
    // just unluckily. `.take(policy.omitted_count_limit + 1)` means a
    // scan's cost is *constant* for every directory above
    // `OMITTED_COUNT_LIMIT` entries: 256 stat'd rows plus
    // `OMITTED_COUNT_LIMIT + 1` bare pulls, no matter how much bigger the
    // real directory gets past that. Comparing a sub-limit directory
    // (300 pulls) against a supra-limit one (`OMITTED_COUNT_LIMIT + 1`
    // pulls) measured a fixed ~33x work difference, not directory-size
    // scaling, and the ratio between fixed-but-different work depends
    // entirely on a given machine's own balance of per-scan overhead
    // against per-pull cost -- measured directly: 1.9 on a 4-core CI
    // runner, 4.94 on a 32-core dev machine, neither matching this
    // comment's own former "stays flat regardless of which machine runs
    // it" claim.
    //
    // Comparing two directories that **both** exceed `OMITTED_COUNT_LIMIT`
    // instead makes both sides do byte-identical work (256 stats,
    // `OMITTED_COUNT_LIMIT + 1` pulls, each), so machine speed cancels and
    // so does contention that affects both sides alike. `SUPRA_LIMIT_BREADTH`
    // (`OMITTED_COUNT_LIMIT * 2`) replaces `SMALL_BREADTH` here; the 1ms
    // floor the old comparison needed (to stop a near-zero small side from
    // making the ratio meaningless) is gone, since there is no near-zero
    // side left to protect against.
    let small_fixture = HostileFixture::build("breadth-small-ratio", SUPRA_LIMIT_BREADTH);
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
    let ratio = big_best.as_secs_f64() / small_best.as_secs_f64();
    eprintln!(
        "PR-052-A/review 689 measurement: best of 5, {SUPRA_LIMIT_BREADTH} entries = \
         {small_best:?}; {BREADTH_ENTRIES} entries = {big_best:?}; ratio = {ratio:.3}"
    );

    // Review 689's own pre-registered bound: measured directly before
    // writing this (not derived from the `.take` cost model alone, per
    // that review's own requirement), well under the ~1.5 threshold that
    // would have meant the cost model was wrong. On this dev machine (32
    // cores): clean, 0.96-0.99 across five isolated runs; under real
    // concurrent load (a second full `cargo test --workspace` running at
    // the same time), 0.77-0.99 across five more -- the comparison stays
    // *below* 1.0 even contended, confirming machine speed and shared
    // contention both cancel between two directories doing identical
    // work, which a sub-limit-vs-supra-limit comparison could never do
    // (see this test's own comment above). This replaces the comment's
    // former "stays flat regardless of which machine runs it" claim with
    // a real figure rather than restating it unchanged.
    //
    // Review 691: this bound was measured on three machines (dev Linux
    // 1.018, CI Linux `gate` 1.001, macOS `macos-measurement` 1.283 --
    // full table in `052-file-explorer/qa-evidence.md`), via the
    // dedicated `--nocapture` CI steps that review required. macOS
    // carries its own size-dependent cost term even in the bounded case
    // -- the "both sides do identical work, so the ratio is 1.0"
    // argument holds at the syscall-count level on every platform and
    // at the cost level only on Linux, which is why macOS's own ratio
    // is the highest of the three and the margin to watch, not a
    // defect.
    //
    // Review 692: a second run moved macOS's own noise from 1.283 to
    // 1.739 (×1.36, asymmetric inflation between the two sides -- APFS's
    // size-dependent term above, hit harder on the bigger directory by
    // whatever degraded that run). The signal side, extrapolated in
    // review 691 at "~3.2-4.3x", is now directly measured by
    // `an_unbounded_policy_makes_the_breadth_ratio_cross_its_bound`
    // below: `gate` 3.774, this dev machine 6.664, macOS 13.664 -- the
    // macOS estimate was 3.2x low. **Re-derived bound: `BREADTH_RATIO_BOUND`
    // = 2.5**, from the window between worst measured noise and worst
    // measured signal, [1.739, 3.774] (geometric mean 2.56) -- 1.44x
    // noise margin, 1.51x signal margin, both in the same band, against
    // 2.0's lopsided 1.15x/1.89x. Raised before a red, not after: the
    // derivation above is what justifies it, the same discipline review
    // 688 required when 8.0 was wrong to keep (there, the baseline
    // itself was machine-dependent; here, both sides of the bound are
    // now measured on three machines and the optimum simply moved once
    // real numbers replaced estimates).
    //
    // Review 693: a third macOS point (1.303, fitted `s` 0.0385
    // µs/entry) settled the question above -- not a climb toward ~1.9;
    // 1.739 was one run slow on *both* sides (fitted `s` 0.167, a 4x
    // move from the other two points' 0.039-0.043). **The size-dependent
    // term is itself load-sensitive**, a platform constant times
    // whatever pressure the volume is under, which is why macOS's ratio
    // wanders while `gate`'s sits at 1.001-1.010 across the same runs.
    // Three points now, not two; full table and the arithmetic showing
    // this re-derivation bought real protection (pairing the fastest
    // observed small side with the worst observed `s` gives 2.31 --
    // would have reddened at the old 2.0, clears 2.5) in
    // `qa-evidence.md`. If a real climb past ~1.9 is ever observed, the
    // honest conclusion will be that a ratio of two supra-limit
    // directories does not work on APFS, and macOS will need a
    // different assertion, not a third bound; the standing step keeps
    // watching.
    assert!(
        ratio < BREADTH_RATIO_BOUND,
        "two directories that both exceed OMITTED_COUNT_LIMIT should cost the same -- both do \
         256 stats and OMITTED_COUNT_LIMIT + 1 bare pulls by construction -- but \
         {BREADTH_ENTRIES} entries took {ratio:.2}x as long as {SUPRA_LIMIT_BREADTH} entries \
         ({big_best:?} vs {small_best:?}), against a bound of {BREADTH_RATIO_BOUND}. This is a \
         scaling assertion, not a load-sensitive one (best-of-5 on both sides, a same-run ratio \
         comparing equal work) -- but a red here was once dispositioned against load with no \
         measurement to back it (rfcs/handoffs/test-process-leak.md, 2026-11-07), so the \
         reading is included rather than assumed either way: {}",
        crate::test_support::load_average_report()
    );
}

/// Review 691's required ablation: the bound just above moved 8.0 -> 2.0
/// (review 689) and then, once this ablation's own measurement replaced
/// an extrapolation, 2.0 -> 2.5 (review 692). RFC-074's own standing
/// rule ("each bound move re-ablates... lowering a bound makes the
/// assertion stricter; it does not re-prove the measurement underneath
/// still discriminates") applies to that move. The noise side is now
/// calibrated on three machines (this test's own comment, above,
/// `qa-evidence.md`); the signal side -- can a real review-612-class
/// regression still make the ratio cross the bound -- had never been
/// measured directly, only extrapolated, before this test existed.
///
/// `FileExplorerScanPolicy::omitted_count_limit` is a `pub` field on a
/// `pub` struct, so review 612's exact regression (the drain walking
/// every omitted entry instead of stopping at the cap) reproduces with
/// no production change and nothing to restore: a policy whose
/// `omitted_count_limit` is far past either fixture's own entry count
/// makes the drain walk all of it, so the two supra-limit fixtures stop
/// doing identical work (`SUPRA_LIMIT_BREADTH - 256` pulls against
/// `BREADTH_ENTRIES - 256`, a real 5x difference, not the bounded
/// comparison's `OMITTED_COUNT_LIMIT + 1` on both sides).
///
/// **Not the same shape as the per-pull-cost class the bounded
/// comparison above cannot see (review 692's correction).** A
/// per-pull-cost regression inside the *bounded* drain leaves both
/// sides pulling `OMITTED_COUNT_LIMIT + 1` entries and makes each pull
/// more expensive -- equal work, more costly, ratio unchanged. This
/// ablation makes the work *unequal* instead; it is a different
/// regression, and this test sees only the size-scaling one (under a
/// per-pull regression its own ratio would rise toward the bounded
/// comparison's own ~5x and pass even more strongly, since it has no
/// upper bound). The per-pull class remains un-guarded by either test
/// -- `052-file-explorer/qa-evidence.md`'s own "2026-11-17 (review 691)"
/// section records it as a known, named, un-guarded property.
///
/// A permanent test, not a one-off capture, per the response's own
/// preference: it keeps discriminating after the next change to the
/// scanner, where a one-off measurement would not.
#[test]
fn an_unbounded_policy_makes_the_breadth_ratio_cross_its_bound() {
    let unbounded = FileExplorerScanPolicy {
        omitted_count_limit: 1_000_000,
        ..FileExplorerScanPolicy::linux_mvp()
    };
    let small_fixture = HostileFixture::build("breadth-small-unbounded", SUPRA_LIMIT_BREADTH);
    let big_fixture = HostileFixture::build("breadth-big-unbounded", BREADTH_ENTRIES);

    let best_of_5 = |fixture: &HostileFixture| -> Duration {
        let mut best = Duration::MAX;
        for _ in 0..5 {
            let start = Instant::now();
            let result = FileExplorerScanner
                .scan_directory(&handle(fixture), "breadth", &unbounded)
                .expect("scans");
            best = best.min(start.elapsed());
            assert_eq!(
                result.nodes.len(),
                256,
                "the per-level cap still bounds the rows"
            );
            assert!(
                result.truncated,
                "max_children_per_directory still caps the rows regardless of omitted_count_limit"
            );
        }
        best
    };

    let small_best = best_of_5(&small_fixture);
    let big_best = best_of_5(&big_fixture);
    let ratio = big_best.as_secs_f64() / small_best.as_secs_f64();
    eprintln!(
        "PR-052-A/review 691 ablation (omitted_count_limit=1_000_000): best of 5, \
         {SUPRA_LIMIT_BREADTH} entries = {small_best:?}; {BREADTH_ENTRIES} entries = \
         {big_best:?}; ratio = {ratio:.3}"
    );

    assert!(
        ratio > BREADTH_RATIO_BOUND,
        "an unbounded omitted_count_limit must reproduce review 612's own regression and cross \
         the bounded comparison's own bound ({BREADTH_RATIO_BOUND}) -- but {BREADTH_ENTRIES} \
         entries took only {ratio:.2}x as long as {SUPRA_LIMIT_BREADTH} entries ({big_best:?} \
         vs {small_best:?}), which would mean that bound cannot see the regression it exists \
         for on this machine: {}",
        crate::test_support::load_average_report()
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
