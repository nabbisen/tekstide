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
// Only `wait_for_exit_draining` (`#[cfg(test)]`) uses this; a non-test
// build never calls it, so the import must be gated the same way or it
// is unused there.
#[cfg(test)]
use super::TerminationOutcome;

pub struct TerminalRuntime {
    pub(super) sessions: HashMap<TerminalId, RunningTerminal>,
}

impl TerminalRuntime {
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

        let (transcript_writer, transcript_capture_mode, transcript_lock_unavailable) =
            prepare_transcript_writer(&spec)?;
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
                master: Some(pty.into_master()),
                transcript_writer,
                transcript_capture_mode,
            },
        );

        let mut events = vec![
            TerminalRuntimeEvent::LaunchAccepted {
                handle: handle.clone(),
            },
            TerminalRuntimeEvent::ProcessStarted {
                handle: handle.clone(),
            },
        ];
        if transcript_lock_unavailable {
            events.push(TerminalRuntimeEvent::TranscriptWriterLockUnavailable { handle });
        }
        Ok((terminal, events))
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

        let (transcript_writer, transcript_capture_mode, transcript_lock_unavailable) =
            prepare_transcript_writer(&spec)?;
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
                master: Some(pty.into_master()),
                transcript_writer,
                transcript_capture_mode,
            },
        );

        let mut events = vec![
            TerminalRuntimeEvent::LaunchAccepted {
                handle: handle.clone(),
            },
            TerminalRuntimeEvent::ProcessStarted {
                handle: handle.clone(),
            },
        ];
        if transcript_lock_unavailable {
            events.push(TerminalRuntimeEvent::TranscriptWriterLockUnavailable { handle });
        }
        Ok((terminal, events))
    }

    pub fn write_input(
        &mut self,
        handle: &TerminalRuntimeHandle,
        input: &[u8],
    ) -> Result<TerminalRuntimeEvent, TerminalRuntimeError> {
        let session = self.session_mut(handle)?;
        session
            .master()
            .write_all(input)
            .map_err(|error| TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!("failed to write PTY input: {error}")),
            })?;
        session
            .master()
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
            match session.master().read(&mut buffer) {
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
                .master()
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
        resize_master(session.master(), dimensions).map_err(|summary| {
            TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to route PTY resize: {}",
                    summary.as_str()
                )),
            }
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

impl Default for TerminalRuntime {
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
    /// `Some` for this value's entire life except during `Drop::drop`,
    /// which takes it early -- see that impl's own comment for why.
    /// Every other call site runs on a still-live session, where this
    /// is always `Some`; use the [`RunningTerminal::master`] accessor
    /// rather than matching on this directly.
    master: Option<fs::File>,
    pub(super) transcript_writer: Option<BoundedTranscriptWriter>,
    /// RFC-011 Amendment 2, D1: carried alongside `transcript_writer` so
    /// `spawn_output_reader` has the capture mode available at the
    /// point it moves the writer into the reader thread -- `Some` iff
    /// `transcript_writer` is `Some`, checked by construction in
    /// `launch_project_shell` (both are set from the same `match` arm).
    pub(super) transcript_capture_mode: Option<TranscriptCaptureMode>,
}

impl RunningTerminal {
    /// Panics if called after `Drop::drop` has already taken `master` --
    /// every real call site (`write_input`, `read_available_bounded_for`,
    /// `spawn_output_reader`, `resize`) runs through `self.session_mut`/
    /// `self.session` on a session that is still in `self.sessions`, and
    /// nothing removes an entry from that map without the value being
    /// dropped in the same motion (`termination.rs`'s own `.remove`
    /// calls immediately drop what they return), so a live caller can
    /// never observe `None` here.
    fn master(&self) -> &fs::File {
        self.master
            .as_ref()
            .expect("RunningTerminal.master is only taken during Drop::drop")
    }

    /// RFC-043 D1's own disjunction, response 342's required close of
    /// the gap response 341 left open: `termination::request_terminate`'s
    /// own use of the same early-close `Drop::drop` already applies --
    /// see that impl's comment for why taking this reference is always
    /// safe regardless of who else holds a duplicate, and effective only
    /// once every other duplicate is already gone. `pub(super)` for
    /// `termination.rs`, a sibling module, to call; not `master()`'s own
    /// caller's job, since only these two termination paths ever need to
    /// take this early rather than let it fall out naturally.
    pub(super) fn close_master(&mut self) {
        self.master.take();
    }
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
/// that drops `TerminalRuntime`/its `sessions` map without an
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
        //
        // RFC-043 D1's own disjunction, response 341's required half:
        // "close the PTY master." Taken (dropped) here, first -- a
        // custom `Drop::drop` body runs to completion before any of its
        // own fields auto-drop, so leaving this to Rust's own field-drop
        // at the end of this function would close it only *after* the
        // SIGHUP/enumerate/SIGKILL sequence below already ran, too late
        // to unblock anything during it (response 341's own measured
        // finding: a session leader blocked in `write(2)` to a
        // saturated pty never processes the pending `SIGHUP` until
        // something unblocks that write). Closing this reference alone
        // is always safe regardless of who else holds a duplicate --
        // `dup`/`try_clone`'d descriptors are independent numbers over
        // the same open file description, so closing one never disturbs
        // another live user of a different one. It only closes the
        // *last* reference, and so only actually triggers the pty's
        // hangup (`EIO` on the slave side), specifically because
        // `TerminalPane` (`crates/tekstide`) declares its own
        // `reader: TerminalReader` field before `runtime:
        // TerminalRuntime` -- by the time this drop runs as part of
        // that struct's own teardown, `TerminalReader::drop` has already
        // shut its thread down and joined it, closing that thread's own
        // `try_clone()`d duplicate of this same master first. **This is
        // not true of `termination::request_terminate`'s own explicit
        // close path** -- see that function's own doc comment for the
        // gap that leaves open there.
        if let Some(master) = self.master.take() {
            drop(master);
        }

