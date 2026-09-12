use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{
    AgentRunId, DomainTimestamp, TerminalId, Transcript, TranscriptLifecycleState,
};
use crate::project::ProjectId;
use crate::transcript::TranscriptStoragePath;
use crate::transcript::{
    BoundedTranscriptWriter, DEFAULT_TRANSCRIPT_MAX_AGE_DAYS, DEFAULT_TRANSCRIPT_MAX_APP_BYTES,
    DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES, DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
    TranscriptBudgetScope, TranscriptCaptureMode, TranscriptCapturePolicy,
    TranscriptLocalDataSummary, TranscriptPathErrorReason, TranscriptPathRequest,
    TranscriptPathResolver, TranscriptRetentionLimits, TranscriptRetentionState,
    TranscriptWriteErrorReason, TranscriptWriterConfig, is_transcript_expired,
    mark_transcript_expired_if_due, most_recent_activity_seconds,
};

#[test]
fn default_capture_policy_is_local_bounded_and_aggregate_limited() {
    let policy = TranscriptCapturePolicy::local_bounded_agent_run_default();

    assert_eq!(policy.mode, TranscriptCaptureMode::LocalBounded);
    assert_eq!(
        policy.retention_limits.max_bytes_per_transcript,
        DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES
    );
    assert_eq!(
        policy.retention_limits.max_bytes_per_project,
        DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES
    );
    assert_eq!(
        policy.retention_limits.max_bytes_app_wide,
        DEFAULT_TRANSCRIPT_MAX_APP_BYTES
    );
    assert_eq!(
        policy.retention_limits.max_age_days,
        DEFAULT_TRANSCRIPT_MAX_AGE_DAYS
    );
    assert!(policy.permits_transcript_byte_persistence());
}

#[test]
fn capture_policy_rejects_unbounded_aggregate_limits() {
    let zero_project_budget = TranscriptRetentionLimits::new(
        DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
        0,
        DEFAULT_TRANSCRIPT_MAX_APP_BYTES,
        DEFAULT_TRANSCRIPT_MAX_AGE_DAYS,
    );
    let smaller_project_budget_than_transcript = TranscriptRetentionLimits::new(
        DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
        DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES - 1,
        DEFAULT_TRANSCRIPT_MAX_APP_BYTES,
        DEFAULT_TRANSCRIPT_MAX_AGE_DAYS,
    );
    let smaller_app_budget_than_project = TranscriptRetentionLimits::new(
        DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
        DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES,
        DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES - 1,
        DEFAULT_TRANSCRIPT_MAX_AGE_DAYS,
    );

    assert!(
        !TranscriptCapturePolicy::local_bounded_agent_run_default()
            .with_limits(zero_project_budget)
            .permits_transcript_byte_persistence()
    );
    assert!(
        !TranscriptCapturePolicy::local_bounded_agent_run_default()
            .with_limits(smaller_project_budget_than_transcript)
            .permits_transcript_byte_persistence()
    );
    assert!(
        !TranscriptCapturePolicy::local_bounded_agent_run_default()
            .with_limits(smaller_app_budget_than_project)
            .permits_transcript_byte_persistence()
    );
}

#[test]
fn disabled_capture_policy_never_persists_bytes() {
    let policy = TranscriptCapturePolicy::metadata_only();

    assert_eq!(policy.mode, TranscriptCaptureMode::Disabled);
    assert!(!policy.mode.captures_bytes());
    assert!(!policy.permits_transcript_byte_persistence());
}

#[test]
fn required_local_bounded_rejects_launch_when_unavailable() {
    let policy = TranscriptCapturePolicy::required_local_bounded(
        TranscriptRetentionLimits::agent_run_default(),
    );

    assert_eq!(policy.mode, TranscriptCaptureMode::RequiredLocalBounded);
    assert!(policy.mode.rejects_launch_when_unavailable());
    assert!(policy.permits_transcript_byte_persistence());
}

#[test]
fn local_data_summary_reports_aggregate_budget_pressure_without_content() {
    let limits = TranscriptRetentionLimits::agent_run_default();
    let within_budget = TranscriptLocalDataSummary::new(
        limits.max_bytes_per_project,
        limits.max_bytes_app_wide,
        12,
        limits,
    );
    let project_pressure = TranscriptLocalDataSummary::new(
        limits.max_bytes_per_project + 1,
        limits.max_bytes_app_wide,
        13,
        limits,
    );
    let app_pressure = TranscriptLocalDataSummary::new(
        limits.max_bytes_per_project,
        limits.max_bytes_app_wide + 1,
        14,
        limits,
    );

    assert_eq!(within_budget.budget_pressure, None);
    assert_eq!(
        project_pressure.budget_pressure,
        Some(TranscriptBudgetScope::Project)
    );
    assert_eq!(
        app_pressure.budget_pressure,
        Some(TranscriptBudgetScope::App)
    );
    assert_eq!(app_pressure.project_transcript_count, 14);
}

