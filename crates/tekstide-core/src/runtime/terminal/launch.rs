use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::approval::{APPROVAL_SOCKET_PATH_ENV_VAR, inject_token_into_environment};
use crate::domain::{TerminalId, TerminalSession, TerminalStatus};
use crate::project::{ProjectId, ProjectSession};
use crate::transcript::{
    BoundedTranscriptWriter, TranscriptCaptureMode, TranscriptWriteError, TranscriptWriteSummary,
};

use super::pty::{OpenPty, close_fd, resize_master};
use super::reader::{TerminalReader, TranscriptCapture};
use super::types::AdapterApprovalConfig;
use super::{
    BoundedRuntimeSummary, TerminalDimensions, TerminalEnvironmentPolicy, TerminalLaunchSpec,
    TerminalOutputSummary, TerminalRuntimeEvent, TerminalRuntimeHandle,
};

pub struct LinuxTerminalRuntime {
    pub(super) sessions: HashMap<TerminalId, RunningTerminal>,
}

impl LinuxTerminalRuntime {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn launch_project_shell(
        &mut self,
        project: &ProjectSession,
        spec: TerminalLaunchSpec,
    ) -> Result<(TerminalSession, Vec<TerminalRuntimeEvent>), TerminalLaunchError> {
        validate_launch_spec(project, &spec)?;

        let (transcript_writer, transcript_capture_mode) = match spec.transcript_writer_config() {
            Some(config) => {
                let mode = config.mode;
                let writer = BoundedTranscriptWriter::create(config.clone()).map_err(|error| {
                    TerminalLaunchError::TranscriptWriterUnavailable {
                        summary: transcript_write_error_summary(&error),
                    }
                })?;
                (Some(writer), Some(mode))
            }
            None => (None, None),
        };
        let mut pty = OpenPty::new(spec.dimensions)
            .map_err(|summary| TerminalLaunchError::PtyUnavailable { summary })?;
        let mut terminal = TerminalSession::new(
            spec.project_id.clone(),
            spec.kind,
            spec.title.clone(),
            spec.cwd.clone(),
            spec.command_line_summary.clone(),
        );
        let handle = TerminalRuntimeHandle::new(terminal.id.clone(), spec.project_id.clone());
        let child = spawn_shell(&spec, &mut pty)?;

        terminal
            .transition_to(TerminalStatus::Running)
            .map_err(|error| TerminalLaunchError::UnexpectedLifecycleTransition {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to mark launched terminal running: {error:?}"
                )),
            })?;

        self.sessions.insert(
            terminal.id.clone(),
            RunningTerminal {
                project_id: spec.project_id,
                process_group_id: child.id() as libc::pid_t,
                child,
                master: pty.into_master(),
                transcript_writer,
                transcript_capture_mode,
            },
        );

        Ok((
            terminal,
            vec![
                TerminalRuntimeEvent::LaunchAccepted {
                    handle: handle.clone(),
                },
                TerminalRuntimeEvent::ProcessStarted { handle },
            ],
        ))
    }

    /// RFC-022 PR-022-C: launches `spec.shell` as an approval-token-bearing
    /// adapter rather than a plain shell -- see `spawn_adapter`'s own doc
    /// comment for exactly what that changes about the child's
    /// environment. `spec.adapter_approval_config()` must be `Some`:
    /// this method's whole purpose is spawning something with an
    /// approval channel to talk to, so a spec without one is a caller
    /// error (`MissingAdapterApprovalConfig`), not a runtime condition to
    /// tolerate.
    ///
    /// Deliberately a **duplicate** of `launch_project_shell`'s own
    /// orchestration shape rather than a shared refactor of it: the only
    /// two differences are the approval-config check and which `spawn_*`
    /// function runs, and `launch_project_shell` is already-reviewed,
    /// security-adjacent code this slice has no reason to touch at all.
    /// `validate_launch_spec` itself *is* shared, unmodified -- so
    /// `ExplicitAllowlist` rejection, cross-project checks, and cwd
    /// containment all apply to this path exactly as they do to
    /// `launch_project_shell`'s, for free, by construction.
    pub fn launch_project_adapter(
        &mut self,
        project: &ProjectSession,
        spec: TerminalLaunchSpec,
    ) -> Result<(TerminalSession, Vec<TerminalRuntimeEvent>), TerminalLaunchError> {
        validate_launch_spec(project, &spec)?;
        let approval = spec
            .adapter_approval_config()
            .ok_or(TerminalLaunchError::MissingAdapterApprovalConfig)?
            .clone();

        let (transcript_writer, transcript_capture_mode) = match spec.transcript_writer_config() {
            Some(config) => {
                let mode = config.mode;
                let writer = BoundedTranscriptWriter::create(config.clone()).map_err(|error| {
                    TerminalLaunchError::TranscriptWriterUnavailable {
                        summary: transcript_write_error_summary(&error),
                    }
                })?;
                (Some(writer), Some(mode))
            }
            None => (None, None),
        };
        let mut pty = OpenPty::new(spec.dimensions)
            .map_err(|summary| TerminalLaunchError::PtyUnavailable { summary })?;
        let mut terminal = TerminalSession::new(
            spec.project_id.clone(),
            spec.kind,
            spec.title.clone(),
            spec.cwd.clone(),
            spec.command_line_summary.clone(),
        );
        let handle = TerminalRuntimeHandle::new(terminal.id.clone(), spec.project_id.clone());
        let child = spawn_adapter(&spec, &mut pty, &approval)?;

        terminal
            .transition_to(TerminalStatus::Running)
            .map_err(|error| TerminalLaunchError::UnexpectedLifecycleTransition {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to mark launched terminal running: {error:?}"
                )),
            })?;

        self.sessions.insert(
            terminal.id.clone(),
            RunningTerminal {
                project_id: spec.project_id,
                process_group_id: child.id() as libc::pid_t,
                child,
                master: pty.into_master(),
                transcript_writer,
                transcript_capture_mode,
            },
        );

        Ok((
            terminal,
            vec![
                TerminalRuntimeEvent::LaunchAccepted {
                    handle: handle.clone(),
                },
                TerminalRuntimeEvent::ProcessStarted { handle },
            ],
        ))
    }

    pub fn write_input(
        &mut self,
        handle: &TerminalRuntimeHandle,
        input: &[u8],
    ) -> Result<TerminalRuntimeEvent, TerminalRuntimeError> {
        let session = self.session_mut(handle)?;
        session
            .master
            .write_all(input)
            .map_err(|error| TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!("failed to write PTY input: {error}")),
            })?;
        session
            .master
            .flush()
            .map_err(|error| TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!("failed to flush PTY input: {error}")),
            })?;

        Ok(TerminalRuntimeEvent::InputWritten {
            handle: handle.clone(),
            bytes: input.len(),
        })
    }

    pub fn read_available_bounded_for(
        &mut self,
        handle: &TerminalRuntimeHandle,
        duration: Duration,
        max_buffered_bytes: usize,
    ) -> Result<(Vec<u8>, TerminalRuntimeEvent), TerminalRuntimeError> {
        let session = self.session_mut(handle)?;
        let started = Instant::now();
        let mut output = Vec::new();
        let mut dropped_bytes = 0;
        let mut buffer = [0_u8; 4096];

        while started.elapsed() < duration {
            match session.master.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    if let Some(writer) = session.transcript_writer.as_mut() {
                        writer.append(&buffer[..bytes_read]).map_err(|error| {
                            TerminalRuntimeError::TranscriptWrite {
                                summary: transcript_write_error_summary(&error),
                            }
                        })?;
                    }
                    let remaining_capacity = max_buffered_bytes.saturating_sub(output.len());
                    let accepted_bytes = remaining_capacity.min(bytes_read);
                    output.extend_from_slice(&buffer[..accepted_bytes]);
                    dropped_bytes += bytes_read - accepted_bytes;
                }
                Err(error)
                    if error.kind() == io::ErrorKind::Interrupted
                        || error.raw_os_error() == Some(libc::EIO) =>
                {
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(TerminalRuntimeError::Io {
                        summary: BoundedRuntimeSummary::new(format!(
                            "failed to read PTY output: {error}"
                        )),
                    });
                }
            }
        }

        let summary = TerminalOutputSummary::new(output.len(), dropped_bytes);
        if let Some(writer) = session.transcript_writer.as_mut() {
            writer
                .flush()
                .map_err(|error| TerminalRuntimeError::TranscriptWrite {
                    summary: transcript_write_error_summary(&error),
                })?;
        }
        Ok((
            output,
            TerminalRuntimeEvent::OutputBuffered {
                handle: handle.clone(),
                summary,
            },
        ))
    }

    /// RFC-017 Amendment 1, PR-A1-A: spawns a dedicated reader thread
    /// over a *duplicate* of this session's PTY master, built alongside
    /// `read_available_bounded_for` rather than in place of it. `master`
    /// stays owned by `RunningTerminal` for `write_input`/`resize`;
    /// the duplicate the reader thread receives is independent of it in
    /// Rust's `File` API but shares the same open file description (and
    /// so the same `O_NONBLOCK` status), which is what lets the reader
    /// thread do non-blocking drains between `poll(2)` wakeups without
    /// affecting how writes on the original handle behave.
    ///
    /// **RFC-011 Amendment 2, D1**: also moves this session's transcript
    /// writer (if configured) into the new reader thread -- `&mut self`
    /// since `Option::take` is how that move happens.
    /// `RunningTerminal.transcript_writer` becomes `None` from this call
    /// onward regardless of whether capture was configured; a second
    /// call to this method (or a later call to `read_available_bounded_for`
    /// on the same session) would find no writer left to use. Nothing in
    /// this crate does that today -- `crates/tekstide` calls this
    /// exactly once per launch (`TerminalPane::launch`), and
    /// `read_available_bounded_for`'s own callers are a separate,
    /// decoupled test/agent-output-capture path that never also calls
    /// this method on the same session (see `qa-evidence.md`'s PR-A2-A
    /// section for why re-homing left that path alone).
    pub fn spawn_output_reader(
        &mut self,
        handle: &TerminalRuntimeHandle,
    ) -> Result<TerminalReader, TerminalRuntimeError> {
        let session = self.session_mut(handle)?;
        let master_for_reader =
            session
                .master
                .try_clone()
                .map_err(|error| TerminalRuntimeError::Io {
                    summary: BoundedRuntimeSummary::new(format!(
                        "failed to duplicate PTY master for reader thread: {error}"
                    )),
                })?;
        let transcript_capture = session.transcript_writer.take().map(|writer| {
            let mode = session
                .transcript_capture_mode
                .expect("transcript_capture_mode is always Some whenever transcript_writer is");
            TranscriptCapture::new(writer, mode)
        });
        TerminalReader::spawn(master_for_reader, transcript_capture).map_err(|error| {
            TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to create reader thread shutdown eventfd: {error}"
                )),
            }
        })
    }

    pub fn resize(
        &mut self,
        handle: &TerminalRuntimeHandle,
        dimensions: TerminalDimensions,
    ) -> Result<TerminalRuntimeEvent, TerminalRuntimeError> {
        let session = self.session_mut(handle)?;
        resize_master(&session.master, dimensions).map_err(|summary| TerminalRuntimeError::Io {
            summary: BoundedRuntimeSummary::new(format!(
                "failed to route PTY resize: {}",
                summary.as_str()
            )),
        })?;

        Ok(TerminalRuntimeEvent::Resized {
            handle: handle.clone(),
            dimensions,
        })
    }

    pub fn transcript_write_summary(
        &self,
        handle: &TerminalRuntimeHandle,
    ) -> Result<Option<TranscriptWriteSummary>, TerminalRuntimeError> {
        Ok(self
            .session(handle)?
            .transcript_writer
            .as_ref()
            .map(BoundedTranscriptWriter::summary))
    }

    pub(super) fn session(
        &self,
        handle: &TerminalRuntimeHandle,
    ) -> Result<&RunningTerminal, TerminalRuntimeError> {
        let session = self.sessions.get(&handle.terminal_id).ok_or(
            TerminalRuntimeError::UnknownTerminal {
                terminal_id: handle.terminal_id.clone(),
            },
        )?;

        if session.project_id != handle.project_id {
            return Err(TerminalRuntimeError::CrossProjectHandle {
                terminal_id: handle.terminal_id.clone(),
            });
        }

        Ok(session)
    }

    pub(super) fn session_mut(
        &mut self,
        handle: &TerminalRuntimeHandle,
    ) -> Result<&mut RunningTerminal, TerminalRuntimeError> {
        let session = self.sessions.get_mut(&handle.terminal_id).ok_or(
            TerminalRuntimeError::UnknownTerminal {
                terminal_id: handle.terminal_id.clone(),
            },
        )?;

        if session.project_id != handle.project_id {
            return Err(TerminalRuntimeError::CrossProjectHandle {
                terminal_id: handle.terminal_id.clone(),
            });
        }

        Ok(session)
    }
}