        // **Load-bearing for a second, independent reason, reviews
        // 625-629: on macOS, a shell that has begun exiting (`P_WEXIT`
        // set in its own `p_flag`, measured directly via `sysctl`) can
        // wedge inside `proc_exit` for as long as its own PTY master end
        // stays open and unread.** Closing it here, before the SIGHUP and
        // the session sweep below, turns that into an immediate `EIO` on
        // the shell's own side rather than a silent, unbounded wait on
        // whoever is waiting for this session to empty. Do not reorder
        // this drop to after the signalling below for tidiness; on macOS
        // that reorder reintroduces the wedge, and only there, and only
        // sometimes.
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
    // Review 632's own required correction: `assert!`'s condition and
    // its format arguments are two independent calls, evaluated one
    // after the other -- a plain `assert!(session_confirmed_empty(id),
    // "...{:?}...", processes_in_session(id))` re-queries live process
    // state for the message *after* the condition already observed and
    // failed on a different reading of it, which can genuinely differ a
    // few milliseconds later (a leader finishing `proc_exit` in the
    // gap would print an empty survivor list under a failed assertion
    // reporting a non-empty one moments earlier). One observation, both
    // decisions, the same shape `BoundedTranscriptWriter::create`'s own
    // rule already uses (RFC-050 D3, review 388): open without
    // truncating, then decide the lock and the truncate from one
    // `fstat`, not two separate reads that could disagree.
    let survivors = processes_in_session(session_id);
    assert!(
        survivors.as_ref().is_some_and(|members| members.is_empty()),
        "RunningTerminal::drop's own containment sequence for session {session_id} finished, \
         but a real re-enumeration afterward still found: {survivors:?}. Possible causes, not \
         narrowed to one: a backgrounded job escaped containment \
         (rfcs/accepted/043-terminal-process-containment.md, the defect this guard exists to \
         catch); or a `/proc` read failed (session_confirmed_empty reports false for that too, \
         on purpose); or a real, live survivor this enumeration has not yet excluded for some \
         other reason. State what was observed here, not which of these it is -- that \
         determination belongs in whoever reads this failure, with the actual pids and their own \
         /proc/<pid>/stat in hand, not in this message."
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
#[cfg(target_os = "linux")]
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

/// RFC-028 D5, review 597/598/607: macOS has no `/proc` -- `proc_
/// listallpids` (`libproc`, declared directly in the vendored `libc`
/// crate's Apple module, no extra feature needed) is this platform's
/// own bulk enumeration. Bounded at [`MAX_ENUMERATED_PIDS`] rather than
/// sized dynamically: a two-call "ask for the size, then allocate
/// exactly that" pattern would still race a pid appearing between the
/// two calls, and this project's own standing lesson (an earlier,
/// genuinely unbounded `/proc` scan froze a shared desktop) is to bound
/// first and accept the rare, explicit truncation case rather than
/// reach for "big enough in practice." A buffer that comes back exactly
/// full is treated as truncated -- see [`list_all_pids`]'s own doc for
/// why that is reported as a failed enumeration, not a partial one.
///
/// **Review 598's own incident, and why this loop uses `?` per pid
/// rather than a `filter`:** the first attempt at this port treated
/// *any* `getsid` failure as "not a member," silently dropping a pid
/// from the result instead of failing the whole enumeration -- `getsid`
/// can fail with `EPERM` for a process in a different session (exactly
/// what a `setsid`-detached session leader is, from this process's own
/// point of view), and that single misread turned a live, running shell
/// into a falsely-"empty" session, which fed an unbounded `wait()`
/// downstream (since fixed independently in `termination.rs`) into a
/// five-hour hang on a real CI runner. `live_member_of_session` below
/// returns `Option<bool>` precisely so a failure this function cannot
/// explain propagates as `None` -- a failed *enumeration* -- rather
/// than a silent exclusion.
///
/// **Progress is printed to `stderr` as this runs, `#[cfg(test)]`
/// only** (compiled out of a shipped binary entirely, not merely
/// silent): a prior attempt's own failure mode was a hang with no
/// observable progress at all, caught only by a 5-hour timeout and
/// diagnosed afterward from `/proc/<pid>/task/*/wchan` on the hung
/// process -- information that does not exist once a CI job is
/// cancelled. If this attempt hangs too, the last line printed is where.
///
/// **Review 630's own required correction: the session leader's own pid
/// can be absent from `list_all_pids`'s own bulk output while it is
/// mid-`proc_exit`**, measured directly -- `proc_listallpids` omitted it
/// on 13 of 13 real firings, the same window `is_gone_or_going`'s own `sysctl`
/// fallback (above) was added for, not merely failing to confirm it once
/// visited but never presenting it as a candidate at all. A pid this
/// loop never visits cannot be added to `members` no matter what
/// [`live_member_of_session`] would have said about it. The session
/// leader's own pid is not a pid this function has to discover, though
/// -- it already *is* `session_id`, by construction (`setsid()` in
/// `pre_exec` makes the child's own pid the session id) -- so its
/// membership is never in question, only its existence and zombie
/// status, checked directly below when the bulk scan never visited it.
#[cfg(target_os = "macos")]
pub(super) fn processes_in_session(session_id: libc::pid_t) -> Option<Vec<libc::pid_t>> {
    if enumeration_forced_to_fail() {
        return None;
    }
    enumeration_progress("processes_in_session: calling list_all_pids");
    let pids = list_all_pids()?;
    enumeration_progress(&format!(
        "processes_in_session: list_all_pids returned {} pids, checking each",
        pids.len()
    ));
    let mut members = Vec::new();
    let mut session_leader_visited = false;
    for pid in pids {
        if pid == session_id {
            session_leader_visited = true;
        }
        enumeration_progress(&format!("processes_in_session: checking pid {pid}"));
        if live_member_of_session(pid, session_id)? {
            members.push(pid);
        }
    }
    if !session_leader_visited {
        enumeration_progress(&format!(
            "processes_in_session: session leader {session_id} absent from list_all_pids's own \
             bulk output -- checking it directly rather than treating absence as departure"
        ));
        // Review 633: one `sysctl` observation, not two independent ones
        // composed after the fact -- see [`SessionLeaderState`]'s own doc
        // comment for why the two-boolean version this replaced was
        // itself a race of the same shape `assert_session_is_empty`'s own
        // fix (below) was just needed for. Only `Alive` is a survivor to
        // contain, matching [`is_gone_or_going`]'s own `SZOMB`-or-`P_WEXIT`
        // exclusion for every other member, so the leader is not held to
        // a looser containment standard than anyone else in the session.
        let Some(leader_state) = session_leader_state(session_id) else {
            enumeration_progress(&format!(
                "processes_in_session: session_leader_state({session_id}) could not be \
                 confirmed, failing the whole enumeration rather than silently excluding it"
            ));
            return None;
        };
        if matches!(leader_state, SessionLeaderState::Alive) {
            members.push(session_id);
        }
    }
    enumeration_progress(&format!(
        "processes_in_session: done, {} member(s) found",
        members.len()
    ));
    Some(members)
}

/// The session leader's own existence and departure state, from **one**
/// measured instant, not two independent kernel reads composed after the
/// fact.
///
/// **Review 633's own required correction to review 632's own fix**: the
/// two-boolean version this replaced (`session_leader_still_alive` plus
/// `session_leader_demonstrably_leaving`, each its own separate `sysctl`
/// call) was two independent observations a process could cross
/// `P_WEXIT` *between* -- the same defect class as the `assert_session_is_
/// empty` message race fixed in the same round, now reappearing one layer
/// up in the code that fix produced. One observation, three states,
/// rather than two booleans whose conjunction every caller must assemble
/// correctly:
#[cfg(target_os = "macos")]
enum SessionLeaderState {
    /// Confirmed gone: `kill(pid, 0)` failed with `ESRCH`, or the
    /// kernel's own `p_stat` already reads `SZOMB`. Has finished leaving;
    /// not a survivor.
    Gone,
    /// Alive, and demonstrably mid-`proc_exit` (`P_WEXIT` set in its own
    /// `p_flag`): has **not** finished leaving -- its real exit status
    /// does not exist yet, so a caller deciding "may I stop waiting" must
    /// not treat this as `Gone` -- but it "holds no resources beyond a
    /// pid table slot and cannot execute code" (response 340's own rule
    /// for a zombie, one step earlier in the same story), so a caller
    /// deciding "is this a survivor to contain" must not treat it as
    /// `Alive` either.
    Leaving,
    /// Alive, and not demonstrably leaving: a survivor to contain.
    Alive,
}

/// The session leader's own pid is always a known quantity to this
/// module -- it *is* `session_id` -- so, unlike [`live_member_of_session`],
/// this never calls `getsid`: whether this exact pid belongs to its own
/// session is true by construction, not something to ask the kernel
/// again. What remains genuinely unknown when the bulk scan omits it is
/// existence and departure state, both answered without libproc:
/// `kill(pid, 0)` (existence, the same `proc_find`-independent check
/// [`is_gone_or_going`]'s own fallback reuses) and `p_stat`/`p_flag`,
/// read in the same [`p_stat_and_flag_via_sysctl`] call, together
/// deciding which [`SessionLeaderState`] applies.
///
/// **Review 632 found that "has this pid finished leaving" and "is this
/// pid a survivor to act on" are two different questions a single
/// predicate cannot answer at once** -- `termination.rs`'s own
/// `wait_for_session_outcome` needs the first (a leader with `P_WEXIT`
/// set has most emphatically *not* finished leaving, so treating it as
/// gone here would reintroduce the exact `OrphanedUnknown` defect
/// reviews 625-630 found and fixed), while [`processes_in_session`]'s own
/// containment decision needs the second (`P_WEXIT` excludes, matching
/// [`is_gone_or_going`]'s own rule for every other member). **Review
/// 633's own correction: both questions are answered from the single
/// [`SessionLeaderState`] this function returns, read from one
/// observation, rather than from two separately-read booleans** -- each
/// caller matches on the variants it cares about (`Gone` vs. the rest,
/// or `Alive` vs. the rest), so the two questions stay visibly distinct
/// in the type without needing two separate kernel reads that could
/// disagree with each other.
///
/// `getsid`'s own behaviour against a pid mid-`proc_exit` has not been
/// measured here, unlike `kill` and `sysctl` (review 628's own
/// `--nocapture` measurement confirmed both, 13 of 13); this function
/// deliberately never calls it, rather than assume it behaves like the
/// two mechanisms that were actually checked. Review 631's own argument
/// for why no probe is needed, not merely a shrug: `getsid` is either
/// never reached for a mid-exit pid at all (`live_member_of_session`
/// only runs over what `proc_listallpids` already returned, and a
/// mid-exit pid is exactly what that list omits), or, if a process
/// enters this window mid-scan, `getsid` failing `ESRCH` yields `Gone` --
/// treated as departed, which matches response 340's own deliberate
/// zombie exclusion (a process on its way out is not a survivor) rather
/// than contradicting it.
///
/// **This function is deliberately asymmetric with
/// [`live_member_of_session`], and that asymmetry is the point, not an
/// inconsistency to "unify" later**: only the session leader gets an
/// existence-authoritative check (`kill` + `sysctl`) bypassing libproc
/// entirely, because the leader is the one pid whose absence from the
/// bulk list is indistinguishable from departure -- it *is* `session_id`,
/// by construction, so there is no other pid in the scan that could ever
/// stand in for it. Every other member is still found (or not) purely
/// through the bulk scan plus [`live_member_of_session`]; adding the
/// same direct check there would mean trusting `kill(pid, 0)` alone to
/// decide survivorship for a pid this module did not already know by
/// invariant, which is a different and unjustified trust.
///
/// **Safe to add a `kill`-confirmed pid to `members` only because
/// `termination.rs`'s own `signal_candidates` re-verifies session
/// membership immediately before every signal**
/// (`if session_id_of(pid) != Some(session_id) { continue; }`) --
/// without that re-check, a leader that had since fully exited and had
/// its pid recycled by the kernel for an unrelated process would read
/// as `kill`-confirmed "alive," enter `members` here, and be fed into a
/// `SIGKILL` sweep aimed at a stranger. That re-verification already
/// existed for a different reason (the same race against any pid, not
/// only this one); this function's own safety now depends on it too,
/// which is why both are named together, here, rather than left for a
/// future reader to rediscover the coupling by accident.
///
/// `None` only when existence itself could not be confirmed (`kill`
/// failing with something other than `ESRCH`) -- propagated out of the
/// whole enumeration by callers, the same direction every other
/// unqueryable case in this file already takes, never silently read as
/// any one variant.
#[cfg(target_os = "macos")]
fn session_leader_state(pid: libc::pid_t) -> Option<SessionLeaderState> {
    // SAFETY: `kill` with signal `0` delivers nothing; it only asks the
    // kernel whether `pid` exists and is signalable, which is safe to
    // ask of any pid, including one that no longer exists.
    let kill_result = unsafe { libc::kill(pid, 0) };
    if kill_result == -1 {
        return match io::Error::last_os_error().raw_os_error() {
            Some(libc::ESRCH) => Some(SessionLeaderState::Gone),
            _ => {
                enumeration_progress(&format!(
                    "session_leader_state: kill({pid}, 0) failed unexpectedly, failing the \
                     whole enumeration rather than silently excluding this pid"
                ));
                None
            }
        };
    }
    match p_stat_and_flag_via_sysctl(pid) {
        Some((p_stat, p_flag)) => Some(if u32::from(p_stat) == libc::SZOMB {
            SessionLeaderState::Gone
        } else if p_flag & P_WEXIT != 0 {
            SessionLeaderState::Leaving
        } else {
            SessionLeaderState::Alive
        }),
        // `kill` already confirmed existence; an unqueryable record
        // stays counted as `Alive` rather than excluded, the same
        // direction `is_gone_or_going`'s own last-resort default already
        // takes.
        None => Some(SessionLeaderState::Alive),
    }
}

/// `termination.rs`'s own cross-platform entry point for
/// [`session_leader_state`] -- answers only "has the leader finished
/// leaving" (`Gone`), the question [`wait_for_session_outcome`] actually
/// needs: both `Alive` and `Leaving` mean "keep waiting," since a leader
/// mid-`proc_exit` has not finished leaving yet even though it is also
/// not a survivor to contain. An unconfirmable result (`None`) is treated
/// the same as "keep waiting" rather than as a reason to fail anything,
/// bounded by the caller's own timeout either way, since the alternative
/// is reporting a guess early, which is the defect this exists to stop.
///
/// A no-op on every other platform: the `OrphanedUnknown` race this
/// unblocks is specifically libproc's own `proc_find` refusing a pid
/// mid-`proc_exit` (reviews 625-630), which has no Linux equivalent --
/// `is_live_member_of_session` there reads `/proc/<pid>/stat` directly,
/// with no such window, so `wait_for_session_outcome`'s own pre-existing
/// behaviour already covers it correctly and must not change there.
#[cfg(target_os = "macos")]
pub(super) fn session_leader_known_to_still_exist(session_id: libc::pid_t) -> bool {
    !matches!(
        session_leader_state(session_id),
        Some(SessionLeaderState::Gone)
    )
}

#[cfg(not(target_os = "macos"))]
pub(super) fn session_leader_known_to_still_exist(_session_id: libc::pid_t) -> bool {
    false
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
#[cfg(target_os = "linux")]
fn is_live_member_of_session(pid: libc::pid_t, session_id: libc::pid_t) -> bool {
    let Some(stat) = process_stat(pid) else {
        return false;
    };
    stat.session_id == session_id && stat.state != 'Z'
}

/// Same property as the Linux version above, by two separate calls
/// rather than one `/proc/<pid>/stat` read: `getsid(2)` (POSIX, declared
/// for every `unix` target, not only Linux) gives the session id
/// directly, and `proc_pidinfo(..., PROC_PIDTBSDINFO, ...)` gives
/// `pbi_status`, compared against `SZOMB` for the same zombie exclusion
/// `is_live_member_of_session`'s own Linux doc explains.
///
/// **Review 598's required correction: `getsid` failing is not one
/// thing.** `ESRCH` means the pid has already exited and been reaped
/// between the enumeration and this check -- the same tolerance
/// `process_stat`'s `None` already gets on Linux, so `Some(false)` is
/// honest here. Any *other* failure (`EPERM`, most notably, which POSIX
/// permits when `pid` is in a different session -- precisely the shape
/// of a `setsid`-detached session leader from this process's own point
/// of view) means this pid's membership genuinely could not be
/// confirmed, and `None` must propagate out of the whole enumeration
/// rather than silently becoming "not a member." Read via `errno`
/// immediately after the call, before anything else can clobber it.
#[cfg(target_os = "macos")]
fn live_member_of_session(pid: libc::pid_t, session_id: libc::pid_t) -> Option<bool> {
    // SAFETY: `getsid` has no preconditions beyond `pid` being a plain
    // integer; it is safe to call against a pid that no longer exists.
    let real_session_id = unsafe { libc::getsid(pid) };
    if real_session_id == -1 {
        let error = io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(libc::ESRCH) => Some(false),
            _ => {
                enumeration_progress(&format!(
                    "live_member_of_session: getsid({pid}) failed with {error:?}, failing the \
                     whole enumeration rather than silently excluding this pid"
                ));
                None
            }
        };
    }
    if real_session_id != session_id {
        return Some(false);
    }
    Some(!is_gone_or_going(pid))
}

/// Offsets into `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_PID)`'s own
/// `struct kinfo_proc` reply, inside Darwin's `struct extern_proc`
/// (`p_un` timeval ∪ pointer-pair, 16 bytes; `p_vmspace`/`p_sigacts`, two
/// zeroed kernel pointers Apple's own sysctl redacts, 8 bytes each;
/// `p_flag`, 4 bytes; `p_stat`, 1 byte). **Measured, not read from a
/// header this project has no way to compile against**: review 627's own
/// `--nocapture` CI step dumped the plain child's first 64 bytes, and the
/// offsets were cross-checked from two independent fields before being
/// trusted here -- offset 32 decoded to `0x00004004` (`P_EXEC |
/// P_LP64`), exactly what a freshly `exec`'d 64-bit process carries, and
/// offset 40 decoded to that exact child's own real pid. Reused here,
/// production code, not only the `#[cfg(test)]` diagnostic that first
/// measured it (`report_child_status_at_timeout`), because review 630's
/// own fix needs the identical field this project already trusts enough
/// to report in CI.
#[cfg(target_os = "macos")]
const P_FLAG_OFFSET: usize = 32;
#[cfg(target_os = "macos")]
const P_STAT_OFFSET: usize = 36;
/// The same hex dump's own offset 40, decoded to the child's real pid
/// (review 627) -- never reused as a module constant until review 634's
/// own `KERN_PROC_SESSION` measurement needed to read which pid each
/// array record belongs to, unlike `P_FLAG_OFFSET`/`P_STAT_OFFSET`,
/// which production code already needed. Test-only scope reflects that:
/// nothing outside the measurement probe below reads it.
#[cfg(all(test, target_os = "macos"))]
const P_PID_OFFSET: usize = 40;
/// Darwin's own `P_WEXIT`: set in `p_flag` the moment a process has
/// called `exit` and begun `proc_exit`, before `p_stat` ever reaches
/// `SZOMB`. Reviews 628/629 measured it set and stable (three samples,
/// 100ms apart) on all 13 real firings of the stuck-shell cluster.
#[cfg(target_os = "macos")]
const P_WEXIT: u32 = 0x2000;

/// `p_stat` and `p_flag`, read through `sysctl(CTL_KERN, KERN_PROC,
/// KERN_PROC_PID)` rather than through libproc. `None` only when the
/// kernel itself says there is no such pid (a genuinely absent process)
/// or the reply itself is malformed/too short to reach either field --
/// treated as "unconfirmable," the same direction every other
/// unqueryable case in this file already takes, never as a silent "must
/// be fine."
///
/// **Why this call, and not `proc_pidinfo`, for review 630/632's own
/// fix**: `proc_pidinfo`/`proc_listallpids` both route through
/// `proc_find`, which refuses a process mid-`proc_exit` (`P_WEXIT` set
/// in its own `p_flag`) -- measured directly, 13 of 13 firings, `ESRCH`
/// from both flavours while `kill(pid, 0)` and this same `sysctl` call
/// both still answered, with `p_stat` reading `SRUN`, stable over three
/// samples 100ms apart. `sysctl` does not go through `proc_find`, so it
/// is the one mechanism proven to answer correctly in exactly the state
/// this fix exists for.
#[cfg(target_os = "macos")]
fn p_stat_and_flag_via_sysctl(pid: libc::pid_t) -> Option<(u8, u32)> {
    let mut mib: [libc::c_int; 4] = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
    let mut buffer = vec![0_u8; 4096];
    let mut written_len: libc::size_t = buffer.len();
    // SAFETY: `buffer` is valid and sized as declared for the duration
    // of the call; every read below is a plain, bounds-checked byte (or
    // byte-slice) index, never a typed/aligned access into the buffer.
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            buffer.as_mut_ptr().cast(),
            &raw mut written_len,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || written_len <= P_FLAG_OFFSET.max(P_STAT_OFFSET) {
        return None;
    }
    let p_flag = u32::from_le_bytes([
        buffer[P_FLAG_OFFSET],
        buffer[P_FLAG_OFFSET + 1],
        buffer[P_FLAG_OFFSET + 2],
        buffer[P_FLAG_OFFSET + 3],
    ]);
    Some((buffer[P_STAT_OFFSET], p_flag))
}

/// Review 636's own required measurement: at the exact moment
/// `a_real_backgrounded_job_is_dead_after_a_real_close` checks its own
/// `kill(pid, 0)`-based condition, what does the job's real `p_stat` say?
///
/// **Why `kill(pid, 0)` alone cannot settle this.** `kill` with signal
/// `0` succeeds against a zombie: the pid table entry still exists, and
/// the kernel simply discards the signal rather than delivering it to
/// anything. So that assertion cannot distinguish a real survivor (a
/// live backgrounded job that escaped containment) from a job that was
/// `SIGKILL`ed exactly as designed and is sitting unreaped for a few
/// more milliseconds because its own parent shell is already dead and
/// the reparenting reaper is behind under load.
///
/// **Review 637's own required correction: called in *both* branches,
/// not only the failing one**, and its own three readings, not the two
/// 636 first wrote:
///
/// 1. `SZOMB` -- a corpse. Nothing escaped; the test's own assertion is
///    the wrong instrument, the same confusion response 340 already
///    corrected in the production enumeration (*"it holds no resources
///    beyond a pid table slot and cannot execute code"*), not containment
///    failing.
/// 2. `SRUN`/`SSLEEP` with `P_WEXIT` clear -- a live survivor containment
///    missed. A real escape; the enumeration question reopens.
/// 3. `P_WEXIT` set -- wedged mid-`proc_exit`. For a *non-leader*,
///    `is_gone_or_going`'s own `sysctl` fallback arm already treats this
///    as "going" and excludes it from `members`, so a non-leader in this
///    state is never signalled at all -- the same hole review 634 first
///    raised and then set aside, now possibly reachable through the
///    fallback arm instead of the bulk path. Flagged loudest of the
///    three: this is not a test defect, it would be the 632 exclusion
///    converting a wedged leaver into a permanent survivor for anything
///    that is not the session leader.
///
/// On a *passing* run the job should be genuinely reaped -- `p_pid` no
/// longer exists at all, so this call returns `None` ("no record"),
/// which is itself the control that makes a `SZOMB` reading on a failing
/// run trustworthy rather than merely assumed about what `KERN_PROC_PID`
/// does to a corpse. A `None` is reported as "could not read," never
/// folded into one of the three readings above.
///
/// `#[cfg(test)]`-only, a no-op everywhere else, the same shape every
/// diagnostic in this file already takes.
#[cfg(all(test, target_os = "macos"))]
pub(super) fn report_pid_p_stat_via_sysctl(pid: libc::pid_t) {
    match p_stat_and_flag_via_sysctl(pid) {
        Some((p_stat, p_flag)) => {
            let meaning = match u32::from(p_stat) {
                libc::SIDL => "SIDL: being created",
                libc::SRUN => "SRUN: runnable",
                libc::SSLEEP => "SSLEEP: asleep, blocked on something",
                libc::SSTOP => "SSTOP: stopped",
                libc::SZOMB => "SZOMB: exited, unreaped -- kill(pid, 0) still succeeds on this",
                _ => "unrecognized",
            };
            eprintln!(
                "[p-stat-at-assertion] pid {pid}: p_stat={p_stat} ({meaning}), p_flag=0x{p_flag:x} \
                 (P_WEXIT {})",
                if p_flag & P_WEXIT != 0 {
                    "set"
                } else {
                    "not set"
                }
            );
        }
        None => {
            eprintln!(
                "[p-stat-at-assertion] pid {pid}: could not read -- no record (p_stat_and_flag_via_sysctl returned None)"
            );
        }
    }
}

#[cfg(all(test, not(target_os = "macos")))]
pub(super) fn report_pid_p_stat_via_sysctl(_pid: libc::pid_t) {}

/// `proc_pidinfo`'s own documented way to read a process's `p_stat`
/// field without `/proc` -- `result != size` covers both an outright
/// failure (the pid is already gone) and a short read, treated the same
/// way `process_stat`'s `None` is on Linux: "could not confirm this one
/// either way" is not "confirmed not a zombie." Deliberately the safe
/// direction regardless (review 598 confirmed this arm, not the one
/// that needed fixing): an unqueryable process stays counted as live
/// rather than excluded.
///
/// **Review 630's own required addition: when `proc_pidinfo` cannot
/// answer, ask `sysctl` before falling back to the weak default.**
/// `proc_pidinfo` failing is not only "the pid is already gone" --
/// review 628/629 measured it also meaning "the pid is mid-`proc_exit`,
/// which `proc_find` refuses to answer about at all, though the process
/// is alive." `sysctl` does not route through `proc_find`, so it can
/// resolve what `proc_pidinfo` could not.
///
/// **Review 632's own required correction: a process demonstrably
/// leaving is excluded too, not only one that has already finished
/// leaving.** `SZOMB` is response 340's own original rule; `P_WEXIT` set
/// is review 632's own extension of the identical reasoning one step
/// earlier in the same story -- a process inside `proc_exit` "holds no
/// resources beyond a pid table slot and cannot execute code" exactly as
/// response 340 already said of a zombie, it simply has not reached
/// `SZOMB` yet. **The distinction that keeps this from being a return to
/// the original defect: this excludes on evidence of departure, never on
/// absence of evidence.** The bug this project started from treated
/// "libproc cannot see it" as "it is gone" -- a presumption from
/// silence. This treats "the kernel affirmatively says it has begun
/// exiting" as "it is going" -- a reading from a call proven to answer in
/// exactly this window, 13 of 13, not a presumption at all. Only when
/// `sysctl` *also* cannot answer does this fall back to the original,
/// weaker "unqueryable, so presume alive" default.
///
/// **Review 639's own required note: this body and the Linux one below
/// share a name but not a semantic, and that is deliberate, not an
/// oversight to "complete" later.** This one excludes on `SZOMB` *or*
/// `P_WEXIT` because macOS has a real, measured, seconds-long window
/// where a process has begun exiting but has not yet become a corpse
/// (reviews 625-629, 13 of 13 real firings); Linux has no observed
/// analogue of that window (its own nearly-unobservable `X` state is not
/// treated as one -- padding this predicate to match would say something
/// untrue about what was actually measured, worse than the two bodies
/// looking asymmetric).
#[cfg(target_os = "macos")]
pub(crate) fn is_gone_or_going(pid: libc::pid_t) -> bool {
    // SAFETY: `info` is a plain, fully-zeroed `proc_bsdinfo` the call
    // writes into; `size` matches its own real size exactly, the
    // buffer `proc_pidinfo` is documented to require.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let result =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    if result == size {
        return info.pbi_status == libc::SZOMB;
    }
    match p_stat_and_flag_via_sysctl(pid) {
        Some((p_stat, p_flag)) => u32::from(p_stat) == libc::SZOMB || p_flag & P_WEXIT != 0,
        None => false,
    }
}

/// Linux's own counterpart to the macOS `is_gone_or_going` above, same
/// name and same question (is this pid a corpse, not a survivor),
/// mirroring [`session_id_of`]'s existing two-body-same-name pattern
/// rather than inventing a second name for one cross-platform caller to
/// branch on. Linux has no `proc_exit`-window equivalent to `P_WEXIT`
/// (reviews 625-630's own finding has no Linux side), so this is exactly
/// [`is_live_member_of_session`]'s own zombie check, read directly: a
/// zombie's real `/proc/<pid>/stat` state field really is `Z` (response
/// 340, confirmed directly, not inferred).
///
/// **Review 638's own required use**: `runtime::terminal::tests::
/// a_real_backgrounded_job_is_dead_after_a_real_close`'s own assertion
/// needed exactly this, cross-platform, to stop trusting `kill(pid, 0)`
/// alone -- `kill(pid, 0)` succeeds against an unreaped zombie on every
/// platform (`test_support::tests::
/// kill_signal_zero_succeeds_against_an_unreaped_zombie` proves it
/// deterministically, no runner needed), so a test built only on it
/// cannot tell a real survivor from a `SIGKILL`ed corpse sitting
/// unreaped for a moment under load.
///
/// `#[cfg(test)]`, unlike the macOS body above: that one has a real,
/// non-test caller (`live_member_of_session`), so it is built into every
/// shipped binary regardless; this one's only caller is itself a test,
/// so a plain (non-test) Linux build would otherwise flag it as dead
/// code.
///
/// **This body is `SZOMB`-only and deliberately not the macOS body's
/// equal**: it has no `P_WEXIT`-style exclusion because this platform
/// has no measured, observable "exiting but not yet a corpse" window to
/// exclude on -- only a real one, not a presumed absence of one, would
/// justify matching the other body's wider exclusion.
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn is_gone_or_going(pid: libc::pid_t) -> bool {
    process_stat(pid).is_some_and(|stat| stat.state == 'Z')
}

/// Bounded enumeration, not sized-to-fit -- see [`processes_in_session`]'s
/// own macOS doc for why. Chosen to match `launch.rs`'s own
/// `close_range_upper_bound`'s upper clamp, the closest existing
/// precedent in this file for "how many of something is implausibly
/// many, for a real machine, right now."
#[cfg(target_os = "macos")]
const MAX_ENUMERATED_PIDS: usize = 65536;

/// `None` only on an outright `proc_listallpids` failure (a negative
/// return) or on a reply that exactly fills the buffer -- the latter
/// cannot be distinguished here from "there were more pids than
/// `MAX_ENUMERATED_PIDS` and the rest were silently dropped," and
/// reporting a confidently-partial list would be exactly the unearned
/// confidence `what-containment-must-not-become.md` §4 forbids. Real
/// machines are nowhere near this bound; hitting it at all is itself
/// worth surfacing as a failure rather than quietly tolerating.
///
/// `pub(super)`, review 625: the libproc-visibility experiment in
/// `terminal::tests` (a plain, PTY-less child against a normal PTY
/// child, queried the same way) needs the identical enumeration this
/// file's own `processes_in_session` uses, not a second copy that could
/// drift from it.
#[cfg(target_os = "macos")]
pub(crate) fn list_all_pids() -> Option<Vec<libc::pid_t>> {
    let mut buffer: Vec<libc::pid_t> = vec![0; MAX_ENUMERATED_PIDS];
    let capacity_bytes = std::mem::size_of_val(buffer.as_slice()) as libc::c_int;
    // SAFETY: `buffer` is valid for `capacity_bytes` for the duration
    // of this call.
    let written_bytes =
        unsafe { libc::proc_listallpids(buffer.as_mut_ptr().cast(), capacity_bytes) };
    if written_bytes <= 0 || written_bytes >= capacity_bytes {
        return None;
    }
    let count = written_bytes as usize / std::mem::size_of::<libc::pid_t>();
    buffer.truncate(count);
    Some(buffer)
}

/// Review 598/607: a visible trail for exactly one failure mode -- this
/// code hanging on the real runner again, with no other way to tell
/// where. `#[cfg(test)]` only, so this is never reachable in a shipped
/// binary regardless of platform; within a test run it is a `stderr`
/// line, interleaved with whichever other tests are printing their own
/// output concurrently, which is still strictly more information than
/// the silence a cancelled, timed-out job gave the first time.
#[cfg(test)]
#[cfg(target_os = "macos")]
fn enumeration_progress(message: &str) {
    eprintln!("[containment-enumeration] {message}");
}

#[cfg(not(test))]
#[cfg(target_os = "macos")]
fn enumeration_progress(_message: &str) {}

/// Compiled out entirely in a non-test build -- the same "no indirection
/// at all outside `#[cfg(test)]`" shape [`proc_root`]'s own two bodies
/// already give the Linux side, not merely a runtime check that always
/// returns `false` in production.
#[cfg(not(test))]
#[cfg(target_os = "macos")]
fn enumeration_forced_to_fail() -> bool {
    false
}

#[cfg(test)]
#[cfg(target_os = "macos")]
fn enumeration_forced_to_fail() -> bool {
    TEST_FORCE_ENUMERATION_FAILURE.with(std::cell::Cell::get)
}

/// Review 608's own named probe for the last item in 596's triage, extended
/// by review 617's own required triangulation after the first run: whether
/// `wait_for_exit` timing out on a real macOS run means the child is
/// genuinely slow to exit, or is stopped (`SSTOP`) and will never be
/// reaped by `try_wait`, which passes no `WUNTRACED` -- a stopped child is
/// indistinguishable from a running one to that call, and returns
/// `Ok(None)` forever. Called from `termination.rs`'s platform-neutral
/// `wait_for_child_outcome`, right before it gives up and returns `None`,
/// so this is the exact moment the real runner's own symptom
/// (`the_wake_notifier_wakes_when_real_pty_output_arrives` and others) is
/// caught in the act rather than inferred after the fact.
///
/// **The first run's own result forced this extension, not a plan.** Every
/// one of 12 firings had `proc_pidinfo` answer `ESRCH` -- the pid does not
/// exist, by libproc's own account, while `try_wait` had just reported
/// neither exited nor errored. Both cannot be true of a healthy process,
/// and `is_gone_or_going` (below) rests on this exact call: if libproc genuinely
/// cannot see an unreaped child's own existence, `is_gone_or_going` can never
/// return `true` on macOS, which would make the containment enumeration
/// treat an unreaped child as a live session member -- a production
/// concern, not only a test one. Two more syscalls, independent of
/// libproc, triangulate which side the contradiction is on:
///
/// - `kill(pid, 0)`: the kernel's own existence/permission check, no
///   signal actually delivered. `0` means the pid exists; `ESRCH` means it
///   does not; `EPERM` means it exists and is not this process's own.
/// - `waitid(P_PID, pid, &info, WNOHANG | WNOWAIT | WEXITED | WSTOPPED)`:
///   what `try_wait` is really seeing, this time including a stopped child
///   (`try_wait` passes no `WUNTRACED`/`WSTOPPED` equivalent and cannot
///   report one) -- **`WNOWAIT` specifically**, so this answers the
///   question without reaping. A first version of this probe used
///   `waitpid(..., WNOHANG | WUNTRACED)` instead, which does reap on a
///   successful, non-zero return. Review 618's own required correction:
///   `RunningTerminal::drop` does not only `try_wait` its own child, it
///   first sweeps the **session** (`processes_in_session(session_id)`,
///   `session_id` derived from this same pid, `setsid()` in `pre_exec`
///   makes the child's pid the session id) and `SIGKILL`s every member --
///   reaping the pid here would free it for the kernel to recycle before
///   that destructor runs, and a recycled pid handed to
///   `processes_in_session` enumerates and kills an unrelated session.
///   Tolerating `try_wait`'s own already-reaped-child error (which is all
///   `drop`'s own comment actually establishes) says nothing about the pid
///   being safe to free for reuse, which is the failure mode that matters.
///   `WNOWAIT` is the fix: it still reports a stopped child via
///   `WSTOPPED`, which was the entire point, but leaves the child waitable
///   by whoever waits on it next.
///
/// **Review 622's own extension, for the 18-test cluster found once the
/// `/tmp` and git-gate classes stopped accounting for the residue**: a
/// `PROC_PIDT_SHORTBSDINFO` read into `libc::proc_bsdshortinfo`, carrying
/// `pbsi_comm` -- up to 16 characters of the real process name -- and its
/// own `pbsi_status`. One datum answers whether `spawn_pty_child` ever
/// became a shell at all (`pbsi_comm == "sh"`: it did, and the shell
/// itself is not reading or acting on its controlling terminal; anything
/// else, most plausibly this test binary's own name: `pre_exec` never
/// finished, between `fork` and `exec`). The less-privileged flavour,
/// not the full `PROC_PIDTBSDINFO` `is_gone_or_going` already uses above, on the
/// chance it answers where the full one does not -- the same
/// `comm == "sleep"` discriminator this project already used once, in
/// the fork/exec `CLOEXEC` work.
///
/// `#[cfg(test)]`-only, the same shape `enumeration_progress` already
/// uses, so this is never reachable in a shipped binary; a no-op on Linux
/// or in a non-test build, so the platform-neutral caller never needs its
/// own `cfg`. Reports the pairs; proposes nothing, per 617's own
/// instruction -- and reads, prints, and consumes nothing, the rule 618
/// restated as applying to the instruction as much as the implementation.
#[cfg(all(test, target_os = "macos"))]
pub(super) fn report_child_status_at_timeout(pid: u32) {
    let pid = pid as libc::pid_t;

    // SAFETY: `kill` with signal `0` delivers nothing; it only asks the
    // kernel whether `pid` exists and is signalable, which is safe to ask
    // of any pid, including one that no longer exists.
    let kill_result = unsafe { libc::kill(pid, 0) };
    let kill_errno = (kill_result == -1).then(io::Error::last_os_error);

    // SAFETY: same as `is_gone_or_going`'s own call -- `info` is a plain,
    // fully-zeroed `proc_bsdinfo` the call writes into, and `size`
    // matches its own real size exactly, the buffer `proc_pidinfo` is
    // documented to require.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let proc_pidinfo_result =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    let proc_pidinfo_label = if proc_pidinfo_result != size {
        format!("failed: {:?}", io::Error::last_os_error())
    } else {
        let status = info.pbi_status;
        let meaning = if status == libc::SSTOP {
            "SSTOP: stopped, not reaped by try_wait (it passes no WUNTRACED)"
        } else if status == libc::SZOMB {
            "SZOMB: a zombie try_wait should already have reaped"
        } else {
            "neither SSTOP nor SZOMB"
        };
        format!("pbi_status = {status} ({meaning})")
    };

