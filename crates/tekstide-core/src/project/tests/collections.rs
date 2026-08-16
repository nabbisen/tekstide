use super::project_session;
use crate::close::CloseResourceProviderState;
use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunStatus, ApprovalRequest, AuditEvent,
    AuditEventClass, ChangeSet, OwnershipError, ReviewState, RiskLevel, TerminalKind,
    TerminalSession, TerminalStatus, TerminalTransitionError, Transcript, VisibleSlot,
};
use crate::project::{ProjectId, ProjectMode, ProjectTerminalError};

#[test]
fn terminal_collection_updates_project_runtime_counts_without_claiming_safe_close() {
    let mut project = project_session(1);
    let mut terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project-1",
        "bash",
    );
    terminal.transition_to(TerminalStatus::Running).unwrap();

    project
        .add_terminal_session(terminal.clone())
        .expect("same-project terminal should attach");

    assert_eq!(project.terminal_sessions(), &[terminal]);
    assert_eq!(project.runtime_summary().terminal_count, Some(1));
    assert_eq!(project.runtime_summary().agent_run_count, Some(0));
    assert_eq!(project.runtime_summary().running_processes, 1);
    assert_eq!(project.close_resource_summary().running_processes, 1);
    assert_eq!(
        project.close_resource_summary().provider_state,
        CloseResourceProviderState::Unavailable,
        "collection-derived counts must not prove all close resources are known"
    );
}

#[test]
fn terminal_lifecycle_updates_project_runtime_summary_from_real_sessions() {
    let mut project = project_session(1);
    let terminal = running_terminal(project.id().clone(), "Shell");
    let terminal_id = terminal.id.clone();

    project.add_terminal_session(terminal).unwrap();
    assert_eq!(project.runtime_summary().running_processes, 1);
    assert_eq!(project.runtime_summary().failed_processes, 0);

    project
        .transition_terminal_status(&terminal_id, TerminalStatus::Terminating)
        .unwrap();
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        TerminalStatus::Terminating
    );
    assert_eq!(project.runtime_summary().running_processes, 1);

    project.mark_terminal_exited(&terminal_id, Some(0)).unwrap();
    let terminal = project.terminal_session(&terminal_id).unwrap();
    assert_eq!(terminal.status(), TerminalStatus::Exited);
    assert_eq!(terminal.exit_status, Some(0));
    assert_eq!(project.runtime_summary().running_processes, 0);
    assert_eq!(project.close_resource_summary().running_processes, 0);
}

#[test]
fn terminal_lifecycle_rejects_invalid_project_transition_without_summary_mutation() {
    let mut project = project_session(1);
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project-1",
        "bash",
    );
    let terminal_id = terminal.id.clone();

    project.add_terminal_session(terminal).unwrap();

    let error = project
        .transition_terminal_status(&terminal_id, TerminalStatus::Exited)
        .expect_err("starting terminal cannot be marked exited");

    assert_eq!(
        error,
        ProjectTerminalError::InvalidTransition(TerminalTransitionError {
            from: TerminalStatus::Starting,
            to: TerminalStatus::Exited,
        })
    );
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        TerminalStatus::Starting
    );
    assert_eq!(project.runtime_summary().running_processes, 1);
}

#[test]
fn visible_terminal_slots_replace_existing_slot_and_cap_visible_count_at_two() {
    let mut project = project_session(1);
    let first = running_terminal(project.id().clone(), "First");
    let second = running_terminal(project.id().clone(), "Second");
    let third = running_terminal(project.id().clone(), "Third");
    let first_id = first.id.clone();
    let second_id = second.id.clone();
    let third_id = third.id.clone();

    project.add_terminal_session(first).unwrap();
    project.add_terminal_session(second).unwrap();
    project.add_terminal_session(third).unwrap();
    project
        .assign_terminal_visible_slot(&first_id, VisibleSlot::Primary)
        .unwrap();
    project
        .assign_terminal_visible_slot(&second_id, VisibleSlot::Secondary)
        .unwrap();
    project
        .assign_terminal_visible_slot(&third_id, VisibleSlot::Primary)
        .unwrap();

    assert_eq!(
        project.terminal_session(&first_id).unwrap().visible_slot(),
        VisibleSlot::Hidden
    );
    assert_eq!(
        project.terminal_session(&second_id).unwrap().visible_slot(),
        VisibleSlot::Secondary
    );
    assert_eq!(
        project.terminal_session(&third_id).unwrap().visible_slot(),
        VisibleSlot::Primary
    );
    assert_eq!(project.visible_terminal_sessions().count(), 2);
}