#[test]
fn retention_state_keeps_purged_transcript_as_tombstone() {
    assert!(TranscriptRetentionState::Purged.is_tombstone());
    assert!(!TranscriptRetentionState::DisabledByOptOut.has_retained_bytes());
    assert!(
        TranscriptRetentionState::Truncated {
            scope: TranscriptBudgetScope::Transcript
        }
        .has_retained_bytes()
    );
}

#[test]
fn transcript_path_resolves_under_state_root_and_outside_project_root() {
    let temp = TestDirs::new("valid-path");
    let request = TranscriptPathRequest::new(
        &temp.state_root,
        &temp.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );

    let resolved = TranscriptPathResolver.resolve_agent_run(request).unwrap();

    assert!(
        resolved
            .transcript_file()
            .starts_with(resolved.state_root())
    );
    assert!(
        !resolved
            .transcript_file()
            .starts_with(resolved.project_root())
    );
    assert!(
        resolved
            .transcript_file()
            .ends_with(Path::new("transcript.log"))
    );
}

#[test]
fn transcript_path_rejects_relative_state_root() {
    let temp = TestDirs::new("relative-state-root");
    let request = TranscriptPathRequest::new(
        PathBuf::from("relative-state"),
        &temp.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );

    let error = TranscriptPathResolver
        .resolve_agent_run(request)
        .unwrap_err();

    assert_eq!(
        error.reason,
        TranscriptPathErrorReason::StateRootNotAbsolute
    );
}

#[test]
fn transcript_path_rejects_state_root_inside_project_root() {
    let temp = TestDirs::new("state-inside-project");
    let state_inside_project = temp.project_root.join(".tekstide-state");
    fs::create_dir_all(&state_inside_project).unwrap();

    let request = TranscriptPathRequest::new(
        state_inside_project,
        &temp.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );

    let error = TranscriptPathResolver
        .resolve_agent_run(request)
        .unwrap_err();

    assert_eq!(
        error.reason,
        TranscriptPathErrorReason::StateRootInsideProjectRoot
    );
}

#[test]
fn transcript_path_allows_project_root_inside_state_root_when_output_stays_outside_project() {
    let temp = TestDirs::new("project-inside-state-root");
    let project_inside_state = temp.state_root.join("workspace");
    fs::create_dir_all(&project_inside_state).unwrap();

    let request = TranscriptPathRequest::new(
        &temp.state_root,
        project_inside_state,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );

    let resolved = TranscriptPathResolver.resolve_agent_run(request).unwrap();

    assert!(
        resolved
            .transcript_file()
            .starts_with(resolved.state_root())
    );
    assert!(
        !resolved
            .transcript_file()
            .starts_with(resolved.project_root())
    );
}

#[test]
fn bounded_writer_creates_file_and_records_byte_count_without_content_summary() {
    let (temp, storage_path) = resolved_storage_path("writer-records-byte-count");
    let mut writer = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path.clone(),
        TranscriptRetentionLimits::agent_run_default(),
        TranscriptCaptureMode::LocalBounded,
    ))
    .unwrap();

    let summary = writer.append(b"secret transcript bytes").unwrap();
    let flushed = writer.flush().unwrap();

    assert_eq!(summary.byte_count, 23);
    assert_eq!(summary.retention_state, TranscriptRetentionState::Active);
    assert_eq!(flushed, summary);
    assert_eq!(fs::read(storage_path.transcript_file()).unwrap().len(), 23);
    assert!(!format!("{summary:?}").contains("secret"));
    drop(temp);
}

#[test]
fn bounded_writer_truncates_at_per_transcript_limit() {
    let (_temp, storage_path) = resolved_storage_path("writer-truncates");
    let limits = TranscriptRetentionLimits::new(5, 5, 5, DEFAULT_TRANSCRIPT_MAX_AGE_DAYS);
    let mut writer = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path.clone(),
        limits,
        TranscriptCaptureMode::LocalBounded,
    ))
    .unwrap();

    let summary = writer.append(b"abcdefghi").unwrap();
    let after_more = writer.append(b"jkl").unwrap();

    assert_eq!(summary.byte_count, 5);
    assert_eq!(
        summary.retention_state,
        TranscriptRetentionState::Truncated {
            scope: TranscriptBudgetScope::Transcript
        }
    );
    assert_eq!(after_more, summary);
    assert_eq!(fs::read(storage_path.transcript_file()).unwrap(), b"abcde");
}

