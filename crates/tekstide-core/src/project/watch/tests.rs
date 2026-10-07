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
use super::directory::WatchedDirectory;
use super::{WatchScope, WatchState};

/// The scope tests build their desired set without a real project: the policy is
/// tested separately, against a real root, below.
fn desired_directories(
    root: &Path,
    expanded: &[PathBuf],
    open_document_directories: &[PathBuf],
) -> BTreeSet<WatchedDirectory> {
    std::iter::once(root.to_path_buf())
        .chain(expanded.iter().map(|relative| root.join(relative)))
        .chain(
            open_document_directories
                .iter()
                .map(|relative| root.join(relative)),
        )
        .map(WatchedDirectory::for_test)
        .collect()
}

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
    let paths: BTreeSet<PathBuf> = desired.iter().map(|d| d.path().to_path_buf()).collect();
    assert_eq!(paths, dirs(&["/project", "/project/notes"]));
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

// --- RFC-026 D7 (review 455): admission goes through the access policy, on a real root ---

fn real_session(label: &str) -> (crate::project::ProjectSession, PathBuf) {
    use crate::project::root::{ProjectRootValidator, SymlinkPolicy};
    use crate::project::{ProjectId, ProjectSession};

    let base = PathBuf::from(format!("/dev/shm/tekadmit-{label}"));
    let _ = std::fs::remove_dir_all(&base);
    let project = base.join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    std::fs::write(project.join("file.txt"), "f").unwrap();
    std::os::unix::fs::symlink(base.join("outside"), project.join("link-out")).unwrap();
    std::os::unix::fs::symlink(project.join("src"), project.join("link-in")).unwrap();

    let valid = ProjectRootValidator
        .validate(&project, SymlinkPolicy::FailClosed)
        .expect("the admission project root validates");
    let session = ProjectSession::new(
        ProjectId::for_test(1),
        valid.display_name,
        valid.selected_path,
        valid.canonical_path,
    );
    (session, base)
}

fn real_root(session: &crate::project::ProjectSession) -> crate::project::root::ProjectRootHandle {
    crate::project::root::ProjectRootHandle::from_project_session(session)
}

