mod launch;
mod pty;
mod reader;
mod security;
mod termination;
#[cfg(test)]
mod tests;
mod types;

pub use launch::{LinuxTerminalRuntime, TerminalLaunchError, TerminalRuntimeError};
pub use reader::{TerminalReader, TerminalReaderDrain, WakeNotifier};
pub(crate) use security::next_token_len;
pub use security::{
    TerminalAcceptedSequence, TerminalBlockedAppEffect, TerminalCursorEffect,
    TerminalInertSequence, TerminalInputDecision, TerminalInputDecisionReason, TerminalInputPolicy,
    TerminalInputSource, TerminalModeEffect, TerminalOutputContentClass, TerminalPasteClass,
    TerminalPolicyReason, TerminalScrollbackEffect, TerminalSecurityDiagnostic,
    TerminalSecurityLabelView, TerminalSecurityParser, TerminalSequenceFamily,
    TerminalSequencePolicy, TerminalSpoofingAssessment, TerminalStyleEffect, TerminalSurfaceEffect,
    TerminalTextEffect, TerminalTrustedSurfaceKind, TerminalTrustedUiBoundary,
    TerminalTrustedUiEffect, TerminalTrustedUiState, classify_private_mode_number,
};
/// RFC-022 PR-022-C: `agent::launch` builds an `AdapterApprovalConfig` to
/// attach to a `TerminalLaunchSpec` via `set_adapter_approval_config` when
/// preparing a Managed adapter launch. `pub(crate)`, not `pub` -- the type
/// itself is already `pub(crate)`; this only makes the path to it
/// reachable from outside `runtime::terminal`, not from outside the crate.
pub(crate) use types::AdapterApprovalConfig;
pub use types::{
    BoundedRuntimeSummary, TerminalDimensions, TerminalEnvironmentPolicy, TerminalLaunchSpec,
    TerminalOutputSummary, TerminalRuntimeEvent, TerminalRuntimeHandle, TerminalRuntimeSnapshot,
    TerminationOutcome, TerminationRequest, TerminationRequestSource, TerminationSignal,
};
