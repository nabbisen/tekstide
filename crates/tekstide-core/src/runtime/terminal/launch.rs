use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::domain::{TerminalId, TerminalKind, TerminalSession, TerminalStatus};
use crate::project::{ProjectId, ProjectSession};

use super::pty::{OpenPty, close_fd, resize_master};
use super::{
    BoundedRuntimeSummary, TerminalDimensions, TerminalLaunchSpec, TerminalOutputSummary,
    TerminalRuntimeEvent, TerminalRuntimeHandle,
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
        Ok((
            output,
            TerminalRuntimeEvent::OutputBuffered {
                handle: handle.clone(),
                summary,
            },
        ))
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

#[derive(Debug, Eq, PartialEq)]
pub enum TerminalLaunchError {
    CrossProject,
    UnsupportedTerminalKind,
    MissingProjectRoot { summary: BoundedRuntimeSummary },
    InvalidCwd { summary: BoundedRuntimeSummary },
    CwdEscapesProjectRoot { summary: BoundedRuntimeSummary },
    ShellUnavailable { summary: BoundedRuntimeSummary },
    PtyUnavailable { summary: BoundedRuntimeSummary },
    SpawnFailed { summary: BoundedRuntimeSummary },
    UnexpectedLifecycleTransition { summary: BoundedRuntimeSummary },
}

#[derive(Debug, Eq, PartialEq)]
pub enum TerminalRuntimeError {
    UnknownTerminal { terminal_id: TerminalId },
    CrossProjectHandle { terminal_id: TerminalId },
    Io { summary: BoundedRuntimeSummary },
}

pub(super) struct RunningTerminal {
    pub(super) project_id: ProjectId,
    pub(super) process_group_id: libc::pid_t,
    pub(super) child: Child,
    pub(super) master: fs::File,
}

fn validate_launch_spec(
    project: &ProjectSession,
    spec: &TerminalLaunchSpec,
) -> Result<(), TerminalLaunchError> {
    if project.id() != &spec.project_id {
        return Err(TerminalLaunchError::CrossProject);
    }
    if spec.kind != TerminalKind::Plain {
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

    Ok(())
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

    let mut command = Command::new(&spec.shell);
    command
        .current_dir(&spec.cwd)
        .env_clear()
        .env("TERM", "xterm-256color")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("PATH", "/usr/bin:/bin")
        .env("PS1", "tekstide$ ")
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
            Ok(())
        });
    }

    let spawn_result = command.spawn();
    close_fd(ctty_fd);
    let child = spawn_result.map_err(|error| TerminalLaunchError::SpawnFailed {
        summary: BoundedRuntimeSummary::new(format!("failed to spawn PTY shell: {error}")),
    })?;
    pty.close_slave();

    Ok(child)
}