impl Default for LinuxTerminalRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalLaunchError {
    CrossProject,
    UnsupportedTerminalKind,
    UnsupportedEnvironmentPolicy {
        summary: BoundedRuntimeSummary,
    },
    MissingProjectRoot {
        summary: BoundedRuntimeSummary,
    },
    InvalidCwd {
        summary: BoundedRuntimeSummary,
    },
    CwdEscapesProjectRoot {
        summary: BoundedRuntimeSummary,
    },
    ShellUnavailable {
        summary: BoundedRuntimeSummary,
    },
    PtyUnavailable {
        summary: BoundedRuntimeSummary,
    },
    SpawnFailed {
        summary: BoundedRuntimeSummary,
    },
    /// RFC-017 Amendment 1, PR-A1-B: the reader thread's shutdown
    /// `eventfd` could not be created (`TerminalReader::spawn`'s only
    /// failure mode, itself only resource exhaustion).
    ReaderUnavailable {
        summary: BoundedRuntimeSummary,
    },
    TranscriptWriterUnavailable {
        summary: BoundedRuntimeSummary,
    },
    UnexpectedLifecycleTransition {
        summary: BoundedRuntimeSummary,
    },
    /// RFC-022 PR-022-C: `launch_project_adapter` was called with a
    /// `TerminalLaunchSpec` that never had `set_adapter_approval_config`
    /// applied to it -- a caller error (this method has no other use for
    /// a spec without one), not a runtime condition.
    MissingAdapterApprovalConfig,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TerminalRuntimeError {
    UnknownTerminal { terminal_id: TerminalId },
    CrossProjectHandle { terminal_id: TerminalId },
    Io { summary: BoundedRuntimeSummary },
    TranscriptWrite { summary: BoundedRuntimeSummary },
}

pub(super) struct RunningTerminal {
    pub(super) project_id: ProjectId,
    pub(super) process_group_id: libc::pid_t,
    pub(super) child: Child,
    pub(super) master: fs::File,
    pub(super) transcript_writer: Option<BoundedTranscriptWriter>,
    /// RFC-011 Amendment 2, D1: carried alongside `transcript_writer` so
    /// `spawn_output_reader` has the capture mode available at the
    /// point it moves the writer into the reader thread -- `Some` iff
    /// `transcript_writer` is `Some`, checked by construction in
    /// `launch_project_shell` (both are set from the same `match` arm).
    pub(super) transcript_capture_mode: Option<TranscriptCaptureMode>,
}

/// `RunningTerminal::drop`'s own bounded grace periods -- short and
/// fixed, unlike `request_terminate`'s caller-supplied ones, because
/// this runs synchronously during an unrelated panic's unwind and must
/// not turn into an open-ended hang. `SIGKILL` reaping was measured at
/// close to instantaneous on an idle machine
/// (`pty-master-fd-inheritance-qa-evidence.md`'s own measurement), so
/// both windows are generous relative to that, not tuned to be minimal.
const DROP_HANGUP_GRACE: std::time::Duration = std::time::Duration::from_millis(200);
const DROP_SIGKILL_GRACE: std::time::Duration = std::time::Duration::from_millis(200);

/// The same poll shape `termination::wait_for_session_outcome` uses,
/// without that function's additional child-outcome bookkeeping --
/// `Drop` has no `TerminationOutcome` to report, only "is it safe to
/// stop escalating yet."
fn wait_briefly_for_session_empty(session_id: libc::pid_t, timeout: std::time::Duration) -> bool {
    let started = std::time::Instant::now();
    loop {
        if session_confirmed_empty(session_id) {
            return true;
        }
        if started.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// `test-process-leak.md`'s second, distinct cause, fixed: until this
/// impl existed, dropping a `RunningTerminal` did nothing, so any path
/// that dropped one without going through [`termination::request_terminate`]
/// first -- most concretely, a panicking test, but structurally anything
/// that drops `LinuxTerminalRuntime`/its `sessions` map without an
/// explicit termination request first -- leaked a real shell process and
/// its PTY indefinitely (found at 3,899 leaked processes with
/// `/dev/pts` at its 4096 limit, 2026-08-25).
///
/// **Every one of the five sites that can drop a `RunningTerminal` was
/// enumerated before this was written** (`self.sessions.remove`/`.insert`,
/// three and two respectively -- see `termination.rs` and this file's
/// own `launch_project_shell`/`launch_project_adapter`): all five are
/// "this terminal is finished," never "this value is moving to survive
/// under a different key."
///
/// - Both `insert` sites key on a freshly minted `TerminalId::new_uuid()`
///   (`TerminalSession::new`'s own construction), so neither can ever
///   evict a session actually stored under that key -- the discarded old
///   value `HashMap::insert` would return is always `None` in practice.
/// - All three `remove` sites in `termination.rs` are reached only after
///   the child has either exited on its own (`wait_for_child_outcome`,
///   confirmed via a real `try_wait` result) or the process group itself
///   has been confirmed gone or given up on after a full SIGTERM/SIGKILL
///   escalation (`wait_for_process_group_outcome`, `request_terminate`'s
///   own final give-up arm). None removes a session the caller still
///   expects to keep running.
///
/// A live terminal is therefore never at risk from this `Drop` firing
/// where the codebase already drops one today -- confirmed by
/// enumeration, not assumed, per this document's own required
/// discipline for a destructor being made consequential.
///
/// **Signals the process group, not only `child`** -- `request_terminate`
/// signals `-process_group_id` because a shell's own children share its
/// group and a plain `child.kill()` would leave them running; this
/// mirrors that, not the narrower single-process guard `test_support`'s
/// `KillOnDropChild` uses for a bare adapter/reference-process `Child`
/// with no group of its own to worry about.
///
/// **A last-resort safety net, not the normal path** -- RFC-039 PR-039-C
/// made `request_terminate` (graceful `SIGTERM`, timeout, `SIGKILL`
/// fallback, its own timeout) the normal way a user-initiated close
/// ends a terminal. Replicating that same graceful-then-forceful
/// escalation here, with its own timeouts, would mean a destructor that
/// blocks for seconds -- unacceptable for a safety net that must run
/// synchronously during an unrelated panic's unwind. This goes straight
/// to `SIGKILL`: by the time nothing else asked this terminal to stop,
/// escalation is not owed, only cleanup.
///
/// **Idempotent by construction, not merely intended to be**: on the
/// normal path `request_terminate` already killed the group and this
/// runs against an already-dead one (`libc::kill` on a vanished group
/// returns `ESRCH`, silently ignored, matching
/// `send_signal_to_process_group`'s own tolerance for it); `child.wait()`
/// on an already-reaped child returns `ECHILD`, likewise discarded. Both
/// are cheap, harmless no-ops in the common case -- this `Drop` fires on
/// every one of the five sites above, not only the leak path, since
/// `HashMap::remove`/a displaced `insert` drop their value unconditionally.
impl Drop for RunningTerminal {
    fn drop(&mut self) {
        // RFC-043 D1/D2/PR-043-B: the same `SIGHUP`-first, session-wide
        // sequence `termination::request_terminate` uses, not the old
        // "`SIGKILL` the shell's own group and hope" this destructor used
        // before this RFC -- discovered still necessary the hard way, not
        // assumed: PR-043-A's guard below still fired against a
        // panicking/dropped-without-`request_terminate` terminal after
        // `request_terminate` itself was already fixed, because *this*
        // function still had the old logic. `request_terminate` is not
        // the only path that ends a terminal; this one needed the same
        // fix, not only the same diagnostic.
        //
        // Bounded, not open-ended, unlike `request_terminate`'s own
        // caller-supplied grace periods: this runs synchronously during
        // an unrelated panic's unwind, so short, fixed grace periods
        // (`DROP_HANGUP_GRACE`/`DROP_SIGKILL_GRACE`) stand in for the
        // "give it real time" this is not the place to offer -- RFC-039
        // PR-039-C's own `request_terminate` is still the normal,
        // user-facing way a close gets real grace periods; this is the
        // last-resort net under it, not a replacement for it.
        let session_id = self.process_group_id;
        // Same `<= 1` refusal `send_signal_to_process_group` already
        // applies before signalling -- group 0 means "this process's own
        // group" (never correct to target here) and group 1 is not a
        // real terminal's group on this platform; a `RunningTerminal`
        // should never be constructed with either, but a destructor is
        // exactly the place to not trust that invariant blindly.
        if session_id > 1 {
            unsafe {
                libc::kill(session_id, libc::SIGHUP);
            }
        }

        if !wait_briefly_for_session_empty(session_id, DROP_HANGUP_GRACE) {
            super::termination::signal_candidates(
                session_id,
                processes_in_session(session_id).into_iter().flatten(),
                libc::SIGKILL,
            );
            let _ = wait_briefly_for_session_empty(session_id, DROP_SIGKILL_GRACE);
        }

        // Reaps this value's own direct child, non-blocking (`try_wait`,
        // not `wait`): the escalation above already gave the leader
        // every bounded chance this destructor is willing to offer, so a
        // leader that still somehow has not exited must not be allowed
        // to hang this destructor forever. Harmless (`Ok(None)` or
        // `Err`, both discarded) if it was already reaped by an earlier
        // `try_wait`/`wait` call before this value ever reached `Drop`.
        let _ = self.child.try_wait();

        // RFC-043 PR-043-A, D4: this is deliberately here, not wired into
        // any specific test -- "one week ago the audit-store slice wired
        // a guard into the 23 sites its handoff named and the suite
        // failed 58 *other* tests reaching the same path" (this RFC's
        // own security document, `what-containment-must-not-become.md`
        // §5). A guard placed at the sites that call it always misses
        // sites; a guard placed where the process is *created* cannot.
        //
        // `self.process_group_id` is also this terminal's session id --
        // `spawn_pty_child`'s `pre_exec` calls `setsid()` before `exec`,
        // which makes the freshly `fork`ed child both its own process
        // group leader and its own session leader, so `child.id()`
        // (what this field is constructed from) equals both at launch.
        // A backgrounded job started inside the shell gets its *own*
        // process group -- that is `test-process-leak.md`'s own
        // corrected finding -- but stays in this same session, which is
        // exactly why session id, not process group id, is what the kill
        // above (a single `-process_group_id` target) cannot reach.
        //
        // RFC-043 PR-043-A shipped before PR-043-B: at that point this
        // comment said "no containment yet, this only observes and
        // reports," and the guard turned four tests red on purpose --
        // that inventory is recorded in this RFC's own `qa-evidence.md`.
        // PR-043-B, above, is what now acts on what PR-043-A found; this
        // assertion is what confirms the action actually worked, on
        // every drop, not only the four tests that first exposed the gap.
        //
        // `#[cfg(any(test, feature = "test-support"))]`, not bare
        // `#[cfg(test)]`: this crate's own `cfg(test)` only activates
        // when *tekstide-core's* test suite is what's compiling, not
        // when a consuming crate's tests are -- most real terminal
        // launches (the flood benchmarks, the close-with-a-backgrounded-
        // descendant tests) live in the `tekstide` binary crate, which
        // depends on this one as an ordinary library. Found the hard
        // way: this guard fired correctly under `cargo test -p
        // tekstide-core` but was silently absent (not skipped, *absent*
        // -- the code did not exist in that build) under `cargo test -p
        // tekstide`, and a benchmark known to leak 28 processes passed
        // clean. `tekstide-core`'s own `test-support` Cargo feature is
        // what `tekstide/Cargo.toml`'s `[dev-dependencies]` enables to
        // close that gap; see that feature's own doc comment.
        #[cfg(any(test, feature = "test-support"))]
        assert_session_is_empty(self.process_group_id);
    }
}

/// The test-and-test-support-only half of RFC-043 D4 -- panics loudly,
/// naming every survivor, if anything is still alive in the session
/// this terminal's shell led, immediately after the kill above. Skipped
/// while the current thread is already unwinding from a different panic
/// (`std::thread::panicking()`): a `Drop` that panics during an
/// existing unwind aborts the whole test process instead of failing one
/// test, which would turn "this test leaked" into "this test run lost
/// every other test's result," the opposite of what a red test is for.
/// A test that panics for its own reason and *also* leaks is still
/// worth knowing about, but not at that cost -- it is not what this
/// slice's own "make the leak red" goal needs, since that test was
/// already red.
#[cfg(any(test, feature = "test-support"))]
fn assert_session_is_empty(session_id: libc::pid_t) {
    if std::thread::panicking() {
        return;
    }
    assert!(
        session_confirmed_empty(session_id),
        "RunningTerminal::drop's own containment sequence for session {session_id} finished, \
         but a real re-enumeration afterward still found: {:?}. Possible causes, not narrowed to \
         one: a backgrounded job escaped containment (rfcs/accepted/043-terminal-process-containment.md, \
         the defect this guard exists to catch); or a `/proc` read failed (session_confirmed_empty \
         reports false for that too, on purpose); or a real, live survivor this enumeration has \
         not yet excluded for some other reason. State what was observed here, not which of these \
         it is -- that determination belongs in whoever reads this failure, with the actual pids \
         and their own /proc/<pid>/stat in hand, not in this message.",
        processes_in_session(session_id)
    );
}

/// D3's own honesty rule, in one place rather than re-derived at every
/// call site: `true` only when a real enumeration positively observed
/// nobody left in `session_id`. `None` from [`processes_in_session`]
/// (the enumeration itself could not be trusted -- a `/proc` read that
/// failed) is `false`, the same as a non-empty survivor list --
/// `what-containment-must-not-become.md` §4's own text names this exact
/// case ("a `/proc` read that failed... `false`. Not 'almost certainly
/// empty.'"). An earlier version of this function let a failed `/proc`
/// read fall through `Vec::new()` to an empty list, which is precisely
/// the unearned "almost certainly empty" confidence that document
/// forbids -- caught while writing this slice's own required "grace
/// period expiring produces false" test, not by inspection.
pub(super) fn session_confirmed_empty(session_id: libc::pid_t) -> bool {
    processes_in_session(session_id).is_some_and(|survivors| survivors.is_empty())
}

/// Every live pid whose `/proc/<pid>/stat` session field equals
/// `session_id` -- read-only, no signal sent, safe to call
/// unconditionally. `/proc` entries that vanish or become unreadable
/// mid-scan (a process exiting during the read) are skipped rather than
/// erroring, the same tolerance `process_group_exists_by_id` already
/// gives a process that may no longer exist by the time it's checked.
///
/// `None` only when `/proc` itself could not be enumerated at all --
/// deliberately distinct from `Some(vec![])` ("enumerated successfully,
/// found nobody"). Conflating the two, by defaulting to an empty `Vec`
/// on a failed read, is exactly the false confidence
/// [`session_confirmed_empty`]'s own doc comment describes; callers that
/// want the honest boolean should use that function, not this one's
/// `Vec` directly, unless they specifically need the pid list itself
/// (`termination.rs`'s own re-verify-before-signalling step does).
///
/// RFC-043 PR-043-B: not test-only any more -- `termination.rs`'s own
/// containment sequence now calls this in production, to know what is
/// left in a session between escalation steps. PR-043-A's own test-only
/// [`assert_session_is_empty`] is now just one more caller of it.
///
/// Reads from [`test_proc_root`]'s override when one is set
/// (`#[cfg(test)]` only) instead of the real `/proc`, so a test can
/// deterministically force the "enumeration failed" branch without
/// racing real process reaping timing to observe it.
pub(super) fn processes_in_session(session_id: libc::pid_t) -> Option<Vec<libc::pid_t>> {
    let entries = std::fs::read_dir(proc_root()).ok()?;
    Some(
        entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<libc::pid_t>().ok())
            .filter(|&pid| is_live_member_of_session(pid, session_id))
            .collect(),
    )
}

/// A pid counts as a live member of `session_id` only if its own
/// `/proc/<pid>/stat` session field matches **and** its state is not
/// `Z` (zombie). Response 340's required correction: a `SIGKILL`ed
/// process becomes a zombie until its parent calls `wait()` on it --
/// it still has a `/proc/<pid>/stat` entry with its session field
/// unchanged, but it holds no resources beyond a pid table slot and
/// cannot execute code. The enumeration was counting exactly this --
/// most often the terminal's own leader, already killed, not yet
/// reaped by the very next enumeration a few milliseconds later -- as a
/// surviving background job, which cost every ordinary close its full
/// grace period for no reason and made `RunningTerminal::drop`'s own
/// PR-043-A guard fire on a corpse rather than an actual escape.
/// Confirmed directly by the reviewer, not inferred: a zombie's own
/// `state` field really is `Z`, and a plain session-field filter really
/// does count it.
fn is_live_member_of_session(pid: libc::pid_t, session_id: libc::pid_t) -> bool {
    let Some(stat) = process_stat(pid) else {
        return false;
    };
    stat.session_id == session_id && stat.state != 'Z'
}

#[cfg(not(test))]
fn proc_root() -> &'static std::path::Path {
    std::path::Path::new("/proc")
}

/// RFC-043 D3's own required negative test ("the grace period expiring
/// produces `false`, not a hopeful `true`") cannot reliably force a real
/// process to survive `SIGKILL` long enough to observe -- reaping on a
/// quiet, idle test machine is close enough to instantaneous that a
/// zero-length grace period still reports the session correctly empty.
/// This override lets that one test force the *enumeration itself* to
/// fail deterministically instead, which `what-containment-must-not-become.md`
/// §4 requires to also report `false` -- a real, reachable failure mode
/// (`/proc` unreadable, a sandboxed environment, resource exhaustion),
/// not a contrived one.
#[cfg(test)]
fn proc_root() -> std::path::PathBuf {
    TEST_PROC_ROOT
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| std::path::PathBuf::from("/proc"))
}

