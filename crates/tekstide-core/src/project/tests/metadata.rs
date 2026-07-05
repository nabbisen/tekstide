use super::project_session;
use crate::close::{
    CloseAssessment, CloseResourceProviderState, CloseResourceSummary, assess_close,
};
use crate::domain::DomainTimestamp;
use crate::project::{
    ProjectFileState, ProjectGitDisplayStatus, ProjectGitSummary, ProjectMetadataCountStatus,
    ProjectMode, ProjectOpenSurface, ProjectProviderState, ProjectResourceLimits,
    ProjectRuntimeSummary, ProjectWarning, ProjectWarningLevel, ProjectWarningState,
};

#[test]
fn new_project_keeps_count_placeholders_until_domain_collections_are_populated() {
    let project = project_session(1);

    assert_eq!(project.runtime_summary(), &ProjectRuntimeSummary::default());
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());
    assert!(project.approval_requests().is_empty());
    assert!(project.transcripts().is_empty());
    assert!(project.change_sets().is_empty());
    assert!(project.audit_events().is_empty());
}

#[test]
fn new_project_initializes_rfc002_session_metadata_with_inert_provider_defaults() {
    let project = project_session(1);

    assert_eq!(project.created_at(), project.last_opened_at());
    assert_eq!(project.last_opened_at(), project.last_activity_at());
    assert!(DomainTimestamp::from_utc_string(project.created_at().as_str()).is_ok());
    assert_eq!(project.open_surface(), ProjectOpenSurface::ProjectDashboard);
    assert_eq!(project.mode(), ProjectMode::Content);
    assert_eq!(
        project.resource_limits(),
        ProjectResourceLimits {
            visible_terminal_limit: Some(2),
            terminal_session_limit: None,
            agent_run_limit: None,
            approval_request_limit: None,
        }
    );
    assert_eq!(project.file_state(), &ProjectFileState::default());
    assert_eq!(project.git_summary(), &ProjectGitSummary::default());
    assert_eq!(project.warning_state(), &ProjectWarningState::default());
    assert_eq!(
        project.file_state().provider_state,
        ProjectProviderState::NotImplemented
    );
    assert_eq!(
        project.git_summary().provider_state,
        ProjectProviderState::NotImplemented
    );
}

#[test]
fn project_session_open_and_activity_timestamps_are_explicitly_mutable() {
    let mut project = project_session(1);
    let created_at = project.created_at().clone();
    let opened_at = DomainTimestamp::from_utc_string("2026-07-05T01:02:03Z").unwrap();
    let activity_at = DomainTimestamp::from_utc_string("2026-07-05T04:05:06Z").unwrap();

    project.mark_opened_at(opened_at.clone());
    assert_eq!(project.created_at(), &created_at);
    assert_eq!(project.last_opened_at(), &opened_at);
    assert_eq!(project.last_activity_at(), &opened_at);

    project.record_activity_at(activity_at.clone());
    assert_eq!(project.last_opened_at(), &opened_at);
    assert_eq!(project.last_activity_at(), &activity_at);
}

#[test]
fn project_session_surface_mode_and_deferred_summaries_are_owned_per_project() {
    let mut project = project_session(1);

    project.set_open_surface(ProjectOpenSurface::DiffReview);
    project.set_mode(ProjectMode::TerminalImmersion);
    project.set_resource_limits(ProjectResourceLimits {
        visible_terminal_limit: Some(2),
        terminal_session_limit: Some(8),
        agent_run_limit: Some(3),
        approval_request_limit: Some(20),
    });
    project.set_file_state(ProjectFileState {
        provider_state: ProjectProviderState::Complete,
        open_buffer_count: 2,
        dirty_file_count: 1,
        active_path_hint: Some("src/lib.rs".into()),
    });
    project.set_git_summary(ProjectGitSummary {
        provider_state: ProjectProviderState::Unavailable,
        branch_name: None,
        changed_file_count: None,
        ahead_count: None,
        behind_count: None,
    });
    project.set_warning_state(ProjectWarningState {
        warnings: vec![ProjectWarning {
            level: ProjectWarningLevel::Risk,
            code: "trust-root-changed".to_owned(),
            message: "Project root identity requires review.".to_owned(),
        }],
    });

    assert_eq!(project.open_surface(), ProjectOpenSurface::DiffReview);
    assert_eq!(project.mode(), ProjectMode::TerminalImmersion);
    assert_eq!(project.resource_limits().terminal_session_limit, Some(8));
    assert_eq!(project.file_state().dirty_file_count, 1);
    assert_eq!(
        project.git_summary().provider_state,
        ProjectProviderState::Unavailable
    );
    assert_eq!(project.runtime_summary().dirty_files, 1);
    assert_eq!(project.close_resource_summary().dirty_files, 1);
    assert!(project.runtime_summary().risk_warning);
}

#[test]
fn incomplete_file_provider_dirty_count_does_not_make_close_resources_authoritative() {
    let mut project = project_session(1);
    project.set_runtime_summary(ProjectRuntimeSummary {
        close_resources: CloseResourceSummary {
            provider_state: CloseResourceProviderState::Complete,
            running_processes: 0,
            dirty_files: 0,
            pending_approvals: 0,
            review_ready_changes: 0,
        },
        ..ProjectRuntimeSummary::default()
    });

    project.set_file_state(ProjectFileState {
        provider_state: ProjectProviderState::NotImplemented,
        open_buffer_count: 0,
        dirty_file_count: 3,
        active_path_hint: None,
    });

    assert_eq!(
        project.file_state().dirty_status(),
        ProjectMetadataCountStatus::NotImplemented
    );
    assert_eq!(project.runtime_summary().dirty_files, 3);
    assert_eq!(project.close_resource_summary().dirty_files, 0);
    assert_eq!(
        project.close_resource_summary().provider_state,
        CloseResourceProviderState::Unavailable
    );
    assert_eq!(
        assess_close(project.close_resource_summary()),
        CloseAssessment::UnsupportedOrUnknown {
            reason: "active-resource state is unavailable".to_owned()
        }
    );
}

#[test]
fn provider_backed_metadata_helpers_do_not_treat_values_as_known_when_provider_is_incomplete() {
    let file_state = ProjectFileState {
        provider_state: ProjectProviderState::Unknown,
        open_buffer_count: 1,
        dirty_file_count: 9,
        active_path_hint: Some("src/lib.rs".into()),
    };
    let git_summary = ProjectGitSummary {
        provider_state: ProjectProviderState::NotImplemented,
        branch_name: Some("main".to_owned()),
        changed_file_count: Some(3),
        ahead_count: Some(1),
        behind_count: Some(0),
    };
    let complete_git_summary = ProjectGitSummary {
        provider_state: ProjectProviderState::Complete,
        branch_name: Some("main".to_owned()),
        changed_file_count: Some(3),
        ahead_count: Some(1),
        behind_count: Some(0),
    };

    assert_eq!(
        file_state.dirty_status(),
        ProjectMetadataCountStatus::Unknown
    );
    assert_eq!(
        git_summary.display_status(),
        ProjectGitDisplayStatus::NotImplemented
    );
    assert_eq!(
        complete_git_summary.display_status(),
        ProjectGitDisplayStatus::Known {
            branch_name: Some("main".to_owned()),
            changed_file_count: Some(3),
            ahead_count: Some(1),
            behind_count: Some(0),
        }
    );
}
