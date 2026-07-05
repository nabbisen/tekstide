use super::project_session;
use crate::domain::{
    AgentCompatibilityLevel, AgentRun, ApprovalRequest, AuditEvent, AuditEventClass, ChangeSet,
    OwnershipError, RiskLevel, TerminalKind, TerminalSession, Transcript,
};

#[test]
fn approval_requests_reject_missing_agent_run_references() {
    let mut project = project_session(1);
    let run = AgentRun::draft(
        project.id().clone(),
        "plain",
        "summarize changes",
        AgentCompatibilityLevel::Plain,
    );
    let approval = ApprovalRequest::pending(
        project.id().clone(),
        Some(run.id),
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/project-1",
    );

    assert_eq!(
        project.add_approval_request(approval),
        Err(OwnershipError::MissingReference)
    );
    assert!(project.approval_requests().is_empty());
}

#[test]
fn transcripts_reject_missing_terminal_references() {
    let mut project = project_session(1);
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project-1",
        "bash",
    );
    let transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id,
        None,
        "transcripts/shell.log",
        "session",
    );

    assert_eq!(
        project.add_transcript(transcript),
        Err(OwnershipError::MissingReference)
    );
    assert!(project.transcripts().is_empty());
}

#[test]
fn transcripts_reject_missing_agent_run_references() {
    let mut project = project_session(1);
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project-1",
        "bash",
    );
    let run = AgentRun::draft(
        project.id().clone(),
        "plain",
        "summarize changes",
        AgentCompatibilityLevel::Plain,
    );
    let transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        Some(run.id),
        "transcripts/shell.log",
        "session",
    );

    project.add_terminal_session(terminal).unwrap();

    assert_eq!(
        project.add_transcript(transcript),
        Err(OwnershipError::MissingReference)
    );
    assert!(project.transcripts().is_empty());
}

#[test]
fn change_sets_reject_missing_agent_run_references() {
    let mut project = project_session(1);
    let run = AgentRun::draft(
        project.id().clone(),
        "plain",
        "summarize changes",
        AgentCompatibilityLevel::Plain,
    );
    let change_set = ChangeSet::unreviewed(
        project.id().clone(),
        Some(run.id),
        vec!["src/lib.rs".into()],
        "core changes",
    );

    assert_eq!(
        project.add_change_set(change_set),
        Err(OwnershipError::MissingReference)
    );
    assert!(project.change_sets().is_empty());
}

#[test]
fn audit_events_reject_missing_terminal_references() {
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

    assert_eq!(
        project.add_audit_event(event),
        Err(OwnershipError::MissingReference)
    );
    assert!(project.audit_events().is_empty());
}

#[test]
fn audit_events_reject_missing_agent_run_references() {
    let mut project = project_session(1);
    let run = AgentRun::draft(
        project.id().clone(),
        "plain",
        "summarize changes",
        AgentCompatibilityLevel::Plain,
    );
    let event = AuditEvent::new(None, AuditEventClass::AgentRunStarted, "agent started")
        .for_agent_run(&run)
        .unwrap();

    assert_eq!(
        project.add_audit_event(event),
        Err(OwnershipError::MissingReference)
    );
    assert!(project.audit_events().is_empty());
}

#[test]
fn audit_events_reject_missing_approval_references() {
    let mut project = project_session(1);
    let approval = ApprovalRequest::pending(
        project.id().clone(),
        None,
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/project-1",
    );
    let event = AuditEvent::new(
        None,
        AuditEventClass::CommandApprovalRequested,
        "approval requested",
    )
    .for_approval(&approval)
    .unwrap();

    assert_eq!(
        project.add_audit_event(event),
        Err(OwnershipError::MissingReference)
    );
    assert!(project.audit_events().is_empty());
}
