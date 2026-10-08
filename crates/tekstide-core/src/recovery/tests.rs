use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::recovery::{
    RecoveryFileSnapshot, RecoveryRecord, RecoveryRecordWriteError, RecoveryRetentionLimits,
    instances_dir, project_recovery_record_bytes, purge_project_recovery_records,
    read_project_recovery_records, record_file_name, records_dir, remove_recovery_record,
    start_instance, write_recovery_record,
};

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

fn sample_record(relative_path: &str, text: &str) -> RecoveryRecord {
    RecoveryRecord {
        version: crate::recovery::RECOVERY_RECORD_VERSION,
        relative_path: relative_path.to_owned(),
        text: text.to_owned(),
        cursor_line: 0,
        cursor_column: 0,
        viewport_first_visible_line: 0,
        viewport_first_visible_column: 0,
        snapshot: RecoveryFileSnapshot::from_system_time(
            Path::new("/project/first.txt"),
            SystemTime::UNIX_EPOCH,
            text.len() as u64,
        ),
    }
}

/// RFC-027 D13, §2 row 6: a written record is `0600` in a `0700` directory, checked by
/// reading the real mode bits, and round-trips exactly through
/// `read_project_recovery_records`.
#[test]
fn a_written_record_has_the_right_permissions_and_reads_back_exactly() {
    use std::os::unix::fs::PermissionsExt;

    let root = test_root("recovery-record-write");
    let record = sample_record("first.txt", "hello\n");
    write_recovery_record(
        &root,
        "proj-1",
        &record,
        RecoveryRetentionLimits::default_limits(),
    )
    .expect("writing a record under the bound should succeed");

    let dir = records_dir(&root, "proj-1");
    let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700, "the records directory must be 0700");

    let file_path = dir.join(record_file_name(Path::new("first.txt")));
    let file_mode = std::fs::metadata(&file_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(file_mode, 0o600, "the record file must be 0600");

    let records = read_project_recovery_records(&root, "proj-1");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].1, record);

    cleanup_root(root);
}

/// RFC-027 PR-027-C, D6/measurement 2 (crash half): the real round trip, proven against a
/// real `SIGKILL`, not a `simulate_crash()` helper (§4 row 15) -- the same discipline
/// `a_real_sigkill_leaves_a_marker_a_later_startup_detects_as_a_crash` already established
/// for PR-027-A's own marker. A real, short-lived process stands in for the crashed
/// instance (as that test's own doc explains: the marker/record writes happen in *this*
/// process, since the stand-in never ran Tekstide's own code, only existed to have a real,
/// killable pid); what is proven here is that both the marker and the record it wrote
/// genuinely survive a real kill and reap, intact and distinct from a merely-simulated
/// crash -- the disk-comparison half of the round trip (unchanged/changed/gone) is
/// `TextDocument::recover`'s own job and is proven directly against real files in
/// `tekstide::content::tests::recover`, which needs no process at all to exercise.
#[test]
fn a_real_sigkill_leaves_the_crashed_instances_own_recovery_record_intact() {
    let _real_process_slot = crate::test_support::RealProcessLimiter::acquire();
    let root = test_root("recovery-real-sigkill-round-trip");

    let mut child = std::process::Command::new("sleep")
        .arg("5")
        .spawn()
        .expect("spawn a real, short-lived sleep process");
    let pid = child.id();

    // Stands in for the crashed instance's own marker (`InstanceMarker::create`) and its
    // own unsaved edit's record (`write_recovery_record`, the real product function) --
    // both written here, exactly as `a_real_sigkill_leaves_a_marker_a_later_startup_
    // detects_as_a_crash`'s own doc explains doing for the marker alone.
    std::fs::create_dir_all(instances_dir(&root)).expect("instances dir should be creatable");
    let stale_marker = instances_dir(&root).join(pid.to_string());
    std::fs::write(&stale_marker, b"").expect("the stale marker should be writable");
    let record = sample_record("unsaved.txt", "edited but never saved\n");
    write_recovery_record(
        &root,
        "proj-1",
        &record,
        RecoveryRetentionLimits::default_limits(),
    )
    .expect("writing the crashed instance's own record should succeed");

    let _ = child.kill();
    let _ = child.wait();

    // "Restart": the real detection scan, and the real record read -- the two facts
    // Amendment 1 says the offer is actually driven by (the record's own presence, with
    // the marker only coloring *how* it is described, never gating whether it appears).
    let startup = start_instance(&root).expect("starting an instance should succeed");
    assert_eq!(
        startup.detected_crashes,
        vec![crate::recovery::DetectedCrash { pid }],
        "the real, now-dead pid must be reported as a detected crash"
    );
    let records = read_project_recovery_records(&root, "proj-1");
    assert_eq!(
        records.len(),
        1,
        "the crashed instance's own record must survive the real kill intact"
    );
    assert_eq!(
        records[0].1, record,
        "the record read back after the real kill must be byte-for-byte what was written \
         before it, not merely present"
    );

    cleanup_root(root);
}