#[test]
fn mode_switch_preserves_running_terminal_state_and_visible_slot() {
    let mut project = project_session(1);
    let terminal = running_terminal(project.id().clone(), "Shell");
    let terminal_id = terminal.id.clone();

    project.add_terminal_session(terminal).unwrap();
    project
        .assign_terminal_visible_slot(&terminal_id, VisibleSlot::Primary)
        .unwrap();

    project.set_mode(ProjectMode::TerminalImmersion);
    project.set_mode(ProjectMode::Content);

    let terminal = project.terminal_session(&terminal_id).unwrap();
    assert_eq!(terminal.status(), TerminalStatus::Running);
    assert_eq!(terminal.visible_slot(), VisibleSlot::Primary);
    assert_eq!(project.runtime_summary().running_processes, 1);
}

#[test]
fn terminal_state_mutation_rejects_missing_terminal_reference() {
    let mut project = project_session(1);
    let other_project_terminal = running_terminal(ProjectId::for_test(2), "Other");

    let error = project
        .assign_terminal_visible_slot(&other_project_terminal.id, VisibleSlot::Primary)
        .expect_err("terminal from another project is not attached to this session");

    assert_eq!(
        error,
        ProjectTerminalError::Ownership(OwnershipError::MissingReference)
    );
    assert!(project.terminal_sessions().is_empty());
}

#[test]
fn agent_approval_and_change_collections_feed_project_runtime_summary() {
    let mut project = project_session(1);
    let mut run = AgentRun::draft(
        project.id().clone(),
        "plain",
        "summarize changes",
        AgentCompatibilityLevel::Plain,
    );
    run.transition_to(AgentRunStatus::Ready).unwrap();
    run.transition_to(AgentRunStatus::Preparing).unwrap();
    let approval = ApprovalRequest::pending(
        project.id().clone(),
        Some(run.id.clone()),
        "command",
        "cargo test",
        RiskLevel::Medium,
        Vec::new(),
        "/workspace/project-1",
    );
    let change_set = ChangeSet::unreviewed(
        project.id().clone(),
        Some(run.id.clone()),
        vec!["src/lib.rs".into()],
        "core changes",
    );

    project.add_agent_run(run).unwrap();
    project.add_approval_request(approval).unwrap();
    let change_set_id = change_set.id.clone();
    project.add_change_set(change_set).unwrap();

    let summary = project.runtime_summary();
    assert_eq!(summary.agent_run_count, Some(1));
    assert_eq!(summary.pending_approvals, 1);
    assert_eq!(summary.review_ready_changes, 1);
    assert_eq!(summary.running_processes, 1);
    assert_eq!(summary.close_resources.pending_approvals, 1);
    assert_eq!(summary.close_resources.review_ready_changes, 1);
    let run = project
        .agent_runs()
        .iter()
        .find(|run| run.profile_id == "plain")
        .unwrap();
    assert_eq!(run.change_set_ids, vec![change_set_id]);
}

/// RFC-022 PR-022-E ("the arrival model"): "expired proposals stop
/// counting toward `pending_approvals`" -- proven against the real
/// field, not asserted about the concept. `decision` staying `Pending`
/// is the point: nobody decided, and `mark_approval_expired` must not
/// change that, only what counts toward attention.
#[test]
fn mark_approval_expired_excludes_it_from_pending_approvals_without_changing_its_decision() {
    let mut project = project_session(1);
    let approval = ApprovalRequest::pending(
        project.id().clone(),
        None,
        "command",
        "cargo test",
        RiskLevel::Medium,
        Vec::new(),
        "/workspace/project-1",
    );
    let approval_id = approval.id.clone();
    project.add_approval_request(approval).unwrap();
    assert_eq!(project.runtime_summary().pending_approvals, 1);

    project.mark_approval_expired(&approval_id).unwrap();

    assert_eq!(
        project.runtime_summary().pending_approvals,
        0,
        "an expired request must stop counting toward pending_approvals"
    );
    let stored = project
        .approval_requests()
        .iter()
        .find(|request| request.id == approval_id)
        .expect("the expired request must still be retained, not removed");
    assert_eq!(
        stored.decision,
        crate::domain::ApprovalDecision::Pending,
        "nobody decided -- expiry must not change that"
    );
    assert!(project.expired_approval_ids().contains(&approval_id));
}

