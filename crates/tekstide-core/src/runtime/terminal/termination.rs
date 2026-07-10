use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use super::{
    BoundedRuntimeSummary, LinuxTerminalRuntime, TerminalRuntimeError, TerminalRuntimeEvent,
    TerminalRuntimeHandle, TerminationOutcome, TerminationRequest, TerminationSignal,
};

impl LinuxTerminalRuntime {
    pub fn wait_for_exit(
        &mut self,
        handle: &TerminalRuntimeHandle,
        timeout: Duration,
    ) -> Result<Option<TerminationOutcome>, TerminalRuntimeError> {
        self.wait_for_child_outcome(handle, timeout)
    }

    pub fn request_terminate(
        &mut self,
        handle: &TerminalRuntimeHandle,
        request: TerminationRequest,
        sigterm_timeout: Duration,
        sigkill_timeout: Duration,
    ) -> Result<Vec<TerminalRuntimeEvent>, TerminalRuntimeError> {
        self.session(handle)?;
        let mut events = vec![TerminalRuntimeEvent::TerminationRequested {
            handle: handle.clone(),
            request,
        }];

        if self.send_signal_to_process_group(handle, TerminationSignal::Sigterm)? {
            events.push(TerminalRuntimeEvent::TerminationSignalSent {
                handle: handle.clone(),
                signal: TerminationSignal::Sigterm,
            });
        }

        if let Some(outcome) = self.wait_for_process_group_outcome(handle, sigterm_timeout)? {
            events.push(TerminalRuntimeEvent::Terminated {
                handle: handle.clone(),
                outcome,
            });
            return Ok(events);
        }

        events.push(TerminalRuntimeEvent::TerminationTimedOut {
            handle: handle.clone(),
            after_signal: TerminationSignal::Sigterm,
        });

        let sigkill_sent = self.send_signal_to_process_group(handle, TerminationSignal::Sigkill)?;
        if sigkill_sent {
            events.push(TerminalRuntimeEvent::TerminationSignalSent {
                handle: handle.clone(),
                signal: TerminationSignal::Sigkill,
            });
        }

        if let Some(outcome) = self.wait_for_process_group_outcome(handle, sigkill_timeout)? {
            let outcome = if sigkill_sent {
                TerminationOutcome::KilledAfterTimeout {
                    initial_signal: TerminationSignal::Sigterm,
                    fallback_signal: TerminationSignal::Sigkill,
                }
            } else {
                outcome
            };
            events.push(TerminalRuntimeEvent::Terminated {
                handle: handle.clone(),
                outcome,
            });
            return Ok(events);
        }

        let process_group_exists = self.process_group_exists(handle)?;
        self.sessions.remove(&handle.terminal_id);
        let outcome = if process_group_exists {
            TerminationOutcome::OrphanedUnknown {
                summary: BoundedRuntimeSummary::new(
                    "process group remained observable after SIGKILL timeout",
                ),
            }
        } else {
            TerminationOutcome::OrphanedUnknown {
                summary: BoundedRuntimeSummary::new(
                    "process group disappeared before child exit status was collected",
                ),
            }
        };
        events.push(TerminalRuntimeEvent::Terminated {
            handle: handle.clone(),
            outcome,
        });
        Ok(events)
    }

    fn wait_for_child_outcome(
        &mut self,
        handle: &TerminalRuntimeHandle,
        timeout: Duration,
    ) -> Result<Option<TerminationOutcome>, TerminalRuntimeError> {
        let started = Instant::now();

        loop {
            if let Some(status) = self
                .session_mut(handle)?
                .child
                .try_wait()
                .map_err(|error| TerminalRuntimeError::Io {
                    summary: BoundedRuntimeSummary::new(format!(
                        "failed to inspect terminal process: {error}"
                    )),
                })?
            {
                self.sessions.remove(&handle.terminal_id);
                return Ok(Some(outcome_from_exit_status(status)));
            }

            if started.elapsed() > timeout {
                return Ok(None);
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_process_group_outcome(
        &mut self,
        handle: &TerminalRuntimeHandle,
        timeout: Duration,
    ) -> Result<Option<TerminationOutcome>, TerminalRuntimeError> {
        let process_group_id = self.session(handle)?.process_group_id;
        let started = Instant::now();
        let mut child_outcome = None;

        loop {
            if child_outcome.is_none() {
                child_outcome = self.try_child_outcome(handle)?;
            }

            if !process_group_exists_by_id(process_group_id)? {
                self.sessions.remove(&handle.terminal_id);
                return Ok(Some(child_outcome.unwrap_or_else(|| {
                    TerminationOutcome::OrphanedUnknown {
                        summary: BoundedRuntimeSummary::new(
                            "process group disappeared before child exit status was collected",
                        ),
                    }
                })));
            }

            if started.elapsed() > timeout {
                return Ok(None);
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn try_child_outcome(
        &mut self,
        handle: &TerminalRuntimeHandle,
    ) -> Result<Option<TerminationOutcome>, TerminalRuntimeError> {
        self.session_mut(handle)?
            .child
            .try_wait()
            .map(|status| status.map(outcome_from_exit_status))
            .map_err(|error| TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to inspect terminal process: {error}"
                )),
            })
    }

    fn send_signal_to_process_group(
        &self,
        handle: &TerminalRuntimeHandle,
        signal: TerminationSignal,
    ) -> Result<bool, TerminalRuntimeError> {
        let process_group_id = self.session(handle)?.process_group_id;
        if process_group_id <= 1 {
            return Err(TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!(
                    "refusing to signal unsafe process group id: {process_group_id}"
                )),
            });
        }

        let result = unsafe { libc::kill(-process_group_id, signal_number(signal)) };
        if result == 0 {
            return Ok(true);
        }

        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(false)
        } else {
            Err(TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to signal terminal process group: {error}"
                )),
            })
        }
    }

    fn process_group_exists(
        &self,
        handle: &TerminalRuntimeHandle,
    ) -> Result<bool, TerminalRuntimeError> {
        let process_group_id = self.session(handle)?.process_group_id;
        process_group_exists_by_id(process_group_id)
    }
}

fn process_group_exists_by_id(process_group_id: libc::pid_t) -> Result<bool, TerminalRuntimeError> {
    let result = unsafe { libc::kill(-process_group_id, 0) };
    if result == 0 {
        return Ok(true);
    }

    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(TerminalRuntimeError::Io {
            summary: BoundedRuntimeSummary::new(format!(
                "failed to inspect terminal process group: {error}"
            )),
        }),
    }
}

fn signal_number(signal: TerminationSignal) -> libc::c_int {
    match signal {
        TerminationSignal::Sigterm => libc::SIGTERM,
        TerminationSignal::Sigkill => libc::SIGKILL,
    }
}

fn outcome_from_exit_status(status: ExitStatus) -> TerminationOutcome {
    if let Some(exit_status) = status.code() {
        return TerminationOutcome::Exited { exit_status };
    }

    match status.signal() {
        Some(libc::SIGTERM) => TerminationOutcome::TerminatedBySignal {
            signal: TerminationSignal::Sigterm,
        },
        Some(libc::SIGKILL) => TerminationOutcome::TerminatedBySignal {
            signal: TerminationSignal::Sigkill,
        },
        Some(signal) => TerminationOutcome::Failed {
            summary: BoundedRuntimeSummary::new(format!(
                "terminal process exited from unsupported signal: {signal}"
            )),
        },
        None => TerminationOutcome::Failed {
            summary: BoundedRuntimeSummary::new(
                "terminal process exited without exit code or signal",
            ),
        },
    }
}
