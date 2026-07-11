use super::BoundedRuntimeSummary;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalSurfaceEffect {
    Text(TerminalTextEffect),
    Cursor(TerminalCursorEffect),
    Style(TerminalStyleEffect),
    Mode(TerminalModeEffect),
    Scrollback(TerminalScrollbackEffect),
    Diagnostic(TerminalSecurityDiagnostic),
}

impl TerminalSurfaceEffect {
    pub fn diagnostic(diagnostic: TerminalSecurityDiagnostic) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalBlockedAppEffect {
    ClipboardAccess,
    AppChromeMutation,
    TrustedUiMutation,
    TrustStateMutation,
    ApprovalStateMutation,
    CommandHistoryMutation,
    AuditStateMutation,
    FileStateMutation,
    ProjectMetadataMutation,
    HostIntegration,
    TerminalGeneratedReply,
}

impl TerminalBlockedAppEffect {
    pub fn policy_reason(self) -> TerminalPolicyReason {
        match self {
            Self::ClipboardAccess => TerminalPolicyReason::ClipboardAccessBlocked,
            Self::AppChromeMutation | Self::TrustedUiMutation => {
                TerminalPolicyReason::AppChromeMutationBlocked
            }
            Self::TrustStateMutation
            | Self::ApprovalStateMutation
            | Self::CommandHistoryMutation
            | Self::AuditStateMutation
            | Self::FileStateMutation
            | Self::ProjectMetadataMutation
            | Self::HostIntegration => TerminalPolicyReason::HostIntegrationBlocked,
            Self::TerminalGeneratedReply => TerminalPolicyReason::TerminalGeneratedReplyBlocked,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalTextEffect {
    Printable { chars: usize },
    InvalidBytesReplaced { bytes: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalCursorEffect {
    Move { rows: i16, cols: i16 },
    CarriageReturn,
    LineFeed,
    Backspace,
    Tab,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStyleEffect {
    Reset,
    SelectGraphicRendition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalModeEffect {
    AlternateScreenEntered,
    AlternateScreenExited,
    BracketedPasteModeChanged { enabled: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalScrollbackEffect {
    ClearLine,
    ClearScreen,
    BoundedScroll { lines: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSecurityDiagnostic {
    pub sequence_family: TerminalSequenceFamily,
    pub policy_reason: TerminalPolicyReason,
    pub payload_bytes: usize,
    summary: BoundedRuntimeSummary,
}

impl TerminalSecurityDiagnostic {
    pub fn blocked_sequence(
        sequence_family: TerminalSequenceFamily,
        policy_reason: TerminalPolicyReason,
        payload_bytes: usize,
    ) -> Self {
        Self::with_summary(
            sequence_family,
            policy_reason,
            payload_bytes,
            format!("{sequence_family} blocked: {policy_reason}"),
        )
    }

    pub fn blocked_app_effect(
        sequence_family: TerminalSequenceFamily,
        blocked_effect: TerminalBlockedAppEffect,
        payload_bytes: usize,
    ) -> Self {
        Self::blocked_sequence(
            sequence_family,
            blocked_effect.policy_reason(),
            payload_bytes,
        )
    }

    fn with_summary(
        sequence_family: TerminalSequenceFamily,
        policy_reason: TerminalPolicyReason,
        payload_bytes: usize,
        summary: impl AsRef<str>,
    ) -> Self {
        Self {
            sequence_family,
            policy_reason,
            payload_bytes,
            summary: BoundedRuntimeSummary::new(summary),
        }
    }

    pub fn summary(&self) -> &BoundedRuntimeSummary {
        &self.summary
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalSequenceFamily {
    PrintableText,
    C0Control,
    C1Control,
    Csi,
    Sgr,
    Osc52Clipboard,
    Osc8Hyperlink,
    OscTitle,
    Dcs,
    Pm,
    Apc,
    PrivateMode,
    MouseFocusReporting,
    KeyboardProtocol,
    TerminalQuery,
    TerminalGeneratedReply,
    UnknownControl,
}

impl std::fmt::Display for TerminalSequenceFamily {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::PrintableText => "printable text",
            Self::C0Control => "C0 control",
            Self::C1Control => "C1 control",
            Self::Csi => "CSI",
            Self::Sgr => "SGR",
            Self::Osc52Clipboard => "OSC 52 clipboard",
            Self::Osc8Hyperlink => "OSC 8 hyperlink",
            Self::OscTitle => "OSC title",
            Self::Dcs => "DCS",
            Self::Pm => "PM",
            Self::Apc => "APC",
            Self::PrivateMode => "private mode",
            Self::MouseFocusReporting => "mouse/focus reporting",
            Self::KeyboardProtocol => "keyboard protocol",
            Self::TerminalQuery => "terminal query",
            Self::TerminalGeneratedReply => "terminal-generated reply",
            Self::UnknownControl => "unknown control",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalPolicyReason {
    SupportedTerminalLocalEffect,
    UnsupportedSequence,
    ClipboardAccessBlocked,
    AppChromeMutationBlocked,
    HostIntegrationBlocked,
    TerminalGeneratedReplyBlocked,
    PrivateModeBlocked,
    InvalidBytes,
}

impl std::fmt::Display for TerminalPolicyReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::SupportedTerminalLocalEffect => "supported terminal-local effect",
            Self::UnsupportedSequence => "unsupported sequence",
            Self::ClipboardAccessBlocked => "clipboard access blocked",
            Self::AppChromeMutationBlocked => "app chrome mutation blocked",
            Self::HostIntegrationBlocked => "host integration blocked",
            Self::TerminalGeneratedReplyBlocked => "terminal-generated reply blocked",
            Self::PrivateModeBlocked => "private mode blocked",
            Self::InvalidBytes => "invalid bytes",
        })
    }
}

#[cfg(test)]
mod tests {
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
}
