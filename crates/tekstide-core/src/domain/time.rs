use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainTimestamp(String);

impl DomainTimestamp {
    pub fn now_utc() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self(format_unix_seconds_utc(seconds))
    }

    pub fn from_utc_string(value: impl Into<String>) -> Result<Self, TimestampParseError> {
        let value = value.into();
        if is_utc_timestamp_shape(&value) {
            Ok(Self(value))
        } else {
            Err(TimestampParseError)
        }
    }

    #[cfg(test)]
    pub fn from_utc_string_unchecked(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// RFC-049 D9: seconds since the Unix epoch, or `None` when this
    /// value does not name a real instant.
    ///
    /// **Age arithmetic needs this and the type did not offer it.**
    /// `DomainTimestamp` wraps a formatted string; comparing two of them
    /// by string ordering happens to work for this exact format and
    /// stops working the moment it gains a suffix, a width change, or an
    /// offset — so RFC-049 compares in seconds, and this is the inverse
    /// of the formatter it compares against.
    ///
    /// **`None` is a real answer, not a failure to handle.**
    /// [`Self::from_utc_string`] validates *shape* only — twenty bytes
    /// with separators in the right places — so `"2026-13-45T99:99:99Z"`
    /// is a `DomainTimestamp` this crate will happily hold, and
    /// `from_persisted` paths can read one back out of a store. A caller
    /// deciding whether to **delete** a user's data on the strength of
    /// an age comparison must treat `None` as *"cannot tell"*, and
    /// RFC-049 §1 settles what that means: not expired.
    pub fn unix_seconds(&self) -> Option<u64> {
        unix_seconds_from_utc_string(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimestampParseError;

fn format_unix_seconds_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// RFC-049 D9: the inverse of [`format_unix_seconds_utc`], **defined by
/// round-trip rather than by a second validator**.
///
/// The last line is the whole design. Parsing the six fields is easy;
/// deciding whether they name a real instant is where a hand-written
/// check would have to know about month lengths and leap years — and
/// would then be a second, independent opinion about the calendar,
/// able to disagree with the formatter. Instead: compute the seconds,
/// format them back, and accept the answer **only if it reproduces the
/// input exactly**. `"2026-13-45T99:99:99Z"` formats back as something
/// else and is rejected without this function knowing what a month is.
///
/// Pre-epoch instants are `None`: `format_unix_seconds_utc` takes `u64`,
/// so it cannot express them, and a transcript timestamped before 1970
/// is corrupt rather than old.
fn unix_seconds_from_utc_string(value: &str) -> Option<u64> {
    if !is_utc_timestamp_shape(value) {
        return None;
    }
    let year: i64 = value.get(0..4)?.parse().ok()?;
    let month: u64 = value.get(5..7)?.parse().ok()?;
    let day: u64 = value.get(8..10)?.parse().ok()?;
    let hour: u64 = value.get(11..13)?.parse().ok()?;
    let minute: u64 = value.get(14..16)?.parse().ok()?;
    let second: u64 = value.get(17..19)?.parse().ok()?;

    let days = days_from_civil(year, month, day);
    let seconds_of_day = i64::try_from(hour * 3_600 + minute * 60 + second).ok()?;
    let seconds = days.checked_mul(86_400)?.checked_add(seconds_of_day)?;
    let seconds = u64::try_from(seconds).ok()?;

    (format_unix_seconds_utc(seconds) == value).then_some(seconds)
}

/// The inverse of [`civil_from_days`] — Howard Hinnant's `days_from_civil`,
/// the companion of the algorithm that function already uses. Kept
/// beside it so the two are read together; neither validates its input,
/// which is what [`unix_seconds_from_utc_string`]'s round-trip check is
/// for.
fn days_from_civil(year: i64, month: u64, day: u64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = if month > 2 { month - 3 } else { month + 9 } as i64;
    let day_of_year = (153 * month_prime + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn is_utc_timestamp_shape(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
        })
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, u64, u64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month as u64, day as u64)
}
