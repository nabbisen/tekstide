use crate::runtime::terminal::TerminalRuntimeHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalInputSource {
    Typed,
    Paste,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalPasteClass {
    Empty,
    SingleLine,
    Multiline,
    ControlContaining,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalTrustedUiState {
    Inactive,
    ApprovalActive,
    PasteConfirmationActive,
    DestructiveDecisionActive,
    SecurityDialogActive,
}

impl TerminalTrustedUiState {
    pub fn is_active_or_modal(self) -> bool {
        !matches!(self, Self::Inactive)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalInputDecision {
    Allow {
        source: TerminalInputSource,
        paste_class: Option<TerminalPasteClass>,
        byte_count: usize,
    },
    RequiresConfirmation {
        paste_class: TerminalPasteClass,
        byte_count: usize,
        reason: TerminalInputDecisionReason,
    },
    Block {
        source: TerminalInputSource,
        paste_class: Option<TerminalPasteClass>,
        byte_count: usize,
        reason: TerminalInputDecisionReason,
    },
}

impl TerminalInputDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalInputDecisionReason {
    WrongProject,
    WrongTerminal,
    MultilinePasteRequiresConfirmation,
    ControlContainingPasteBlocked,
    PasteBlockedByTrustedUi(TerminalTrustedUiState),
}

#[derive(Default)]
pub struct TerminalInputPolicy;

impl TerminalInputPolicy {
    pub fn evaluate(
        &self,
        target: &TerminalRuntimeHandle,
        active_terminal: Option<&TerminalRuntimeHandle>,
        source: TerminalInputSource,
        bytes: &[u8],
        trusted_ui: TerminalTrustedUiState,
    ) -> TerminalInputDecision {
        if let Some(active_terminal) = active_terminal {
            if active_terminal.project_id != target.project_id {
                return TerminalInputDecision::Block {
                    source,
                    paste_class: paste_class_for_source(source, bytes),
                    byte_count: bytes.len(),
                    reason: TerminalInputDecisionReason::WrongProject,
                };
            }
            if active_terminal.terminal_id != target.terminal_id {
                return TerminalInputDecision::Block {
                    source,
                    paste_class: paste_class_for_source(source, bytes),
                    byte_count: bytes.len(),
                    reason: TerminalInputDecisionReason::WrongTerminal,
                };
            }
        }

        match source {
            TerminalInputSource::Typed => TerminalInputDecision::Allow {
                source,
                paste_class: None,
                byte_count: bytes.len(),
            },
            TerminalInputSource::Paste => {
                let paste_class = classify_paste(bytes);

                if trusted_ui.is_active_or_modal() {
                    return TerminalInputDecision::Block {
                        source,
                        paste_class: Some(paste_class),
                        byte_count: bytes.len(),
                        reason: TerminalInputDecisionReason::PasteBlockedByTrustedUi(trusted_ui),
                    };
                }

                match paste_class {
                    TerminalPasteClass::Empty | TerminalPasteClass::SingleLine => {
                        TerminalInputDecision::Allow {
                            source,
                            paste_class: Some(paste_class),
                            byte_count: bytes.len(),
                        }
                    }
                    TerminalPasteClass::Multiline => TerminalInputDecision::RequiresConfirmation {
                        paste_class,
                        byte_count: bytes.len(),
                        reason: TerminalInputDecisionReason::MultilinePasteRequiresConfirmation,
                    },
                    TerminalPasteClass::ControlContaining => TerminalInputDecision::Block {
                        source,
                        paste_class: Some(paste_class),
                        byte_count: bytes.len(),
                        reason: TerminalInputDecisionReason::ControlContainingPasteBlocked,
                    },
                }
            }
        }
    }
}

fn paste_class_for_source(source: TerminalInputSource, bytes: &[u8]) -> Option<TerminalPasteClass> {
    match source {
        TerminalInputSource::Typed => None,
        TerminalInputSource::Paste => Some(classify_paste(bytes)),
    }
}

fn classify_paste(bytes: &[u8]) -> TerminalPasteClass {
    if bytes.is_empty() {
        return TerminalPasteClass::Empty;
    }

    if bytes
        .iter()
        .any(|byte| matches!(*byte, 0x00..=0x08 | 0x0b..=0x0c | 0x0e..=0x1f | 0x7f..=0x9f))
    {
        return TerminalPasteClass::ControlContaining;
    }

    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        TerminalPasteClass::Multiline
    } else {
        TerminalPasteClass::SingleLine
    }
}

#[cfg(test)]
mod tests;
