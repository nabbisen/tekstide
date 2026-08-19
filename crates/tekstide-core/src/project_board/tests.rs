use super::{
    AttentionState, BoardRowKind, CountDisplay, ProjectBoardViewModel, calculate_attention,
};
use crate::app::AppState;
use crate::domain::{TerminalKind, TerminalSession, TerminalStatus};
use crate::project::recent::{RecentProject, RecentProjectState, Timestamp};
use crate::project::{ProjectId, ProjectRuntimeSummary, WorkspaceTrust};
use crate::security::RestrictedModeFeature;

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

/// **status-mapping-honesty-fixes, Fix 1**: a freshly opened project has
/// not yet had `refresh_runtime_summary_from_collections` run (no
/// terminal or agent run has ever been added), so `terminal_count`/
/// `agent_run_count` must read `Unknown` -- "nothing counted yet" -- not
/// `NotImplemented`, which would falsely claim the features do not
/// exist. See `CountDisplay`'s own doc comment.
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
    assert_eq!(row.terminal_count, CountDisplay::Unknown);
    assert_eq!(row.agent_run_count, CountDisplay::Unknown);
    assert_eq!(row.approval_count, CountDisplay::KnownCount(0));
    assert_eq!(row.review_count, CountDisplay::KnownCount(0));
    assert_eq!(row.dirty_file_count, CountDisplay::KnownCount(0));
    assert_eq!(row.trust_label, "Restricted");
    assert_eq!(row.security_mode_label, "Restricted Mode");
    assert!(row.restricted_mode);
    assert_eq!(
        row.blocked_automation_count,
        u32::try_from(RestrictedModeFeature::ENFORCED.len()).unwrap(),
        "response 274: must report what is actually enforced, not the whole reserved \
         vocabulary"
    );
    assert_eq!(
        row.blocked_automation_labels.len(),
        RestrictedModeFeature::ENFORCED.len()
    );
    assert!(
        row.blocked_automation_labels
            .contains(&"workspace AI prompt loading".to_owned())
    );
    assert_eq!(row.row_kind, BoardRowKind::ActiveSession);
}

#[test]
fn trusted_project_board_row_uses_security_policy_summary() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Trusted", "/workspace/trusted", "/workspace/trusted");
    state
        .project_mut(&project_id)
        .expect("project should exist")
        .grant_trust("trusted for test");

    let view_model = ProjectBoardViewModel::from_app_state(&state);
    let row = &view_model.rows[0];

    assert_eq!(row.trust_label, "Trusted");
    assert_eq!(row.security_mode_label, "Trusted Mode");
    assert!(!row.restricted_mode);
    assert_eq!(row.blocked_automation_count, 0);
    assert!(row.blocked_automation_labels.is_empty());
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
            WorkspaceTrust::Trusted,
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
    // RFC-032: `recent_project_row` now reads the real cached
    // `trust_state` (was hardcoded `Restricted` regardless of input --
    // PR-032-B's own evidence flagged this as a real, separate gap left
    // for this slice). This fixture's project is `Trusted` and its
    // availability is `FolderMissing`, not `PathChanged`, so the cached
    // value carries through -- a folder being missing says nothing about
    // whether its canonical path would still match on reopen, unlike a
    // detected redirect.
    assert_eq!(view_model.rows[0].trust_label, "Trusted");
    assert_eq!(view_model.rows[0].security_mode_label, "Trusted Mode");
    assert!(!view_model.rows[0].restricted_mode);
    assert_eq!(view_model.rows[0].blocked_automation_labels.len(), 0);
    assert_eq!(view_model.rows[0].row_kind, BoardRowKind::RecentMissing);
}

/// RFC-032: the one case the real cached `trust_state` must **not**
/// carry through to the board -- a cached `Trusted` project whose
/// canonical path no longer matches (`PathChanged`) is a project
/// `AppState::add_project_session`'s own canonical-path-keyed lookup
/// (PR-032-B) would *not* restore trust for on reopen. Showing "Trusted"
/// here would claim a grant reopening will not actually honour.
#[test]
fn a_cached_trusted_recent_project_with_a_changed_canonical_path_shows_restricted() {
    let root = std::env::temp_dir().join(format!(
        "tekstide-project-board-test-path-changed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();

    let mut state = AppState::default();
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            ProjectId::for_test(1),
            "Redirected Project",
            root.clone(),
            // A canonical path this real directory does not actually
            // resolve to -- the same synthesized-mismatch shape
            // `recent::tests::availability_reports_path_changed` already
            // uses to trigger `RecentProjectAvailability::PathChanged`
            // without needing a real symlink redirect for this
            // display-only property.
            root.join("stale-canonical-path"),
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            WorkspaceTrust::Trusted,
        )],
    });

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    assert_eq!(view_model.rows.len(), 1);
    assert_eq!(
        view_model.rows[0].availability_label.as_deref(),
        Some("Path changed"),
        "test precondition: the mismatch must actually trigger PathChanged"
    );
    assert_eq!(view_model.rows[0].trust_label, "Restricted");
    assert_eq!(view_model.rows[0].security_mode_label, "Restricted Mode");
    assert!(view_model.rows[0].restricted_mode);

    let _ = std::fs::remove_dir_all(&root);
}