    // Review 622's own named probe, for the 18-test cluster: `pbsi_comm`
    // (up to 16 characters of the real process name) decides between the
    // two suspects in one datum. `"sh"` means the child exec'd the shell
    // and the shell itself is not reading or acting on its controlling
    // terminal (`TIOCSCTTY` ordering, which fd ends up on 0/1/2, the
    // slave's own open flags); this test binary's own name means
    // `pre_exec` never finished and the child never became a shell at all
    // (`setsid`, `TIOCSCTTY`, the `dup2`s, or the `fcntl(F_SETFD,
    // FD_CLOEXEC)` loop over `close_range_upper_bound()`, all between
    // `fork` and `exec` in `spawn_pty_child`). `PROC_PIDT_SHORTBSDINFO`
    // (13) into `proc_bsdshortinfo`, not the full `PROC_PIDTBSDINFO`
    // flavour `is_gone_or_going` already uses above -- the short flavour is the
    // less-privileged one, so it may answer where the full one does not,
    // the same reasoning this project used for `comm == "sleep"` in the
    // fork/exec `CLOEXEC` work.
    let mut short_info: libc::proc_bsdshortinfo = unsafe { std::mem::zeroed() };
    let short_size = std::mem::size_of::<libc::proc_bsdshortinfo>() as libc::c_int;
    let proc_bsdshortinfo_result = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDT_SHORTBSDINFO,
            0,
            (&raw mut short_info).cast(),
            short_size,
        )
    };
    let proc_bsdshortinfo_label = if proc_bsdshortinfo_result != short_size {
        format!("failed: {:?}", io::Error::last_os_error())
    } else {
        // SAFETY: `pbsi_comm` is a fixed-size, zeroed-then-written buffer;
        // `proc_pidinfo` is documented to NUL-terminate it on success, the
        // same contract any `c_char` buffer from this API carries.
        let comm = unsafe { std::ffi::CStr::from_ptr(short_info.pbsi_comm.as_ptr()) };
        format!(
            "pbsi_comm={:?} pbsi_status={}",
            comm.to_string_lossy(),
            short_info.pbsi_status
        )
    };

    // SAFETY: `wait_info` is a plain, fully-zeroed `siginfo_t` the call
    // writes into on a real state change; `WNOWAIT` is the property this
    // entire call exists for -- see this function's own doc comment for
    // why a reap here, which plain `waitpid` would risk, is not
    // acceptable from a diagnostic.
    let mut wait_info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let options = libc::WNOHANG | libc::WNOWAIT | libc::WEXITED | libc::WSTOPPED;
    let waitid_result =
        unsafe { libc::waitid(libc::P_PID, pid as libc::id_t, &raw mut wait_info, options) };
    let waitid_label = if waitid_result == -1 {
        format!("failed: {:?}", io::Error::last_os_error())
    } else if wait_info.si_pid == 0 {
        "no state change available right now".to_owned()
    } else {
        // Darwin's own si_code values for a child (not named constants in
        // `libc`, the same shape `EILSEQ = 92` already took elsewhere in
        // this file): 1 CLD_EXITED, 2 CLD_KILLED, 3 CLD_DUMPED,
        // 4 CLD_TRAPPED, 5 CLD_STOPPED, 6 CLD_CONTINUED. `5` is the
        // hypothesis; nothing here was reaped either way.
        format!(
            "si_pid={}, si_code={}, si_status={} -- not reaped",
            wait_info.si_pid, wait_info.si_code, wait_info.si_status
        )
    };

    // Review 623's own correction to review 622's restatement of 619:
    // narrowing the probe to `pbsi_comm` alone dropped the pairing that
    // decides whether `proc_pidinfo` itself is sound in this environment,
    // or only this one pid is unreachable through it -- and 623's own run
    // found `proc_pidinfo` failing `ESRCH` on all 18 firings, so the
    // question is live, not hypothetical.
    //
    // 1. The same flavour, on this process's own pid. A pid this call
    //    must be able to read if the binding or its usage is sound at
    //    all, since it is asking about its own caller.
    // SAFETY: `self_info` is a plain, fully-zeroed `proc_bsdinfo` the
    // call writes into; `getpid()` has no preconditions and is always
    // valid for the duration of this call.
    let mut self_info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let self_pid = unsafe { libc::getpid() };
    let self_result = unsafe {
        libc::proc_pidinfo(
            self_pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&raw mut self_info).cast(),
            size,
        )
    };
    let self_check_label = if self_result == size {
        "ok".to_owned()
    } else {
        format!("failed: {:?}", io::Error::last_os_error())
    };

    // 2. Whether the child's own pid appears in `proc_listallpids`'s own
    //    enumeration -- the same call the containment sweep already
    //    relies on and already confirmed working on this runner (its own
    //    progress log prints pid counts). If the child is in the list but
    //    `proc_pidinfo` on it still fails, the failure is narrower than
    //    "libproc cannot see this child" at all.
    let in_pid_list = list_all_pids().map(|pids| pids.contains(&pid));
    let in_pid_list_label = match in_pid_list {
        Some(true) => "yes".to_owned(),
        Some(false) => "no".to_owned(),
        None => "list_all_pids itself failed".to_owned(),
    };

    // Review 627: `p_stat`/`p_flag`, read through `sysctl(CTL_KERN,
    // KERN_PROC, KERN_PROC_PID)` rather than through libproc -- the whole
    // point of this reconciliation is that `proc_pidinfo` above has
    // already failed `ESRCH` for this exact pid, so an independent kernel
    // view is the only way to learn whether the "exiting" bash from
    // review 627's own farewell finding is `SRUN`/`SSLEEP` (alive,
    // blocked on something) or `SZOMB` (exited, unreaped) -- and, review
    // 628's own falsifier for its `proc_exit` hypothesis, whether
    // `P_WEXIT` (`0x2000`) is set in `p_flag`. `P_FLAG_OFFSET`/
    // `P_STAT_OFFSET`/`P_WEXIT` are this file's own module-level
    // constants -- see their own doc comment, by `is_gone_or_going`, for how
    // they were measured.

    // Review 628: three samples, 100ms apart, not one -- a single
    // `SRUN` cannot distinguish a child genuinely spinning inside
    // `proc_exit` from one merely caught runnable by an instant's own
    // sampling luck.
    let mut samples = Vec::with_capacity(3);
    for sample_index in 0..3 {
        if sample_index > 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
        let mut mib: [libc::c_int; 4] = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
        let mut sysctl_buffer = vec![0_u8; 4096];
        let mut sysctl_written_len: libc::size_t = sysctl_buffer.len();
        // SAFETY: `sysctl_buffer` is valid and sized as declared for the
        // duration of the call; every read of it afterward is a plain
        // byte (or byte-slice) read, bounds-checked by indexing, never
        // a typed/aligned access into the buffer.
        let sysctl_result = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                sysctl_buffer.as_mut_ptr().cast(),
                &raw mut sysctl_written_len,
                std::ptr::null_mut(),
                0,
            )
        };
        let sample = if sysctl_result != 0 {
            format!("failed: {:?}", io::Error::last_os_error())
        } else if sysctl_written_len <= P_STAT_OFFSET {
            format!("record too short ({sysctl_written_len} bytes written)")
        } else {
            let p_stat = sysctl_buffer[P_STAT_OFFSET];
            let meaning = match u32::from(p_stat) {
                libc::SIDL => "SIDL: being created",
                libc::SRUN => "SRUN: runnable",
                libc::SSLEEP => "SSLEEP: asleep, blocked on something",
                libc::SSTOP => "SSTOP: stopped",
                libc::SZOMB => "SZOMB: exited, unreaped",
                _ => "unrecognized",
            };
            let p_flag = u32::from_le_bytes([
                sysctl_buffer[P_FLAG_OFFSET],
                sysctl_buffer[P_FLAG_OFFSET + 1],
                sysctl_buffer[P_FLAG_OFFSET + 2],
                sysctl_buffer[P_FLAG_OFFSET + 3],
            ]);
            format!(
                "p_stat={p_stat} ({meaning}), p_flag=0x{p_flag:x} (P_WEXIT {})",
                if p_flag & P_WEXIT != 0 {
                    "set"
                } else {
                    "not set"
                }
            )
        };
        samples.push(format!("t+{}ms: {sample}", sample_index * 100));
    }

    eprintln!(
        "[wait-for-exit-timeout] pid {pid}: kill(pid, 0) = {kill_result} (errno={kill_errno:?}); \
         proc_pidinfo: {proc_pidinfo_label}; proc_bsdshortinfo: {proc_bsdshortinfo_label}; \
         waitid(WNOHANG|WNOWAIT|WEXITED|WSTOPPED) = {waitid_label}; \
         proc_pidinfo(getpid()) = {self_check_label}; in proc_listallpids = {in_pid_list_label}; \
         sysctl(KERN_PROC_PID) samples: [{}]",
        samples.join("; ")
    );
}

