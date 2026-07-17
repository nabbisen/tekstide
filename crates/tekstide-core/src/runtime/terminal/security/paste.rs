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
mod tests {
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
}