/// **status-mapping-honesty-fixes, Fix 1: `recent_project_row`'s own
/// five fields, fixed the same way, not disclosed as a separate
/// limitation.** A recent-but-unopened project has no `ProjectSession`
/// to count from at all -- the same "nothing has happened yet" shape as
/// a freshly opened project before its first collection mutation, not a
/// claim that any of these features do not exist.
#[test]
fn a_recent_unopened_project_reports_unknown_counts_not_not_implemented() {
    let mut state = AppState::default();
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            ProjectId::for_test(1),
            "Recent Project",
            "/recent/project",
            "/recent/project",
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            WorkspaceTrust::Trusted,
        )],
    });

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    assert_eq!(view_model.rows.len(), 1);
    let row = &view_model.rows[0];
    assert_eq!(row.terminal_count, CountDisplay::Unknown);
    assert_eq!(row.agent_run_count, CountDisplay::Unknown);
    assert_eq!(row.approval_count, CountDisplay::Unknown);
    assert_eq!(row.review_count, CountDisplay::Unknown);
    assert_eq!(row.dirty_file_count, CountDisplay::Unknown);
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

/// **status-mapping-honesty-fixes, Fix 1's own required proof: a
/// positive control before the negative.** Asserting only that an
/// empty project reads `Unknown` would pass against a board that always
/// reports `Unknown` regardless of real state -- proven wrong first, on
/// a real terminal actually added to a *different* project, before the
/// empty project's own row is checked.
#[test]
fn a_project_with_a_terminal_reports_a_real_count_and_an_empty_one_reports_unknown() {
    let mut state = AppState::default();
    let populated_id =
        state.add_project_session("Populated", "/workspace/populated", "/workspace/populated");
    let mut terminal = TerminalSession::new(
        populated_id.clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/populated",
        "bash",
    );
    terminal.transition_to(TerminalStatus::Running).unwrap();
    state
        .project_mut(&populated_id)
        .expect("project should exist")
        .add_terminal_session(terminal)
        .unwrap();
    let empty_id = state.add_project_session("Empty", "/workspace/empty", "/workspace/empty");

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    let populated_row = view_model
        .rows
        .iter()
        .find(|row| row.project_id == populated_id)
        .expect("populated project must have a row");
    assert_eq!(
        populated_row.terminal_count,
        CountDisplay::KnownCount(1),
        "the positive control: a project with a real terminal must report a real count"
    );

    let empty_row = view_model
        .rows
        .iter()
        .find(|row| row.project_id == empty_id)
        .expect("empty project must have a row");
    assert_eq!(
        empty_row.terminal_count,
        CountDisplay::Unknown,
        "a project that has never added a terminal must read Unknown, not NotImplemented"
    );
    assert_eq!(
        empty_row.agent_run_count,
        CountDisplay::Unknown,
        "agent_run_count has the identical defect on the same two lines"
    );
}

#[test]
fn view_model_uses_real_terminal_collection_summary() {
    let mut state = AppState::default();
    let project_id = state.add_project_session("Active", "/workspace/active", "/workspace/active");
    let mut terminal = TerminalSession::new(
        project_id.clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/active",
        "bash",
    );
    terminal.transition_to(TerminalStatus::Running).unwrap();
    state
        .project_mut(&project_id)
        .expect("project should exist")
        .add_terminal_session(terminal)
        .unwrap();

    let view_model = ProjectBoardViewModel::from_app_state(&state);

    let row = &view_model.rows[0];
    assert_eq!(row.terminal_count, CountDisplay::KnownCount(1));
    assert_eq!(row.attention, AttentionState::Running);
    assert_eq!(view_model.global_attention_summary, "Running");
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
            WorkspaceTrust::Restricted,
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