/// RFC-027: writing the same document's own record twice replaces it in place -- one
/// file per document, not one per write.
#[test]
fn writing_the_same_document_twice_replaces_its_own_record() {
    let root = test_root("recovery-record-overwrite");
    let first = sample_record("first.txt", "v1\n");
    let second = sample_record("first.txt", "v1 edited\n");

    write_recovery_record(
        &root,
        "proj-1",
        &first,
        RecoveryRetentionLimits::default_limits(),
    )
    .unwrap();
    write_recovery_record(
        &root,
        "proj-1",
        &second,
        RecoveryRetentionLimits::default_limits(),
    )
    .unwrap();

    let records = read_project_recovery_records(&root, "proj-1");
    assert_eq!(
        records.len(),
        1,
        "the second write must replace the first, not add to it"
    );
    assert_eq!(records[0].1.text, "v1 edited\n");

    cleanup_root(root);
}

/// RFC-027 D9, measurement 5: a record whose own serialized size exceeds the per-record
/// bound is refused, naming the buffer and the limit, and nothing is written.
#[test]
fn a_record_over_the_per_record_bound_is_refused_and_nothing_is_written() {
    let root = test_root("recovery-record-too-large");
    let huge_text = "x".repeat(1024);
    let record = sample_record("huge.txt", &huge_text);
    let tiny_limits = RecoveryRetentionLimits {
        max_bytes_per_record: 16,
        max_bytes_total: 1024 * 1024,
    };

    let result = write_recovery_record(&root, "proj-1", &record, tiny_limits);
    match result {
        Err(RecoveryRecordWriteError::TooLarge(refusal)) => {
            assert_eq!(refusal.relative_path, "huge.txt");
            assert_eq!(refusal.max_bytes_per_record, 16);
            assert!(refusal.record_bytes > 16);
        }
        other => panic!("expected TooLarge, got {other:?}"),
    }
    assert!(
        read_project_recovery_records(&root, "proj-1").is_empty(),
        "a refused write must leave no record behind"
    );

    cleanup_root(root);
}

/// RFC-027 D11, measurement 6: removing a record deletes its file; removing one that was
/// never written (a clean document, or one the bound refused) is not an error.
#[test]
fn removing_a_record_deletes_it_and_removing_a_missing_one_is_not_an_error() {
    let root = test_root("recovery-record-remove");
    let record = sample_record("first.txt", "hello\n");
    write_recovery_record(
        &root,
        "proj-1",
        &record,
        RecoveryRetentionLimits::default_limits(),
    )
    .unwrap();
    assert_eq!(read_project_recovery_records(&root, "proj-1").len(), 1);

    remove_recovery_record(&root, "proj-1", Path::new("first.txt")).unwrap();
    assert!(read_project_recovery_records(&root, "proj-1").is_empty());

    remove_recovery_record(&root, "proj-1", Path::new("never-written.txt"))
        .expect("removing a record that was never written must not be an error");

    cleanup_root(root);
}

/// RFC-027 D9/D10: byte and record counts match what was actually written, and a purge
/// removes exactly those bytes -- the same "the dialog never promises less than it
/// removes" rule the transcript purge already follows.
#[test]
fn project_byte_counts_and_purge_agree_with_what_is_on_disk() {
    let root = test_root("recovery-record-purge");
    let first = sample_record("first.txt", "hello\n");
    let second = sample_record("second.txt", "world, a little longer\n");
    write_recovery_record(
        &root,
        "proj-1",
        &first,
        RecoveryRetentionLimits::default_limits(),
    )
    .unwrap();
    write_recovery_record(
        &root,
        "proj-1",
        &second,
        RecoveryRetentionLimits::default_limits(),
    )
    .unwrap();

    let (count, bytes) = project_recovery_record_bytes(&root, "proj-1");
    assert_eq!(count, 2);
    let dir = records_dir(&root, "proj-1");
    let on_disk_bytes: u64 = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.metadata().unwrap().len())
        .sum();
    assert_eq!(bytes, on_disk_bytes);

    let removed = purge_project_recovery_records(&root, "proj-1").unwrap();
    assert_eq!(removed, on_disk_bytes);
    assert!(read_project_recovery_records(&root, "proj-1").is_empty());
    assert_eq!(project_recovery_record_bytes(&root, "proj-1"), (0, 0));

    cleanup_root(root);
}