#[cfg(not(all(test, target_os = "macos")))]
pub(super) fn report_child_status_at_timeout(_pid: u32) {}

/// Review 634's own named measurement: does `sysctl(CTL_KERN, KERN_PROC,
/// KERN_PROC_SESSION, session_id)` see a session member the bulk
/// `proc_listallpids` scan omitted?
///
/// **Why this question has no answer yet.** Reviews 625-633's own
/// recovery for the session *leader* works only because the leader's pid
/// is known a priori -- it *is* `session_id`, by construction -- so
/// [`session_leader_state`] can check it directly when the bulk scan
/// never visits it. A non-leader member (a backgrounded job, most
/// plausibly) has no such handle: if the bulk scan omits it, there is no
/// pid to check directly, because nothing else in this enumeration ever
/// learned it. `KERN_PROC_SESSION` is the kernel's own session-scoped
/// view, asked directly rather than assembled from `proc_listallpids`'s
/// bulk (and, per review 628, sometimes blind) one.
///
/// **One `sysctl` call for the array, one more to learn its own record
/// stride.** `kinfo_proc` is not declared anywhere in this project
/// (review 626's own deviation, adopted: a wrong hand-reconstructed
/// struct layout reads garbage silently, unlike a wrong constant), so
/// the only way to know how many bytes one array record occupies is to
/// ask for exactly one record and measure the reply:
/// `sysctl(..., KERN_PROC_PID, ...)` against this *test process's own*
/// pid (`getpid()`, always alive, always answerable, unlike the pid
/// under investigation) returns exactly one record, and its own
/// `written_len` *is* that record's stride. `KERN_PROC_SESSION`'s own
/// reply is documented as an array of the identical record type, so
/// dividing its own `written_len` by that measured stride gives the
/// element count without ever needing the struct's real shape.
///
/// Reports every record's `p_pid`/`p_stat`/`p_flag` and whether
/// `looked_for_pid` appears among them; proposes nothing, the same rule
/// every diagnostic in this file follows. `#[cfg(test)]`-only, a no-op
/// everywhere else, the same shape `report_child_status_at_timeout`
/// above already takes.
///
/// **Review 635's own two required corrections.**
///
/// 1. **A truncated reply must not read as `ABSENT`.** The same rule
///    [`list_all_pids`]'s own doc comment already states for the bulk
///    scan -- a reply that exactly fills the buffer cannot be
///    distinguished from "there were more records and the rest were
///    silently dropped" -- applies here with higher stakes, since this
///    probe's entire output is a single pid's `FOUND`/`ABSENT`. If
///    `session_len` reaches `session_buffer.len()`, this reports
///    "could not enumerate", never `ABSENT`, so a buffer that was too
///    small is never read as "the kernel does not see it either."
/// 2. **`label` distinguishes the control call from the one under
///    investigation.** Fired once against a pid known alive and known
///    in the session (before the close, where `FOUND` is the expected
///    answer and anything else indicts the instrument, not the
///    hypothesis) and once in the failure branch -- the same discipline
///    626's plain-child-vs-PTY-child control already used: measure the
///    instrument against a known positive before trusting its negative.
///    An `ABSENT` with no control beside it cannot tell "the mechanism
///    cannot see a mid-exit process either" apart from "the job
///    genuinely left the session," so this function no longer answers
///    that question alone -- both calls' own output must be read
///    together.
#[cfg(all(test, target_os = "macos"))]
pub(super) fn report_session_members_via_kern_proc_session(
    label: &str,
    session_id: libc::pid_t,
    looked_for_pid: libc::pid_t,
) {
    // SAFETY: `single_buffer` is valid and sized as declared for the
    // duration of the call; this process's own pid always exists for
    // the duration of its own call into itself.
    let self_pid = unsafe { libc::getpid() };
    let mut single_mib: [libc::c_int; 4] = [
        libc::CTL_KERN,
        libc::KERN_PROC,
        libc::KERN_PROC_PID,
        self_pid,
    ];
    let mut single_buffer = vec![0_u8; 4096];
    let mut single_len: libc::size_t = single_buffer.len();
    let single_result = unsafe {
        libc::sysctl(
            single_mib.as_mut_ptr(),
            single_mib.len() as libc::c_uint,
            single_buffer.as_mut_ptr().cast(),
            &raw mut single_len,
            std::ptr::null_mut(),
            0,
        )
    };
    if single_result != 0 || single_len == 0 {
        // Guards the division below too: `stride` is never 0 past this
        // point, so this probe fails loudly here rather than panicking
        // on a zero-length divisor later.
        eprintln!(
            "[kern-proc-session] {label}: KERN_PROC_PID({self_pid}) (this test process, to \
             learn the array's own record stride) failed: {:?} -- cannot interpret \
             KERN_PROC_SESSION without it",
            io::Error::last_os_error()
        );
        return;
    }
    let stride = single_len;

    let mut session_mib: [libc::c_int; 4] = [
        libc::CTL_KERN,
        libc::KERN_PROC,
        libc::KERN_PROC_SESSION,
        session_id,
    ];
    // 1024 records is far beyond any real test session's own membership;
    // sized against the measured stride rather than a fixed byte count,
    // since the stride itself is only known at runtime.
    // SAFETY: `session_buffer` is valid and sized as declared; every
    // read below is a plain, bounds-checked byte (or byte-slice) index,
    // never a typed/aligned access into the buffer.
    let mut session_buffer = vec![0_u8; stride.saturating_mul(1024)];
    let mut session_len: libc::size_t = session_buffer.len();
    let session_result = unsafe {
        libc::sysctl(
            session_mib.as_mut_ptr(),
            session_mib.len() as libc::c_uint,
            session_buffer.as_mut_ptr().cast(),
            &raw mut session_len,
            std::ptr::null_mut(),
            0,
        )
    };
    if session_result != 0 {
        eprintln!(
            "[kern-proc-session] {label}: sysctl(KERN_PROC_SESSION, {session_id}) failed: {:?}",
            io::Error::last_os_error()
        );
        return;
    }
    if session_len >= session_buffer.len() {
        // Review 635's required correction: a reply that exactly fills
        // the buffer cannot be told apart from a reply that was
        // silently truncated, the same reasoning `list_all_pids`'s own
        // doc comment states for the bulk scan. Reported distinctly
        // from `ABSENT` so a buffer that was too small is never read as
        // "the kernel does not see this pid either."
        eprintln!(
            "[kern-proc-session] {label}: could not enumerate -- the reply filled the entire \
             {}-byte buffer (stride={stride}), indistinguishable from a truncated one; looked_for_pid \
             {looked_for_pid} is UNKNOWN, not ABSENT",
            session_buffer.len()
        );
        return;
    }

    let record_count = session_len / stride;
    let mut found_looked_for_pid = false;
    let mut records = Vec::with_capacity(record_count);
    for index in 0..record_count {
        let base = index * stride;
        if base + P_PID_OFFSET + 4 > base + stride {
            records.push(format!("record {index}: too short to read p_pid"));
            continue;
        }
        let p_pid = i32::from_le_bytes([
            session_buffer[base + P_PID_OFFSET],
            session_buffer[base + P_PID_OFFSET + 1],
            session_buffer[base + P_PID_OFFSET + 2],
            session_buffer[base + P_PID_OFFSET + 3],
        ]);
        let p_stat = session_buffer[base + P_STAT_OFFSET];
        let p_flag = u32::from_le_bytes([
            session_buffer[base + P_FLAG_OFFSET],
            session_buffer[base + P_FLAG_OFFSET + 1],
            session_buffer[base + P_FLAG_OFFSET + 2],
            session_buffer[base + P_FLAG_OFFSET + 3],
        ]);
        if p_pid == looked_for_pid {
            found_looked_for_pid = true;
        }
        records.push(format!(
            "pid={p_pid} p_stat={p_stat} p_flag=0x{p_flag:x}{}",
            if p_pid == looked_for_pid {
                " <-- looked_for_pid"
            } else {
                ""
            }
        ));
    }

    eprintln!(
        "[kern-proc-session] {label}: session {session_id}: stride={stride} bytes, \
         written_len={session_len}, {record_count} record(s), looked_for_pid {looked_for_pid} \
         {}: [{}]",
        if found_looked_for_pid {
            "FOUND"
        } else {
            "ABSENT"
        },
        records.join(", ")
    );
}

