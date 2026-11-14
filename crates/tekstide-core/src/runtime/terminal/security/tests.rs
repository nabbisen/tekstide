use super::*;

#[test]
fn diagnostic_keeps_review_metadata_without_payload_text() {
    let diagnostic = TerminalSecurityDiagnostic::blocked_sequence(
        TerminalSequenceFamily::Osc52Clipboard,
        TerminalPolicyReason::ClipboardAccessBlocked,
        4096,
    );

    assert_eq!(
        diagnostic.sequence_family,
        TerminalSequenceFamily::Osc52Clipboard
    );
    assert_eq!(
        diagnostic.policy_reason,
        TerminalPolicyReason::ClipboardAccessBlocked
    );
    assert_eq!(diagnostic.payload_bytes, 4096);
    assert_eq!(
        diagnostic.summary().as_str(),
        "OSC 52 clipboard blocked: clipboard access blocked"
    );
}

#[test]
fn diagnostic_summary_is_bounded() {
    let diagnostic = TerminalSecurityDiagnostic::with_summary(
        TerminalSequenceFamily::UnknownControl,
        TerminalPolicyReason::UnsupportedSequence,
        1,
        "x".repeat(BoundedRuntimeSummary::MAX_CHARS + 1),
    );

    assert_eq!(
        diagnostic.summary().as_str().chars().count(),
        BoundedRuntimeSummary::MAX_CHARS
    );
    assert!(diagnostic.summary().was_truncated());
}

#[test]
fn public_blocked_sequence_constructor_does_not_store_raw_payload() {
    let raw_payload = "SECRET_TOKEN=must-not-appear";
    let diagnostic = TerminalSecurityDiagnostic::blocked_sequence(
        TerminalSequenceFamily::OscTitle,
        TerminalPolicyReason::AppChromeMutationBlocked,
        raw_payload.len(),
    );

    assert!(!diagnostic.summary().as_str().contains(raw_payload));
    assert_eq!(diagnostic.payload_bytes, raw_payload.len());
}

#[test]
fn public_blocked_app_effect_constructor_does_not_store_raw_payload() {
    let raw_payload = "APPROVAL_SECRET=must-not-appear";
    let diagnostic = TerminalSecurityDiagnostic::blocked_app_effect(
        TerminalSequenceFamily::TerminalGeneratedReply,
        TerminalBlockedAppEffect::TerminalGeneratedReply,
        raw_payload.len(),
    );

    assert_eq!(
        diagnostic.policy_reason,
        TerminalPolicyReason::TerminalGeneratedReplyBlocked
    );
    assert_eq!(diagnostic.payload_bytes, raw_payload.len());
    assert!(!diagnostic.summary().as_str().contains(raw_payload));
    assert!(!diagnostic.summary().as_str().contains("project"));
    assert!(!diagnostic.summary().as_str().contains("approval"));
}

#[test]
fn blocked_app_effect_vocabulary_includes_trusted_state_mutations() {
    let blocked_effects = [
        TerminalBlockedAppEffect::ClipboardAccess,
        TerminalBlockedAppEffect::AppChromeMutation,
        TerminalBlockedAppEffect::TrustedUiMutation,
        TerminalBlockedAppEffect::TrustStateMutation,
        TerminalBlockedAppEffect::ApprovalStateMutation,
        TerminalBlockedAppEffect::CommandHistoryMutation,
        TerminalBlockedAppEffect::AuditStateMutation,
        TerminalBlockedAppEffect::FileStateMutation,
        TerminalBlockedAppEffect::ProjectMetadataMutation,
        TerminalBlockedAppEffect::HostIntegration,
        TerminalBlockedAppEffect::TerminalGeneratedReply,
    ];

    assert_eq!(blocked_effects.len(), 11);
}

#[test]
fn surface_effect_model_is_terminal_local() {
    let effects = [
        TerminalSurfaceEffect::Text(TerminalTextEffect::Printable { chars: 12 }),
        TerminalSurfaceEffect::Cursor(TerminalCursorEffect::CarriageReturn),
        TerminalSurfaceEffect::Style(TerminalStyleEffect::SelectGraphicRendition),
        TerminalSurfaceEffect::Mode(TerminalModeEffect::BracketedPasteModeChanged {
            enabled: true,
        }),
        TerminalSurfaceEffect::Scrollback(TerminalScrollbackEffect::ClearScreen),
        TerminalSurfaceEffect::diagnostic(TerminalSecurityDiagnostic::blocked_sequence(
            TerminalSequenceFamily::TerminalGeneratedReply,
            TerminalPolicyReason::TerminalGeneratedReplyBlocked,
            8,
        )),
    ];

    assert_eq!(effects.len(), 6);
}
