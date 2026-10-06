use std::path::PathBuf;
use std::time::Instant;

use super::{GIT_SUBPROCESSES_PER_SCAN, SCAN_WINDOW, ScanBatcher, ScanRequest};

/// Feeds `events` (a change, given as the directory it happened in, already in time
/// order) through a batcher, draining before each one as a real loop would, then once
/// more at `end`. Returns every request issued.
fn run(events: &[(Instant, PathBuf)], end: Instant) -> Vec<ScanRequest> {
    let mut batcher = ScanBatcher::new();
    let mut requests = Vec::new();
    for (at, directory) in events {
        requests.extend(batcher.drain_due(*at));
        batcher.record(directory.clone(), *at);
    }
    requests.extend(batcher.drain_due(end));
    requests
}

/// The two numbers RFC-026's acceptance asks every batching claim to carry.
fn counts(requests: &[ScanRequest]) -> (usize, usize) {
    let scans = requests.len();
    (scans, scans * GIT_SUBPROCESSES_PER_SCAN as usize)
}

/// A build writing a thousand files into one directory inside one window costs one
/// scan, and two git subprocesses (RFC-026 R2). The events are a real burst: spread
/// through the first half of the window, not a loop with nothing between them.
#[test]
fn a_burst_of_a_thousand_events_into_one_directory_yields_one_scan_request_per_window() {
    let t0 = Instant::now();
    let dir = PathBuf::from("project/src");
    let events: Vec<(Instant, PathBuf)> = (0..1000u32)
        .map(|i| (t0 + SCAN_WINDOW * i / 2000, dir.clone()))
        .collect();

    let requests = run(&events, t0 + SCAN_WINDOW);
    let (scans, git) = counts(&requests);
    println!(
        "burst of 1000 into one directory, one window: scan requests {scans}, git subprocesses {git}"
    );

    assert_eq!(scans, 1, "one window, one scan: {requests:?}");
    assert_eq!(requests[0].directory, dir);
    assert_eq!(
        requests[0].coalesced_events, 1000,
        "every event is answered by that scan"
    );
    assert_eq!(git, 2);
}

/// The same thousand events spread across five windows cost one scan per window --
/// bounded by time, not by how many events arrived. The burst keeps going past the
/// first window, so this is not a single-window accident.
#[test]
fn sustained_churn_costs_one_scan_per_window_it_spans() {
    let t0 = Instant::now();
    let dir = PathBuf::from("project/build");
    let span = SCAN_WINDOW * 5;
    let events: Vec<(Instant, PathBuf)> = (0..1000u32)
        .map(|i| (t0 + span * i / 1000, dir.clone()))
        .collect();

    // Six windows of room, so the fifth window's own scan has time to close.
    let requests = run(&events, t0 + SCAN_WINDOW * 6);
    let (scans, git) = counts(&requests);
    println!(
        "1000 events over five windows, one directory: scan requests {scans}, git subprocesses {git}"
    );

    assert_eq!(
        scans, 5,
        "one scan per window the burst spans: {requests:?}"
    );
    assert_eq!(
        requests.iter().map(|r| r.coalesced_events).sum::<u32>(),
        1000
    );
    assert_eq!(git, 10);
}

/// Changes in different directories are never merged into one request: each
/// directory gets its own window, its own scan, and its own count.
#[test]
fn events_for_different_directories_do_not_silently_merge() {
    let t0 = Instant::now();
    let mut events = Vec::new();
    for i in 0..500u32 {
        let at = t0 + SCAN_WINDOW * i / 1000;
        events.push((at, PathBuf::from("project/a")));
        events.push((at, PathBuf::from("project/b")));
    }
    events.sort_by_key(|(at, _)| *at);

    let requests = run(&events, t0 + SCAN_WINDOW * 2);
    assert_eq!(
        requests.len(),
        2,
        "two directories, two requests: {requests:?}"
    );
    assert!(
        requests.iter().all(|r| r.coalesced_events == 500),
        "{requests:?}"
    );
    let directories: Vec<_> = requests.iter().map(|r| r.directory.clone()).collect();
    assert_eq!(
        directories,
        vec![PathBuf::from("project/a"), PathBuf::from("project/b")]
    );
}

