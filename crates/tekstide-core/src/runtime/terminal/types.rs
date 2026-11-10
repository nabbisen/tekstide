use std::path::PathBuf;

use crate::approval::RunCapabilityToken;
use crate::domain::{
    AgentCompatibilityLevel, TerminalId, TerminalKind, TerminalStatus, VisibleSlot,
};
use crate::project::ProjectId;
use crate::transcript::TranscriptWriterConfig;

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
    transcript_writer_config: Option<TranscriptWriterConfig>,
    adapter_approval_config: Option<AdapterApprovalConfig>,
    launch_authority: TerminalLaunchAuthority,
}

/// RFC-022 PR-022-C: the token and socket path a `spawn_adapter` launch
/// delivers to the child process, carried on `TerminalLaunchSpec` the same
/// way `TranscriptWriterConfig` is -- set only through
/// `set_adapter_approval_config`, `None` for every plain-shell launch.
/// `Some` here is what `launch_project_adapter` (as opposed to
/// `launch_project_shell`) actually reads to build the child's
/// environment; nothing about `TerminalKind` or `environment_policy`
/// alone selects the spawn path -- the caller decides which `launch_*`
/// method to call, and this field is what that path needs once chosen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdapterApprovalConfig {
    pub(crate) socket_path: PathBuf,
    pub(crate) token: RunCapabilityToken,
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
            transcript_writer_config: None,
            adapter_approval_config: None,
            launch_authority: TerminalLaunchAuthority::PlainShell,
        }
    }

    pub(crate) fn authorize_validated_agent_run(
        &mut self,
        compatibility_level: AgentCompatibilityLevel,
    ) {
        self.launch_authority = TerminalLaunchAuthority::ValidatedAgentRun {
            compatibility_level,
        };
    }

    pub(crate) fn has_launch_authority_for_kind(&self) -> bool {
        match self.launch_authority {
            TerminalLaunchAuthority::PlainShell => self.kind == TerminalKind::Plain,
            TerminalLaunchAuthority::ValidatedAgentRun {
                compatibility_level,
            } => self.kind == terminal_kind_from_compatibility(compatibility_level),
        }
    }

    pub(crate) fn set_transcript_writer_config(&mut self, config: Option<TranscriptWriterConfig>) {
        self.transcript_writer_config = config;
    }

    pub(crate) fn transcript_writer_config(&self) -> Option<&TranscriptWriterConfig> {
        self.transcript_writer_config.as_ref()
    }

    pub(crate) fn set_adapter_approval_config(&mut self, config: Option<AdapterApprovalConfig>) {
        self.adapter_approval_config = config;
    }

    pub(crate) fn adapter_approval_config(&self) -> Option<&AdapterApprovalConfig> {
        self.adapter_approval_config.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalLaunchAuthority {
    PlainShell,
    ValidatedAgentRun {
        compatibility_level: AgentCompatibilityLevel,
    },
}

fn terminal_kind_from_compatibility(level: AgentCompatibilityLevel) -> TerminalKind {
    match level {
        AgentCompatibilityLevel::Plain => TerminalKind::Plain,
        AgentCompatibilityLevel::Supervised => TerminalKind::Supervised,
        AgentCompatibilityLevel::Managed => TerminalKind::Managed,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEnvironmentPolicy {
    Minimal,
    Named(String),
    ExplicitAllowlist(Vec<String>),
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
    /// RFC-050 D3: the run's transcript file was locked by another handle,
    /// so no writer was created and the process started without capture.
    TranscriptWriterLockUnavailable {
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
    /// RFC-043 D3: `request_terminate`'s step 4, and *only* step 4 --
    /// pushed once, right before `Terminated`, carrying exactly what the
    /// real re-enumeration of the session observed. `true` only when
    /// that `/proc` scan found nothing left; `false` for a grace period
    /// that expired with survivors, an enumeration that could not run,
    /// or anything else short of a positive, current observation of
    /// empty. Never inferred from which `TerminationOutcome` variant the
    /// session leader itself produced -- that inference is the exact gap
    /// this event exists to close (a backgrounded job can survive in its
    /// own process group regardless of how the leader's own termination
    /// is classified).
    SessionConfirmedEmpty {
        handle: TerminalRuntimeHandle,
        confirmed: bool,
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
    /// RFC-043 D1's own sequence step 1: sent to the session leader
    /// alone (a single pid, never a `-group`/`-session` target) so an
    /// interactive shell's own job control does the cooperative half of
    /// the work -- a shell with job control on hangs up its own
    /// background jobs when it is itself hung up, which is the whole
    /// reason those jobs are in separate process groups to begin with.
    Sighup,
    Sigterm,
    Sigkill,
}

/// Deliberately not `#[non_exhaustive]`, the same decision already made
/// for `ApprovalChannelErrorReason` (see that type's own doc comment)
/// and for the same reason: adding `CommittedToExit` (see its own doc
/// comment) is a breaking change for any exhaustive `match` over this
/// enum in a downstream crate (`CHANGELOG.md`'s `Unreleased` entry), and
/// marking this `#[non_exhaustive]` now would make the *next* addition
/// non-breaking at the cost of permanently losing the compiler's own
/// ability to enumerate, at every call site that matches an outcome,
/// exactly where a new case needs a real decision rather than a
/// wildcard's silent catch. That is not a hypothetical cost here: this
/// variant's own review (657 round 3) found two call sites
/// (`crates/tekstide/src/shell.rs`) that `#[non_exhaustive]`-shaped
/// wildcards had already been hiding, found only by reading the code,
/// not by the compiler -- the opposite failure mode this enum staying
/// exhaustive-capable exists to keep rare.
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
    /// The child has committed to exiting -- the kernel's own
    /// bookkeeping says so, and that reading does not reverse -- but no
    /// exit status became available inside the budget this runtime gave
    /// it. **Not [`Self::OrphanedUnknown`]: nothing here is unknown.** The
    /// kernel's own answer to "what is this process doing" is precise;
    /// what is absent is a *status*, not a *reading*. **Not a timeout
    /// in the ordinary sense either**: a caller that gave up on a child
    /// that was never observed to have committed to exiting still gets
    /// `None`, not this variant -- this is reported only when the
    /// commitment was itself observed before the budget ran out.
    ///
    /// Added 2026-11-10 (owner's decision on the proposal review 657
    /// round 3 produced): the one case `TerminalRuntime::wait_for_exit`
    /// used to fold into a bare `None`, indistinguishable from "this
    /// budget never saw anything resembling exit". See
    /// `crates/tekstide-core/src/runtime/terminal/termination.rs`'s own
    /// `wait_for_child_outcome` for the real producer, and
    /// `rfcs/handoffs/test-process-leak.md`'s `proc_exit`/`P_WEXIT`
    /// cluster for why the case exists in practice, not only in theory.
    /// This variant makes the *reporting* honest; it does not explain
    /// *why* a committed child can still sit without a status for tens
    /// of seconds, which stays a reserved, thread-level question.
    CommittedToExit {
        summary: BoundedRuntimeSummary,
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
