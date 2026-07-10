use std::path::PathBuf;

use crate::domain::{TerminalId, TerminalKind, TerminalStatus, VisibleSlot};
use crate::project::ProjectId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLaunchSpec {
    pub project_id: ProjectId,
    pub title: String,
    pub cwd: PathBuf,
    pub shell: PathBuf,
    pub command_line_summary: String,
    pub environment_policy: TerminalEnvironmentPolicy,
    pub kind: TerminalKind,
    pub dimensions: TerminalDimensions,
}

impl TerminalLaunchSpec {
    pub fn plain_shell(
        project_id: ProjectId,
        title: impl Into<String>,
        cwd: impl Into<PathBuf>,
        shell: impl Into<PathBuf>,
    ) -> Self {
        let shell = shell.into();
        Self {
            project_id,
            title: title.into(),
            cwd: cwd.into(),
            command_line_summary: shell.display().to_string(),
            shell,
            environment_policy: TerminalEnvironmentPolicy::Minimal,
            kind: TerminalKind::Plain,
            dimensions: TerminalDimensions::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEnvironmentPolicy {
    Minimal,
    Named(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalDimensions {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalDimensions {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalRuntimeHandle {
    pub terminal_id: TerminalId,
    pub project_id: ProjectId,
}

impl TerminalRuntimeHandle {
    pub fn new(terminal_id: TerminalId, project_id: ProjectId) -> Self {
        Self {
            terminal_id,
            project_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalRuntimeSnapshot {
    pub handle: TerminalRuntimeHandle,
    pub status: TerminalStatus,
    pub visible_slot: VisibleSlot,
    pub dimensions: TerminalDimensions,
    pub buffered_output: TerminalOutputSummary,
    pub termination: Option<TerminationOutcome>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalOutputSummary {
    pub buffered_bytes: usize,
    pub dropped_bytes: usize,
    pub truncated: bool,
}

impl TerminalOutputSummary {
    pub fn new(buffered_bytes: usize, dropped_bytes: usize) -> Self {
        Self {
            buffered_bytes,
            dropped_bytes,
            truncated: dropped_bytes > 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalRuntimeEvent {
    LaunchAccepted {
        handle: TerminalRuntimeHandle,
    },
    ProcessStarted {
        handle: TerminalRuntimeHandle,
    },
    OutputBuffered {
        handle: TerminalRuntimeHandle,
        summary: TerminalOutputSummary,
    },
    InputWritten {
        handle: TerminalRuntimeHandle,
        bytes: usize,
    },
    Resized {
        handle: TerminalRuntimeHandle,
        dimensions: TerminalDimensions,
    },
    TerminationRequested {
        handle: TerminalRuntimeHandle,
        request: TerminationRequest,
    },
    TerminationSignalSent {
        handle: TerminalRuntimeHandle,
        signal: TerminationSignal,
    },
    TerminationTimedOut {
        handle: TerminalRuntimeHandle,
        after_signal: TerminationSignal,
    },
    Terminated {
        handle: TerminalRuntimeHandle,
        outcome: TerminationOutcome,
    },
    Failed {
        handle: TerminalRuntimeHandle,
        error: BoundedRuntimeSummary,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminationRequest {
    pub source: TerminationRequestSource,
    pub reason: BoundedRuntimeSummary,
}

impl TerminationRequest {
    pub fn user_requested(reason: impl AsRef<str>) -> Self {
        Self {
            source: TerminationRequestSource::User,
            reason: BoundedRuntimeSummary::new(reason),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminationRequestSource {
    User,
    ProjectClose,
    AppClose,
    RuntimeCleanup,
    TestHarness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminationSignal {
    Sigterm,
    Sigkill,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminationOutcome {
    Exited {
        exit_status: i32,
    },
    TerminatedBySignal {
        signal: TerminationSignal,
    },
    KilledAfterTimeout {
        initial_signal: TerminationSignal,
        fallback_signal: TerminationSignal,
    },
    OrphanedUnknown {
        summary: BoundedRuntimeSummary,
    },
    Failed {
        summary: BoundedRuntimeSummary,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedRuntimeSummary {
    text: String,
    truncated: bool,
}

impl BoundedRuntimeSummary {
    pub const MAX_CHARS: usize = 240;

    pub fn new(text: impl AsRef<str>) -> Self {
        let mut bounded = String::new();
        let mut truncated = false;

        for (index, character) in text.as_ref().chars().enumerate() {
            if index >= Self::MAX_CHARS {
                truncated = true;
                break;
            }
            bounded.push(character);
        }

        Self {
            text: bounded,
            truncated,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn was_truncated(&self) -> bool {
        self.truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_shell_launch_spec_is_project_owned_and_plain() {
        let project_id = ProjectId::for_test(1);
        let spec = TerminalLaunchSpec::plain_shell(
            project_id.clone(),
            "Shell",
            "/workspace/project",
            "/bin/sh",
        );

        assert_eq!(spec.project_id, project_id);
        assert_eq!(spec.kind, TerminalKind::Plain);
        assert_eq!(spec.cwd, PathBuf::from("/workspace/project"));
        assert_eq!(spec.shell, PathBuf::from("/bin/sh"));
        assert_eq!(spec.environment_policy, TerminalEnvironmentPolicy::Minimal);
        assert_eq!(spec.dimensions, TerminalDimensions { rows: 24, cols: 80 });
    }

    #[test]
    fn runtime_handle_carries_identity_without_process_handles() {
        let terminal_id = TerminalId::for_test(1);
        let project_id = ProjectId::for_test(2);
        let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project_id.clone());

        assert_eq!(handle.terminal_id, terminal_id);
        assert_eq!(handle.project_id, project_id);
    }

    #[test]
    fn output_summary_records_truncation_from_dropped_bytes() {
        assert_eq!(
            TerminalOutputSummary::new(1024, 0),
            TerminalOutputSummary {
                buffered_bytes: 1024,
                dropped_bytes: 0,
                truncated: false,
            }
        );
        assert_eq!(
            TerminalOutputSummary::new(1024, 256),
            TerminalOutputSummary {
                buffered_bytes: 1024,
                dropped_bytes: 256,
                truncated: true,
            }
        );
    }

    #[test]
    fn bounded_runtime_summary_truncates_long_text() {
        let summary = BoundedRuntimeSummary::new("x".repeat(BoundedRuntimeSummary::MAX_CHARS + 1));

        assert_eq!(
            summary.as_str().chars().count(),
            BoundedRuntimeSummary::MAX_CHARS
        );
        assert!(summary.was_truncated());
    }

    #[test]
    fn termination_request_bounds_reason_text() {
        let request = TerminationRequest::user_requested(
            "user requested terminal close with a bounded human-readable reason",
        );

        assert_eq!(request.source, TerminationRequestSource::User);
        assert!(!request.reason.was_truncated());
    }
}
