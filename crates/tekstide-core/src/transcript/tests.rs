use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::AgentRunId;
use crate::project::ProjectId;
use crate::transcript::TranscriptStoragePath;
use crate::transcript::{
    BoundedTranscriptWriter, DEFAULT_TRANSCRIPT_MAX_AGE_DAYS, DEFAULT_TRANSCRIPT_MAX_APP_BYTES,
    DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES, DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
    TranscriptBudgetScope, TranscriptCaptureMode, TranscriptCapturePolicy,
    TranscriptLocalDataSummary, TranscriptPathErrorReason, TranscriptPathRequest,
    TranscriptPathResolver, TranscriptRetentionLimits, TranscriptRetentionState,
    TranscriptWriteErrorReason, TranscriptWriterConfig,
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
    let mut writer =
        BoundedTranscriptWriter::create(TranscriptWriterConfig::new(storage_path.clone(), limits))
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
    let mut writer =
        BoundedTranscriptWriter::create(TranscriptWriterConfig::new(storage_path.clone(), limits))
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

    let error =
        BoundedTranscriptWriter::create(TranscriptWriterConfig::new(storage_path.clone(), limits))
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
