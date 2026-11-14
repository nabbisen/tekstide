use super::*;
use crate::domain::TerminalId;
use crate::project::ProjectId;

#[test]
fn typed_input_is_allowed_without_paste_classification() {
    let handle = handle(1, 1);
    let decision = TerminalInputPolicy.evaluate(
        &handle,
        Some(&handle),
        TerminalInputSource::Typed,
        b"echo ok\n",
        TerminalTrustedUiState::Inactive,
    );

    assert_eq!(
        decision,
        TerminalInputDecision::Allow {
            source: TerminalInputSource::Typed,
            paste_class: None,
            byte_count: 8,
        }
    );
}

#[test]
fn single_line_paste_is_allowed_without_returning_payload() {
    let handle = handle(1, 1);
    let raw_paste = b"SECRET_TOKEN=must-not-be-stored";
    let decision = TerminalInputPolicy.evaluate(
        &handle,
        Some(&handle),
        TerminalInputSource::Paste,
        raw_paste,
        TerminalTrustedUiState::Inactive,
    );

    assert_eq!(
        decision,
        TerminalInputDecision::Allow {
            source: TerminalInputSource::Paste,
            paste_class: Some(TerminalPasteClass::SingleLine),
            byte_count: raw_paste.len(),
        }
    );
    assert!(!format!("{decision:?}").contains("SECRET_TOKEN"));
}

#[test]
fn multiline_paste_requires_confirmation_before_pty_write() {
    let handle = handle(1, 1);
    let decision = TerminalInputPolicy.evaluate(
        &handle,
        Some(&handle),
        TerminalInputSource::Paste,
        b"first\nsecond",
        TerminalTrustedUiState::Inactive,
    );

    assert_eq!(
        decision,
        TerminalInputDecision::RequiresConfirmation {
            paste_class: TerminalPasteClass::Multiline,
            byte_count: 12,
            reason: TerminalInputDecisionReason::MultilinePasteRequiresConfirmation,
        }
    );
}

#[test]
fn control_containing_paste_is_blocked() {
    let handle = handle(1, 1);
    let decision = TerminalInputPolicy.evaluate(
        &handle,
        Some(&handle),
        TerminalInputSource::Paste,
        b"echo\x1b[31m",
        TerminalTrustedUiState::Inactive,
    );

    assert_eq!(
        decision,
        TerminalInputDecision::Block {
            source: TerminalInputSource::Paste,
            paste_class: Some(TerminalPasteClass::ControlContaining),
            byte_count: 9,
            reason: TerminalInputDecisionReason::ControlContainingPasteBlocked,
        }
    );
}

#[test]
fn c1_control_containing_paste_is_blocked() {
    let handle = handle(1, 1);

    for bytes in [
        b"\x90private".as_slice(),
        b"\x9b31m".as_slice(),
        b"\x9d52;c;SECRET".as_slice(),
    ] {
        let decision = TerminalInputPolicy.evaluate(
            &handle,
            Some(&handle),
            TerminalInputSource::Paste,
            bytes,
            TerminalTrustedUiState::Inactive,
        );

        assert_eq!(
            decision,
            TerminalInputDecision::Block {
                source: TerminalInputSource::Paste,
                paste_class: Some(TerminalPasteClass::ControlContaining),
                byte_count: bytes.len(),
                reason: TerminalInputDecisionReason::ControlContainingPasteBlocked,
            }
        );
        assert!(!format!("{decision:?}").contains("SECRET"));
    }
}

#[test]
fn non_control_binary_paste_is_single_line_by_current_policy() {
    let handle = handle(1, 1);
    let bytes = &[0xc3, 0x28];
    let decision = TerminalInputPolicy.evaluate(
        &handle,
        Some(&handle),
        TerminalInputSource::Paste,
        bytes,
        TerminalTrustedUiState::Inactive,
    );

    assert_eq!(
        decision,
        TerminalInputDecision::Allow {
            source: TerminalInputSource::Paste,
            paste_class: Some(TerminalPasteClass::SingleLine),
            byte_count: bytes.len(),
        }
    );
}

#[test]
fn paste_is_blocked_while_trusted_ui_is_active_or_modal() {
    let handle = handle(1, 1);
    for trusted_ui in [
        TerminalTrustedUiState::ApprovalActive,
        TerminalTrustedUiState::PasteConfirmationActive,
        TerminalTrustedUiState::DestructiveDecisionActive,
        TerminalTrustedUiState::SecurityDialogActive,
    ] {
        let decision = TerminalInputPolicy.evaluate(
            &handle,
            Some(&handle),
            TerminalInputSource::Paste,
            b"safe-looking",
            trusted_ui,
        );

        assert_eq!(
            decision,
            TerminalInputDecision::Block {
                source: TerminalInputSource::Paste,
                paste_class: Some(TerminalPasteClass::SingleLine),
                byte_count: 12,
                reason: TerminalInputDecisionReason::PasteBlockedByTrustedUi(trusted_ui),
            }
        );
    }
}

#[test]
fn cross_project_paste_routing_is_rejected() {
    let target = handle(1, 1);
    let active = handle(2, 1);
    let decision = TerminalInputPolicy.evaluate(
        &target,
        Some(&active),
        TerminalInputSource::Paste,
        b"safe-looking",
        TerminalTrustedUiState::Inactive,
    );

    assert_eq!(
        decision,
        TerminalInputDecision::Block {
            source: TerminalInputSource::Paste,
            paste_class: Some(TerminalPasteClass::SingleLine),
            byte_count: 12,
            reason: TerminalInputDecisionReason::WrongProject,
        }
    );
}

#[test]
fn wrong_terminal_paste_routing_is_rejected() {
    let target = handle(1, 1);
    let active = handle(1, 2);
    let decision = TerminalInputPolicy.evaluate(
        &target,
        Some(&active),
        TerminalInputSource::Paste,
        b"safe-looking",
        TerminalTrustedUiState::Inactive,
    );

    assert_eq!(
        decision,
        TerminalInputDecision::Block {
            source: TerminalInputSource::Paste,
            paste_class: Some(TerminalPasteClass::SingleLine),
            byte_count: 12,
            reason: TerminalInputDecisionReason::WrongTerminal,
        }
    );
}

fn handle(project: u64, terminal: u64) -> TerminalRuntimeHandle {
    TerminalRuntimeHandle::new(TerminalId::for_test(terminal), ProjectId::for_test(project))
}