// Unlike `report_child_status_at_timeout` above, this function's only
// caller is itself inside a `#[test]` fn, so the stub below is scoped to
// `cfg(test)` too, not left reachable from a plain (non-test) build on
// any platform -- that wider reachability is exactly what a plain `cargo
// clippy --lib` build on Linux flagged as dead code the first time this
// was written.
#[cfg(all(test, not(target_os = "macos")))]
pub(super) fn report_session_members_via_kern_proc_session(
    _label: &str,
    _session_id: libc::pid_t,
    _looked_for_pid: libc::pid_t,
) {
}

/// Review 625's own required datum, at the exact moment
/// `wait_for_child_outcome` is about to give up: **re-send the readiness
/// probe, right there, rather than guess from the 13-firing shape alone.**
/// Every surviving member of the former 18-cluster now does
/// `wait_for_shell_ready` (confirmed: `tek2` came back, so the shell was
/// reading and executing), then writes its own real command, then times
/// out with the child still alive -- which means the failure moved from
/// "the first write is discarded at startup" to "the shell executed one
/// command and did not act on the next." One more send of the exact same
/// probe, at the timeout, settles which half: `tek2` returning again means
/// the shell is still alive and reading, and the question becomes the
/// real command's own fate (did it ever reach the input queue, was it
/// echoed); `tek2` not returning means input stopped being honoured
/// sometime after the first command, and the suspect moves to what this
/// crate's own reads do to the terminal between two writes.
///
/// Deliberately reports the **raw captured bytes**, not a yes/no verdict
/// on whether the caller's own real command was echoed: this function has
/// no way to know what text any given caller wrote (`"exit 0\n"`,
/// `"exit 7\n"`, a `printf` marker, …), and guessing at one would be
/// exactly the unearned narrative `what-containment-must-not-become.md`
/// §4 forbids. The dump itself answers that question for whoever reads
/// it, for every call site, without this function needing to know which
/// one it is.
///
/// **Why this is safe against review 616/618's own rule** ("a diagnostic
/// must not be able to change the result it is diagnosing"): the result
/// being diagnosed is `wait_for_child_outcome`'s return value, which is
/// already decided (`Ok(None)`) by the time this runs -- nothing here
/// changes that. Writing an idempotent, side-effect-free probe command
/// is not the hazard 618 found: that hazard was specifically reaping the
/// child (freeing its pid for reuse before `RunningTerminal::drop`'s own
/// session-wide sweep, which derives the session id from that same
/// pid), and nothing here calls anything that reaps.
///
/// **But the rule is wider than reaping, review 626's own required
/// correction: a diagnostic must not consume state the subject under
/// test depends on, and reading a PTY is a consuming operation.**
/// `runtime::terminal::reader::tests::required_local_bounded_marks_
/// capture_failed_stops_reading_and_stalls_the_child_without_killing_it`
/// returns through this exact `Ok(None)` path *on purpose* -- its own
/// precondition is a child deliberately stalled on a `write(2)` into a
/// full PTY buffer, and this probe's own 250ms read would drain that
/// buffer and release it, destroying the state the test's own assertion
/// is about (and, in passing, could change the shape of the still-parked
/// `RequiredLocalBounded` 43-byte finding before anyone has looked at
/// it). So: **armed explicitly, not fired on every timeout** --
/// [`arm_pty_responsiveness_probe_for_test`] opts a specific test in, the
/// same `#[cfg(test)]` thread-local-guard shape
/// [`force_session_enumeration_failure_for_test`] already uses for the
/// identical reason (a default production path must not gain a new,
/// always-on side effect just because a handful of tests need to probe
/// it). A test that expects a timeout and keeps its own state unarmed
/// gets exactly that; an armed site gets the probe's answer. Every
/// fallible step here is still caught and reported, never `.expect`-ed
/// or propagated -- a failure in the probe itself is one more datum, not
/// a new way for this path to panic.
#[cfg(all(test, target_os = "macos"))]
pub(super) fn report_pty_responsiveness_at_timeout(
    runtime: &mut TerminalRuntime,
    handle: &TerminalRuntimeHandle,
) {
    if !pty_responsiveness_probe_armed() {
        return;
    }
    if let Err(error) = runtime.write_input(handle, b"echo tek$((1+1))\n") {
        eprintln!("[wait-for-exit-timeout] re-sent readiness probe: write failed: {error:?}");
        return;
    }
    match runtime.read_available_bounded_for(handle, Duration::from_millis(250), 4096) {
        Ok((chunk, _)) => {
            let tek2_returned = chunk.windows(4).any(|window| window == b"tek2");
            eprintln!(
                "[wait-for-exit-timeout] re-sent readiness probe: tek2 returned = \
                 {tek2_returned}; captured: {:?}",
                String::from_utf8_lossy(&chunk)
            );
        }
        Err(error) => {
            eprintln!("[wait-for-exit-timeout] re-sent readiness probe: read failed: {error:?}");
        }
    }
}