#[cfg(test)]
thread_local! {
    static TEST_PROC_ROOT: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// RAII guard for [`test_proc_root`] -- restores the real `/proc` on
/// drop.
#[cfg(test)]
pub(super) struct TestProcRootGuard {
    _private: (),
}

#[cfg(test)]
impl Drop for TestProcRootGuard {
    fn drop(&mut self) {
        TEST_PROC_ROOT.with(|cell| *cell.borrow_mut() = None);
    }
}

/// Forces [`processes_in_session`] to read `path` instead of the real
/// `/proc` for the returned guard's lifetime. Pointed at a path that
/// cannot be listed (does not exist, or exists with no read permission),
/// this deterministically forces the "enumeration failed" branch --
/// see [`proc_root`]'s own doc comment for why a real test needs this
/// rather than racing `SIGKILL` reaping speed.
#[cfg(test)]
pub(super) fn test_proc_root(path: &std::path::Path) -> TestProcRootGuard {
    TEST_PROC_ROOT.with(|cell| {
        let mut cell = cell.borrow_mut();
        assert!(
            cell.is_none(),
            "test_proc_root called twice on the same thread without dropping the first guard"
        );
        *cell = Some(path.to_path_buf());
    });
    TestProcRootGuard { _private: () }
}

/// `/proc/<pid>/stat`'s own format: `pid (comm) state ppid pgrp session
/// ...`, space-separated after the `)` that closes `comm` -- `comm`
/// itself can contain spaces or even `)`, so this splits on the *last*
/// `)` in the line rather than the first space, the standard way to
/// parse this file safely. `state` is field 0 of what remains; `ppid`,
/// `pgrp`, `session` are 1-3.
struct ProcessStat {
    state: char,
    session_id: libc::pid_t,
}

fn process_stat(pid: libc::pid_t) -> Option<ProcessStat> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    let mut fields = after_comm.split_whitespace();
    let state = fields.next()?.chars().next()?;
    // `ppid` and `pgrp` are consumed (`nth(2)` skips them) to reach
    // `session`, the third field after `state`.
    let session_id = fields.nth(2)?.parse().ok()?;
    Some(ProcessStat { state, session_id })
}