/// The policy admits a real directory by its canonical path, and admits an in-root
/// symlink to one, but refuses a symlink that leaves the root, a file, and nothing else.
#[test]
fn the_access_policy_decides_what_the_scope_may_ever_hold() {
    use super::{WatchAdmissionError, WatchedDirectory};

    let (session, base) = real_session("admit");
    let root = real_root(&session);
    let project = base.join("project");

    let src = WatchedDirectory::admit(&root, "src").expect("a real directory inside the root");
    assert_eq!(
        src.path(),
        std::fs::canonicalize(project.join("src")).unwrap()
    );

    let inside = WatchedDirectory::admit(&root, "link-in")
        .expect("a symlink that stays inside the root is admitted");
    assert_eq!(
        inside, src,
        "it is watched by its canonical path, so it is the same directory"
    );

    assert!(
        WatchedDirectory::admit(&root, "link-out").is_err(),
        "a symlink leaving the root is never admitted"
    );
    assert!(matches!(
        WatchedDirectory::admit(&root, "file.txt"),
        Err(WatchAdmissionError::NotADirectory)
    ));
    assert!(
        WatchedDirectory::admit(&root, "").is_ok(),
        "the root itself is admitted"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// The session computes the scope's desired set from its own tree and documents: an
/// expanded folder is wanted, a refused path is reported and left out.
#[test]
fn the_session_wires_the_expanded_folders_into_the_desired_set() {
    let (mut session, base) = real_session("wire");
    let _ = session.toggle_explorer_directory(Path::new("src"));
    let _ = session.toggle_explorer_directory(Path::new("link-out"));

    let (desired, refused) = session.watched_directories();
    let paths: BTreeSet<PathBuf> = desired.iter().map(|d| d.path().to_path_buf()).collect();
    assert!(
        paths.contains(&std::fs::canonicalize(base.join("project/src")).unwrap()),
        "the expanded src is wanted: {paths:?}"
    );
    assert!(
        refused
            .iter()
            .any(|(path, _)| path == Path::new("link-out")),
        "the escaping symlink is refused and reported, not silently watched: {refused:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// B2 step 2 (review 456): a real change in a directory the owner watches reaches its
/// event stream. The kernel reports it; the owner only forwards that it happened.
#[test]
fn a_change_in_a_watched_directory_reaches_the_owner_event_stream() {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    let directory = Path::new("/dev/shm/tekwatch-owner-events");
    let _ = std::fs::remove_dir_all(directory);
    std::fs::create_dir_all(directory).unwrap();

    let mut watcher = super::ProjectWatcher::open();
    let desired = BTreeSet::from([WatchedDirectory::for_test(directory)]);
    watcher.reconcile(&desired, Vec::new());
    assert_eq!(watcher.state(), &WatchState::Live);
    assert_eq!(watcher.scope().watched_count(), 1);

    let mut events = watcher
        .events()
        .expect("notify starts on Linux")
        .take()
        .expect("the receiver is handed out once, to the waiter");
    let (sender, reported) = channel();
    std::thread::spawn(move || sender.send(events.wait_for_event()).unwrap());
    std::fs::write(directory.join("created.txt"), b"x").unwrap();

    assert_eq!(
        reported.recv_timeout(Duration::from_secs(10)),
        Ok(true),
        "the kernel reports the new file, and the owner forwards that it happened"
    );
    drop(watcher);
    let _ = std::fs::remove_dir_all(directory);
}

/// R6 by ownership (review 456): dropping the owner (closing the project) ends the event
/// stream, so nothing is left waiting on a project that no longer exists.
#[test]
fn dropping_the_owner_ends_its_event_stream() {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    let watcher = super::ProjectWatcher::open();
    let mut events = watcher
        .events()
        .expect("notify starts on Linux")
        .take()
        .expect("the receiver is handed out once, to the waiter");
    let (sender, reported) = channel();
    std::thread::spawn(move || sender.send(events.wait_for_event()).unwrap());

    drop(watcher);

    assert_eq!(
        reported.recv_timeout(Duration::from_secs(10)),
        Ok(false),
        "closing the project ends the stream: the watches and their sender went with the owner"
    );
}

/// A platform that never started is not silent: the first reconcile records the same
/// stop a runtime refusal would, with the reason kept for logs.
#[test]
fn a_platform_that_never_started_stops_watching_on_the_first_reconcile() {
    // A directory that exists: a missing one is dropped before the platform is asked.
    let root = Path::new("/dev/shm");
    let desired = desired_directories(root, &[], &[]);
    let mut watcher = super::ProjectWatcher::refused_for_test(WatchRefusal::new("no inotify"));
    assert!(watcher.events().is_none());

    watcher.reconcile(&desired, Vec::new());

    assert_eq!(watcher.state(), &WatchState::Stopped);
    assert_eq!(
        watcher.scope().last_refusal().map(WatchRefusal::detail),
        Some("no inotify")
    );
}

/// Review 457: at most one waiter, by the type. The receiver is handed out once; a second
/// take, from a clone of the slot or from the same one, gets nothing and does not block.
#[test]
fn the_event_receiver_is_handed_out_once_so_only_one_waiter_can_exist() {
    let watcher = super::ProjectWatcher::open();
    let slot = watcher.events().expect("notify starts on Linux");
    let clone = slot.clone();

    assert!(slot.take().is_some(), "the first taker gets the receiver");
    assert!(
        clone.take().is_none(),
        "a clone of the slot gets nothing once taken"
    );
    assert!(slot.take().is_none(), "and nothing on any later take");
}

/// Review 457, choice 3: the admission refusals are kept on the owner, with their reasons,
/// so a log can read them and nothing is discarded at the call site.
#[test]
fn the_owner_keeps_the_admission_refusals_it_was_given() {
    let mut watcher = super::ProjectWatcher::open();
    let refused = vec![(
        PathBuf::from("link-out"),
        super::WatchAdmissionError::NotADirectory,
    )];

    watcher.reconcile(&BTreeSet::new(), refused.clone());

    assert_eq!(watcher.admission_refusals(), refused.as_slice());
}

/// Review 457: the render-thread cost of one reconcile trigger, reported per expanded
/// directory, against NFR-PERF-002's p95 of 32 ms (the nearest stated budget, since no
/// trigger is an editor keystroke). The trigger is the same pair the app runs:
/// `watched_directories()` then `reconcile`. Run by hand:
/// `cargo test -p tekstide-core measured_reconcile_cost -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check: see qa-evidence.md § Review 457"]
fn measured_reconcile_cost_per_expanded_directory() {
    use crate::project::root::{ProjectRootValidator, SymlinkPolicy};
    use crate::project::{ProjectId, ProjectSession};
    use std::time::{Duration, Instant};

    const TRIGGERS: usize = 40;
    for count in [1usize, 10, 100, 500] {
        let base = PathBuf::from(format!("/dev/shm/tekmeasure-{count}"));
        let _ = std::fs::remove_dir_all(&base);
        let project = base.join("project");
        for index in 0..count {
            let folder = project.join(format!("d{index:04}"));
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("file.txt"), "f").unwrap();
        }
        let valid = ProjectRootValidator
            .validate(&project, SymlinkPolicy::FailClosed)
            .expect("the measurement project root validates");
        let mut session = ProjectSession::new(
            ProjectId::for_test(1),
            valid.display_name,
            valid.selected_path,
            valid.canonical_path,
        );
        for index in 0..count {
            let _ = session.toggle_explorer_directory(Path::new(&format!("d{index:04}")));
        }
        let mut watcher = super::ProjectWatcher::open();

        let trigger = |watcher: &mut super::ProjectWatcher| -> Duration {
            let started = Instant::now();
            let (desired, refused) = session.watched_directories();
            watcher.reconcile(&desired, refused);
            started.elapsed()
        };
        let first = trigger(&mut watcher);
        let mut steady: Vec<Duration> = (0..TRIGGERS).map(|_| trigger(&mut watcher)).collect();
        steady.sort();
        let p95 = steady[(TRIGGERS * 95).div_ceil(100) - 1];
        let per_expanded_us = p95.as_secs_f64() * 1e6 / count as f64;
        println!(
            "expanded={count} desired={} first_trigger_ms={:.3} steady_p95_ms={:.3} \
             steady_p95_us_per_expanded_directory={per_expanded_us:.2} within_32ms={}",
            count + 1,
            first.as_secs_f64() * 1e3,
            p95.as_secs_f64() * 1e3,
            p95 <= Duration::from_millis(32),
        );
        drop(watcher);
        let _ = std::fs::remove_dir_all(&base);
    }
}
