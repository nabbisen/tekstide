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
        // Same `<= 1` refusal `send_signal_to_process_group` already
        // applies before signalling -- group 0 means "this process's own
        // group" (never correct to target here) and group 1 is not a
        // real terminal's group on this platform; a `RunningTerminal`
        // should never be constructed with either, but a destructor is
        // exactly the place to not trust that invariant blindly.
        if self.process_group_id > 1 {
            unsafe {
                libc::kill(-self.process_group_id, libc::SIGKILL);
            }
        }
        // Reaps this value's own direct child specifically, so it does
        // not linger as a zombie -- `SIGKILL` cannot be caught, ignored,
        // or blocked, so the wait below returns essentially immediately
        // once the kernel delivers it, not a genuine block. Harmless
        // (`Err`, discarded) if the child was already reaped by an
        // earlier `try_wait`/`wait` call before this value ever reached
        // `Drop`.
        let _ = self.child.wait();

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
        // **No containment yet.** This only observes and reports; PR-B
        // is what acts on what this finds. Expected, by design, to turn
        // tests red the moment this lands -- that is the inventory this
        // slice exists to produce, not a regression in this commit.
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
    let survivors = processes_in_session(session_id);
    assert!(
        survivors.is_empty(),
        "RunningTerminal::drop killed process group {session_id}, but session {session_id} \
         still has live process(es) after the kill and wait above: {survivors:?} -- a \
         backgrounded job inside this terminal escaped, exactly the defect \
         rfcs/accepted/043-terminal-process-containment.md exists to fix. This is the \
         reproduction PR-043-A exists to turn red, not a false alarm to silence."
    );
}

/// Every live pid whose `/proc/<pid>/stat` session field equals
/// `session_id` -- read-only, no signal sent, safe to call
/// unconditionally. `/proc` entries that vanish or become unreadable
/// mid-scan (a process exiting during the read) are skipped rather than
/// erroring, the same tolerance `process_group_exists_by_id` already
/// gives a process that may no longer exist by the time it's checked.
#[cfg(any(test, feature = "test-support"))]
fn processes_in_session(session_id: libc::pid_t) -> Vec<libc::pid_t> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<libc::pid_t>().ok())
        .filter(|&pid| session_id_of(pid) == Some(session_id))
        .collect()
}

/// `/proc/<pid>/stat`'s own format: `pid (comm) state ppid pgrp session
/// ...`, space-separated after the `)` that closes `comm` -- `comm`
/// itself can contain spaces or even `)`, so this splits on the *last*
/// `)` in the line rather than the first space, the standard way to
/// parse this file safely. `state`, `ppid`, `pgrp`, `session` are then
/// fields 0-3 of what remains.
#[cfg(any(test, feature = "test-support"))]
fn session_id_of(pid: libc::pid_t) -> Option<libc::pid_t> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    after_comm.split_whitespace().nth(3)?.parse().ok()
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
