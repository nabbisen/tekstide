use std::path::{Path, PathBuf};
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

// --- RFC-026 slice B: the scope, the refusal, and the platform seam -----------------

use std::collections::BTreeSet;

use super::backend::{WatchBackend, WatchRefusal, guard_watch_call};
use super::{WatchScope, WatchState, desired_directories};

/// A backend that places watches in a set, and refuses once `budget` are placed --
/// the stand-in for the kernel's per-user limit (RFC-026 H2), which a test cannot
/// lower on a development machine.
struct FakeBackend {
    placed: BTreeSet<PathBuf>,
    budget: usize,
    refusal_detail: &'static str,
    /// Directories that do not exist. Every other directory exists.
    missing: BTreeSet<PathBuf>,
}

impl FakeBackend {
    fn with_budget(budget: usize) -> Self {
        Self {
            placed: BTreeSet::new(),
            budget,
            refusal_detail: "the fake backend's budget is exhausted",
            missing: BTreeSet::new(),
        }
    }
}

impl WatchBackend for FakeBackend {
    fn watch_directory(&mut self, directory: &Path) -> Result<(), WatchRefusal> {
        if self.placed.len() >= self.budget {
            return Err(WatchRefusal::new(self.refusal_detail));
        }
        self.placed.insert(directory.to_path_buf());
        Ok(())
    }

    fn unwatch_directory(&mut self, directory: &Path) {
        self.placed.remove(directory);
    }

    fn directory_exists(&self, directory: &Path) -> bool {
        !self.missing.contains(directory)
    }
}

fn dirs(paths: &[&str]) -> BTreeSet<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

/// Expanding, collapsing and closing a project move the watched count by exactly the
/// directories each one names (R6, and the "scope that only grows is a defect" rule).
/// Counted before and after every step, not inferred.
#[test]
fn expanding_collapsing_and_closing_move_the_watched_count_exactly() {
    let root = Path::new("/project");
    let mut scope = WatchScope::new();
    let mut backend = FakeBackend::with_budget(100);

    let desired = desired_directories(root, &[], &[]);
    scope.reconcile(&desired, &mut backend);
    assert_eq!(
        scope.watched_count(),
        1,
        "the root alone, before any expansion"
    );

    let desired = desired_directories(root, &[PathBuf::from("src"), PathBuf::from("src/bin")], &[]);
    let change = scope.reconcile(&desired, &mut backend);
    assert_eq!(change.added, 2);
    assert_eq!(scope.watched_count(), 3, "root, src, src/bin");

    let desired = desired_directories(root, &[PathBuf::from("src")], &[]);
    let change = scope.reconcile(&desired, &mut backend);
    assert_eq!(change.removed, 1, "collapsing src/bin removes its watch");
    assert_eq!(scope.watched_count(), 2);
    assert!(!scope.is_watched(&root.join("src/bin")));

    let change = scope.reconcile(&BTreeSet::new(), &mut backend);
    assert_eq!(change.removed, 2, "closing the project removes every watch");
    assert_eq!(scope.watched_count(), 0);
    assert!(
        backend.placed.is_empty(),
        "the platform holds nothing for a closed project"
    );
}

/// An open document's folder is watched without being expanded (D1).
#[test]
fn an_open_documents_folder_is_watched_without_being_expanded() {
    let root = Path::new("/project");
    let desired = desired_directories(root, &[], &[PathBuf::from("notes")]);
    assert_eq!(desired, dirs(&["/project", "/project/notes"]));
}

/// Budget exhaustion, forced (R3): the refusal stops watching, drops every watch the
/// scope held, and says so. No panic reaches the caller.
#[test]
fn a_refused_watch_stops_watching_and_drops_every_watch_without_crashing() {
    let root = Path::new("/project");
    let expanded: Vec<PathBuf> = (0..10).map(|i| PathBuf::from(format!("d{i}"))).collect();
    let desired = desired_directories(root, &expanded, &[]);
    let mut scope = WatchScope::new();
    let mut backend = FakeBackend::with_budget(4);

    scope.reconcile(&desired, &mut backend);

    assert_eq!(scope.state(), &WatchState::Stopped);
    assert_eq!(
        scope.watched_count(),
        0,
        "every watch is dropped, not left half-placed"
    );
    assert!(
        backend.placed.is_empty(),
        "the platform holds none of them either"
    );
    assert!(scope.last_refusal().is_some(), "the cause is kept for logs");
}

/// C1: the degradation depends on the fact of a refusal, not its wording. Two
/// different refusals produce the same state, the same empty scope, and no retry.
#[test]
fn the_degradation_does_not_depend_on_which_refusal_it_gets() {
    let root = Path::new("/project");
    let desired = desired_directories(root, &[PathBuf::from("a"), PathBuf::from("b")], &[]);

    let mut states = Vec::new();
    for detail in [
        "the platform limit was reached",
        "some other failure entirely",
    ] {
        let mut scope = WatchScope::new();
        let mut backend = FakeBackend::with_budget(1);
        backend.refusal_detail = detail;
        scope.reconcile(&desired, &mut backend);
        states.push((scope.state().clone(), scope.watched_count()));
    }
    assert_eq!(states[0], states[1]);
    assert_eq!(states[0], (WatchState::Stopped, 0));
}