/// RFC-027: a purge of one project's own records must never touch another project's --
/// the same per-project scoping the transcript purge already has.
#[test]
fn purging_one_project_leaves_another_projects_records_untouched() {
    let root = test_root("recovery-record-purge-scope");
    let mine = sample_record("mine.txt", "mine\n");
    let theirs = sample_record("theirs.txt", "theirs\n");
    write_recovery_record(
        &root,
        "proj-mine",
        &mine,
        RecoveryRetentionLimits::default_limits(),
    )
    .unwrap();
    write_recovery_record(
        &root,
        "proj-theirs",
        &theirs,
        RecoveryRetentionLimits::default_limits(),
    )
    .unwrap();

    purge_project_recovery_records(&root, "proj-mine").unwrap();

    assert!(read_project_recovery_records(&root, "proj-mine").is_empty());
    assert_eq!(read_project_recovery_records(&root, "proj-theirs").len(), 1);

    cleanup_root(root);
}

/// RFC-027: a file in a project's own records directory that this build does not
/// recognise (an unparseable name's own content, or a stray non-record file) is skipped
/// by the reader and left alone by the purge -- the same "do not delete what you do not
/// recognise" rule PR-027-A's marker scan already follows.
#[test]
fn an_unrecognised_file_in_the_records_directory_is_skipped_and_left_alone() {
    let root = test_root("recovery-record-stranger");
    let dir = records_dir(&root, "proj-1");
    std::fs::create_dir_all(&dir).unwrap();
    let stranger = dir.join("not-a-record.txt");
    std::fs::write(&stranger, b"not json, not ours").unwrap();

    assert!(read_project_recovery_records(&root, "proj-1").is_empty());
    assert_eq!(project_recovery_record_bytes(&root, "proj-1"), (0, 0));

    purge_project_recovery_records(&root, "proj-1").unwrap();
    assert!(
        stranger.exists(),
        "a file this module does not recognise must survive a purge"
    );

    cleanup_root(root);
}

/// RFC-027 D9, measurement 5: the total bound is app-wide and is checked before the
/// write, not cleaned up after -- a record that would push the total over the limit is
/// refused, naming the buffer and the limit, and nothing new is written.
#[test]
fn a_record_over_the_total_bound_is_refused_and_nothing_new_is_written() {
    let root = test_root("recovery-record-total-bound");
    let generous_limits = RecoveryRetentionLimits {
        max_bytes_per_record: 1024,
        max_bytes_total: 1024 * 1024,
    };
    let first = sample_record("first.txt", "already on disk\n");
    write_recovery_record(&root, "proj-1", &first, generous_limits).unwrap();
    let (_, bytes_so_far) = project_recovery_record_bytes(&root, "proj-1");

    let tight_total_limits = RecoveryRetentionLimits {
        max_bytes_per_record: 1024,
        max_bytes_total: bytes_so_far,
    };
    let second = sample_record("second.txt", "a second document's own text\n");
    let result = write_recovery_record(&root, "proj-2", &second, tight_total_limits);
    match result {
        Err(RecoveryRecordWriteError::TotalBoundExceeded {
            relative_path,
            max_bytes_total,
            ..
        }) => {
            assert_eq!(relative_path, "second.txt");
            assert_eq!(max_bytes_total, bytes_so_far);
        }
        other => panic!("expected TotalBoundExceeded, got {other:?}"),
    }
    assert!(
        read_project_recovery_records(&root, "proj-2").is_empty(),
        "a refused write must leave no record behind"
    );

    // Replacing the *same* document's own existing record must not count its own old
    // bytes against itself -- a same-size rewrite always fits even at an exact-total limit.
    let same_project_limits = RecoveryRetentionLimits {
        max_bytes_per_record: 1024,
        max_bytes_total: bytes_so_far,
    };
    let replacement = sample_record("first.txt", "already on disk\n");
    write_recovery_record(&root, "proj-1", &replacement, same_project_limits)
        .expect("replacing a document's own record must not double-count its own old bytes");

    cleanup_root(root);
}