/// A window closes at exactly `SCAN_WINDOW` after the change that opened it, not
/// before: at the instant it closes the scan is due, and a change arriving then
/// opens the next window rather than joining the closing one.
#[test]
fn a_window_closes_exactly_one_window_after_the_change_that_opened_it() {
    let t0 = Instant::now();
    let dir = PathBuf::from("project");
    let mut batcher = ScanBatcher::new();

    batcher.record(dir.clone(), t0);
    assert!(
        batcher
            .drain_due(t0 + SCAN_WINDOW - std::time::Duration::from_nanos(1))
            .is_empty(),
        "not due a nanosecond early"
    );
    let closing = batcher.drain_due(t0 + SCAN_WINDOW);
    assert_eq!(closing.len(), 1, "due exactly at the window's close");

    batcher.record(dir.clone(), t0 + SCAN_WINDOW);
    assert_eq!(
        batcher.pending_directories(),
        1,
        "a change at the close opens a new window"
    );
    assert_eq!(batcher.drain_due(t0 + SCAN_WINDOW * 2).len(), 1);
}

/// RFC-026 review 450: `GIT_SUBPROCESSES_PER_SCAN` checked against a real scan, not
/// taken on the strength of its provenance.
///
/// The product resolves `git` from its reviewed system directories before it ever
/// reads `PATH`, so a shim on `PATH` is not reached by a real scan (measured: zero
/// invocations). The scan is therefore driven through its own oracle seam
/// ([`ExplorerScanRequest::run_with_oracle`]), with the one difference that the git
/// executable is the logging shim rather than the resolved one. Everything else is
/// the production path: the directory read, and the same ignore step
/// (`ignored_entries_in_environment`: the gate, then `check-ignore`) that
/// `ignored_entries` runs. Every subprocess that step spawns is counted.
///
/// Run by hand (see `qa-evidence.md` § PR-026-A):
/// `cargo test -p tekstide-core --lib measured_git_subprocesses_per_real_scan -- --ignored --nocapture`
#[test]
#[ignore = "a measurement, not a check: see qa-evidence.md § PR-026-A (review 450)"]
fn measured_git_subprocesses_per_real_scan() {
    use crate::project::root::{ProjectRootHandle, ProjectRootValidator, SymlinkPolicy};
    use crate::project::{ExplorerTree, ProjectId, ProjectSession};
    use crate::runtime::git::ignored_entries_in_environment;
    use std::path::Path;
    use std::process::Command;

    let shim = "/dev/shm/tekshim/git";
    let shim_log = Path::new("/dev/shm/tekshim/log");
    assert!(
        Path::new(shim).is_file() && shim_log.exists(),
        "the logging git shim must exist; see qa-evidence.md § PR-026-A"
    );
    let repo = Path::new("/dev/shm/tekwatch-repo");
    let home = Path::new("/dev/shm/tekwatch-home");
    let _ = std::fs::remove_dir_all(repo);
    std::fs::create_dir_all(repo.join("sub")).unwrap();
    std::fs::create_dir_all(home).unwrap();
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    std::fs::write(repo.join("sub/b.txt"), "b").unwrap();
    std::fs::write(repo.join("sub/c.log"), "c").unwrap();
    std::fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
    assert!(
        Command::new("/usr/bin/git")
            .args(["init", "-q"])
            .current_dir(repo)
            .status()
            .unwrap()
            .success()
    );

    let valid = ProjectRootValidator
        .validate(repo, SymlinkPolicy::FailClosed)
        .expect("the measurement repository validates");
    let session = ProjectSession::new(
        ProjectId::for_test(1),
        valid.display_name,
        valid.selected_path,
        valid.canonical_path,
    );
    let root = ProjectRootHandle::from_project_session(&session);
    let env = vec![
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("HOME".to_owned(), home.display().to_string()),
    ];

    let lines = || std::fs::read_to_string(shim_log).unwrap();
    std::fs::write(shim_log, "").unwrap();
    let mut tree = ExplorerTree::default();
    let run_scans = |tree: &mut ExplorerTree, path: &str| -> Vec<String> {
        tree.request_scan(Path::new(path));
        let before = lines().lines().count();
        for request in tree.scan_requests(&root) {
            let completed = request.run_with_oracle(&|directory, names| {
                ignored_entries_in_environment(directory, names, shim, &env)
            });
            assert!(tree.apply(completed));
        }
        lines().lines().skip(before).map(str::to_owned).collect()
    };

    let first = run_scans(&mut tree, "");
    let warm = run_scans(&mut tree, "sub");
    let warm_again = run_scans(&mut tree, "sub");

    for (label, commands) in [
        ("first scan in this process", &first),
        ("second scan, warm", &warm),
        ("third scan, warm", &warm_again),
    ] {
        println!("{label}: {} git subprocesses", commands.len());
        for command in commands.iter() {
            println!("    git {command}");
        }
    }
    assert!(
        !warm.is_empty(),
        "the shim recorded nothing: the measurement would be vacuous"
    );
}