/// RFC-043 PR-043-B: `termination.rs`'s own §1 re-verification ("the
/// session id is re-verified immediately before every signal") calls
/// this directly, right before each `kill`, not only through
/// [`processes_in_session`]'s own bulk enumeration -- a pid found in an
/// earlier scan may have exited and been replaced by an unrelated
/// process with the same number by the time a signal is about to be
/// sent, and that race is exactly what this second call closes.
///
/// Deliberately not zombie-aware the way [`is_live_member_of_session`]
/// is -- a zombie's session field is still authoritative (nothing has
/// reused the pid, it just hasn't been reaped), and `kill(2)` on a
/// zombie is already a harmless no-op at the kernel level, so there is
/// no false-positive-survivor risk here the way there was in the
/// enumeration this function is not part of.
pub(super) fn session_id_of(pid: libc::pid_t) -> Option<libc::pid_t> {
    process_stat(pid).map(|stat| stat.session_id)
}

fn transcript_write_error_summary(error: &TranscriptWriteError) -> BoundedRuntimeSummary {
    BoundedRuntimeSummary::new(format!(
        "transcript write failed: {:?} after {} bytes at {}",
        error.reason,
        error.byte_count,
        error.path.display()
    ))
}

fn validate_launch_spec(
    project: &ProjectSession,
    spec: &TerminalLaunchSpec,
) -> Result<(), TerminalLaunchError> {
    if project.id() != &spec.project_id {
        return Err(TerminalLaunchError::CrossProject);
    }
    if !spec.has_launch_authority_for_kind() {
        return Err(TerminalLaunchError::UnsupportedTerminalKind);
    }
    let root = canonical_existing_dir(project.canonical_root_path()).map_err(|summary| {
        TerminalLaunchError::MissingProjectRoot {
            summary: BoundedRuntimeSummary::new(summary),
        }
    })?;
    let cwd =
        canonical_existing_dir(&spec.cwd).map_err(|summary| TerminalLaunchError::InvalidCwd {
            summary: BoundedRuntimeSummary::new(summary),
        })?;

    if !cwd.starts_with(&root) {
        return Err(TerminalLaunchError::CwdEscapesProjectRoot {
            summary: BoundedRuntimeSummary::new(format!(
                "terminal cwd is outside project root: {}",
                cwd.display()
            )),
        });
    }

    if !shell_is_executable_file(&spec.shell) {
        return Err(TerminalLaunchError::ShellUnavailable {
            summary: BoundedRuntimeSummary::new(format!(
                "shell is not an executable file: {}",
                spec.shell.display()
            )),
        });
    }

    if let Some(summary) = unsupported_environment_policy_summary(&spec.environment_policy) {
        return Err(TerminalLaunchError::UnsupportedEnvironmentPolicy { summary });
    }

    Ok(())
}

