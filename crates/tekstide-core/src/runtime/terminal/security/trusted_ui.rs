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
mod tests;
