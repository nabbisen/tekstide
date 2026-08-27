use super::project_session;
use crate::domain::{
    AgentCompatibilityLevel, AgentRun, ApprovalRequest, ChangeSet, OwnershipError, RiskLevel,
    TerminalKind, TerminalSession, Transcript,
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
        Vec::new(),
        "/workspace/project-1",
    );

    assert_eq!(
        project.add_approval_request(approval),
        Err(crate::project::ProjectApprovalError::Ownership(
            OwnershipError::MissingReference
        ))
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