fn unsupported_environment_policy_summary(
    policy: &TerminalEnvironmentPolicy,
) -> Option<BoundedRuntimeSummary> {
    match policy {
        TerminalEnvironmentPolicy::Minimal => None,
        TerminalEnvironmentPolicy::Named(name) => Some(BoundedRuntimeSummary::new(format!(
            "named terminal environment policy is not applied by the Linux runtime yet: {name}"
        ))),
        TerminalEnvironmentPolicy::ExplicitAllowlist(names) => {
            Some(BoundedRuntimeSummary::new(format!(
                "explicit terminal environment allowlist is not applied by the Linux runtime yet: {}",
                names.join(", ")
            )))
        }
    }
}

fn shell_is_executable_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn canonical_existing_dir(path: &Path) -> Result<PathBuf, String> {
    let canonical = path.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize directory {}: {error}",
            path.display()
        )
    })?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(format!("path is not a directory: {}", canonical.display()))
    }
}

fn spawn_shell(spec: &TerminalLaunchSpec, pty: &mut OpenPty) -> Result<Child, TerminalLaunchError> {
    let mut command = Command::new(&spec.shell);
    command
        .current_dir(&spec.cwd)
        .env_clear()
        .env("TERM", "xterm-256color")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("PATH", "/usr/bin:/bin")
        .env("PS1", "tekstide$ ");
    spawn_pty_child(command, pty)
}

