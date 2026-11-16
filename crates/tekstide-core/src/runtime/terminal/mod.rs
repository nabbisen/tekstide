mod launch;
mod pty;
mod reader;
mod security;
mod termination;
#[cfg(test)]
pub(crate) mod tests;
mod types;

pub use launch::{TerminalLaunchError, TerminalRuntime, TerminalRuntimeError};
#[cfg(test)]
pub(crate) use launch::{
    arm_pty_responsiveness_probe_for_test, process_is_alive, wait_for_exit_draining,
    wait_for_shell_ready,
};
// `list_all_pids` only exists under this same `cfg` in `launch.rs` itself
// (review 596's own macOS libproc port) -- a separate re-export line,
// not folded into the one above, because that one is platform-neutral
// and this one cannot be.
#[cfg(test)]
#[cfg(target_os = "macos")]
pub(crate) use launch::list_all_pids;
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
/// RFC-077 Stage 8: `project::session` builds an `AdapterApprovalConfig`
/// to attach to a `TerminalLaunchSpec` via `set_adapter_approval_config`
/// when assembling a Managed adapter launch from `agent`'s own
/// runtime-free intent. `pub(crate)`, not `pub` -- the type itself is
/// already `pub(crate)`; this only makes the path to it reachable from
/// outside `runtime::terminal`, not from outside the crate.
pub(crate) use types::AdapterApprovalConfig;
/// RFC-077 Stage 8: the single mapping from an agent run's compatibility
/// level to the terminal kind it launches as -- `project::session`'s own
/// assembly step needs this exact function, not a second copy.
pub(crate) use types::terminal_kind_from_compatibility;
pub use types::{
    BoundedRuntimeSummary, TerminalDimensions, TerminalEnvironmentPolicy, TerminalLaunchSpec,
    TerminalOutputSummary, TerminalRuntimeEvent, TerminalRuntimeHandle, TerminalRuntimeSnapshot,
    TerminationOutcome, TerminationRequest, TerminationRequestSource, TerminationSignal,
};
