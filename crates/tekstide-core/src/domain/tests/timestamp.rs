use crate::domain::DomainTimestamp;

#[test]
fn timestamp_constructor_validates_utc_shape() {
    let timestamp = DomainTimestamp::from_utc_string("2026-07-05T01:02:03Z")
        .expect("valid UTC timestamp shape should parse");

    assert_eq!(timestamp.as_str(), "2026-07-05T01:02:03Z");
    assert!(DomainTimestamp::from_utc_string("not-a-time").is_err());
}

// --- RFC-049 D9: seconds arithmetic on a formatted string -------------

/// The round-trip that defines the inverse. Every value the formatter
/// can produce must come back as the seconds it was produced from —
/// checked at the boundaries that bite rather than at a convenient
/// midpoint.
#[test]
fn unix_seconds_round_trips_through_the_formatter_at_the_boundaries() {
    for (seconds, expected, what) in [
        (0u64, "1970-01-01T00:00:00Z", "the epoch itself"),
        (
            86_399,
            "1970-01-01T23:59:59Z",
            "the last second of the first day",
        ),
        (
            86_400,
            "1970-01-02T00:00:00Z",
            "the first second of the second day",
        ),
        // 2024 is a leap year: 2024-02-29 exists and 2023-02-29 does not.
        (1_709_164_800, "2024-02-29T00:00:00Z", "a leap day"),
        (
            1_709_251_199,
            "2024-02-29T23:59:59Z",
            "the end of a leap day",
        ),
        (
            1_709_251_200,
            "2024-03-01T00:00:00Z",
            "the day after a leap day",
        ),
        // 2100 is not a leap year -- the century rule, which a naive
        // every-fourth-year implementation gets wrong.
        (
            4_107_542_400,
            "2100-03-01T00:00:00Z",
            "past a skipped leap year",
        ),
    ] {
        let formatted = DomainTimestamp::from_utc_string(expected)
            .unwrap_or_else(|_| panic!("{what}: fixture must be a valid shape"));
        assert_eq!(
            formatted.unix_seconds(),
            Some(seconds),
            "{what}: {expected} must read back as {seconds}"
        );
    }
}

/// **The test that makes the string-ordering shortcut fail loudly**, per
/// D9's own instruction. `format_unix_seconds_utc` pads the year to four
/// digits, so a year past 9999 formats wider — and once it does, byte
/// ordering and chronological ordering disagree: `"10000-…"` sorts
/// *before* `"2026-…"` because `'1' < '2'`, while the instant it names
/// is nearly eight thousand years later.
///
/// Anyone who replaces the seconds comparison with `a.as_str() < b.as_str()`
/// fails here, and the failure says why.
#[test]
fn string_ordering_and_seconds_ordering_disagree_and_seconds_is_the_correct_one() {
    let far_future = DomainTimestamp::from_utc_string_unchecked("10000-01-01T00:00:00Z");
    let near_past = DomainTimestamp::from_utc_string_unchecked("2026-01-01T00:00:00Z");

    assert!(
        far_future.as_str() < near_past.as_str(),
        "precondition: byte ordering really does put the far future first"
    );

    // The wide year is outside the formatter's own four-digit shape, so
    // the honest answer for it is `None` -- "cannot tell" -- and §1 makes
    // that *not expired*. What must never happen is the string comparison
    // above being treated as an age.
    assert_eq!(
        far_future.unix_seconds(),
        None,
        "a value the formatter cannot produce must not be given an age"
    );
    assert_eq!(
        near_past.unix_seconds(),
        Some(1_767_225_600),
        "and a value it can produce must read back exactly"
    );
}

/// Shape-valid and calendar-impossible. `from_utc_string` admits these —
/// it checks twenty bytes and separator positions, nothing more — and a
/// persisted store can hand one back. The round-trip definition rejects
/// them without this code knowing how long a month is.
#[test]
fn a_shape_valid_but_impossible_instant_has_no_age() {
    for impossible in [
        "2026-13-01T00:00:00Z", // month 13
        "2026-02-30T00:00:00Z", // February 30th
        "2023-02-29T00:00:00Z", // February 29th in a non-leap year
        "2026-01-01T25:00:00Z", // hour 25
        "2026-01-01T00:60:00Z", // minute 60
        "2026-00-01T00:00:00Z", // month zero
        "2026-01-00T00:00:00Z", // day zero
    ] {
        let timestamp = DomainTimestamp::from_utc_string(impossible)
            .expect("precondition: the shape check admits this, which is the point");
        assert_eq!(
            timestamp.unix_seconds(),
            None,
            "{impossible} names no instant, so it must not be given an age"
        );
    }
}

/// Pre-epoch is corrupt, not old: the formatter takes `u64` and cannot
/// express it, so there is no seconds value to compare against.
#[test]
fn a_pre_epoch_instant_has_no_age() {
    let timestamp = DomainTimestamp::from_utc_string("1969-12-31T23:59:59Z")
        .expect("precondition: the shape is valid");
    assert_eq!(timestamp.unix_seconds(), None);
}

/// `now_utc()` is the one value production produces, so it must be
/// readable back — a formatter/parser pair that agrees only on fixtures
/// would be worth nothing.
#[test]
fn the_clocks_own_output_reads_back_as_seconds() {
    assert!(
        DomainTimestamp::now_utc().unix_seconds().is_some(),
        "the timestamp production actually creates must have an age"
    );
}