/// RFC-022 PR-022-E: `approval_requests` retention is bounded by
/// `approval_history_limit` (response 225: a field of its own, separate
/// from `approval_request_limit`'s fd-driven live-queue bound, since an
/// expired-or-decided entry holds no file descriptor and the two are
/// different costs) -- it evicts the oldest **terminal** (decided or
/// expired) entry to make room, rather than refusing outright, since
/// refusing would silently desync this registry from whatever
/// `ApprovalCoordinator` already accepted.
#[test]
fn approval_request_retention_limit_evicts_the_oldest_terminal_entry() {
    let mut project = project_session(1);
    project.set_resource_limits(crate::project::ProjectResourceLimits {
        visible_terminal_limit: None,
        terminal_session_limit: None,
        agent_run_limit: None,
        approval_request_limit: None,
        agent_run_approval_limit: None,
        approval_history_limit: Some(2),
    });

    let first = ApprovalRequest::pending(
        project.id().clone(),
        None,
        "command",
        "first",
        RiskLevel::Low,
        Vec::new(),
        "/workspace/project-1",
    );
    let first_id = first.id.clone();
    let second = ApprovalRequest::pending(
        project.id().clone(),
        None,
        "command",
        "second",
        RiskLevel::Low,
        Vec::new(),
        "/workspace/project-1",
    );
    let second_id = second.id.clone();
    project.add_approval_request(first).unwrap();
    project.add_approval_request(second).unwrap();
    assert_eq!(project.approval_requests().len(), 2);

    // Mark the first (oldest) as expired -- now evictable.
    project.mark_approval_expired(&first_id).unwrap();

    let third = ApprovalRequest::pending(
        project.id().clone(),
        None,
        "command",
        "third",
        RiskLevel::Low,
        Vec::new(),
        "/workspace/project-1",
    );
    let third_id = third.id.clone();
    project
        .add_approval_request(third)
        .expect("at capacity, but the expired entry should be evicted to make room");

    assert_eq!(
        project.approval_requests().len(),
        2,
        "the retained total must stay at the limit, not grow past it"
    );
    let ids: Vec<_> = project
        .approval_requests()
        .iter()
        .map(|request| request.id.clone())
        .collect();
    assert!(
        !ids.contains(&first_id),
        "the oldest, now-expired entry must be the one evicted"
    );
    assert!(
        ids.contains(&second_id),
        "the still-live entry must survive"
    );
    assert!(ids.contains(&third_id), "the new entry must be admitted");
}

/// The backstop half: if every retained entry is genuinely still live
/// (nothing terminal to evict), a new one at capacity is refused rather
/// than silently dropping an answerable request from view.
#[test]
fn approval_request_retention_limit_refuses_when_nothing_is_evictable() {
    let mut project = project_session(1);
    project.set_resource_limits(crate::project::ProjectResourceLimits {
        visible_terminal_limit: None,
        terminal_session_limit: None,
        agent_run_limit: None,
        approval_request_limit: None,
        agent_run_approval_limit: None,
        approval_history_limit: Some(1),
    });

    let first = ApprovalRequest::pending(
        project.id().clone(),
        None,
        "command",
        "first",
        RiskLevel::Low,
        Vec::new(),
        "/workspace/project-1",
    );
    project.add_approval_request(first).unwrap();

    let second = ApprovalRequest::pending(
        project.id().clone(),
        None,
        "command",
        "second",
        RiskLevel::Low,
        Vec::new(),
        "/workspace/project-1",
    );
    assert_eq!(
        project.add_approval_request(second),
        Err(crate::project::ProjectApprovalError::RetentionLimitExceeded { limit: 1 }),
        "with the one retained entry still live, there is nothing to evict"
    );
    assert_eq!(project.approval_requests().len(), 1);
}

#[test]
fn changeset_review_transition_updates_project_runtime_counts_without_file_effects() {
    let mut project = project_session(1);
    let change_set = ChangeSet::unreviewed(
        project.id().clone(),
        None,
        vec!["src/lib.rs".into()],
        "review me",
    );
    let change_set_id = change_set.id.clone();

    project.add_change_set(change_set).unwrap();
    assert_eq!(project.runtime_summary().review_ready_changes, 1);

    project
        .transition_change_set_review_state(&change_set_id, ReviewState::Accepted)
        .unwrap();

    assert_eq!(
        project
            .change_sets()
            .iter()
            .find(|change_set| change_set.id == change_set_id)
            .unwrap()
            .review_state,
        ReviewState::Accepted
    );
    assert_eq!(project.runtime_summary().review_ready_changes, 0);
    assert_eq!(project.close_resource_summary().review_ready_changes, 0);
}

fn running_terminal(project_id: ProjectId, title: &str) -> TerminalSession {
    let mut terminal = TerminalSession::new(
        project_id,
        TerminalKind::Plain,
        title,
        "/workspace/project-1",
        "bash",
    );
    terminal.transition_to(TerminalStatus::Running).unwrap();
    terminal
}

