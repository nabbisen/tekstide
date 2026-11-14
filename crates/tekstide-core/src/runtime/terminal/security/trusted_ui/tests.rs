use crate::domain::TerminalKind;

use super::super::{TerminalSecurityParser, TerminalTextEffect};
use super::*;

#[test]
fn approval_like_terminal_output_remains_untrusted_terminal_content() {
    let parser = TerminalSecurityParser;
    let effects = parser.parse(b"APPROVE COMMAND? [y/N]\nTrust workspace?\n");
    let boundary = TerminalTrustedUiBoundary::new(TerminalTrustedSurfaceKind::ApprovalDialog);

    let assessment = boundary.assess_terminal_output(&effects);

    assert_eq!(
        assessment.content_class,
        TerminalOutputContentClass::UntrustedTerminalContent
    );
    assert_eq!(assessment.trusted_ui_effect, None);
    assert!(!assessment.can_mutate_trusted_ui());
    assert!(effects.iter().any(|effect| {
        matches!(
            effect,
            TerminalSurfaceEffect::Text(TerminalTextEffect::Printable { .. })
        )
    }));
}

#[test]
fn terminal_output_cannot_synthesize_trusted_dialog_decisions() {
    let effects = [
        TerminalSurfaceEffect::Text(TerminalTextEffect::Printable { chars: 24 }),
        TerminalSurfaceEffect::Text(TerminalTextEffect::Printable { chars: 18 }),
    ];

    for trusted_surface in [
        TerminalTrustedSurfaceKind::ApprovalDialog,
        TerminalTrustedSurfaceKind::TrustDialog,
        TerminalTrustedSurfaceKind::PasteConfirmationDialog,
        TerminalTrustedSurfaceKind::DestructiveDecisionDialog,
        TerminalTrustedSurfaceKind::SecurityDialog,
    ] {
        let boundary = TerminalTrustedUiBoundary::new(trusted_surface);
        let assessment = boundary.assess_terminal_output(&effects);

        assert_eq!(assessment.trusted_ui_effect, None);
        assert_eq!(assessment.terminal_effects_seen, effects.len());
    }
}

#[test]
fn paste_security_and_destructive_spoof_text_remains_terminal_content() {
    let parser = TerminalSecurityParser;
    let effects =
        parser.parse(b"Paste 42 lines?\nSECURITY WARNING\nDelete workspace? [Approve] [Reject]\n");

    for trusted_surface in [
        TerminalTrustedSurfaceKind::PasteConfirmationDialog,
        TerminalTrustedSurfaceKind::DestructiveDecisionDialog,
        TerminalTrustedSurfaceKind::SecurityDialog,
    ] {
        let boundary = TerminalTrustedUiBoundary::new(trusted_surface);
        let assessment = boundary.assess_terminal_output(&effects);

        assert_eq!(
            assessment.content_class,
            TerminalOutputContentClass::UntrustedTerminalContent
        );
        assert_eq!(assessment.trusted_ui_effect, None);
        assert!(!assessment.can_mutate_trusted_ui());
    }
}

#[test]
fn plain_and_supervised_labels_do_not_claim_managed_command_approval() {
    let plain =
        TerminalSecurityLabelView::from_level(AiSessionSecurityLevel::from(TerminalKind::Plain));
    let supervised = TerminalSecurityLabelView::from_level(AiSessionSecurityLevel::from(
        TerminalKind::Supervised,
    ));
    let managed =
        TerminalSecurityLabelView::from_level(AiSessionSecurityLevel::from(TerminalKind::Managed));

    assert_eq!(plain.surface_label, "Plain Terminal");
    assert_eq!(
        plain.command_approval_claim,
        "Managed command approval not guaranteed"
    );
    assert!(!plain.managed_command_approval_claimed);

    assert_eq!(supervised.surface_label, "Supervised AgentRun");
    assert_eq!(
        supervised.command_approval_claim,
        "Managed command approval not guaranteed"
    );
    assert!(!supervised.managed_command_approval_claimed);

    assert_eq!(managed.surface_label, "Managed AgentRun");
    assert_eq!(
        managed.command_approval_claim,
        "Managed command approval eligible"
    );
    assert!(managed.managed_command_approval_claimed);
}
