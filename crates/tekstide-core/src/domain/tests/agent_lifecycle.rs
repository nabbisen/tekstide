use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunStatus, ApprovalDecision, ApprovalDecisionError,
    ApprovalRequest, OwnershipError, RiskLevel, TerminalKind, TerminalSession,
};
use crate::project::ProjectId;

#[test]
fn terminal_session_has_explicit_project_ownership() {
    let project_id = ProjectId::for_test(1);
    let terminal = TerminalSession::new(
        project_id.clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project",
        "bash",
    );

    assert_eq!(terminal.project_id, project_id);
    assert_eq!(terminal.kind, TerminalKind::Plain);
    assert!(terminal.created_at.as_str().ends_with('Z'));
    assert!(terminal.last_output_at.is_none());
    assert!(terminal.exit_status.is_none());
}

#[test]
fn agent_run_uses_canonical_lifecycle_transitions() {
    let project_id = ProjectId::for_test(1);
    let mut run = AgentRun::draft(
        project_id,
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Supervised,
    );

    assert_eq!(run.status, AgentRunStatus::Draft);
    run.transition_to(AgentRunStatus::Ready).unwrap();
    run.transition_to(AgentRunStatus::Preparing).unwrap();
    run.transition_to(AgentRunStatus::Running).unwrap();
    run.transition_to(AgentRunStatus::ReviewReady).unwrap();
    run.transition_to(AgentRunStatus::Completed).unwrap();
    assert_eq!(run.status, AgentRunStatus::Completed);
}

#[test]
fn invalid_agent_run_transition_is_rejected() {
    let project_id = ProjectId::for_test(1);
    let mut run = AgentRun::draft(
        project_id,
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Managed,
    );

    let error = run
        .transition_to(AgentRunStatus::Completed)
        .expect_err("draft cannot complete directly");

    assert_eq!(error.from, AgentRunStatus::Draft);
    assert_eq!(error.to, AgentRunStatus::Completed);
}

#[test]
fn detached_status_does_not_transition_back_to_managed_states() {
    let project_id = ProjectId::for_test(1);
    let mut run = AgentRun::draft(
        project_id,
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Supervised,
    );
    run.transition_to(AgentRunStatus::Ready).unwrap();
    run.transition_to(AgentRunStatus::Preparing).unwrap();
    run.transition_to(AgentRunStatus::Running).unwrap();
    run.transition_to(AgentRunStatus::Detached).unwrap();

    assert!(run.transition_to(AgentRunStatus::Running).is_err());
}

#[test]
fn agent_run_rejects_cross_project_terminal_attachment() {
    let mut run = AgentRun::draft(
        ProjectId::for_test(1),
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Supervised,
    );
    let terminal = TerminalSession::new(
        ProjectId::for_test(2),
        TerminalKind::Supervised,
        "Agent",
        "/workspace/other",
        "codex",
    );

    let error = run
        .attach_terminal(&terminal)
        .expect_err("cross-project terminal attachment must fail");

    assert_eq!(error, OwnershipError::CrossProject);
}

#[test]
fn agent_run_terminal_attachment_is_idempotent_for_same_terminal() {
    let mut run = AgentRun::draft(
        ProjectId::for_test(1),
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Supervised,
    );
    let terminal = TerminalSession::new(
        ProjectId::for_test(1),
        TerminalKind::Supervised,
        "Agent",
        "/workspace/project",
        "codex",
    );

    run.attach_terminal(&terminal).unwrap();
    run.attach_terminal(&terminal).unwrap();

    assert_eq!(run.terminal_id, Some(terminal.id));
}

#[test]
fn agent_run_rejects_replacing_terminal_attachment() {
    let mut run = AgentRun::draft(
        ProjectId::for_test(1),
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Supervised,
    );
    let first = TerminalSession::new(
        ProjectId::for_test(1),
        TerminalKind::Supervised,
        "Agent 1",
        "/workspace/project",
        "codex",
    );
    let second = TerminalSession::new(
        ProjectId::for_test(1),
        TerminalKind::Supervised,
        "Agent 2",
        "/workspace/project",
        "codex",
    );

    run.attach_terminal(&first).unwrap();
    let error = run
        .attach_terminal(&second)
        .expect_err("terminal replacement must be explicit");

    assert_eq!(error, OwnershipError::DuplicateAttachment);
    assert_eq!(run.terminal_id, Some(first.id));
}

#[test]
fn approval_decision_is_append_only_after_decided() {
    let mut approval = ApprovalRequest::pending(
        ProjectId::for_test(1),
        None,
        "command",
        "rm -rf target",
        RiskLevel::High,
        "/workspace/project",
    );

    approval.decide(ApprovalDecision::Rejected).unwrap();
    assert!(approval.decided_at.is_some());
    let error = approval
        .decide(ApprovalDecision::ApprovedOnce)
        .expect_err("approval cannot be re-decided");

    assert_eq!(error, ApprovalDecisionError::AlreadyDecided);
}

#[test]
fn duplicate_agent_run_approval_attachment_is_rejected() {
    let mut run = AgentRun::draft(
        ProjectId::for_test(1),
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Managed,
    );
    let approval = ApprovalRequest::pending(
        ProjectId::for_test(1),
        Some(run.id.clone()),
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/project",
    );

    run.add_approval(&approval).unwrap();
    let error = run
        .add_approval(&approval)
        .expect_err("duplicate approval attachment should fail");

    assert_eq!(error, OwnershipError::DuplicateAttachment);
}

#[test]
fn agent_run_approval_must_match_project_and_run() {
    let mut run = AgentRun::draft(
        ProjectId::for_test(1),
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Managed,
    );
    let wrong_project = ApprovalRequest::pending(
        ProjectId::for_test(2),
        Some(run.id.clone()),
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/other",
    );

    assert_eq!(
        run.add_approval(&wrong_project),
        Err(OwnershipError::CrossProject)
    );

    let wrong_run = ApprovalRequest::pending(
        ProjectId::for_test(1),
        None,
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/project",
    );

    assert_eq!(
        run.add_approval(&wrong_run),
        Err(OwnershipError::WrongAgentRun)
    );
}
