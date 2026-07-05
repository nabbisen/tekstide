use super::{
    AttentionState, BoardRowKind, CountDisplay, ProjectBoardViewModel, calculate_attention,
};
use crate::app::AppState;
use crate::project::{ProjectId, ProjectRuntimeSummary};
use crate::recent_project::{RecentProject, RecentProjectState, Timestamp};

#[test]
fn empty_project_board_has_first_run_state() {
    let state = AppState::default();

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    assert!(view_model.rows.is_empty());
    let empty_state = view_model
        .empty_state
        .expect("empty board should expose first-run state");
    assert_eq!(empty_state.heading, "No projects yet.");
    assert_eq!(empty_state.primary_action, "Add Project");
    assert_eq!(empty_state.secondary_action, "Open from path");
    assert_eq!(view_model.global_attention_summary, "Calm");
}

#[test]
fn project_rows_preserve_placeholder_field_shape_without_probing() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Tekstide", "/workspace/tekstide", "/workspace/tekstide");

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    assert_eq!(view_model.active_project_id, Some(project_id));
    assert_eq!(view_model.rows.len(), 1);
    let row = &view_model.rows[0];
    assert_eq!(row.branch_status, CountDisplay::Unavailable);
    assert_eq!(row.terminal_count, CountDisplay::NotImplemented);
    assert_eq!(row.agent_run_count, CountDisplay::NotImplemented);
    assert_eq!(row.approval_count, CountDisplay::KnownCount(0));
    assert_eq!(row.review_count, CountDisplay::KnownCount(0));
    assert_eq!(row.dirty_file_count, CountDisplay::KnownCount(0));
    assert_eq!(row.trust_label, "Restricted");
    assert_eq!(row.row_kind, BoardRowKind::ActiveSession);
}

#[test]
fn restored_stale_recent_project_is_displayed_without_active_session() {
    let mut state = AppState::default();
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            ProjectId::for_test(1),
            "Missing Project",
            "/missing/project",
            "/missing/project",
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            "Trusted",
        )],
    });

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    assert_eq!(state.projects().len(), 0);
    assert_eq!(view_model.rows.len(), 1);
    assert_eq!(view_model.rows[0].display_name, "Missing Project");
    assert_eq!(
        view_model.rows[0].availability_label.as_deref(),
        Some("Folder missing")
    );
    assert_eq!(view_model.rows[0].trust_label, "Restricted");
    assert_eq!(view_model.rows[0].row_kind, BoardRowKind::RecentMissing);
}

#[test]
fn attention_calculation_follows_priority_order() {
    assert_eq!(
        calculate_attention(&ProjectRuntimeSummary {
            risk_warning: true,
            pending_approvals: 1,
            review_ready_changes: 1,
            failed_processes: 1,
            running_processes: 1,
            dirty_files: 1,
            terminal_count: Some(1),
            agent_run_count: Some(1),
            close_resources: crate::close::CloseResourceSummary::provider_missing(),
        }),
        AttentionState::Risk
    );
    assert_eq!(
        calculate_attention(&ProjectRuntimeSummary {
            pending_approvals: 1,
            review_ready_changes: 1,
            ..ProjectRuntimeSummary::default()
        }),
        AttentionState::ApprovalNeeded
    );
    assert_eq!(
        calculate_attention(&ProjectRuntimeSummary {
            review_ready_changes: 1,
            failed_processes: 1,
            ..ProjectRuntimeSummary::default()
        }),
        AttentionState::Review
    );
    assert_eq!(
        calculate_attention(&ProjectRuntimeSummary {
            failed_processes: 1,
            running_processes: 1,
            ..ProjectRuntimeSummary::default()
        }),
        AttentionState::Failed
    );
    assert_eq!(
        calculate_attention(&ProjectRuntimeSummary {
            running_processes: 1,
            dirty_files: 1,
            ..ProjectRuntimeSummary::default()
        }),
        AttentionState::Running
    );
    assert_eq!(
        calculate_attention(&ProjectRuntimeSummary {
            dirty_files: 1,
            ..ProjectRuntimeSummary::default()
        }),
        AttentionState::Dirty
    );
    assert_eq!(
        calculate_attention(&ProjectRuntimeSummary::default()),
        AttentionState::Calm
    );
}

#[test]
fn view_model_uses_runtime_summary_for_known_counts_and_attention() {
    let mut state = AppState::default();
    let project_id = state.add_project_session("Active", "/workspace/active", "/workspace/active");
    state
        .project_mut(&project_id)
        .expect("project should exist")
        .set_runtime_summary(ProjectRuntimeSummary {
            pending_approvals: 2,
            review_ready_changes: 3,
            running_processes: 1,
            dirty_files: 5,
            terminal_count: Some(1),
            agent_run_count: Some(1),
            ..ProjectRuntimeSummary::default()
        });

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    let row = &view_model.rows[0];
    assert_eq!(row.terminal_count, CountDisplay::KnownCount(1));
    assert_eq!(row.agent_run_count, CountDisplay::KnownCount(1));
    assert_eq!(row.approval_count, CountDisplay::KnownCount(2));
    assert_eq!(row.review_count, CountDisplay::KnownCount(3));
    assert_eq!(row.dirty_file_count, CountDisplay::KnownCount(5));
    assert_eq!(row.attention, AttentionState::ApprovalNeeded);
    assert_eq!(view_model.global_attention_summary, "Approval needed");
}

#[test]
fn rows_sort_by_attention_then_active_recent_status_then_name() {
    let mut state = AppState::default();
    let calm_id = state.add_project_session("Calm", "/workspace/calm", "/workspace/calm");
    let risk_id = state.add_project_session("Risk", "/workspace/risk", "/workspace/risk");
    let waiting_id =
        state.add_project_session("Waiting", "/workspace/waiting", "/workspace/waiting");
    state
        .project_mut(&risk_id)
        .unwrap()
        .set_runtime_summary(ProjectRuntimeSummary {
            risk_warning: true,
            ..ProjectRuntimeSummary::default()
        });
    state
        .project_mut(&waiting_id)
        .unwrap()
        .set_runtime_summary(ProjectRuntimeSummary {
            pending_approvals: 1,
            ..ProjectRuntimeSummary::default()
        });
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            ProjectId::for_test(99),
            "Recent",
            "/missing/recent",
            "/missing/recent",
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            "Restricted",
        )],
    });

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    let names = view_model
        .rows
        .iter()
        .map(|row| row.display_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Risk", "Waiting", "Calm", "Recent"]);
    assert_eq!(view_model.rows[2].project_id, calm_id);
}
