use std::path::PathBuf;
use std::time::Instant;

use super::{GIT_SUBPROCESSES_PER_SCAN, SCAN_WINDOW, ScanBatcher, ScanRequest};

/// Feeds `events` (already in time order) through a batcher, draining before each
/// one as a real loop would, then once more at `end`. Returns every request issued.
fn run(events: &[(Instant, PathBuf)], end: Instant) -> Vec<ScanRequest> {
    let mut batcher = ScanBatcher::new();
    let mut requests = Vec::new();
    for (at, path) in events {
        requests.extend(batcher.drain_due(*at));
        batcher.record(path, *at);
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
        .map(|i| {
            (
                t0 + SCAN_WINDOW * i / 2000,
                dir.join(format!("file-{i}.rs")),
            )
        })
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
        .map(|i| (t0 + span * i / 1000, dir.join(format!("out-{i}.o"))))
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
        events.push((at, PathBuf::from("project/a").join(format!("x-{i}"))));
        events.push((at, PathBuf::from("project/b").join(format!("y-{i}"))));
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

    batcher.record(&dir.join("first"), t0);
    assert!(
        batcher
            .drain_due(t0 + SCAN_WINDOW - std::time::Duration::from_nanos(1))
            .is_empty(),
        "not due a nanosecond early"
    );
    let closing = batcher.drain_due(t0 + SCAN_WINDOW);
    assert_eq!(closing.len(), 1, "due exactly at the window's close");

    batcher.record(&dir.join("second"), t0 + SCAN_WINDOW);
    assert_eq!(
        batcher.pending_directories(),
        1,
        "a change at the close opens a new window"
    );
    assert_eq!(batcher.drain_due(t0 + SCAN_WINDOW * 2).len(), 1);
}