#[cfg(not(all(test, target_os = "macos")))]
pub(super) fn report_pty_responsiveness_at_timeout(
    _runtime: &mut TerminalRuntime,
    _handle: &TerminalRuntimeHandle,
) {
}

#[cfg(test)]
thread_local! {
    /// Default off, same reasoning as [`TEST_FORCE_ENUMERATION_FAILURE`]:
    /// the re-sent readiness probe is a consuming read on a PTY, which a
    /// handful of tests (reader-thread stall/backpressure tests
    /// specifically) depend on *not* happening at their own timeout. Not
    /// `target_os`-gated: the arming function and guard are plain
    /// `#[cfg(test)]` so every call site across the crate can bind the
    /// guard unconditionally, the same ergonomics `wait_for_shell_ready`
    /// already has; only [`report_pty_responsiveness_at_timeout`]'s own
    /// macOS-only real body ever reads the flag.
    static TEST_PTY_RESPONSIVENESS_PROBE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

// Only `report_pty_responsiveness_at_timeout`'s own macOS-only real body
// ever reads this; on every other platform/profile shape the thread-local
// above is written (by the arming guard, below) but never read, which is
// correct, not dead code left over -- the flag is still load-bearing
// there, for whichever platform a given CI run actually measures.
#[cfg(all(test, target_os = "macos"))]
fn pty_responsiveness_probe_armed() -> bool {
    TEST_PTY_RESPONSIVENESS_PROBE.with(std::cell::Cell::get)
}

/// Opts the calling test into [`report_pty_responsiveness_at_timeout`]'s
/// real probe for the guard's lifetime. Bind the returned guard to a
/// local that lives for the whole test -- dropping it early re-disarms
/// the probe before the timeout this test is waiting for ever fires.
#[cfg(test)]
pub(crate) fn arm_pty_responsiveness_probe_for_test() -> PtyResponsivenessProbeGuard {
    TEST_PTY_RESPONSIVENESS_PROBE.with(|cell| cell.set(true));
    PtyResponsivenessProbeGuard { _private: () }
}

#[cfg(test)]
pub(crate) struct PtyResponsivenessProbeGuard {
    _private: (),
}

#[cfg(test)]
impl Drop for PtyResponsivenessProbeGuard {
    fn drop(&mut self) {
        TEST_PTY_RESPONSIVENESS_PROBE.with(|cell| cell.set(false));
    }
}

/// Review 623's own fix for the PTY-startup race review 622 first found as
/// an 18-test cluster, then partly retracted: every member of that cluster
/// writes to a just-launched shell's PTY and expects it to act on that
/// input immediately. `/bin/sh` is `bash` on macOS, which reconfigures its
/// own terminal modes at startup; a terminal-mode change can discard input
/// the kernel's line discipline has *already echoed* -- which is exactly
/// why these tests see their own marker and conclude the command ran: the
/// echo is indistinguishable from real output to any assertion whose
/// marker also appears in the written command itself. Row 8's own register
/// entry (`test-process-leak.md`) describes the identical shape from a
/// Linux flake, which is why this helper is not `target_os`-gated: one
/// mechanism, two platforms, different window widths.
///
/// Writes a command whose answer the echo cannot produce --
/// `echo tek$((1+1))` answers `tek2` only once a real shell evaluates the
/// arithmetic expansion, where the echo of the command itself contains the
/// literal text `tek$((1+1))` -- and waits for that answer before
/// returning. Callers write their own, real input only after this
/// returns, which is the choke-point this project already applies to
/// process creation, for the same reason: a guard placed at only the
/// sites that happened to be seen failing always misses the ones that
/// didn't fail in that particular run.
///
/// **Review 624's own required correction: the probe is re-sent every
/// iteration, not written once.** A single write is itself a write
/// landing before the shell has set its terminal modes -- exactly the
/// race this helper exists to detect -- so a first version that wrote
/// once could be eaten by the same race and panic after a flat timeout,
/// turning 18 informative assertion failures into 18 uninformative
/// handshake panics and adding the full timeout to every run that hit
/// it. `echo` is idempotent and side-effect-free, so resending it costs
/// nothing and the first send that survives ends the loop.
///
/// **The criterion for where this belongs, from review 624:** a test
/// that writes after launch and expects the write to be *executed*
/// should call this first. It must **not** go where the test's own
/// property under test is silence, an exact byte count, or reader-thread
/// timing -- this helper's whole purpose is to guarantee output has
/// already arrived, which is the negation of what such a test asserts.
/// `runtime::terminal::reader::tests` is the one call site that needed
/// this judged test by test rather than applied file-wide: its wake
/// notifier test needed the handshake (it expects execution), its other
/// ten `spawn_output_reader` sites do not (several assert that *no*
/// output has arrived yet).
///
/// The probe command, marker and timeout are shared with `tekstide`'s own
/// `shell::tests::wait_for_shell_ready` (a separate copy, since that one
/// drives `TerminalPane`, not `TerminalRuntime`, and nothing reaches
/// across the crate boundary here) via `crate::test_support`'s own `pub`
/// constants, so "what readiness means" has one definition.
///
/// `#[cfg(test)]`, not `pub(super)`: every call site is in a different
/// test module across this crate (`agent::tests`, `audit::tests::
/// integration`, `transcript::reader::tests`, `runtime::terminal::reader::
/// tests`, `runtime::terminal::tests`), so this needs the same crate-wide
/// test-only reach `super::tests` already has, re-exported by `mod.rs`.
#[cfg(test)]
pub(crate) fn wait_for_shell_ready(runtime: &mut TerminalRuntime, handle: &TerminalRuntimeHandle) {
    use crate::test_support::{
        SHELL_READINESS_MARKER, SHELL_READINESS_PROBE_COMMAND, SHELL_READINESS_TIMEOUT,
    };

    let started = Instant::now();
    let mut output = Vec::new();
    while started.elapsed() < SHELL_READINESS_TIMEOUT {
        runtime
            .write_input(handle, SHELL_READINESS_PROBE_COMMAND)
            .expect("readiness probe should write to PTY");
        let (chunk, _) = runtime
            .read_available_bounded_for(handle, Duration::from_millis(250), 16 * 1024)
            .expect("readiness probe read should succeed");
        output.extend_from_slice(&chunk);
        if output
            .windows(SHELL_READINESS_MARKER.len())
            .any(|window| window == SHELL_READINESS_MARKER)
        {
            return;
        }
    }
    panic!(
        "shell did not become ready before the readiness deadline; captured: {}",
        String::from_utf8_lossy(&output)
    );
}

/// Review 625-629's own remedy, generalising the measurement that proved
/// it: on macOS, a shell that has begun exiting can wedge inside
/// `proc_exit` for as long as its own PTY master end stays open and
/// unread (`P_WEXIT` set, `p_stat` still `SRUN`, stable over repeated
/// samples -- see `request_terminate`'s and `RunningTerminal::drop`'s own
/// doc comments for the production-side reasoning this mirrors).
/// `project_session_launches_agent_run_through_terminal_runtime_and_
/// completes` proved, as a one-test throwaway measurement, that spawning
/// the output reader before the exit command goes in is sufficient; this
/// is that same fix, generalised into one helper so the other eleven
/// armed sites use the identical mechanism rather than eleven copies that
/// could drift from it or from the worked example that proved it.
///
/// Reuses `spawn_output_reader`'s own background thread -- already this
/// crate's established draining mechanism (the wake notifier and both
/// `transcript::reader` tests the handshake itself fixed all drain this
/// way) -- rather than building a second, parallel drain loop that reads
/// the master directly. Nothing is done with the reader's own output;
/// the mechanism under test/remedy is the background thread's continuous
/// `read()` keeping the master's kernel buffer from filling, not
/// anything a caller does with the bytes it carries.
///
/// A drop-in replacement for `runtime.wait_for_exit(handle, timeout)`, not
/// a new API shape: same arguments, same return type, so a call site
/// changes only its own function name.
#[cfg(test)]
pub(crate) fn wait_for_exit_draining(
    runtime: &mut TerminalRuntime,
    handle: &TerminalRuntimeHandle,
    timeout: Duration,
) -> Result<Option<TerminationOutcome>, TerminalRuntimeError> {
    let _draining_reader = runtime.spawn_output_reader(handle)?;
    runtime.wait_for_exit(handle, timeout)
}

#[cfg(test)]
#[cfg(target_os = "macos")]
thread_local! {
    /// Set only by [`force_session_enumeration_failure_for_test`] --
    /// macOS's own analogue of [`test_proc_root`]'s directory override,
    /// needed because there is no path here to point enumeration at
    /// instead; this flag short-circuits the real `libproc` calls the
    /// same deterministic way.
    ///
    /// **Thread-local, not a global `AtomicBool`, deliberately**: this
    /// suite runs tests concurrently in one process, and a global flag
    /// would let one test's forced failure corrupt every other test's
    /// enumeration running at the same time -- the intermittent kind of
    /// failure this project has already spent a week proving is the
    /// expensive kind to diagnose. The corollary is also deliberate: if
    /// `processes_in_session` is ever called from a different thread
    /// than the one holding the guard (a future refactor moving this
    /// call off the test's own thread, for example), the override stops
    /// applying and this test's assertion *fails* rather than silently
    /// stopping coverage -- the right failure direction, not a silent gap.
    static TEST_FORCE_ENUMERATION_FAILURE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(not(test))]
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
fn proc_root() -> std::path::PathBuf {
    TEST_PROC_ROOT
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| std::path::PathBuf::from("/proc"))
}

