use crate::security::AiSessionSecurityLevel;

use super::TerminalSurfaceEffect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalTrustedSurfaceKind {
    ApprovalDialog,
    TrustDialog,
    PasteConfirmationDialog,
    DestructiveDecisionDialog,
    SecurityDialog,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalTrustedUiEffect {
    MoveFocus,
    Dismiss,
    Approve,
    Reject,
    TrustWorkspace,
    MutateProjectBoardState,
    MutateTrustedChrome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSpoofingAssessment {
    pub content_class: TerminalOutputContentClass,
    pub trusted_ui_effect: Option<TerminalTrustedUiEffect>,
    pub terminal_effects_seen: usize,
}

impl TerminalSpoofingAssessment {
    pub fn terminal_content(effects: &[TerminalSurfaceEffect]) -> Self {
        Self {
            content_class: TerminalOutputContentClass::UntrustedTerminalContent,
            trusted_ui_effect: None,
            terminal_effects_seen: effects.len(),
        }
    }

    pub fn can_mutate_trusted_ui(&self) -> bool {
        self.trusted_ui_effect.is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalOutputContentClass {
    UntrustedTerminalContent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTrustedUiBoundary {
    pub trusted_surface: TerminalTrustedSurfaceKind,
}

impl TerminalTrustedUiBoundary {
    pub fn new(trusted_surface: TerminalTrustedSurfaceKind) -> Self {
        Self { trusted_surface }
    }

    pub fn assess_terminal_output(
        &self,
        effects: &[TerminalSurfaceEffect],
    ) -> TerminalSpoofingAssessment {
        let _trusted_surface = self.trusted_surface;
        TerminalSpoofingAssessment::terminal_content(effects)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSecurityLabelView {
    pub level: AiSessionSecurityLevel,
    pub surface_label: &'static str,
    pub command_approval_claim: &'static str,
    pub managed_command_approval_claimed: bool,
}

impl TerminalSecurityLabelView {
    pub fn from_level(level: AiSessionSecurityLevel) -> Self {
        Self {
            level,
            surface_label: level.surface_label(),
            command_approval_claim: level.command_approval_claim_label(),
            managed_command_approval_claimed: level.can_claim_managed_command_approval(),
        }
    }
}

#[cfg(test)]
mod tests {
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
        let effects = parser
            .parse(b"Paste 42 lines?\nSECURITY WARNING\nDelete workspace? [Approve] [Reject]\n");

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
        let plain = TerminalSecurityLabelView::from_level(AiSessionSecurityLevel::from(
            TerminalKind::Plain,
        ));
        let supervised = TerminalSecurityLabelView::from_level(AiSessionSecurityLevel::from(
            TerminalKind::Supervised,
        ));
        let managed = TerminalSecurityLabelView::from_level(AiSessionSecurityLevel::from(
            TerminalKind::Managed,
        ));

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
}
