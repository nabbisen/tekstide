use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use super::launch::{processes_in_session, session_confirmed_empty, session_id_of};
use super::{
    BoundedRuntimeSummary, LinuxTerminalRuntime, TerminalRuntimeError, TerminalRuntimeEvent,
    TerminalRuntimeHandle, TerminationOutcome, TerminationRequest, TerminationSignal,
};

/// RFC-043 security document §1: "bound your iterations." A defensive
/// cap, not a realistic limit -- a terminal's own session realistically
/// holds a handful of processes -- so a pathological session can never
/// turn the enumerate-then-signal step below into an unbounded loop.
const MAX_SESSION_SIGNAL_ITERATIONS: usize = 256;

impl LinuxTerminalRuntime {
    pub fn wait_for_exit(
        &mut self,
        handle: &TerminalRuntimeHandle,
        timeout: Duration,
    ) -> Result<Option<TerminationOutcome>, TerminalRuntimeError> {
        self.wait_for_child_outcome(handle, timeout)
    }

    /// RFC-043 D1/D2: signals the **session**, and nothing else. Every
    /// job-control process group inside it (a user's or an agent's own
    /// `&`) is in scope; anything that left the session entirely
    /// (`nohup`, `disown`, `setsid`) is out of scope **by design** --
    /// D2's own opt-out, not a gap this routine tries to close.
    ///
    /// The sequence RFC-043's own "Decided on acceptance" section
    /// requires, in this order, because the order is the fix (a
    /// SIGKILL-first order destroys the one thing -- the shell itself --
    /// that would otherwise hang up its own jobs cooperatively):
    ///
    /// 1. `SIGHUP` the session leader alone (a single pid, never a
    ///    `-group` target) and wait `hangup_timeout`. An interactive
    ///    shell with job control on (every shell this runtime launches
    ///    has it, `spawn_pty_child`'s own `TIOCSCTTY`) hangs up its own
    ///    background jobs when it is itself hung up -- most of the work
    ///    is done here, by the shell that already knows what it started.
    /// 2. Enumerate whoever is left in the session and `SIGKILL` each
    ///    survivor -- re-verifying, immediately before every signal,
    ///    that the pid is still a member of this session (security
    ///    document §1: a pid can exit and be reused by an unrelated
    ///    process in the gap between enumeration and signalling; leaving
    ///    an orphan is a bug, killing a stranger is an incident. If the
    ///    check fails, the pid is left unsignalled). Wait
    ///    `sigkill_timeout`.
    /// 3. Re-enumerate to confirm empty -- this observation, and only
    ///    this one, is what [`TerminalRuntimeEvent::SessionConfirmedEmpty`]
    ///    reports (D3): never inferred from which signal was sent or
    ///    which `TerminationOutcome` the session leader itself produced.
    ///
    /// RFC-043 D1's own disjunction, response 341's required follow-up:
    /// **this function does not close the PTY master before its own
    /// `SIGHUP`**, unlike [`super::launch::RunningTerminal::drop`],
    /// which now does -- see that impl's own comment for the mechanism.
    /// The reason is not that closing here would be unsafe (closing
    /// this crate's own reference is always safe, regardless of who
    /// else holds a duplicate); it is that closing it here would not be
    /// *effective*: this function's real caller
    /// (`crates/tekstide/src/shell.rs`'s project-close path) removes the
    /// owning `TerminalPane` from its tracked list and calls this method
    /// directly on the removed value, with nothing draining
    /// `TerminalPane`'s own `reader` thread for the entire span of this
    /// call -- so that thread's independent duplicate of this same
    /// master is still open throughout the SIGHUP/wait/SIGKILL sequence
    /// below, and closing this crate's own copy first would not be the
    /// last reference, would not trigger a real hangup, and would not
    /// unblock a session leader stuck writing into a saturated pty. That
    /// is a real, unclosed gap in this specific call path -- making it
    /// effective here would mean `TerminalPane` shutting its reader down
    /// (or at least draining it) as part of requesting termination,
    /// before this function's own `SIGHUP`, not only as part of
    /// eventually dropping the pane afterward. Recorded rather than
    /// silently left implied: on a busy terminal, `request_terminate`
    /// still relies on `SIGHUP`-then-`SIGKILL` alone.
    pub fn request_terminate(
        &mut self,
        handle: &TerminalRuntimeHandle,
        request: TerminationRequest,
        hangup_timeout: Duration,
        sigkill_timeout: Duration,
    ) -> Result<Vec<TerminalRuntimeEvent>, TerminalRuntimeError> {
        let session_id = self.session(handle)?.process_group_id;
        let mut events = vec![TerminalRuntimeEvent::TerminationRequested {
            handle: handle.clone(),
            request,
        }];

        if self.signal_session_leader(handle, TerminationSignal::Sighup)? {
            events.push(TerminalRuntimeEvent::TerminationSignalSent {
                handle: handle.clone(),
                signal: TerminationSignal::Sighup,
            });
        }

        if let Some(outcome) = self.wait_for_session_outcome(handle, session_id, hangup_timeout)? {
            self.sessions.remove(&handle.terminal_id);
            events.push(TerminalRuntimeEvent::SessionConfirmedEmpty {
                handle: handle.clone(),
                confirmed: true,
            });
            events.push(TerminalRuntimeEvent::Terminated {
                handle: handle.clone(),
                outcome,
            });
            return Ok(events);
        }

        events.push(TerminalRuntimeEvent::TerminationTimedOut {
            handle: handle.clone(),
            after_signal: TerminationSignal::Sighup,
        });

        let signalled = signal_session_survivors(session_id, libc::SIGKILL);
        if signalled > 0 {
            events.push(TerminalRuntimeEvent::TerminationSignalSent {
                handle: handle.clone(),
                signal: TerminationSignal::Sigkill,
            });
        }

        if let Some(outcome) = self.wait_for_session_outcome(handle, session_id, sigkill_timeout)? {
            let outcome = if signalled > 0 {
                TerminationOutcome::KilledAfterTimeout {
                    initial_signal: TerminationSignal::Sighup,
                    fallback_signal: TerminationSignal::Sigkill,
                }
            } else {
                outcome
            };
            self.sessions.remove(&handle.terminal_id);
            events.push(TerminalRuntimeEvent::SessionConfirmedEmpty {
                handle: handle.clone(),
                confirmed: true,
            });
            events.push(TerminalRuntimeEvent::Terminated {
                handle: handle.clone(),
                outcome,
            });
            return Ok(events);
        }

        // Step 4, named and re-run explicitly rather than relied on from
        // the wait loop's own last iteration above: this call, and only
        // this call, is what `SessionConfirmedEmpty` actually rests on.
        // A grace period expiring with survivors still present reports
        // `false` here -- not a hopeful `true` inferred from "SIGKILL was
        // sent."
        let confirmed = session_confirmed_empty(session_id);
        self.sessions.remove(&handle.terminal_id);
        events.push(TerminalRuntimeEvent::SessionConfirmedEmpty {
            handle: handle.clone(),
            confirmed,
        });
        let outcome = TerminationOutcome::OrphanedUnknown {
            summary: BoundedRuntimeSummary::new(if confirmed {
                "session confirmed empty only after the grace period had already expired"
            } else {
                "session still has live process(es) after SIGKILL and the grace period"
            }),
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

    /// The session-scoped replacement for the old `-process_group_id`
    /// polling: returns `Some(outcome)` only once a real `/proc`
    /// enumeration observes the *session* empty, not merely the session
    /// leader's own process group -- a backgrounded job in a sibling
    /// group inside the same session is exactly what the old check could
    /// not see. Still collects the leader's own real exit status
    /// opportunistically (`try_child_outcome`), the same as before, so a
    /// clean `Exited`/`TerminatedBySignal` outcome is reported when one
    /// is available rather than always falling back to `OrphanedUnknown`.
    fn wait_for_session_outcome(
        &mut self,
        handle: &TerminalRuntimeHandle,
        session_id: libc::pid_t,
        timeout: Duration,
    ) -> Result<Option<TerminationOutcome>, TerminalRuntimeError> {
        let started = Instant::now();
        let mut child_outcome = None;

        loop {
            if child_outcome.is_none() {
                child_outcome = self.try_child_outcome(handle)?;
            }

            if session_confirmed_empty(session_id) {
                // response 340's own zombie fix means this can now be
                // reached with `child_outcome` still `None` even though
                // the leader is, in fact, gone: `is_live_member_of_session`
                // excludes a zombie from the enumeration, so the session
                // can read "empty" a few microseconds before this same
                // loop's own `try_child_outcome` call happens to observe
                // and reap it. Nothing else can have reaped this specific
                // child (only its real parent, this process, can), so if
                // the session is confirmed empty and we have not reaped
                // it ourselves yet, it must currently be exactly that
                // unreaped zombie -- a blocking `wait()` on it returns
                // essentially instantly rather than genuinely blocking.
                if child_outcome.is_none() {
                    child_outcome = self
                        .session_mut(handle)?
                        .child
                        .wait()
                        .map(outcome_from_exit_status)
                        .ok();
                }
                return Ok(Some(child_outcome.unwrap_or_else(|| {
                    TerminationOutcome::OrphanedUnknown {
                        summary: BoundedRuntimeSummary::new(
                            "session became empty before the leader's own exit status was \
                             collected",
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

    /// Signals the session **leader alone** -- a single, positive pid,
    /// never a `-group`/`-session` target. `self.process_group_id` is
    /// also the session id (`spawn_pty_child`'s `pre_exec` calls
    /// `setsid()` before `exec`, making the freshly forked shell both),
    /// so this is the same value under a different name depending on
    /// which fact about it a caller cares about.
    fn signal_session_leader(
        &self,
        handle: &TerminalRuntimeHandle,
        signal: TerminationSignal,
    ) -> Result<bool, TerminalRuntimeError> {
        let session_id = self.session(handle)?.process_group_id;
        if session_id <= 1 {
            return Err(TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!(
                    "refusing to signal unsafe session id: {session_id}"
                )),
            });
        }

        let result = unsafe { libc::kill(session_id, signal_number(signal)) };
        if result == 0 {
            return Ok(true);
        }

        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(false)
        } else {
            Err(TerminalRuntimeError::Io {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to signal terminal session leader: {error}"
                )),
            })
        }
    }
}

/// Enumerates the session and signals every survivor found. Returns how
/// many were actually signalled.
fn signal_session_survivors(session_id: libc::pid_t, signal: libc::c_int) -> usize {
    signal_candidates(
        session_id,
        processes_in_session(session_id).into_iter().flatten(),
        signal,
    )
}

/// The re-verifying signal loop itself, separated from
/// [`signal_session_survivors`]'s own enumeration so a test can hand it
/// a *controlled* candidate list -- including a pid deliberately not a
/// member of `session_id` -- without needing to win a real, inherently
/// racy PID-reuse timing window to prove the check matters.
///
/// Re-verifies, immediately before each individual `kill`, that the pid
/// is still a member of `session_id` (security document §1: the pid
/// could have exited and been reused by an unrelated process in the gap
/// between an earlier enumeration and this signal; if it is no longer a
/// member, it is left unsignalled rather than risking a stranger --
/// leaving an orphan is a bug, killing a stranger is an incident).
/// Bounded by [`MAX_SESSION_SIGNAL_ITERATIONS`], per the same security
/// document's "bound your iterations."
pub(super) fn signal_candidates(
    session_id: libc::pid_t,
    candidates: impl IntoIterator<Item = libc::pid_t>,
    signal: libc::c_int,
) -> usize {
    let mut signalled = 0;
    for pid in candidates.into_iter().take(MAX_SESSION_SIGNAL_ITERATIONS) {
        if session_id_of(pid) != Some(session_id) {
            continue;
        }
        if unsafe { libc::kill(pid, signal) } == 0 {
            signalled += 1;
        }
    }
    signalled
}

fn signal_number(signal: TerminationSignal) -> libc::c_int {
    match signal {
        TerminationSignal::Sighup => libc::SIGHUP,
        TerminationSignal::Sigterm => libc::SIGTERM,
        TerminationSignal::Sigkill => libc::SIGKILL,
    }
}

fn outcome_from_exit_status(status: ExitStatus) -> TerminationOutcome {
    if let Some(exit_status) = status.code() {
        return TerminationOutcome::Exited { exit_status };
    }

    match status.signal() {
        Some(libc::SIGHUP) => TerminationOutcome::TerminatedBySignal {
            signal: TerminationSignal::Sighup,
        },
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
