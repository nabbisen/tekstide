use crate::domain::{
    AgentCompatibilityLevel, AgentRun, ApprovalDecision, ApprovalRequest, AuditEvent,
    AuditEventClass, AuditEventError, OwnershipError, RiskLevel, TerminalKind, TerminalSession,
};
use crate::project::ProjectId;

#[test]
fn audit_event_classes_include_security_and_lifecycle_events() {
    let classes = [
        AuditEventClass::ProjectAdded,
        AuditEventClass::TrustGranted,
        AuditEventClass::TrustRevoked,
        AuditEventClass::TerminalStarted,
        AuditEventClass::AgentRunStarted,
        AuditEventClass::CommandApprovalRequested,
        AuditEventClass::CommandApproved,
        AuditEventClass::CommandRejected,
        AuditEventClass::PasteBlocked,
        AuditEventClass::ProcessTerminated,
        AuditEventClass::SafeCloseDecision,
        AuditEventClass::ConfigChanged,
        AuditEventClass::TranscriptPurged,
    ];

    assert_eq!(classes.len(), 13);
}

#[test]
fn audit_event_constructor_sets_timestamp_and_checks_project_links() {
    let project_id = ProjectId::for_test(1);
    let terminal = TerminalSession::new(
        project_id.clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project",
        "bash",
    );
    let audit_event = AuditEvent::new(
        Some(project_id.clone()),
        AuditEventClass::TerminalStarted,
        "terminal started",
    )
    .for_terminal(&terminal)
    .expect("same-project terminal link should work");

    assert_eq!(audit_event.project_id, Some(project_id));
    assert_eq!(audit_event.terminal_id, Some(terminal.id));
    assert!(audit_event.created_at.as_str().ends_with('Z'));
}

#[test]
fn projectless_audit_event_adopts_terminal_project_id() {
    let terminal = TerminalSession::new(
        ProjectId::for_test(7),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project",
        "bash",
    );

    let audit_event = AuditEvent::new(None, AuditEventClass::TerminalStarted, "terminal started")
        .for_terminal(&terminal)
        .expect("projectless audit event should adopt terminal project");

    assert_eq!(audit_event.project_id, Some(ProjectId::for_test(7)));
}

#[test]
fn projectless_audit_event_adopts_agent_run_project_id() {
    let run = AgentRun::draft(
        ProjectId::for_test(8),
        "codex",
        "implement feature",
        AgentCompatibilityLevel::Managed,
    );

    let audit_event = AuditEvent::new(None, AuditEventClass::AgentRunStarted, "agent started")
        .for_agent_run(&run)
        .expect("projectless audit event should adopt agent run project");

    assert_eq!(audit_event.project_id, Some(ProjectId::for_test(8)));
}

#[test]
fn projectless_audit_event_adopts_approval_project_id() {
    let approval = ApprovalRequest::pending(
        ProjectId::for_test(9),
        None,
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/project",
    );

    let audit_event = AuditEvent::new(
        None,
        AuditEventClass::CommandApprovalRequested,
        "approval requested",
    )
    .for_approval(&approval)
    .expect("projectless audit event should adopt approval project");

    assert_eq!(audit_event.project_id, Some(ProjectId::for_test(9)));
}

#[test]
fn global_audit_event_without_project_link_remains_global() {
    let audit_event = AuditEvent::new(
        None,
        AuditEventClass::ConfigChanged,
        "global config changed",
    );

    assert!(audit_event.project_id.is_none());
    assert!(audit_event.terminal_id.is_none());
}

#[test]
fn audit_event_rejects_cross_project_links() {
    let terminal = TerminalSession::new(
        ProjectId::for_test(2),
        TerminalKind::Plain,
        "Shell",
        "/workspace/other",
        "bash",
    );
    let error = AuditEvent::new(
        Some(ProjectId::for_test(1)),
        AuditEventClass::TerminalStarted,
        "terminal started",
    )
    .for_terminal(&terminal)
    .expect_err("cross-project terminal audit link should fail");

    assert_eq!(error, OwnershipError::CrossProject);
}

#[test]
fn security_audit_constructors_create_project_scoped_events() {
    let project_id = ProjectId::for_test(11);

    let granted = AuditEvent::trust_granted(project_id.clone(), "workspace trust granted");
    let revoked = AuditEvent::trust_revoked(project_id.clone(), "workspace trust revoked");
    let purged = AuditEvent::transcript_purged(project_id.clone(), "transcript purged");
    let close = AuditEvent::safe_close_decision(project_id.clone(), "close cancelled");

    assert_eq!(granted.project_id, Some(project_id.clone()));
    assert_eq!(granted.class, AuditEventClass::TrustGranted);
    assert_eq!(revoked.class, AuditEventClass::TrustRevoked);
    assert_eq!(purged.class, AuditEventClass::TranscriptPurged);
    assert_eq!(close.class, AuditEventClass::SafeCloseDecision);
}

#[test]
fn approval_audit_constructor_reflects_pending_and_decided_states() {
    let mut approval = ApprovalRequest::pending(
        ProjectId::for_test(12),
        None,
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/project",
    );

    let requested = AuditEvent::command_approval_requested(&approval, "approval requested")
        .expect("approval audit should link to approval project");

    assert_eq!(requested.class, AuditEventClass::CommandApprovalRequested);
    assert_eq!(requested.approval_id, Some(approval.id.clone()));
    assert_eq!(
        AuditEvent::command_approval_decided(&approval, "approval still pending"),
        Err(AuditEventError::ApprovalStillPending)
    );

    approval.decide(ApprovalDecision::Rejected).unwrap();
    let rejected = AuditEvent::command_approval_decided(&approval, "approval rejected")
        .expect("approval decision audit should link to approval project");

    assert_eq!(rejected.class, AuditEventClass::CommandRejected);
    assert_eq!(rejected.approval_id, Some(approval.id));
}

#[test]
fn approval_audit_decision_constructor_maps_approved_decisions() {
    let mut approved_once = ApprovalRequest::pending(
        ProjectId::for_test(13),
        None,
        "command",
        "cargo test",
        RiskLevel::Medium,
        "/workspace/project",
    );
    approved_once
        .decide(ApprovalDecision::ApprovedOnce)
        .unwrap();

    let approved_once_event =
        AuditEvent::command_approval_decided(&approved_once, "approval granted")
            .expect("approved decision should produce audit event");

    assert_eq!(approved_once_event.class, AuditEventClass::CommandApproved);

    let mut edited = ApprovalRequest::pending(
        ProjectId::for_test(14),
        None,
        "command",
        "cargo test --package tekstide",
        RiskLevel::Medium,
        "/workspace/project",
    );
    edited.decide(ApprovalDecision::EditedAndApproved).unwrap();

    let edited_event = AuditEvent::command_approval_decided(&edited, "edited approval granted")
        .expect("edited approval decision should produce audit event");

    assert_eq!(edited_event.class, AuditEventClass::CommandApproved);
}
