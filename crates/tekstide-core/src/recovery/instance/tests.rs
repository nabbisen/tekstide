use super::*;

#[test]
fn pid_is_alive_is_true_for_this_very_process() {
    assert!(pid_is_alive(std::process::id()));
}

#[test]
fn pid_is_alive_is_false_for_a_pid_that_does_not_exist() {
    // RFC-027: not a magic number -- `pid_t` is a 32-bit signed int on Linux, so this
    // is the largest value the type can hold, chosen because a real system is
    // vanishingly unlikely to have ever allocated it.
    assert!(!pid_is_alive(i32::MAX as u32));
}

/// Review 482: asserted directly against the parse, not against `detected_crashes` --
/// `0` and `u32::MAX` leave a marker on disk and report no crash whether they are
/// rejected here or merely misread as "alive" downstream, so only this function's own
/// return value can tell the filter apart from its absence.
#[test]
fn marker_filename_to_pid_rejects_zero_and_anything_past_i32_max() {
    assert_eq!(marker_filename_to_pid("999999999"), Some(999_999_999));
    assert_eq!(marker_filename_to_pid("0"), None);
    assert_eq!(marker_filename_to_pid(&u32::MAX.to_string()), None);
    assert_eq!(
        marker_filename_to_pid(&(i32::MAX as u32).to_string()),
        Some(i32::MAX as u32),
        "i32::MAX itself is still a valid pid_t"
    );
    assert_eq!(marker_filename_to_pid("not-a-pid"), None);
}