#[test]
fn bounded_writer_allows_exact_limit_without_truncation() {
    let (_temp, storage_path) = resolved_storage_path("writer-exact-limit");
    let limits = TranscriptRetentionLimits::new(5, 5, 5, DEFAULT_TRANSCRIPT_MAX_AGE_DAYS);
    let mut writer = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path.clone(),
        limits,
        TranscriptCaptureMode::LocalBounded,
    ))
    .unwrap();

    let summary = writer.append(b"abcde").unwrap();

    assert_eq!(summary.byte_count, 5);
    assert_eq!(summary.retention_state, TranscriptRetentionState::Active);
    assert_eq!(fs::read(storage_path.transcript_file()).unwrap(), b"abcde");
}

#[test]
fn bounded_writer_empty_append_keeps_current_summary() {
    let (_temp, storage_path) = resolved_storage_path("writer-empty-append");
    let mut writer = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path,
        TranscriptRetentionLimits::agent_run_default(),
        TranscriptCaptureMode::LocalBounded,
    ))
    .unwrap();

    let summary = writer.append(b"").unwrap();

    assert_eq!(summary.byte_count, 0);
    assert_eq!(summary.retention_state, TranscriptRetentionState::Active);
}

#[test]
fn bounded_writer_rejects_unbounded_retention_without_creating_file() {
    let (_temp, storage_path) = resolved_storage_path("writer-rejects-unbounded");
    let limits = TranscriptRetentionLimits::new(0, 0, 0, 0);

    let error = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path.clone(),
        limits,
        TranscriptCaptureMode::LocalBounded,
    ))
    .unwrap_err();

    assert_eq!(error.reason, TranscriptWriteErrorReason::UnboundedRetention);
    assert_eq!(error.byte_count, 0);
    assert!(!storage_path.transcript_file().exists());
}

#[test]
fn bounded_writer_open_error_is_bounded_and_content_free() {
    let (_temp, storage_path) = resolved_storage_path("writer-open-error");
    fs::create_dir_all(storage_path.transcript_file()).unwrap();

    let error = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path,
        TranscriptRetentionLimits::agent_run_default(),
        TranscriptCaptureMode::LocalBounded,
    ))
    .unwrap_err();

    assert_eq!(error.reason, TranscriptWriteErrorReason::OpenFileFailed);
    assert_eq!(error.byte_count, 0);
    assert!(!format!("{error}").contains("secret transcript bytes"));
}

#[test]
fn bounded_writer_rejects_forged_project_root_storage_path_before_side_effects() {
    let temp = TestDirs::new("writer-rejects-forged-project-path");
    let forged_dir = temp.project_root.join("transcripts");
    let forged_file = forged_dir.join("transcript.log");
    let storage_path = TranscriptStoragePath::for_test_unchecked(
        temp.state_root.clone(),
        temp.project_root.clone(),
        forged_dir.clone(),
        forged_file.clone(),
    );

    let error = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path,
        TranscriptRetentionLimits::agent_run_default(),
        TranscriptCaptureMode::LocalBounded,
    ))
    .unwrap_err();

    assert_eq!(error.reason, TranscriptWriteErrorReason::InvalidStoragePath);
    assert_eq!(error.byte_count, 0);
    assert!(!forged_dir.exists());
    assert!(!forged_file.exists());
}

#[cfg(unix)]
#[test]
fn transcript_path_rejects_symlinked_state_root_inside_project_root() {
    use std::os::unix::fs::symlink;

    let temp = TestDirs::new("symlink-state-inside-project");
    let project_local_state = temp.project_root.join("state-target");
    let state_link = temp.base.join("state-link");
    fs::create_dir_all(&project_local_state).unwrap();
    symlink(&project_local_state, &state_link).unwrap();

    let request = TranscriptPathRequest::new(
        state_link,
        &temp.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );

    let error = TranscriptPathResolver
        .resolve_agent_run(request)
        .unwrap_err();

    assert_eq!(
        error.reason,
        TranscriptPathErrorReason::StateRootInsideProjectRoot
    );
}

struct TestDirs {
    base: PathBuf,
    state_root: PathBuf,
    project_root: PathBuf,
}

