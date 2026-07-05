use super::project_session;
use crate::close::CloseResourceProviderState;
use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunStatus, ApprovalRequest, AuditEvent,
    AuditEventClass, ChangeSet, OwnershipError, RiskLevel, TerminalKind, TerminalSession,
    TerminalStatus, Transcript,
};
use crate::project::ProjectId;

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
    terminal.status = TerminalStatus::Running;

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
    project.add_change_set(change_set).unwrap();

    let summary = project.runtime_summary();
    assert_eq!(summary.agent_run_count, Some(1));
    assert_eq!(summary.pending_approvals, 1);
    assert_eq!(summary.review_ready_changes, 1);
    assert_eq!(summary.running_processes, 1);
    assert_eq!(summary.close_resources.pending_approvals, 1);
    assert_eq!(summary.close_resources.review_ready_changes, 1);
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
        Err(OwnershipError::CrossProject)
    );
    assert_eq!(
        project.add_agent_run(run),
        Err(OwnershipError::CrossProject)
    );
    assert_eq!(
        project.add_approval_request(approval),
        Err(OwnershipError::CrossProject)
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
        Err(OwnershipError::DuplicateAttachment)
    );
    assert_eq!(project.terminal_sessions().len(), 1);
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