/// RFC-022 PR-022-C: a spawn path distinct from `spawn_shell`, launching
/// an AI CLI as an adapter rather than a plain interactive shell. Reuses
/// `spawn_pty_child` for the PTY/session mechanics common to both --
/// fd duplication, `setsid`/`TIOCSCTTY`, spawn, cleanup -- since none of
/// that depends on what is being launched or how its environment is
/// built. `spec.shell` names the adapter's own executable here (the
/// field is shared with `spawn_shell`'s use of it, not renamed, since it
/// genuinely is "the executable this terminal launches" in both cases).
///
/// **`.env_clear()` plus the same five fixed variables `spawn_shell`
/// sets, unchanged** -- RFC-022's own text describes token delivery as
/// "a sixth" `.env(...)` call *on top of* that existing set, not a
/// redesigned one, so this does not invent a different fixed environment
/// for adapters. The token (`inject_token_into_environment` -- this is
/// that function's first production caller) and the socket path
/// (`APPROVAL_SOCKET_PATH_ENV_VAR`) are the sixth and seventh. Nothing is
/// inherited: `ExplicitAllowlist` is not consulted here at all, and
/// `validate_launch_spec` (shared, unmodified, run before either spawn
/// path) still rejects it before any process exists.
///
/// No `argv` is passed to the adapter. A real AI CLI decides its own
/// actions; it does not take "the command to propose" as a launch
/// argument. The reference adapter (PR-022-B) falls back to its own
/// fixed default proposal when spawned with none -- see its own doc
/// comment.
fn spawn_adapter(
    spec: &TerminalLaunchSpec,
    pty: &mut OpenPty,
    approval: &AdapterApprovalConfig,
) -> Result<Child, TerminalLaunchError> {
    let mut command = Command::new(&spec.shell);
    command
        .current_dir(&spec.cwd)
        .env_clear()
        .env("TERM", "xterm-256color")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("PATH", "/usr/bin:/bin")
        .env("PS1", "tekstide$ ");
    inject_token_into_environment(&mut command, &approval.token);
    command.env(APPROVAL_SOCKET_PATH_ENV_VAR, &approval.socket_path);
    spawn_pty_child(command, pty)
}

