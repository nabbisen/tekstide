use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::{Criterion, Measurement, parse_meminfo_snapshot};

fn scratch_log_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "tekstide-measurement-test-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// `record_input` writes one `"input <microseconds>"` line per call, and
/// the elapsed time is real (non-zero for a real, if tiny, delay) --
/// proving this path measures actual elapsed wall-clock time rather than
/// always producing a placeholder value.
#[test]
fn record_input_writes_a_real_nonzero_elapsed_sample() {
    let log_path = scratch_log_path("record-input");
    let mut measurement = Measurement::for_test(Criterion::Typing, &log_path, 1);

    let sent_at = Instant::now();
    std::thread::sleep(Duration::from_millis(2));
    measurement.record_input(sent_at);

    let contents = std::fs::read_to_string(&log_path).unwrap();
    let line = contents.lines().next().expect("one line must be written");
    assert!(
        line.starts_with("input "),
        "expected an `input `-prefixed line: {line:?}"
    );
    let microseconds: u128 = line.trim_start_matches("input ").parse().unwrap();
    assert!(
        microseconds >= 1_000,
        "a real 2ms sleep must not be measured as ~0us: {microseconds}"
    );
}

/// `is_done` for `Typing` is reached exactly at `target`, not before or
/// after -- the boundary the exit-detection subscription relies on.
#[test]
fn typing_is_done_exactly_at_target() {
    let log_path = scratch_log_path("typing-done");
    let mut measurement = Measurement::for_test(Criterion::Typing, &log_path, 3);

    for _ in 0..2 {
        assert!(!measurement.is_done());
        measurement.record_input(Instant::now());
    }
    assert!(
        !measurement.is_done(),
        "2 of 3 samples must not be done yet"
    );
    measurement.record_input(Instant::now());
    assert!(measurement.is_done(), "3 of 3 samples must be done");
}

/// `Startup` is done after exactly one recorded frame, and a second
/// `record_startup_frame` call must not overwrite the first (only the
/// first frame after process start is the honest startup figure).
#[test]
fn startup_is_done_after_one_frame_and_does_not_record_a_second() {
    let log_path = scratch_log_path("startup-done");
    let mut measurement = Measurement::for_test(Criterion::Startup, &log_path, 1100);

    assert!(!measurement.is_done());
    measurement.record_startup_frame(Instant::now());
    assert!(measurement.is_done());

    std::thread::sleep(Duration::from_millis(2));
    measurement.record_startup_frame(Instant::now());

    let contents = std::fs::read_to_string(&log_path).unwrap();
    assert_eq!(
        contents.lines().count(),
        1,
        "a second Frame after startup was already recorded must not add a second line: {contents:?}"
    );
}

/// `record_startup_frame` is a no-op for `Typing` -- proven directly,
/// since `Typing` never subscribes to `frames()` at all (the module
/// doc's whole reason for existing) and must never accidentally write a
/// startup-shaped line if it somehow received one.
#[test]
fn record_startup_frame_is_a_no_op_for_typing() {
    let log_path = scratch_log_path("startup-frame-ignored-for-typing");
    let mut measurement = Measurement::for_test(Criterion::Typing, &log_path, 1100);
    measurement.record_startup_frame(Instant::now());
    let contents = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        contents.is_empty(),
        "Typing must not write a startup-shaped line: {contents:?}"
    );
}

/// RFC-015 PR-015-E: `ModeSwitch` reuses `Typing`'s exact `is_done`
/// boundary -- both use the input-to-state-change decomposition, and
/// this proves the new criterion did not silently fall through to some
/// other arm.
#[test]
fn mode_switch_is_done_exactly_at_target() {
    let log_path = scratch_log_path("mode-switch-done");
    let mut measurement = Measurement::for_test(Criterion::ModeSwitch, &log_path, 3);

    for _ in 0..2 {
        assert!(!measurement.is_done());
        measurement.record_input(Instant::now());
    }
    assert!(
        !measurement.is_done(),
        "2 of 3 samples must not be done yet"
    );
    measurement.record_input(Instant::now());
    assert!(measurement.is_done(), "3 of 3 samples must be done");
}

/// `record_startup_frame` is a no-op for `ModeSwitch` too, same reason
/// as `Typing`: neither criterion subscribes to `frames()`.
#[test]
fn record_startup_frame_is_a_no_op_for_mode_switch() {
    let log_path = scratch_log_path("startup-frame-ignored-for-mode-switch");
    let mut measurement = Measurement::for_test(Criterion::ModeSwitch, &log_path, 1100);
    measurement.record_startup_frame(Instant::now());
    let contents = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        contents.is_empty(),
        "ModeSwitch must not write a startup-shaped line: {contents:?}"
    );
}

/// RFC-017 PR-017-G: `TerminalFlood` reuses `Typing`/`ModeSwitch`'s exact
/// `is_done` boundary -- the input-to-state-change half of the
/// decomposition, even though (unlike those two) it does not use the
/// view-build half. This proves the new criterion did not silently fall
/// through to `Startup`'s single-frame arm.
#[test]
fn terminal_flood_is_done_exactly_at_target() {
    let log_path = scratch_log_path("terminal-flood-done");
    let mut measurement = Measurement::for_test(Criterion::TerminalFlood, &log_path, 3);

    for _ in 0..2 {
        assert!(!measurement.is_done());
        measurement.record_input(Instant::now());
    }
    assert!(
        !measurement.is_done(),
        "2 of 3 samples must not be done yet"
    );
    measurement.record_input(Instant::now());
    assert!(measurement.is_done(), "3 of 3 samples must be done");
}

/// `record_startup_frame` is a no-op for `TerminalFlood` too: it never
/// subscribes to `frames()` either.
#[test]
fn record_startup_frame_is_a_no_op_for_terminal_flood() {
    let log_path = scratch_log_path("startup-frame-ignored-for-terminal-flood");
    let mut measurement = Measurement::for_test(Criterion::TerminalFlood, &log_path, 1100);
    measurement.record_startup_frame(Instant::now());
    let contents = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        contents.is_empty(),
        "TerminalFlood must not write a startup-shaped line: {contents:?}"
    );
}

/// RFC-017 PR-017-G response 156: the environment snapshot's parser
/// reads real `/proc/meminfo` field names and computes swap-in-use as
/// `SwapTotal - SwapFree`, not `SwapFree` itself -- proven against a
/// realistic fixture (fields in a different order than they appear on
/// a real Linux system, and extra fields interspersed, to prove this
/// does not depend on `/proc/meminfo`'s field order).
#[test]
fn parse_meminfo_snapshot_reads_available_and_computed_swap_used() {
    let fixture = "\
MemTotal:       61865472 kB
SwapTotal:      61865472 kB
Cached:          8317972 kB
MemAvailable:   18294740 kB
SwapFree:        5700000 kB
";
    let (mem_available_kib, swap_used_kib) =
        parse_meminfo_snapshot(fixture).expect("a well-formed fixture must parse");
    assert_eq!(mem_available_kib, 18294740);
    assert_eq!(swap_used_kib, 61865472 - 5700000);
}

/// A field this parser needs missing entirely (not merely zero) must
/// fail closed to `None` rather than silently reporting a wrong number
/// -- proven by removing `SwapFree` specifically, the field the swap
/// computation depends on.
#[test]
fn parse_meminfo_snapshot_is_none_when_a_needed_field_is_missing() {
    let fixture = "\
MemTotal:       61865472 kB
MemAvailable:   18294740 kB
";
    assert!(parse_meminfo_snapshot(fixture).is_none());
}
