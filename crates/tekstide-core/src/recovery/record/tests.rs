use super::*;

fn sample_record(relative_path: &str, text: &str) -> RecoveryRecord {
    RecoveryRecord {
        version: RECOVERY_RECORD_VERSION,
        relative_path: relative_path.to_owned(),
        text: text.to_owned(),
        cursor_line: 2,
        cursor_column: 5,
        viewport_first_visible_line: 0,
        viewport_first_visible_column: 0,
        snapshot: RecoveryFileSnapshot::from_system_time(
            Path::new("/project/first.txt"),
            SystemTime::UNIX_EPOCH,
            11,
        ),
    }
}

#[test]
fn record_file_name_is_stable_and_distinct_per_path() {
    let first = record_file_name(Path::new("first.txt"));
    let first_again = record_file_name(Path::new("first.txt"));
    let second = record_file_name(Path::new("second.txt"));
    assert_eq!(
        first, first_again,
        "the same path must always name the same file"
    );
    assert_ne!(
        first, second,
        "different paths must not collide in the common case"
    );
    assert!(first.ends_with(".json"));
}

#[test]
fn record_round_trips_through_json_exactly() {
    let record = sample_record("src/main.rs", "fn main() {}\n");
    let json = serde_json::to_vec(&record).unwrap();
    let back: RecoveryRecord = serde_json::from_slice(&json).unwrap();
    assert_eq!(record, back);
}

#[test]
fn file_snapshot_round_trips_sub_second_precision() {
    let modified_at = SystemTime::UNIX_EPOCH + std::time::Duration::new(1_700_000_000, 123_456_789);
    let snapshot = RecoveryFileSnapshot::from_system_time(Path::new("/p/f.txt"), modified_at, 42);
    assert_eq!(snapshot.modified_at_secs, 1_700_000_000);
    assert_eq!(snapshot.modified_at_nanos, 123_456_789);
    assert_eq!(snapshot.len, 42);
}