/// The PTY/process-group mechanics `spawn_shell` and `spawn_adapter`
/// share: duplicate the slave four ways (stdin/stdout/stderr plus a
/// fourth held only long enough to make it the controlling terminal),
/// wire the first three onto `command`, `setsid()` plus `TIOCSCTTY` in
/// the child before exec, spawn, then close every fd this function
/// itself does not hand off. `command` arrives with its program,
/// environment, and `current_dir` already set -- this function adds
/// nothing about *what* is launched, only *how* it is attached to the
/// PTY.
fn spawn_pty_child(mut command: Command, pty: &mut OpenPty) -> Result<Child, TerminalLaunchError> {
    let stdin_fd = pty
        .duplicate_slave("duplicate PTY slave for stdin")
        .map_err(|summary| TerminalLaunchError::PtyUnavailable { summary })?;
    let stdout_fd = pty
        .duplicate_slave("duplicate PTY slave for stdout")
        .map_err(|summary| TerminalLaunchError::PtyUnavailable { summary })?;
    let stderr_fd = pty
        .duplicate_slave("duplicate PTY slave for stderr")
        .map_err(|summary| TerminalLaunchError::PtyUnavailable { summary })?;
    let ctty_fd = pty
        .duplicate_slave("duplicate PTY slave for controlling terminal")
        .map_err(|summary| TerminalLaunchError::PtyUnavailable { summary })?;

    command
        .stdin(unsafe { Stdio::from_raw_fd(stdin_fd) })
        .stdout(unsafe { Stdio::from_raw_fd(stdout_fd) })
        .stderr(unsafe { Stdio::from_raw_fd(stderr_fd) });

    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::ioctl(ctty_fd, libc::TIOCSCTTY, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            libc::close(ctty_fd);
            // pty-master-fd-inheritance handoff, item 3: belt and braces,
            // not a substitute for `OpenPty::new`'s own `O_CLOEXEC` --
            // this catches any descriptor a future change opens without
            // that discipline, not only the PTY master this defect was
            // found through. stdin/stdout/stderr are already at their
            // final fds 0-2 by this point (`Command` wires them before
            // running `pre_exec`), so starting at 3 cannot touch them.
            // Raw `syscall(2)`, not `libc::close_range` directly: the
            // libc wrapper is a real symbol this binary would fail to
            // *load* on a glibc older than 2.34 if linked directly, where
            // going through the stable, decades-old `syscall(2)` entry
            // point instead fails only this one call, at run time, with
            // `ENOSYS` -- silently ignored, since this is a bonus layer
            // over the fix that already closed the known hole, not
            // something this spawn may fail over.
            libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 0);
            Ok(())
        });
    }

    let spawn_result = command.spawn();
    close_fd(ctty_fd);
    let child = spawn_result.map_err(|error| TerminalLaunchError::SpawnFailed {
        summary: BoundedRuntimeSummary::new(format!("failed to spawn PTY child: {error}")),
    })?;
    pty.close_slave();

    Ok(child)
}