impl TestDirs {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-transcript-{label}-{}-{unique}",
            std::process::id()
        ));
        let state_root = base.join("state");
        let project_root = base.join("project");
        fs::create_dir_all(&state_root).unwrap();
        fs::create_dir_all(&project_root).unwrap();
        Self {
            base,
            state_root,
            project_root,
        }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn resolved_storage_path(label: &str) -> (TestDirs, crate::transcript::TranscriptStoragePath) {
    let temp = TestDirs::new(label);
    let request = TranscriptPathRequest::new(
        &temp.state_root,
        &temp.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );
    let storage_path = TranscriptPathResolver.resolve_agent_run(request).unwrap();
    (temp, storage_path)
}

// --- RFC-049 PR-049-A: expiry marking, no deletion --------------------
//
// Every instant below is a **literal** timestamp, not one computed from
// seconds by a helper. A helper would have to reimplement the civil-date
// arithmetic these tests exist to check, which is a second opinion about
// the calendar able to disagree with the code under test — the same
// reason `unix_seconds_from_utc_string` is defined by round-trip rather
// than by its own validator. Ten days after 2026-01-01 is 2026-01-11,
// and a reader can confirm that without running anything.

fn at(value: &str) -> DomainTimestamp {
    DomainTimestamp::from_utc_string(value).expect("fixture instants must be a valid shape")
}

fn transcript_created_at(created: &str) -> Transcript {
    let mut transcript = Transcript::metadata(
        ProjectId::for_test(1),
        TerminalId::for_test(1),
        Some(AgentRunId::for_test(1)),
        "/state/transcripts/run.log",
        "local-bounded-agent-run",
    );
    transcript.created_at = at(created);
    transcript.last_write_at = None;
    transcript.byte_count = 1_024;
    transcript
}

fn ten_day_limits() -> TranscriptRetentionLimits {
    let mut limits = TranscriptRetentionLimits::agent_run_default();
    limits.max_age_days = 10;
    limits
}

/// **§1, and the ablation target the plan names.** A transcript exactly
/// at its limit has not passed it. Ablate by changing `>` to `>=` in
/// `is_transcript_expired` and this fails alone.
#[test]
fn exactly_at_the_limit_is_not_expired() {
    let transcript = transcript_created_at("2026-01-01T00:00:00Z");

    assert!(
        !is_transcript_expired(&transcript, ten_day_limits(), &at("2026-01-11T00:00:00Z")),
        "exactly ten days on a ten-day limit: a transcript survives its last day -- §1"
    );
    assert!(
        is_transcript_expired(&transcript, ten_day_limits(), &at("2026-01-11T00:00:01Z")),
        "and one second past it is expired, or the limit would mean nothing"
    );
}

/// Age is measured from the most recent evidence of activity, not from
/// creation: a transcript written recently is not old because it was
/// created long ago. §1 again — the later timestamp keeps more.
#[test]
fn age_is_measured_from_the_last_write_not_the_creation() {
    let mut transcript = transcript_created_at("2026-01-01T00:00:00Z");
    transcript.last_write_at = Some(at("2026-01-20T00:00:00Z"));

    assert!(
        !is_transcript_expired(&transcript, ten_day_limits(), &at("2026-01-25T00:00:00Z")),
        "created 24 days earlier but written 5 days earlier: not expired on a 10-day limit"
    );
    assert!(
        is_transcript_expired(&transcript, ten_day_limits(), &at("2026-02-01T00:00:00Z")),
        "twelve days after that write, it is"
    );
}

/// A store that hands back a write earlier than the creation is corrupt
/// in a small way, and §1 says which way to resolve it: take the later
/// instant, which keeps the transcript longer.
#[test]
fn a_write_recorded_before_the_creation_does_not_shorten_the_life() {
    let mut transcript = transcript_created_at("2026-01-20T00:00:00Z");
    transcript.last_write_at = Some(at("2026-01-01T00:00:00Z"));

    assert_eq!(
        most_recent_activity_seconds(&transcript),
        at("2026-01-20T00:00:00Z").unix_seconds(),
        "the later of the two instants is the reference"
    );
    assert!(
        !is_transcript_expired(&transcript, ten_day_limits(), &at("2026-01-25T00:00:00Z")),
        "so this is five days old, not twenty-four"
    );
}

/// **An age that cannot be computed is not an age.** A shape-valid but
/// calendar-impossible timestamp is something a persisted store can hand
/// back, and it must not be read as "very old".
#[test]
fn a_transcript_whose_timestamp_names_no_instant_is_never_expired() {
    let mut transcript = transcript_created_at("2026-01-01T00:00:00Z");
    transcript.created_at = at("2026-13-45T00:00:00Z");
    transcript.last_write_at = None;

    assert_eq!(most_recent_activity_seconds(&transcript), None);
    assert!(
        !is_transcript_expired(&transcript, ten_day_limits(), &at("2099-01-01T00:00:00Z")),
        "an uncomputable age must not delete a user's data"
    );
}