#[cfg(test)]
#[cfg(target_os = "linux")]
thread_local! {
    static TEST_PROC_ROOT: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// RAII guard for [`test_proc_root`] -- restores the real `/proc` on
/// drop.
#[cfg(test)]
#[cfg(target_os = "linux")]
pub(super) struct TestProcRootGuard {
    _private: (),
}

#[cfg(test)]
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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

/// RFC-028 D5: the platform-neutral guard RFC-043 D3's own required
/// negative test (`session_confirmed_empty_reports_false_when_its_own_
/// enumeration_fails`) uses, so that one test exercises the real
/// enumeration-failure path on whichever platform it runs on rather
/// than being written against `/proc` specifically. Linux: points
/// [`test_proc_root`] at a path that was never created (`ENOENT` is as
/// deterministic as "exists with no read permission," and needs no
/// cleanup). macOS: sets [`TEST_FORCE_ENUMERATION_FAILURE`] directly,
/// since there is no path to point a syscall-based enumeration at
/// instead.
#[cfg(test)]
pub(super) fn force_session_enumeration_failure_for_test() -> ForcedEnumerationFailureGuard {
    #[cfg(target_os = "linux")]
    {
        let never_created = std::env::temp_dir().join(format!(
            "tekstide-forced-proc-enumeration-failure-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ));
        ForcedEnumerationFailureGuard {
            _proc_root_guard: test_proc_root(&never_created),
        }
    }
    #[cfg(target_os = "macos")]
    {
        TEST_FORCE_ENUMERATION_FAILURE.with(|cell| cell.set(true));
        ForcedEnumerationFailureGuard { _private: () }
    }
}

#[cfg(test)]
pub(super) struct ForcedEnumerationFailureGuard {
    #[cfg(target_os = "linux")]
    _proc_root_guard: TestProcRootGuard,
    #[cfg(target_os = "macos")]
    _private: (),
}

#[cfg(test)]
#[cfg(target_os = "macos")]
impl Drop for ForcedEnumerationFailureGuard {
    fn drop(&mut self) {
        TEST_FORCE_ENUMERATION_FAILURE.with(|cell| cell.set(false));
    }
}

/// `/proc/<pid>/stat`'s own format: `pid (comm) state ppid pgrp session
/// ...`, space-separated after the `)` that closes `comm` -- `comm`
/// itself can contain spaces or even `)`, so this splits on the *last*
/// `)` in the line rather than the first space, the standard way to
/// parse this file safely. `state` is field 0 of what remains; `ppid`,
/// `pgrp`, `session` are 1-3.
#[cfg(target_os = "linux")]
struct ProcessStat {
    state: char,
    session_id: libc::pid_t,
}

#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
pub(super) fn session_id_of(pid: libc::pid_t) -> Option<libc::pid_t> {
    process_stat(pid).map(|stat| stat.session_id)
}

/// Same contract as the Linux version above, directly from `getsid(2)`
/// -- no `proc_pidinfo` call needed here at all, since this one is
/// deliberately not zombie-aware either.
#[cfg(target_os = "macos")]
pub(super) fn session_id_of(pid: libc::pid_t) -> Option<libc::pid_t> {
    // SAFETY: `getsid` has no preconditions beyond `pid` being a plain
    // integer; it is safe to call against a pid that no longer exists.
    let session_id = unsafe { libc::getsid(pid) };
    (session_id != -1).then_some(session_id)
}

/// Creates the transcript writer for a launch, or decides the launch goes
/// ahead without one. **Shared by `launch_project_shell` and
/// `launch_project_adapter`** (response 390): this decision needed the same
/// fix at both sites once, and a test through only one of them left the other
/// unguarded. The rest of each launch function stays duplicated, for
/// RFC-022's reason.
///
/// Returns the writer, its capture mode, and whether capture was skipped
/// because the file was locked — the last is what lets the session record why
/// a run has no transcript.
///
/// RFC-050 D3: for `LocalBounded`, a transcript file another handle has locked
/// is not a reason to refuse the launch — the run is unrecorded, not more
/// dangerous — and it is never a reason to write unlocked, so the process
/// starts without capture. **`RequiredLocalBounded` refuses** (response 388):
/// RFC-011 says that mode rejects a launch whose capture cannot be prepared.
/// Every other writer failure refuses in every mode.
fn prepare_transcript_writer(
    spec: &TerminalLaunchSpec,
) -> Result<
    (
        Option<BoundedTranscriptWriter>,
        Option<TranscriptCaptureMode>,
        bool,
    ),
    TerminalLaunchError,
> {
    let Some(config) = spec.transcript_writer_config() else {
        return Ok((None, None, false));
    };
    let mode = config.mode;
    match BoundedTranscriptWriter::create(config.clone()) {
        Ok(writer) => Ok((Some(writer), Some(mode), false)),
        Err(error)
            if error.reason == crate::transcript::TranscriptWriteErrorReason::LockUnavailable
                && !mode.rejects_launch_when_unavailable() =>
        {
            Ok((None, None, true))
        }
        Err(error) => Err(TerminalLaunchError::TranscriptWriterUnavailable {
            summary: transcript_write_error_summary(&error),
        }),
    }
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

    // RFC-028 D5, review 589: computed here, in the parent, and moved
    // into `pre_exec` below -- `sysconf` is not on POSIX's async-signal-
    // safe list, so it cannot be called inside the post-`fork`, pre-
    // `exec` child the closure runs in, only before the fork happens.
    let close_from_bound = close_range_upper_bound();

    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            // RFC-028 D5: `ioctl`'s own `request` parameter is `c_ulong`
            // on macOS against `TIOCSCTTY`'s own `c_uint` there -- a
            // mismatch Linux never has (both are the same `Ioctl` alias
            // there), so widening unconditionally would make clippy's
            // own `useless_conversion` lint fail the Linux build.
            #[cfg(target_os = "macos")]
            let ctty_request = u64::from(libc::TIOCSCTTY);
            #[cfg(target_os = "linux")]
            let ctty_request = libc::TIOCSCTTY;
            if libc::ioctl(ctty_fd, ctty_request, 0) == -1 {
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
            //
            // RFC-028 D5, review 589: a bounded loop, not
            // `SYS_close_range`/`closefrom` -- neither exists on macOS
            // (`closefrom` is a BSD-ism Apple never adopted, absent from
            // the vendored `libc` for Apple targets; checked directly,
            // not assumed), and `proc_pidinfo`-based emulation is not
            // async-signal-safe, so it is not a legal substitute inside
            // this closure regardless of availability. `fcntl` itself
            // *is* on POSIX's async-signal-safe list, so a loop is legal
            // where almost nothing else is. The bound (`close_from_
            // bound`, above) was computed in the parent precisely so this
            // loop never needs to call anything that is not on that
            // list. Cheaper than it sounds in practice -- macOS's default
            // soft `RLIMIT_NOFILE` is commonly 256 -- but still only a
            // bonus layer over the fix that already closed the known
            // hole (`OpenPty::new`'s own `O_CLOEXEC`), not something this
            // spawn may fail over, the same framing this mechanism has
            // always had.
            //
            // **`F_SETFD`/`FD_CLOEXEC`, not `close(2)` -- review 593's
            // follow-up, and a real, confirmed regression, not a style
            // change.** This loop used `close(2)` from review 589 until
            // now. `Command::spawn()` (the `pre_exec` path) creates its
            // own internal pipe before `fork` to report a genuine
            // `exec()` failure back to the parent with the real `errno`;
            // closing that pipe's write end early -- its fd is ordinary,
            // not reserved, and sits inside `3..close_from_bound` same as
            // anything else -- makes the parent see `EOF`, std's own
            // "`exec` succeeded" signal, before `exec` was ever
            // attempted. **Confirmed directly, by the reviewer, after an
            // earlier version of this comment cited a probe that could
            // not show it**: a file marked executable with a `#!`
            // shebang naming an interpreter that does not exist fails
            // `exec` with `ENOENT` (unlike a missing shebang entirely,
            // which fails with `ENOEXEC` -- glibc retries *that* case
            // via `/bin/sh -c`, running the file as a script and masking
            // the defect regardless of which loop body is used, which is
            // why an earlier probe built against `ENOEXEC` was
            // meaningless evidence either way). Against the real
            // `ENOENT` case: `close(2)` here reports `spawn() -> Ok`
            // followed by the child dying on `SIGABRT` with no
            // diagnostic attached (std's own child-side code aborts on
            // the errno pipe it needed being gone); `fcntl` here reports
            // `spawn() -> Err(ENOENT)` correctly, immediately, to the
            // real caller. See
            // `launching_a_shell_with_a_nonexistent_interpreter_reports_
            // failure_not_a_dead_child` for the regression test this
            // produced. Marking the fd close-
            // on-exec instead of closing it outright keeps the identical
            // property for every *other* descriptor (nothing survives a
            // *successful* `exec`) without touching what happens on a
            // *failed* one: std's own pipe is already `CLOEXEC` for
            // exactly that reason, so this loop is a no-op against it
            // either way, and a failed `exec` can still write the real
            // `errno` through it.
            for fd in 3..close_from_bound {
                libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }
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

/// RFC-028 D5, review 589: `sysconf(_SC_OPEN_MAX)`, called here in the
/// parent -- `sysconf` is not async-signal-safe, so `spawn_pty_child`'s
/// own `pre_exec` closure (running post-`fork`, pre-`exec`) must never
/// call it directly; this value is computed before the fork and moved
/// in instead. Clamped on both ends: `<= 0` covers `sysconf`'s own error
/// return and the (rare) "no defined limit" case alike, since either one
/// becoming an unbounded loop is worse than a conservative fallback; the
/// upper clamp keeps a pathological limit from turning a spawn into a
/// long, visible pause, which is the one thing this bonus layer must
/// never cost.
fn close_range_upper_bound() -> libc::c_int {
    const FALLBACK: libc::c_int = 4096;
    const CLAMP: libc::c_int = 65536;
    // SAFETY: `sysconf` takes a plain `c_int` name and has no
    // preconditions beyond that; it cannot fail in a way that corrupts
    // anything, only in a way that returns -1.
    let limit = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
    if limit <= 0 {
        FALLBACK
    } else {
        libc::c_int::try_from(limit).unwrap_or(CLAMP).min(CLAMP)
    }
}
