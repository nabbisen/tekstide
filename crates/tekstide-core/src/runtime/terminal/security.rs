use super::BoundedRuntimeSummary;

mod parser;
mod paste;
mod trusted_ui;

/// RFC-011 Amendment 1: `transcript::reader`'s resynchronization
/// primitive. Crate-visible only, matching `next_token_len` itself --
/// this module's own boundary logic stays the one place it is defined,
/// reused rather than duplicated, without becoming part of this crate's
/// external API.
pub(crate) use parser::next_token_len;
pub use parser::{
    TerminalAcceptedSequence, TerminalInertSequence, TerminalSecurityParser,
    TerminalSequencePolicy, classify_private_mode_number,
};
pub use paste::{
    TerminalInputDecision, TerminalInputDecisionReason, TerminalInputPolicy, TerminalInputSource,
    TerminalPasteClass, TerminalTrustedUiState,
};
pub use trusted_ui::{
    TerminalOutputContentClass, TerminalSecurityLabelView, TerminalSpoofingAssessment,
    TerminalTrustedSurfaceKind, TerminalTrustedUiBoundary, TerminalTrustedUiEffect,
};

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
mod tests;