/// Stopped means stopped: a reconcile does not retry the platform on its own, and
/// only a reopen (`resume`) places watches again.
#[test]
fn a_stopped_scope_does_not_retry_until_it_is_resumed() {
    let root = Path::new("/project");
    let desired = desired_directories(root, &[PathBuf::from("a")], &[]);
    let mut scope = WatchScope::new();
    let mut backend = FakeBackend::with_budget(0);
    scope.reconcile(&desired, &mut backend);
    assert_eq!(scope.state(), &WatchState::Stopped);

    backend.budget = 100;
    let change = scope.reconcile(&desired, &mut backend);
    assert_eq!(change.added, 0, "no retry while stopped");
    assert_eq!(scope.state(), &WatchState::Stopped);

    scope.resume();
    let change = scope.reconcile(&desired, &mut backend);
    assert_eq!(change.added, 2, "a reopen places the watches again");
    assert_eq!(scope.state(), &WatchState::Live);
}

/// H1: a panic inside a watch call is caught and reported as a refusal. The panic is
/// real, not simulated by returning an error.
#[test]
fn a_panic_inside_a_watch_call_is_a_refusal_not_a_crash() {
    let refused: Result<(), WatchRefusal> =
        guard_watch_call::<()>(|| panic!("notify's event thread is gone"));
    assert!(refused.is_err());
    assert_eq!(
        refused.unwrap_err().detail(),
        "the watcher's thread panicked"
    );
}

/// The production backend places and removes a real, non-recursive watch on a real
/// directory on this machine. The kernel answers; nothing is simulated.
#[test]
fn the_real_backend_places_and_removes_a_watch_on_a_real_directory() {
    let directory = Path::new("/dev/shm/tekwatch-real");
    let _ = std::fs::remove_dir_all(directory);
    std::fs::create_dir_all(directory).unwrap();

    let (sender, _receiver) = std::sync::mpsc::channel();
    let mut backend = super::NotifyBackend::new(sender).expect("notify starts on Linux");
    backend
        .watch_directory(directory)
        .expect("the kernel places a watch on a directory this process owns");
    backend.unwatch_directory(directory);
    let _ = std::fs::remove_dir_all(directory);
}

/// Review 453: a directory that has gone is not a refusal and does not stop watching.
/// The policy checks existence itself, so the platform is never asked about it.
#[test]
fn a_directory_that_has_gone_is_not_watched_and_does_not_stop_watching() {
    let root = Path::new("/project");
    let desired = desired_directories(root, &[PathBuf::from("src"), PathBuf::from("build")], &[]);
    let mut scope = WatchScope::new();
    let mut backend = FakeBackend::with_budget(10);
    backend.missing.insert(root.join("build"));

    let change = scope.reconcile(&desired, &mut backend);

    assert_eq!(
        scope.state(),
        &WatchState::Live,
        "a gone directory is not a refusal"
    );
    assert!(scope.last_refusal().is_none());
    assert_eq!(change.added, 2, "the root and src are watched");
    assert!(!scope.is_watched(&root.join("build")));
    assert!(
        !backend.placed.contains(&root.join("build")),
        "the platform was never asked"
    );
}

/// Review 453: a watched directory that is deleted later is dropped as an ordinary
/// removal. Nothing stops, and no further watch is placed for it.
#[test]
fn a_watched_directory_that_is_deleted_later_is_dropped_without_stopping() {
    let root = Path::new("/project");
    let desired = desired_directories(root, &[PathBuf::from("build")], &[]);
    let mut scope = WatchScope::new();
    let mut backend = FakeBackend::with_budget(10);
    scope.reconcile(&desired, &mut backend);
    assert_eq!(scope.watched_count(), 2);

    backend.missing.insert(root.join("build"));
    let change = scope.reconcile(&desired, &mut backend);

    assert_eq!(
        change.removed, 1,
        "the deleted directory's watch is dropped"
    );
    assert_eq!(scope.watched_count(), 1);
    assert_eq!(scope.state(), &WatchState::Live);
}

/// Review 453, on the real filesystem: a directory the expanded tree still lists, but
/// which a clean has removed, does not stop watching. The production backend answers
/// the existence question itself.
#[test]
fn a_real_directory_removed_before_reconciling_does_not_stop_watching() {
    let base = Path::new("/dev/shm/tekwatch-gone");
    let _ = std::fs::remove_dir_all(base);
    std::fs::create_dir_all(base.join("keep")).unwrap();
    std::fs::create_dir_all(base.join("build")).unwrap();

    let (sender, _receiver) = std::sync::mpsc::channel();
    let mut backend = super::NotifyBackend::new(sender).expect("notify starts on Linux");
    let desired = desired_directories(base, &[PathBuf::from("keep"), PathBuf::from("build")], &[]);
    let mut scope = WatchScope::new();

    std::fs::remove_dir_all(base.join("build")).unwrap();
    let change = scope.reconcile(&desired, &mut backend);

    assert_eq!(
        scope.state(),
        &WatchState::Live,
        "a removed directory is not a refusal"
    );
    assert_eq!(change.added, 2, "the base and keep are watched");
    assert!(!scope.is_watched(&base.join("build")));
    scope.reconcile(&BTreeSet::new(), &mut backend);
    let _ = std::fs::remove_dir_all(base);
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