#[test]
fn project_collections_reject_cross_project_entities() {
    let mut project = project_session(1);
    let other_project_id = ProjectId::for_test(2);
    let terminal = TerminalSession::new(
        other_project_id.clone(),
        TerminalKind::Plain,
        "Other",
        "/workspace/other",
        "bash",
    );
    let run = AgentRun::draft(
        other_project_id.clone(),
        "plain",
        "other prompt",
        AgentCompatibilityLevel::Plain,
    );
    let approval = ApprovalRequest::pending(
        other_project_id.clone(),
        None,
        "command",
        "cargo test",
        RiskLevel::Low,
        Vec::new(),
        "/workspace/other",
    );
    let transcript = Transcript::metadata(
        other_project_id.clone(),
        terminal.id.clone(),
        None,
        "transcripts/other.log",
        "session",
    );
    let change_set = ChangeSet::unreviewed(other_project_id, None, vec![], "other changes");

    assert_eq!(
        project.add_terminal_session(terminal),
        Err(ProjectTerminalError::Ownership(
            OwnershipError::CrossProject
        ))
    );
    assert_eq!(
        project.add_agent_run(run),
        Err(OwnershipError::CrossProject)
    );
    assert_eq!(
        project.add_approval_request(approval),
        Err(crate::project::ProjectApprovalError::Ownership(
            OwnershipError::CrossProject
        ))
    );
    assert_eq!(
        project.add_transcript(transcript),
        Err(OwnershipError::CrossProject)
    );
    assert_eq!(
        project.add_change_set(change_set),
        Err(OwnershipError::CrossProject)
    );
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());
    assert!(project.approval_requests().is_empty());
    assert!(project.transcripts().is_empty());
    assert!(project.change_sets().is_empty());
}

#[test]
fn project_collections_reject_duplicate_entity_ids() {
    let mut project = project_session(1);
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project-1",
        "bash",
    );

    project.add_terminal_session(terminal.clone()).unwrap();

    assert_eq!(
        project.add_terminal_session(terminal),
        Err(ProjectTerminalError::Ownership(
            OwnershipError::DuplicateAttachment
        ))
    );
    assert_eq!(project.terminal_sessions().len(), 1);
}

/// Terminal launch UX handoff: `terminal_session_limit` is refused with
/// a typed error naming the real limit, not a panic and not a silent
/// no-op -- enforced in `add_terminal_session` itself, not by a caller
/// that could forget to check.
#[test]
fn terminal_session_limit_is_enforced_with_a_typed_refusal() {
    let mut project = project_session(1);
    project.set_resource_limits(crate::project::ProjectResourceLimits {
        visible_terminal_limit: Some(2),
        terminal_session_limit: Some(2),
        agent_run_limit: None,
        approval_request_limit: None,
        agent_run_approval_limit: None,
        approval_history_limit: None,
    });

    for index in 0..2 {
        let terminal = TerminalSession::new(
            project.id().clone(),
            TerminalKind::Plain,
            format!("Shell {index}"),
            "/workspace/project-1",
            "bash",
        );
        project
            .add_terminal_session(terminal)
            .expect("the first two sessions must be accepted, under the limit");
    }

    let third = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell 3",
        "/workspace/project-1",
        "bash",
    );
    assert_eq!(
        project.add_terminal_session(third),
        Err(ProjectTerminalError::SessionLimitExceeded { limit: 2 })
    );
    assert_eq!(
        project.terminal_sessions().len(),
        2,
        "a refused session must not be added"
    );
}

#[test]
fn project_audit_collection_requires_project_owned_events() {
    let mut project = project_session(1);
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project-1",
        "bash",
    );
    let event = AuditEvent::new(None, AuditEventClass::TerminalStarted, "terminal started")
        .for_terminal(&terminal)
        .unwrap();
    let global_event = AuditEvent::new(None, AuditEventClass::ConfigChanged, "config changed");
    let cross_project_event = AuditEvent::new(
        Some(ProjectId::for_test(2)),
        AuditEventClass::ConfigChanged,
        "other project",
    );

    project.add_terminal_session(terminal).unwrap();
    project.add_audit_event(event.clone()).unwrap();

    assert_eq!(project.audit_events(), &[event]);
    assert_eq!(
        project.add_audit_event(global_event),
        Err(OwnershipError::MissingProject)
    );
    assert_eq!(
        project.add_audit_event(cross_project_event),
        Err(OwnershipError::CrossProject)
    );
}
