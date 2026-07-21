use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::AgentRunId;
use crate::project::ProjectId;
use crate::transcript::{
    DEFAULT_TRANSCRIPT_MAX_AGE_DAYS, DEFAULT_TRANSCRIPT_MAX_APP_BYTES,
    DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES, DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
    TranscriptBudgetScope, TranscriptCaptureMode, TranscriptCapturePolicy,
    TranscriptLocalDataSummary, TranscriptPathErrorReason, TranscriptPathRequest,
    TranscriptPathResolver, TranscriptRetentionLimits, TranscriptRetentionState,
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

    assert!(resolved.transcript_file.starts_with(&resolved.state_root));
    assert!(!resolved.transcript_file.starts_with(&resolved.project_root));
    assert!(
        resolved
            .transcript_file
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
fn transcript_path_rejects_project_root_inside_state_root_when_transcript_would_be_project_local() {
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

    assert!(resolved.transcript_file.starts_with(&resolved.state_root));
    assert!(!resolved.transcript_file.starts_with(&resolved.project_root));
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
