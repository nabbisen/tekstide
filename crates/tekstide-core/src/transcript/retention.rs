use crate::domain::{DomainTimestamp, Transcript, TranscriptLifecycleState};

use super::policy::TranscriptRetentionLimits;

/// RFC-049 PR-049-A: whether a transcript is past `max_age_days`.
///
/// **`now` is a parameter, and that is the whole clock seam** (D7).
/// There is no clock abstraction in this crate — `DomainTimestamp::now_utc`
/// calls `SystemTime::now` directly, everywhere — so a function that
/// consulted the clock itself would make every test of it depend on wall
/// time, in a project whose flake register is largely populated by tests
/// that do. Production passes `now_utc()` at RFC-049 D2's two trigger
/// points; tests pass a fixed value and are deterministic by
/// construction rather than by tolerance.
///
/// **Every ambiguity here resolves toward keeping** (§1 of
/// `what-deleting-a-transcript-must-not-do.md`), because this answer
/// eventually deletes private local data that no undo recovers:
///
/// - **Exactly at the limit is not expired.** Strictly greater, so a
///   transcript on its last day survives its last day.
/// - **An age that cannot be computed is not an age.** A timestamp that
///   names no instant (`DomainTimestamp` validates shape, not calendar)
///   yields `None` from `unix_seconds`, and this returns `false`.
/// - **A clock that appears to run backwards expires nothing.** If `now`
///   precedes the transcript's own reference time, the elapsed span is
///   not negative, it is unknown.
/// - **Limits that are not bounded expire nothing.** `is_bounded()`
///   already treats `max_age_days == 0` as unbounded, and RFC-045's
///   parser accepts a configured `0`. Rather than read zero as "delete
///   everything immediately" — the most destructive available reading of
///   a value a user could plausibly have typed by accident — this
///   defers to the existing validity check. **Flagged for review**: it
///   means `transcript_retention_days = 0` enforces nothing rather than
///   everything, and §1 is why.
///
/// Age is measured from the **most recent evidence of activity**:
/// `last_write_at` when present, `created_at` otherwise, and the later
/// of the two if a store ever hands back a pair where the write precedes
/// the creation. A transcript written yesterday is not thirty days old
/// because it was created thirty-one days ago, and taking the later
/// timestamp is also the reading that keeps more.
pub fn is_transcript_expired(
    transcript: &Transcript,
    limits: TranscriptRetentionLimits,
    now: &DomainTimestamp,
) -> bool {
    if !limits.is_bounded() {
        return false;
    }
    let Some(reference_seconds) = most_recent_activity_seconds(transcript) else {
        return false;
    };
    let Some(now_seconds) = now.unix_seconds() else {
        return false;
    };
    let Some(elapsed_seconds) = now_seconds.checked_sub(reference_seconds) else {
        return false;
    };
    let Some(max_age_seconds) = u64::from(limits.max_age_days).checked_mul(86_400) else {
        return false;
    };
    elapsed_seconds > max_age_seconds
}

/// RFC-049 D8's "oldest", as a number the caller can order by — the
/// later of `created_at` and `last_write_at`, in seconds.
///
/// `None` when neither timestamp names a real instant, which PR-049-B's
/// selection must read as *"liveness and age cannot be determined"* and
/// therefore as not a candidate, the same direction §1 sends every other
/// unknown.
pub fn most_recent_activity_seconds(transcript: &Transcript) -> Option<u64> {
    let created = transcript.created_at.unix_seconds();
    let written = transcript
        .last_write_at
        .as_ref()
        .and_then(DomainTimestamp::unix_seconds);
    match (created, written) {
        (Some(created), Some(written)) => Some(created.max(written)),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

/// RFC-049 PR-049-A, D3: mark an expired transcript **without touching
/// its bytes**.
///
/// `Expired` means *past its limit, bytes still present, eligible for
/// cleanup* — not *deleted*. That is what keeps
/// `TranscriptLifecycleState::has_retained_bytes()` correct as already
/// written (it returns `true` for `Expired`), so no existing caller
/// changes meaning, and it is what D2's "not hidden from the user"
/// requires in practice: a transcript is **visibly eligible before it is
/// removed**.
///
/// Returns whether this call changed the state, so a caller can tell a
/// transcript it just marked from one that was already marked — and so
/// RFC-049 §4's "a cleanup that deletes nothing writes nothing" has
/// something to count.
///
/// Only a transcript that still has retained bytes is marked: one
/// already `Purged`, `DisabledByOptOut` or `CaptureFailed` has no bytes
/// to expire, and moving it to `Expired` would claim it has.
pub fn mark_transcript_expired_if_due(
    transcript: &mut Transcript,
    limits: TranscriptRetentionLimits,
    now: &DomainTimestamp,
) -> bool {
    if transcript.lifecycle_state == TranscriptLifecycleState::Expired {
        return false;
    }
    if !transcript.lifecycle_state.has_retained_bytes() {
        return false;
    }
    if !is_transcript_expired(transcript, limits, now) {
        return false;
    }
    transcript.record_lifecycle_state(TranscriptLifecycleState::Expired);
    true
}