/// A clock that appears to run backwards expires nothing: the elapsed
/// span is unknown, not negative.
#[test]
fn a_now_that_precedes_the_transcript_expires_nothing() {
    let transcript = transcript_created_at("2026-06-01T00:00:00Z");

    assert!(!is_transcript_expired(
        &transcript,
        ten_day_limits(),
        &at("2026-01-01T00:00:00Z")
    ));
}

/// **Flagged decision (see `qa-evidence.md`).** `is_bounded()` already
/// treats `max_age_days == 0` as unbounded, and RFC-045's parser accepts
/// a configured `0`. Reading zero as "delete everything now" is the most
/// destructive available reading of a value a user could have typed by
/// accident, so §1 sends it the other way: unbounded limits enforce
/// nothing.
#[test]
fn limits_that_are_not_bounded_expire_nothing() {
    let transcript = transcript_created_at("2026-01-01T00:00:00Z");
    let mut zero_age = TranscriptRetentionLimits::agent_run_default();
    zero_age.max_age_days = 0;
    assert!(
        !zero_age.is_bounded(),
        "precondition: zero is what is_bounded() already calls unbounded"
    );

    assert!(!is_transcript_expired(
        &transcript,
        zero_age,
        &at("2099-01-01T00:00:00Z")
    ));
}

/// D3: marking keeps the bytes. `Expired` means *eligible*, not
/// *deleted* — which is what keeps `has_retained_bytes()` true and lets
/// a user see a transcript is due before it goes.
#[test]
fn marking_expired_keeps_the_bytes_and_still_reports_them_as_retained() {
    let mut transcript = transcript_created_at("2026-01-01T00:00:00Z");
    let byte_count_before = transcript.byte_count;
    let storage_path_before = transcript.storage_path.clone();

    assert!(mark_transcript_expired_if_due(
        &mut transcript,
        ten_day_limits(),
        &at("2026-02-01T00:00:00Z")
    ));

    assert_eq!(
        transcript.lifecycle_state,
        TranscriptLifecycleState::Expired
    );
    assert_eq!(
        transcript.byte_count, byte_count_before,
        "D3: Expired keeps its bytes -- cleanup deletes them, marking does not"
    );
    assert_eq!(
        transcript.storage_path, storage_path_before,
        "and the bytes must still have somewhere to be"
    );
    assert!(
        transcript.has_retained_bytes(),
        "has_retained_bytes() must stay true for Expired, so no existing caller changes meaning"
    );
}

/// Marking is idempotent and reports it: the second call changes
/// nothing, so §4's "a cleanup that deletes nothing writes nothing" has
/// something honest to count.
#[test]
fn marking_an_already_expired_transcript_reports_no_change() {
    let mut transcript = transcript_created_at("2026-01-01T00:00:00Z");
    assert!(mark_transcript_expired_if_due(
        &mut transcript,
        ten_day_limits(),
        &at("2026-02-01T00:00:00Z")
    ));
    assert!(
        !mark_transcript_expired_if_due(
            &mut transcript,
            ten_day_limits(),
            &at("2026-02-01T00:00:00Z")
        ),
        "the second call marked nothing new"
    );
}

/// A transcript with no bytes left has nothing to expire, and claiming
/// otherwise would say it has bytes.
#[test]
fn a_purged_transcript_is_not_marked_expired() {
    let mut transcript = transcript_created_at("2026-01-01T00:00:00Z");
    transcript.mark_purged();

    assert!(!mark_transcript_expired_if_due(
        &mut transcript,
        ten_day_limits(),
        &at("2026-02-01T00:00:00Z")
    ));
    assert_eq!(transcript.lifecycle_state, TranscriptLifecycleState::Purged);
}

/// D7, stated as a test: the same inputs give the same answer every run,
/// and the signature is what guarantees it — there is no path from this
/// function to the wall clock. A test that called `now_utc()` instead
/// would pass today and depend on the date.
#[test]
fn a_fixed_now_gives_the_same_answer_every_run() {
    let transcript = transcript_created_at("2026-01-01T00:00:00Z");
    let now = at("2026-02-01T00:00:00Z");
    let first = is_transcript_expired(&transcript, ten_day_limits(), &now);

    for _ in 0..100 {
        assert_eq!(
            is_transcript_expired(&transcript, ten_day_limits(), &now),
            first
        );
    }
    assert!(first);
}
