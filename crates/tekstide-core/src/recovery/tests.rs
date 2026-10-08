use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::recovery::{instances_dir, start_instance};

fn test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

/// RFC-027 PR-027-A: the first run under a fresh state root has nothing to detect, and
/// starting one writes exactly one file -- this instance's own marker, nothing else
/// (§1 row 1: no buffer content anywhere).
#[test]
fn a_first_run_detects_nothing_and_writes_only_its_own_marker() {
    let root = test_root("recovery-first-run");

    let startup = start_instance(&root).expect("starting an instance should succeed");
    assert!(
        startup.detected_crashes.is_empty(),
        "a fresh state root has no prior crash to detect"
    );

    let entries: Vec<_> = std::fs::read_dir(instances_dir(&root))
        .expect("the instances directory should exist")
        .map(|entry| entry.expect("a readable directory entry").path())
        .collect();
    assert_eq!(
        entries,
        vec![startup.marker.path().to_path_buf()],
        "the only file on disk must be this instance's own marker"
    );

    cleanup_root(root);
}

/// RFC-027 D1/D12, checklist row 1 (clean-exit half): dropping the marker -- the same
/// thing a normal application exit does to `shell::State`'s own field -- removes it from
/// disk.
#[test]
fn a_clean_exit_leaves_no_marker_behind() {
    let root = test_root("recovery-clean-exit");

    let startup = start_instance(&root).expect("starting an instance should succeed");
    let marker_path = startup.marker.path().to_path_buf();
    assert!(marker_path.is_file(), "the marker must exist while running");

    drop(startup);

    assert!(
        !marker_path.exists(),
        "a clean exit (the marker's own Drop) must remove the marker"
    );

    cleanup_root(root);
}

/// RFC-027 D1/D12, checklist row 1 (crash half): a real `SIGKILL` of a real process,
/// reaped so its pid genuinely leaves the process table, is detected by a later startup's
/// scan as a crash -- not a `simulate_crash()` helper (§4 row 15).
#[test]
fn a_real_sigkill_leaves_a_marker_a_later_startup_detects_as_a_crash() {
    let _real_process_slot = crate::test_support::RealProcessLimiter::acquire();
    let root = test_root("recovery-real-sigkill");

    let mut child = std::process::Command::new("sleep")
        .arg("5")
        .spawn()
        .expect("spawn a real, short-lived sleep process");
    let pid = child.id();

    // This stands in for "a previous Tekstide instance with this real pid wrote its own
    // marker at startup" -- the marker a real instance would have created via
    // `InstanceMarker::create`, written directly here since the stand-in process is a
    // plain `sleep`, not Tekstide itself.
    std::fs::create_dir_all(instances_dir(&root)).expect("instances dir should be creatable");
    let stale_marker = instances_dir(&root).join(pid.to_string());
    std::fs::write(&stale_marker, b"").expect("the stale marker should be writable");

    let _ = child.kill();
    let _ = child.wait();

    let startup = start_instance(&root).expect("starting an instance should succeed");
    assert_eq!(
        startup.detected_crashes,
        vec![crate::recovery::DetectedCrash { pid }],
        "the real, now-dead pid must be reported as a detected crash"
    );
    assert!(
        !stale_marker.exists(),
        "a detected crash's own stale marker must be removed, the only consumer of the \
         fact being this same scan"
    );

    cleanup_root(root);
}

/// RFC-027 D12, checklist row 2: two concurrent instances do not make each other look
/// crashed. A real, still-running process stands in for the sibling instance's own
/// marker; starting a second instance alongside it must neither report it as a crash nor
/// remove its marker.
#[test]
fn a_concurrent_sibling_instance_is_not_reported_as_a_crash() {
    let _real_process_slot = crate::test_support::RealProcessLimiter::acquire();
    let root = test_root("recovery-concurrent-siblings");

    let mut sibling = std::process::Command::new("sleep")
        .arg("5")
        .spawn()
        .expect("spawn a real, short-lived sleep process to stand in for a sibling instance");
    let sibling_pid = sibling.id();

    std::fs::create_dir_all(instances_dir(&root)).expect("instances dir should be creatable");
    let sibling_marker = instances_dir(&root).join(sibling_pid.to_string());
    std::fs::write(&sibling_marker, b"").expect("the sibling's own marker should be writable");

    let startup = start_instance(&root).expect("starting an instance should succeed");
    assert!(
        startup.detected_crashes.is_empty(),
        "a live sibling instance must never be reported as a crash: {:?}",
        startup.detected_crashes
    );
    assert!(
        sibling_marker.exists(),
        "a live sibling's own marker must be left untouched"
    );
    assert_ne!(
        startup.marker.path(),
        sibling_marker,
        "the new instance's own marker must be a distinct file, named by its own pid"
    );

    let _ = sibling.kill();
    let _ = sibling.wait();

    cleanup_root(root);
}

// Review 482: the out-of-range-pid property moved to a direct unit test against
// `marker_filename_to_pid` itself (`recovery::instance::tests`), since an end-to-end test
// asserting only `detected_crashes` cannot tell "rejected at parse time" from "accepted,
// then misread as alive" -- both leave the file on disk and report nothing either way.
