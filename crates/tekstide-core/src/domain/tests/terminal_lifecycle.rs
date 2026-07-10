use crate::domain::{
    TerminalKind, TerminalSession, TerminalStatus, TerminalTransitionError, VisibleSlot,
};
use crate::project::ProjectId;

#[test]
fn terminal_session_uses_canonical_lifecycle_transitions() {
    let mut terminal = terminal_session();

    assert_eq!(terminal.status(), TerminalStatus::Starting);
    terminal.transition_to(TerminalStatus::Running).unwrap();
    terminal.transition_to(TerminalStatus::Terminating).unwrap();
    terminal.transition_to(TerminalStatus::Exited).unwrap();

    assert_eq!(terminal.status(), TerminalStatus::Exited);
}

#[test]
fn terminal_session_rejects_invalid_lifecycle_transition() {
    let mut terminal = terminal_session();

    let error = terminal
        .transition_to(TerminalStatus::Exited)
        .expect_err("starting terminal cannot exit before it is observed running");

    assert_eq!(
        error,
        TerminalTransitionError {
            from: TerminalStatus::Starting,
            to: TerminalStatus::Exited,
        }
    );
    assert_eq!(terminal.status(), TerminalStatus::Starting);
}

#[test]
fn orphaned_terminal_can_only_recover_to_exited_in_current_model() {
    let mut terminal = terminal_session();

    terminal.transition_to(TerminalStatus::Running).unwrap();
    terminal
        .transition_to(TerminalStatus::OrphanedUnknown)
        .unwrap();

    assert!(terminal.transition_to(TerminalStatus::Running).is_err());
    terminal.transition_to(TerminalStatus::Exited).unwrap();
    assert_eq!(terminal.status(), TerminalStatus::Exited);
}

#[test]
fn terminal_visible_slot_is_explicit_runtime_summary_state() {
    let mut terminal = terminal_session();

    assert_eq!(terminal.visible_slot(), VisibleSlot::Hidden);
    terminal.assign_visible_slot(VisibleSlot::Primary);
    assert_eq!(terminal.visible_slot(), VisibleSlot::Primary);
}

fn terminal_session() -> TerminalSession {
    TerminalSession::new(
        ProjectId::for_test(1),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project",
        "bash",
    )
}
