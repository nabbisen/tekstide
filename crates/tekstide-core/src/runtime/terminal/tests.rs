use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::os::unix::fs::PermissionsExt;

use crate::domain::{TerminalId, TerminalKind, TerminalStatus};
use crate::project::{ProjectId, ProjectSession};

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

#[test]
fn linux_runtime_rejects_cross_project_launch() {
    let root = test_root("cross-project-launch");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(ProjectId::for_test(2), "Shell", &root, "/bin/sh");

    let error = TerminalRuntime::new()
        .launch_project_shell(&project, spec)
        .expect_err("cross-project terminal launch must be rejected");

    assert_eq!(error, TerminalLaunchError::CrossProject);
    cleanup_root(root);
}

#[test]
fn linux_runtime_rejects_raw_managed_launch_without_agent_authority() {
    let root = test_root("raw-managed-launch");
    let project = project_session(ProjectId::for_test(1), &root);
    let mut spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    spec.kind = TerminalKind::Managed;

    let error = TerminalRuntime::new()
        .launch_project_shell(&project, spec)
        .expect_err("raw terminal launch must not mint Managed AgentRun label");

    assert_eq!(error, TerminalLaunchError::UnsupportedTerminalKind);
    cleanup_root(root);
}

#[test]
fn linux_runtime_rejects_cwd_escape() {
    let root = test_root("cwd-escape-project");
    let outside = test_root("cwd-escape-outside");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &outside, "/bin/sh");

    let error = TerminalRuntime::new()
        .launch_project_shell(&project, spec)
        .expect_err("cwd outside project root must be rejected");

    assert!(matches!(
        error,
        TerminalLaunchError::CwdEscapesProjectRoot { .. }
    ));
    cleanup_root(root);
    cleanup_root(outside);
}

#[test]
fn linux_runtime_rejects_non_executable_shell_before_spawn() {
    let root = test_root("non-executable-shell");
    let shell = root.join("fake-sh");
    std::fs::write(&shell, "#!/bin/sh\n").expect("fake shell should be created");
    let mut permissions = std::fs::metadata(&shell)
        .expect("fake shell metadata should be readable")
        .permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(&shell, permissions).expect("fake shell should be non-executable");

    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, &shell);

    let error = TerminalRuntime::new()
        .launch_project_shell(&project, spec)
        .expect_err("non-executable shell path must be rejected");

    assert!(matches!(
        error,
        TerminalLaunchError::ShellUnavailable { .. }
    ));
    cleanup_root(root);
}

/// RFC-028 D5, review 594: the regression review 593's follow-up found
/// and fixed. The shell here passes every pre-check above (exists,
/// executable bit set) and only fails once `exec` actually runs,
/// against an interpreter named in its own `#!` line that does not
/// exist -- `ENOENT`, not `ENOEXEC`. That distinction matters: a
/// shebang-less file fails with `ENOEXEC`, which glibc's own `execvp`
/// silently retries via `/bin/sh -c`, running the file as a script and
/// masking this exact defect regardless of which `pre_exec` close-loop
/// body is used (an earlier probe built against that case produced no
/// usable evidence either way, for exactly this reason). `ENOENT` is
/// not retried, so this is the one shape that actually exercises
/// `spawn_pty_child`'s own `pre_exec` closure racing `Command::spawn`'s
/// internal errno pipe -- see that closure's own comment for the full
/// mechanism. **Must report failure, not `Ok` followed by a dead
/// child with no diagnostic attached.**
#[test]
fn launching_a_shell_with_a_nonexistent_interpreter_reports_failure_not_a_dead_child() {
    let root = test_root("nonexistent-interpreter-shell");
    let shell = root.join("shell-with-missing-interpreter");
    std::fs::write(
        &shell,
        "#!/definitely/not/a/real/interpreter/tekstide-probe\n",
    )
    .expect("fake shell should be created");
    let mut permissions = std::fs::metadata(&shell)
        .expect("fake shell metadata should be readable")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&shell, permissions).expect("fake shell should be marked executable");

    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, &shell);

    let error = TerminalRuntime::new()
        .launch_project_shell(&project, spec)
        .expect_err(
            "a shell whose interpreter does not exist must be reported as a launch failure, not \
             as a successful launch of a child that is about to die with no diagnostic",
        );

    assert!(matches!(error, TerminalLaunchError::SpawnFailed { .. }));
    cleanup_root(root);
}

#[test]
fn linux_runtime_launches_project_shell_and_reads_marker() {
    let root = test_root("launch-shell");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, events) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    assert_eq!(terminal.project_id, project.id().clone());
    assert_eq!(terminal.kind, TerminalKind::Plain);
    assert_eq!(terminal.status(), TerminalStatus::Running);
    assert_eq!(
        events,
        vec![
            TerminalRuntimeEvent::LaunchAccepted {
                handle: handle.clone(),
            },
            TerminalRuntimeEvent::ProcessStarted {
                handle: handle.clone(),
            },
        ]
    );

    wait_for_shell_ready(&mut runtime, &handle);
    runtime
        .write_input(&handle, b"printf 'tekstide-runtime-ok\\n'\nexit\n")
        .expect("marker command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"tekstide-runtime-ok");
    assert!(
        contains_subsequence(&output, b"tekstide-runtime-ok"),
        "PTY output should contain marker; captured: {}",
        String::from_utf8_lossy(&output)
    );

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    cleanup_root(root);
}

#[test]
fn linux_runtime_rejects_cross_project_input_handle() {
    // Review 626: armed, under investigation for review 625's finding.
    let _pty_probe_guard = arm_pty_responsiveness_probe_for_test();
    let root = test_root("cross-project-input");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let cross_project_handle =
        TerminalRuntimeHandle::new(terminal.id.clone(), ProjectId::for_test(2));

    let error = runtime
        .write_input(&cross_project_handle, b"printf 'must-not-write\\n'\n")
        .expect_err("cross-project input handle must be rejected");

    assert_eq!(
        error,
        TerminalRuntimeError::CrossProjectHandle {
            terminal_id: terminal.id.clone(),
        }
    );

    wait_for_shell_ready(&mut runtime, &handle);
    runtime
        .write_input(&handle, b"exit\n")
        .expect("cleanup exit should write to PTY");
    let outcome = wait_for_exit_draining(&mut runtime, &handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    cleanup_root(root);
}

#[test]
fn linux_runtime_reads_output_through_bounded_buffer() {
    let root = test_root("bounded-output");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    wait_for_shell_ready(&mut runtime, &handle);

    runtime
        .write_input(
            &handle,
            b"i=0; while [ \"$i\" -lt 200 ]; do printf 'tekstide-output-%03d-abcdefghijklmnopqrstuvwxyz\\n' \"$i\"; i=$((i + 1)); done\nexit\n",
        )
        .expect("output flood command should write to PTY");
    let (output, event) = runtime
        .read_available_bounded_for(&handle, Duration::from_secs(2), 256)
        .expect("bounded PTY read should succeed");

    assert_eq!(output.len(), 256);
    // Review 624: the echo of the command's own unexpanded `printf`
    // format string carries the literal text `%03d`, which satisfies a
    // bare length assertion whether or not the shell ever ran it --
    // exactly the debt review 622 named. Only a running shell expands
    // `"$i"` and produces the real first line.
    assert!(
        contains_subsequence(&output, b"tekstide-output-000-"),
        "flood output should contain the real, expanded first line, not just enough echoed \
         bytes to fill the buffer; captured: {}",
        String::from_utf8_lossy(&output)
    );
    assert!(matches!(
        event,
        TerminalRuntimeEvent::OutputBuffered {
            summary:
                TerminalOutputSummary {
                    buffered_bytes: 256,
                    dropped_bytes,
                    truncated: true,
                },
            ..
        } if dropped_bytes > 0
    ));

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    cleanup_root(root);
}

#[test]
fn linux_runtime_routes_resize_to_project_terminal() {
    let root = test_root("resize-terminal");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let dimensions = TerminalDimensions {
        rows: 40,
        cols: 100,
    };

    let event = runtime
        .resize(&handle, dimensions)
        .expect("PTY resize should be routed");
    assert_eq!(
        event,
        TerminalRuntimeEvent::Resized {
            handle: handle.clone(),
            dimensions,
        }
    );

    runtime
        .write_input(&handle, b"stty size\nexit\n")
        .expect("stty command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"40 100");
    assert!(
        contains_subsequence(&output, b"40 100"),
        "PTY output should contain resized dimensions; captured: {}",
        String::from_utf8_lossy(&output)
    );

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    cleanup_root(root);
}

/// Review 579: `request_terminate` closes the PTY master (line 86) before
/// sending `SIGHUP` (line 93) -- and this test's own child is `/bin/cat`,
/// not a shell. `cat`'s blocking read on the slave can return EOF (the
/// master closed cleanly, `cat` exits 0) or `EIO` (the master closed
/// mid-read, `cat` reports exit 1) *before* the `SIGHUP` a few lines
/// later ever arrives -- the two `TerminationOutcome::Exited` values this
/// row's flake has shown are exactly those two cases, not a timing
/// coincidence. `request_terminate`'s own doc comment (D3) already states
/// the contract this test used to violate: outcomes are reported
/// "as `waitpid` reports... never inferred from which signal was sent or
/// which `TerminationOutcome` the session leader itself produced." Pinning
/// one specific `TerminationOutcome` asserted exactly that inference.
/// What the contract actually promises -- a `SIGHUP` sent, and the session
/// re-confirmed empty by real enumeration -- is still asserted above,
/// unconditionally; only the session leader's own, genuinely racy exit
/// shape is now accepted in any of its real forms.
#[test]
fn linux_runtime_terminates_session_leader_with_sighup() {
    let root = test_root("terminate-sighup");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Cat", &root, "/bin/cat");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain terminal process launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("terminate process group smoke"),
    };
    let events = runtime
        .request_terminate(
            &handle,
            request.clone(),
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .expect("termination request should complete");

    assert_eq!(
        events.first(),
        Some(&TerminalRuntimeEvent::TerminationRequested {
            handle: handle.clone(),
            request,
        })
    );
    assert!(
        events.contains(&TerminalRuntimeEvent::TerminationSignalSent {
            handle: handle.clone(),
            signal: TerminationSignal::Sighup,
        })
    );
    assert!(
        events.contains(&TerminalRuntimeEvent::SessionConfirmedEmpty {
            handle: handle.clone(),
            confirmed: true,
        }),
        "the session must be confirmed empty by a real re-enumeration, not merely inferred: \
         {events:?}"
    );
    match events.last() {
        Some(TerminalRuntimeEvent::Terminated {
            handle: event_handle,
            outcome,
        }) if *event_handle == handle => {
            assert!(
                matches!(
                    outcome,
                    TerminationOutcome::TerminatedBySignal {
                        signal: TerminationSignal::Sighup
                    } | TerminationOutcome::Exited { exit_status: 0 }
                        | TerminationOutcome::Exited { exit_status: 1 }
                ),
                "the session leader's own exit shape is genuinely racy (D3: reported as \
                 `waitpid` sees it, never inferred) -- `SIGHUP` winning, `cat` seeing EOF on the \
                 closed master (exit 0), or `cat` seeing EIO (exit 1) are all the contract \
                 promises; got {outcome:?}"
            );
        }
        other => panic!("expected a final Terminated event for this handle, got {other:?}"),
    }

    let error = runtime
        .write_input(&handle, b"printf 'after-close\\n'\n")
        .expect_err("terminated runtime session must be removed");
    assert_eq!(
        error,
        TerminalRuntimeError::UnknownTerminal {
            terminal_id: terminal.id,
        }
    );
    cleanup_root(root);
}

#[test]
fn linux_runtime_rejects_cross_project_termination_handle() {
    // Review 626: armed, under investigation for review 625's finding.
    let _pty_probe_guard = arm_pty_responsiveness_probe_for_test();
    let root = test_root("cross-project-terminate");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let cross_project_handle =
        TerminalRuntimeHandle::new(terminal.id.clone(), ProjectId::for_test(2));
    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("cross-project terminate smoke"),
    };

    let error = runtime
        .request_terminate(
            &cross_project_handle,
            request,
            Duration::from_millis(100),
            Duration::from_millis(100),
        )
        .expect_err("cross-project termination handle must be rejected");

    assert_eq!(
        error,
        TerminalRuntimeError::CrossProjectHandle {
            terminal_id: terminal.id.clone(),
        }
    );

    wait_for_shell_ready(&mut runtime, &handle);
    runtime
        .write_input(&handle, b"exit\n")
        .expect("cleanup exit should write to PTY");
    let outcome = wait_for_exit_draining(&mut runtime, &handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    cleanup_root(root);
}

/// A plain foreground child (a bare `sleep 30`, no trap) turned out not
/// to exercise this path at all under the new sequence: a session
/// leader blocked in a foreground `wait()` still receives and acts on
/// `SIGHUP` immediately (confirmed directly -- an earlier version of
/// this test using a bare foreground `sleep` found the whole session
/// already confirmed empty after step 1 alone, `TerminatedBySignal
/// { signal: Sighup }`, no timeout, no escalation). A job has to
/// deliberately ignore `SIGHUP` (`trap '' HUP`) the same way the
/// overclaim test's own descendant ignores `SIGTERM` -- to survive the
/// shell's own hangup and force the real escalation to `SIGKILL`.
#[test]
fn linux_runtime_uses_sigkill_fallback_for_a_job_that_ignores_sighup() {
    let root = test_root("terminate-sigkill-fallback");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    runtime
        .write_input(
            &handle,
            b"(trap '' HUP; printf 'hup-ignoring-job-started\\n'; sleep 30) &\n",
        )
        .expect("hup-ignoring background command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"hup-ignoring-job-started");
    assert!(
        contains_subsequence(&output, b"hup-ignoring-job-started"),
        "PTY output should contain the background-job marker; captured: {}",
        String::from_utf8_lossy(&output)
    );

    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("force sigkill fallback against a sighup-ignoring job"),
    };
    let events = runtime
        .request_terminate(
            &handle,
            request,
            Duration::from_millis(100),
            Duration::from_secs(2),
        )
        .expect("termination request should complete with fallback");

    assert!(
        events.contains(&TerminalRuntimeEvent::TerminationSignalSent {
            handle: handle.clone(),
            signal: TerminationSignal::Sighup,
        })
    );
    assert!(events.contains(&TerminalRuntimeEvent::TerminationTimedOut {
        handle: handle.clone(),
        after_signal: TerminationSignal::Sighup,
    }));
    assert!(
        events.contains(&TerminalRuntimeEvent::TerminationSignalSent {
            handle: handle.clone(),
            signal: TerminationSignal::Sigkill,
        })
    );
    assert!(
        events.contains(&TerminalRuntimeEvent::SessionConfirmedEmpty {
            handle: handle.clone(),
            confirmed: true,
        }),
        "the session must be confirmed empty by a real re-enumeration: {events:?}"
    );
    assert_eq!(
        events.last(),
        Some(&TerminalRuntimeEvent::Terminated {
            handle,
            outcome: TerminationOutcome::KilledAfterTimeout {
                initial_signal: TerminationSignal::Sighup,
                fallback_signal: TerminationSignal::Sigkill,
            },
        })
    );
    cleanup_root(root);
}

/// RFC-043's own required claim, checked the way its own README insists
/// on: "an OS-level check, not an inference from the dialog." Whatever
/// `request_terminate`'s returned events say, `kill(pid, 0)` on the
/// backgrounded job's own real pid is the thing this test actually
/// trusts.
///
/// This test's own history is worth keeping: it used to be named
/// `..._does_not_overclaim_when_child_outlives_direct_shell_after_sigterm`,
/// with a descendant that trapped `SIGTERM` specifically to survive the
/// old (process-group-only) termination path and demonstrate that path
/// lying about success. Under RFC-043's new sequence that exact
/// scenario no longer survives at all: a `SIGTERM`-trapping job does not
/// trap `SIGHUP`, so the shell's own job-control hangup (step 1) reaps
/// it before `SIGKILL` is ever needed -- confirmed directly, not assumed
/// (`confirmed: true`, `TerminatedBySignal { signal: Sighup }`, no
/// escalation at all). That is not a broken test; it is the fix working
/// on the exact case that used to demonstrate the defect. Replaced with
/// this plainer scenario -- an ordinary backgrounded job, no traps --
/// since the interesting question after this RFC is no longer "does the
/// report lie," it's "is the job actually gone," which is what RFC-043's
/// own acceptance claim asks for directly.
#[test]
fn a_real_backgrounded_job_is_dead_after_a_real_close() {
    let root = CleanupOnDrop(test_root("terminate-real-backgrounded-job"));
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &*root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    // Captured before the close, not derived from `background_pid` after
    // it -- review 634's own measurement needs the real session id even
    // if the backgrounded job's own `getsid` became unanswerable by the
    // time the diagnostic runs.
    let session_id = runtime
        .sessions
        .get(&terminal.id)
        .expect("just-launched session must be present")
        .process_group_id;

    runtime
        .write_input(&handle, b"sleep 300 & echo BGPID=$!\n")
        .expect("backgrounded sleep command should write to PTY");
    let background_pid =
        read_until_parsed(&mut runtime, &handle, parse_bgpid).unwrap_or_else(|output| {
            panic!(
                "real backgrounded job's own pid should be parseable from PTY output; \
                 captured: {}",
                String::from_utf8_lossy(&output)
            )
        });
    // Review 639's own required correction: `kill(pid, 0)` alone reads
    // `true` for a zombie too, so this precondition would have passed on
    // exactly the case it exists to exclude -- a backgrounded job that
    // was never really alive going into the close (e.g. `sleep 300`
    // failing to start and dying immediately, unreaped). `!is_gone_or_going`
    // is the same gone-or-departing exclusion the corrected post-close
    // assertion below now also uses.
    assert!(
        crate::test_support::process_is_alive(background_pid)
            && !super::launch::is_gone_or_going(background_pid as libc::pid_t),
        "test precondition: the real backgrounded job must be alive before the close"
    );
    // Review 635's own required control: fired here, against a pid known
    // alive and known in the session, before trusting the same call's
    // negative answer in the failure branch below. `FOUND` is the
    // expected answer; anything else indicts the instrument rather than
    // the hypothesis under test.
    super::launch::report_session_members_via_kern_proc_session(
        "control, pre-close",
        session_id,
        background_pid as libc::pid_t,
    );

    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("real backgrounded job smoke"),
    };
    let events = runtime
        .request_terminate(
            &handle,
            request,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .expect("termination request should complete");

    assert!(
        events.contains(&TerminalRuntimeEvent::SessionConfirmedEmpty {
            handle: handle.clone(),
            confirmed: true,
        }),
        "the session must be confirmed empty by a real re-enumeration: {events:?}"
    );
    let background_job_still_signalable = crate::test_support::process_is_alive(background_pid);
    // Review 637's own required correction: fired in *both* branches, not
    // only the failing one. This test is intermittent -- it has failed
    // suite-wide and passed on an isolated re-run seconds later -- so a
    // failure-branch-only diagnostic never produces the passing-run
    // control ("no record") that makes a `SZOMB` reading on a failing run
    // trustworthy rather than merely assumed.
    super::launch::report_pid_p_stat_via_sysctl(background_pid as libc::pid_t);
    // Review 638's own required correction: `kill(pid, 0)` alone cannot
    // tell a real survivor from an unreaped zombie -- it succeeds against
    // both (proved deterministically, no runner needed, in
    // `test_support::tests::kill_signal_zero_succeeds_against_an_unreaped_zombie`).
    // "Gone or departing" is the property RFC-043 actually asks for: a
    // corpse satisfies "closing a project kills the backgrounded
    // descendant" exactly as hard reaping does. This is a correction of
    // the assertion, not a relaxation of the guarantee -- the same
    // distinction production's own containment already draws
    // (`is_gone_or_going`, response 340's zombie exclusion), now applied
    // here too.
    let background_job_gone_or_departing = !background_job_still_signalable
        || super::launch::is_gone_or_going(background_pid as libc::pid_t);
    if !background_job_gone_or_departing {
        // Review 634's own named measurement, fired only in the branch it
        // answers: does `KERN_PROC_SESSION` see this exact pid when the
        // bulk `proc_listallpids` scan and the ordinary member check did
        // not stop it from surviving the close? See the function's own
        // doc comment for the mechanism and what it does and does not
        // establish.
        super::launch::report_session_members_via_kern_proc_session(
            "failure branch, post-close",
            session_id,
            background_pid as libc::pid_t,
        );
    }
    assert!(
        background_job_gone_or_departing,
        "the real backgrounded job's own pid ({background_pid}) must be gone or departing after \
         a real close, not merely 'kill(pid, 0) failed' -- kill(pid, 0) alone cannot tell a real \
         survivor from a SIGKILLed corpse sitting unreaped for a moment under load"
    );
}

/// RFC-043 D2's own opt-out, asserted on purpose rather than merely
/// tolerated: `setsid` is the standard, documented way a user (or a
/// script) says "this should outlive my terminal," by leaving the
/// session entirely. A containment routine that reached it anyway would
/// have a blast radius wider than the RFC's own contract describes --
/// `what-containment-must-not-become.md` §2's own "the opt-out is
/// load-bearing, not a nicety."
#[test]
fn a_job_that_leaves_the_session_via_setsid_survives_a_real_close() {
    // Named precondition, not an inferred one: `perl` is an image-provided
    // tool on the macOS CI runner (confirmed present, `actions/runner-
    // images`'s own `macos-26-arm64-Readme.md` lists it), not an OS
    // guarantee -- Apple has been removing scripting-language runtimes
    // from the base system for several releases, so a developer's own Mac
    // may not have it. Checked here so a missing `perl` is a sentence
    // before the clock starts, not a five-second timeout to explain.
    assert!(
        std::process::Command::new("perl")
            .arg("-e")
            .arg("exit 0")
            .status()
            .is_ok_and(|status| status.success()),
        "this test needs `perl` for `POSIX::setsid()`; `setsid(1)` is util-linux and absent on \
         macOS"
    );

    let root = CleanupOnDrop(test_root("terminate-setsid-survives"));
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &*root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    // review 596/609's own macOS triage, item 6: `setsid(1)` is a
    // util-linux command and does not exist on macOS at all -- only the
    // `setsid(2)` syscall does. `perl`'s `POSIX::setsid()` reaches the
    // same syscall directly and ships on both platforms (a core module,
    // present by default on every Linux distribution this project
    // targets and on macOS/the GitHub `macos-latest` runner image),
    // making it the portable replacement rather than a Linux-only one.
    //
    // `fork and exit` before calling it, not a bare `POSIX::setsid()` in
    // the backgrounded process itself: this job is already its own
    // process group leader by the time it runs (bash's own job-control
    // backgrounding put it there), and `setsid(2)` refuses to run on a
    // process that already leads its own group -- the same reason
    // `setsid(1)`'s own `--fork` flag exists, confirmed directly on
    // Linux with the bare form first (it died immediately with no
    // visible error, `setsid`'s own failure message racing past this
    // test's own read). Forking first and calling `setsid` from the
    // child mirrors `--fork` exactly: the child inherits the parent's
    // process group (not its own), so it is not yet a leader and the
    // call succeeds. A failure prints a parseable, distinctly-named
    // marker to the same stream this test reads and exits before
    // `exec`, rather than proceeding to fake a detached job anyway --
    // so a real failure times out at the pid-parse below with a message
    // in the captured output, not a confusing assertion failure further
    // down from a job that was never actually detached. `sh -c 'echo
    // ...; exec sleep 300'` inside the forked child reports its own pid
    // before `exec` replaces it with `sleep`, keeping the same pid
    // throughout, which is the one this test actually needs.
    runtime
        .write_input(
            &handle,
            b"perl -e 'fork and exit; use POSIX; POSIX::setsid() \
              or do { print \"SETSIDFAILED=$!\\n\"; exit 1 }; \
              exec @ARGV' -- sh -c 'echo DETACHEDPID=$$; exec sleep 300' &\n",
        )
        .expect("setsid-detached command should write to PTY");
    let detached_pid = read_until_parsed(&mut runtime, &handle, |output| {
        parse_pid_after(output, "DETACHEDPID=")
    })
    .unwrap_or_else(|output| {
        panic!(
            "real setsid-detached job's own pid should be parseable from PTY output; \
             captured: {}",
            String::from_utf8_lossy(&output)
        )
    });
    assert!(
        crate::test_support::process_is_alive(detached_pid),
        "test precondition: the real detached job must be alive before the close"
    );

    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("setsid opt-out smoke"),
    };
    let events = runtime
        .request_terminate(
            &handle,
            request,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .expect("termination request should complete");

    assert!(
        crate::test_support::process_is_alive(detached_pid),
        "a process that left the session via setsid is out of scope by design -- D2's own \
         opt-out -- and a real close must not have touched it"
    );
    // RFC-043 PR-043-C: this is `SessionConfirmedEmpty`'s own claim made
    // precise, not merely an unused return value -- the *session* really
    // is empty once the shell itself is gone (the setsid'd job left it),
    // so `confirmed: true` here is correct and honest at the same time
    // `detached_pid` is still alive above. `confirmed` claims the
    // session was re-scanned and found empty; it has never claimed
    // "every process this terminal ever launched is gone" -- D2's own
    // opt-out is exactly the gap between those two readings, and this is
    // the test that proves the field does not quietly close it.
    assert!(
        events.contains(&TerminalRuntimeEvent::SessionConfirmedEmpty {
            handle: handle.clone(),
            confirmed: true,
        }),
        "the session itself (minus the process that left it) should be confirmed empty: \
         {events:?}"
    );
    // Not this test's own cleanup responsibility to kill -- proving the
    // survival is the point -- but leaving a real `sleep 300` running
    // for the rest of the test binary's life is its own kind of leak.
    unsafe {
        libc::kill(detached_pid as libc::pid_t, libc::SIGKILL);
    }
}

/// RFC-043 security document §1's own required property: "the session
/// id is re-verified immediately before every signal... if you cannot
/// establish a pid is still in the target session, do not signal it."
///
/// **Not built by winning a real PID-reuse race** -- that race is
/// inherently timing-dependent and would make this test flaky by
/// construction. Instead this calls
/// [`super::termination::signal_candidates`] directly (the re-verifying
/// signal loop, factored out from its own live-enumeration caller
/// specifically so this is possible) with a *controlled* candidate list:
/// a real survivor genuinely in the target session, and a real,
/// completely unrelated process (spawned by this test, in this test
/// binary's own session, never the target's) standing in for exactly
/// the pid a reuse race could hand the enumeration above. The stranger
/// must survive; only the real survivor may be signalled.
#[test]
fn signal_candidates_never_signals_a_pid_outside_the_target_session() {
    let root = test_root("terminate-race-reverification");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let session_id = runtime
        .sessions
        .get(&terminal.id)
        .expect("just-launched session must be present")
        .process_group_id;

    let mut stranger = std::process::Command::new("sleep")
        .arg("300")
        .spawn()
        .expect("a real, unrelated stranger process should spawn");
    let stranger_pid = stranger.id() as libc::pid_t;
    assert_ne!(
        super::launch::session_id_of(stranger_pid),
        Some(session_id),
        "test precondition: the stranger must genuinely not belong to the target session"
    );

    let signalled = super::termination::signal_candidates(
        session_id,
        vec![session_id, stranger_pid],
        libc::SIGKILL,
    );

    assert_eq!(
        signalled, 1,
        "exactly the one real member of the target session must have been signalled"
    );
    assert!(
        crate::test_support::process_is_alive(stranger_pid as u32),
        "the stranger, though present in the candidate list, must never have been signalled -- \
         leaving an orphan is a bug, killing a stranger is an incident"
    );

    let _ = stranger.kill();
    let _ = stranger.wait();
    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("re-verification test cleanup"),
    };
    let _ = runtime.request_terminate(
        &handle,
        request,
        Duration::from_secs(2),
        Duration::from_secs(2),
    );
    cleanup_root(root);
}

/// RFC-043 D3, the required negative case: "the grace period expiring
/// produces `false` from step 4, not a hopeful `true`."
///
/// **Not built by racing real `SIGKILL` reaping speed, and not routed
/// through a real `request_terminate` call.** A first attempt gave a
/// `SIGHUP`-ignoring job `Duration::ZERO` grace periods, expecting the
/// enumeration right after `SIGKILL` to still catch it alive -- measured,
/// not assumed, and it did not work: reaping on this machine is close
/// enough to instantaneous that even a zero-length window still observed
/// the session correctly empty. A genuinely unkillable process
/// (uninterruptible I/O sleep is the only real way `SIGKILL` can fail to
/// remove something) is not portable or fast to construct either.
///
/// A second attempt routed a forced enumeration failure through a real
/// `request_terminate` call against a real session -- but
/// `RunningTerminal::drop`'s own PR-043-A guard reads the exact same
/// `session_confirmed_empty` this test wants to force `false`, and that
/// `Drop` fires *inside* `request_terminate` itself
/// (`self.sessions.remove` drops the removed value immediately), before
/// this test's own assertion on the returned events ever runs. The
/// override does not stay narrow enough to affect only the return value
/// and not the guard; it panics before returning.
///
/// This instead tests [`session_confirmed_empty`] directly, with no real
/// session or `request_terminate` call at all:
/// [`force_session_enumeration_failure_for_test`] forces
/// `processes_in_session` to return `None` deterministically -- the
/// *other* honest-`false` case `what-containment-must-not-become.md` §4
/// names explicitly ("a `/proc` read that failed"). Found dishonest
/// while writing this very test: an earlier version of
/// `session_confirmed_empty` defaulted a failed read to an empty `Vec`,
/// which is precisely the unearned "almost certainly empty" confidence
/// that document forbids.
///
/// RFC-028 D5: platform-neutral since the override moved behind
/// [`force_session_enumeration_failure_for_test`] -- this test exercises
/// the real enumeration-failure path (a never-created directory on
/// Linux, a thread-local flag on macOS, since there is no path to point
/// a `libproc` call at instead) on whichever platform it runs on.
#[test]
fn session_confirmed_empty_reports_false_when_its_own_enumeration_fails() {
    let _enumeration_failure_guard = super::launch::force_session_enumeration_failure_for_test();

    assert!(
        !super::launch::session_confirmed_empty(1),
        "a session whose own enumeration could not run at all must report false -- not infer \
         success just because nothing could be observed"
    );
}

fn parse_pid_after(output: &[u8], marker: &str) -> Option<u32> {
    let text = String::from_utf8_lossy(output);
    let (_, after_marker) = text.rsplit_once(marker)?;
    let digits: String = after_marker
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn project_session(project_id: ProjectId, root: &Path) -> ProjectSession {
    ProjectSession::new(project_id, "Project", root, root)
}

fn test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&root).expect("test root should be created");
    // Review 617/618: same SymlinkAmbiguous class as watch_test_root() --
    // $TMPDIR is reached through a symlink on macOS.
    std::fs::canonicalize(&root).unwrap_or(root)
}

fn cleanup_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

/// `test-process-leak.md` review 480's own addendum: a failing test in this family fails the
/// gate twice -- once on its assertion, once on the fixture it leaves behind -- because
/// `cleanup_root` is an ordinary call at the end of the test body, skipped by any panic before
/// it. Used by the two tests this same disposition pass fixed the marker-wait race in, since
/// those are exactly the ones a flaky assertion above used to be able to skip past. `Deref`
/// to `Path` so a `&CleanupOnDrop` passes anywhere `&root: PathBuf` already did, with no call
/// site needing its own type to change.
///
/// `pub(crate)`, review 527: `audit::tests::first_creation_race` needed the identical
/// drop-guard shape for a different fixture directory, in the same crate -- a reuse, not the
/// cross-crate copy review 524 Required 3 made for `rfc_docs_invariants.rs` (a different test
/// binary, where this type is unreachable regardless of visibility).
pub(crate) struct CleanupOnDrop(pub(crate) PathBuf);

impl std::ops::Deref for CleanupOnDrop {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for CleanupOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn read_until_contains(
    runtime: &mut TerminalRuntime,
    handle: &TerminalRuntimeHandle,
    marker: &[u8],
) -> Vec<u8> {
    let started = Instant::now();
    let mut output = Vec::new();

    while started.elapsed() < Duration::from_secs(5) {
        let (chunk, _) = runtime
            .read_available_bounded_for(handle, Duration::from_millis(50), 16 * 1024)
            .expect("PTY read should succeed");
        output.extend_from_slice(&chunk);
        if contains_subsequence(&output, marker) {
            return output;
        }
    }

    output
}

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// `test-process-leak.md` review 478's own diagnosis, fixed: `read_until_contains` plus a
/// separate, later `parse_*` call is two steps where the first can succeed on the shell's own
/// echo of the *unexpanded* command (`echo BGPID=$!` itself contains the literal marker text,
/// with no digits after it yet) before the real, digit-bearing line has arrived -- the wait is
/// weaker than the condition the caller actually needs. This polls the *parse* itself, not a
/// substring, so it keeps reading until a real value is extractable or the deadline passes,
/// never returning on a marker that is merely present.
///
/// Review 614: returns the captured output on failure (`Err`), not `None` --
/// a timeout used to drop everything the PTY had produced on the floor, so
/// an unexpected line (a shell error, a missing command, this file's own
/// `SETSIDFAILED=` marker) never reached a human on a platform where
/// nobody can attach a debugger. Every caller must read `Err`'s own payload
/// into its panic message, not merely name the timeout.
fn read_until_parsed<T>(
    runtime: &mut TerminalRuntime,
    handle: &TerminalRuntimeHandle,
    mut parse: impl FnMut(&[u8]) -> Option<T>,
) -> Result<T, Vec<u8>> {
    let started = Instant::now();
    let mut output = Vec::new();

    while started.elapsed() < Duration::from_secs(5) {
        let (chunk, _) = runtime
            .read_available_bounded_for(handle, Duration::from_millis(50), 16 * 1024)
            .expect("PTY read should succeed");
        output.extend_from_slice(&chunk);
        if let Some(value) = parse(&output) {
            return Ok(value);
        }
    }

    Err(output)
}

/// Parses the real pid a shell's own `$!` reported, out of raw PTY
/// output containing a `BGPID=<digits>` marker -- the shell's own
/// authority on what pid it just backgrounded, not a guess derived from
/// this process's own bookkeeping.
fn parse_bgpid(output: &[u8]) -> Option<u32> {
    // The *last* occurrence of the marker, not the first: the shell's
    // own terminal echo of the still-unexpanded command (`echo
    // BGPID=$!`) contains the literal marker text too, before the real,
    // digit-bearing one the command's actual output produces.
    parse_pid_after(output, "BGPID=")
}

/// `test-process-leak.md`'s second cause, shown fixed: the gate's own
/// required "leak happening, then not happening" form, the same shape
/// `test_support`'s `kill_on_drop_child_does_not_leak_across_a_panic`
/// already established for a bare `Child` -- here for a real,
/// `TerminalRuntime`-launched terminal instead. `Drop::drop` runs
/// synchronously during unwinding, so by the time `catch_unwind` returns
/// control here, `RunningTerminal`'s own `Drop` impl has already fired.
///
/// **Ablated**: temporarily removed the `Drop for RunningTerminal` impl
/// in `launch.rs`, re-ran this test alone -- failed, `process_is_alive`
/// still `true`, reproducing the exact defect this fix exists to
/// prevent. Restored, re-ran, green.
#[test]
fn dropping_a_running_terminal_kills_the_real_process_group() {
    let _real_process_slot = crate::test_support::RealProcessLimiter::acquire();
    let root = test_root("drop-kills-process-group");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();
    let (terminal, _events) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let pid = runtime
        .sessions
        .get(&terminal.id)
        .expect("just-launched session must be present")
        .child
        .id();
    assert!(
        crate::test_support::process_is_alive(pid),
        "test precondition: the real shell must be alive before the panic"
    );

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _runtime = runtime;
        panic!("deliberate panic, before this closure's own cleanup would run");
    }));
    assert!(
        panicked.is_err(),
        "test precondition: the closure must actually have panicked"
    );

    assert!(
        !crate::test_support::process_is_alive(pid),
        "Drop for RunningTerminal must have killed the real process group during unwind, \
         before catch_unwind returned control here"
    );
    cleanup_root(root);
}

/// The five sites that can drop a `RunningTerminal` were enumerated
/// before writing its `Drop` impl (see that impl's own doc comment):
/// both `sessions.insert` sites key on a freshly minted `TerminalId`, so
/// neither can ever evict a session actually stored under it. Proven
/// here against two real, independently launched terminals rather than
/// merely asserted from reading `TerminalId::new_uuid`'s own guarantee
/// -- the second launch's own `insert` must not have killed the first.
#[test]
fn launching_a_second_terminal_does_not_kill_the_first() {
    let _real_process_slot = crate::test_support::RealProcessLimiter::acquire();
    let root = test_root("insert-does-not-evict-a-live-session");
    let project = project_session(ProjectId::for_test(1), &root);
    let mut runtime = TerminalRuntime::new();

    let spec_a = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell A", &root, "/bin/sh");
    let (terminal_a, _events) = runtime
        .launch_project_shell(&project, spec_a)
        .expect("first plain shell launch should succeed");
    let pid_a = runtime
        .sessions
        .get(&terminal_a.id)
        .expect("just-launched session must be present")
        .child
        .id();

    let spec_b = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell B", &root, "/bin/sh");
    let (terminal_b, _events) = runtime
        .launch_project_shell(&project, spec_b)
        .expect("second plain shell launch should succeed");

    assert_ne!(
        terminal_a.id, terminal_b.id,
        "test precondition: the two launches must have received distinct ids"
    );
    assert!(
        crate::test_support::process_is_alive(pid_a),
        "launching a second, independently-keyed terminal must not kill the first -- both \
         `sessions.insert` sites key on a freshly minted TerminalId, so neither can ever evict \
         a session actually stored under it"
    );

    // Real cleanup through the normal path, not relying on the `Drop`
    // guarantee this test is not the one proving.
    let handle_a = TerminalRuntimeHandle::new(terminal_a.id.clone(), project.id().clone());
    let handle_b = TerminalRuntimeHandle::new(terminal_b.id.clone(), project.id().clone());
    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("insert-does-not-evict test cleanup"),
    };
    let _ = runtime.request_terminate(
        &handle_a,
        request.clone(),
        Duration::from_secs(2),
        Duration::from_secs(2),
    );
    let _ = runtime.request_terminate(
        &handle_b,
        request,
        Duration::from_secs(2),
        Duration::from_secs(2),
    );
    cleanup_root(root);
}

/// The symlink target every PTY master shows under `/proc/<pid>/fd`,
/// regardless of which of possibly many open PTYs it actually is --
/// `openpty(3)` masters and slaves alike are opened against `/dev/ptmx`/
/// `/dev/pts/N` but the master's own fd entry resolves to this fixed
/// target, the same one `pty-master-fd-inheritance.md`'s own measurement
/// on a live survivor used to identify the leaked descriptors.
fn open_fd_targets(pid: u32) -> Vec<PathBuf> {
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| std::fs::read_link(entry.path()).ok())
        .collect()
}

/// pty-master-fd-inheritance handoff: the property the fix exists to
/// hold. `Command::spawn` only returns `Ok` once the child has already
/// `exec`'d -- libstd's own fork/exec implementation reports a failed
/// `exec` back through a `CLOEXEC` pipe before returning, so a
/// successful return is not a race against the child still being a
/// pre-exec fork of this process; reading `/proc/<pid_b>/fd` immediately
/// after `launch_project_shell` returns observes the child's real,
/// post-exec fd table.
///
/// **Ablated**: temporarily removed the `set_cloexec` calls `OpenPty::new`
/// added -- failed, terminal B's own fd table contained a `/dev/ptmx`
/// entry for terminal A's master, reproducing the exact defect this test
/// exists to prevent. Restored, re-ran, green.
#[test]
fn a_second_terminals_child_inherits_no_descriptor_for_the_first_terminals_pty_master() {
    let _real_process_slot = crate::test_support::RealProcessLimiter::acquire();
    let root = test_root("no-cross-terminal-master-inheritance");
    let project = project_session(ProjectId::for_test(1), &root);
    let mut runtime = TerminalRuntime::new();

    let spec_a = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell A", &root, "/bin/sh");
    let (terminal_a, _events) = runtime
        .launch_project_shell(&project, spec_a)
        .expect("first plain shell launch should succeed");

    let spec_b = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell B", &root, "/bin/sh");
    let (terminal_b, _events) = runtime
        .launch_project_shell(&project, spec_b)
        .expect("second plain shell launch should succeed");
    let pid_b = runtime
        .sessions
        .get(&terminal_b.id)
        .expect("just-launched session must be present")
        .child
        .id();

    let inherited_masters: Vec<_> = open_fd_targets(pid_b)
        .into_iter()
        .filter(|target| target == Path::new("/dev/ptmx"))
        .collect();
    assert!(
        inherited_masters.is_empty(),
        "the second terminal's real child must not hold any descriptor for the first \
         terminal's PTY master -- found {} such descriptor(s) in /proc/{pid_b}/fd",
        inherited_masters.len()
    );

    let handle_a = TerminalRuntimeHandle::new(terminal_a.id.clone(), project.id().clone());
    let handle_b = TerminalRuntimeHandle::new(terminal_b.id.clone(), project.id().clone());
    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("no-cross-terminal-master-inheritance test cleanup"),
    };
    let _ = runtime.request_terminate(
        &handle_a,
        request.clone(),
        Duration::from_secs(2),
        Duration::from_secs(2),
    );
    let _ = runtime.request_terminate(
        &handle_b,
        request,
        Duration::from_secs(2),
        Duration::from_secs(2),
    );
    cleanup_root(root);
}

/// Review 579: a sibling `fork` on another thread, landing in the window
/// between `posix_openpt`/`open` and this process's own `set_cloexec`
/// (before this fix, between `openpty` and a follow-up `fcntl`), inherits
/// a PTY fd without close-on-exec -- and since the child typically
/// `exec`s immediately, the inherited copy survives that `exec` and
/// persists for the rest of the child's life, crossing RFC-009's own
/// terminal security boundary. Hard-bounded, all of it, after an
/// unbounded first attempt at this same check froze a shared desktop
/// during development: at most 4 concurrent children, each living only
/// 50ms, the whole check capped by **wall clock** (2s) rather than a
/// fixed iteration count that could balloon on a slow machine, and the
/// holder scan narrowed to *only this test's own tracked child pids* --
/// never a scan of the whole system's `/proc`.
///
/// Detection: whether a child's `/proc/<pid>/fd/<n>` readlink target is
/// literally the text `/dev/ptmx`. Not an fd identity check (`fstat`
/// dev/ino) -- every fd ever opened through `/dev/ptmx` reports that
/// device node's own *static* identity regardless of which distinct pty
/// instance it represents, confirmed by hand (`stat -L /dev/ptmx`), so an
/// identity match would report a "leak" on every single sample
/// regardless of whether anything actually leaked. A plain `sleep`
/// binary has no legitimate reason to ever hold a `/dev/ptmx` fd at all,
/// so any match proves inheritance regardless of which instance --
/// **once the child has actually `exec`ed into it**, which is its own,
/// separate check below.
///
/// **Quarantined and un-quarantined the same day, review 583: the CI-only
/// failure was this test's own premise, not the product fix.**
/// `FD_CLOEXEC` is honoured at `execve`, not at `fork` -- a forked-but-
/// not-yet-`exec`ed child's `/proc/<pid>/fd` still lists a properly
/// close-on-exec fd, because nothing has reached the point where the
/// kernel acts on the flag yet. Checking fd presence alone could not tell
/// "leaked" apart from "about to vanish, exactly as intended"; only
/// whether the child has already `exec`ed can, which is what
/// `process_comm` below checks. On a 2-4 vCPU CI runner the fork-to-exec
/// window is wide enough, relative to this test's own scan cadence, to
/// land inside it routinely; on this project's own 32-core development
/// machine it almost never did, which is why this never reproduced
/// locally. The pid-reuse theory raised first was independently
/// disproven (the recorded start time matched across every report), and
/// stays fixed below since it is a real, separate correctness property,
/// orthogonal to this one. **Re-ablated with the comm check in place**,
/// against the real pre-fix `pty.rs`: still fails loudly (2,974 leaks,
/// every one explicitly past its own `exec`), confirming the new gate
/// did not become vacuous and independently re-confirming the original
/// defect really did persist for the child's whole life, not only for a
/// fork-to-exec instant. Full account in
/// `rfcs/handoffs/test-process-leak.md`'s own dated entry.
#[test]
fn no_concurrently_forked_sibling_inherits_a_pty_master_fd() {
    use std::os::fd::AsRawFd;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    const MAX_CONCURRENT_CHILDREN: usize = 4;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);

    let keep_forking = Arc::new(AtomicBool::new(true));
    let fork_count = Arc::new(AtomicUsize::new(0));
    // `(pid, start_time)`, not a bare pid: a bare pid list is vulnerable to
    // a kernel reusing a reaped child's pid for an unrelated process
    // before this test's own scan loop gets back around to it, which
    // would misreport that unrelated process's own, entirely legitimate
    // `/dev/ptmx` fd (e.g. a *different*, concurrently-running test's own
    // real terminal) as this test's leak. `start_time` (the kernel's own
    // per-process monotonic creation tick, `/proc/<pid>/stat` field 22)
    // is recorded the instant this test spawns the child and re-checked
    // at scan time; a mismatch means the pid was recycled, not leaked into.
    let known_children: Arc<Mutex<Vec<(u32, u64)>>> = Arc::new(Mutex::new(Vec::new()));

    let forker_flag = keep_forking.clone();
    let forker_count = fork_count.clone();
    let forker_children = known_children.clone();
    let forker = std::thread::spawn(move || {
        let mut children: Vec<std::process::Child> = Vec::new();
        while forker_flag.load(Ordering::Relaxed) {
            children.retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));
            if children.len() < MAX_CONCURRENT_CHILDREN {
                if let Ok(child) = std::process::Command::new("sleep").arg("0.05").spawn() {
                    forker_count.fetch_add(1, Ordering::Relaxed);
                    if let Some(start_time) = process_start_time(child.id()) {
                        forker_children
                            .lock()
                            .unwrap()
                            .push((child.id(), start_time));
                    }
                    children.push(child);
                }
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        for mut child in children {
            let _ = child.wait();
        }
    });

    let mut leaks = Vec::new();
    let mut iterations = 0usize;
    while std::time::Instant::now() < deadline {
        iterations += 1;
        let pty = super::pty::OpenPty::new(TerminalDimensions::default()).expect("open pty");
        let master = pty.into_master();
        let _ = master.as_raw_fd(); // kept alive only to hold the master open briefly

        let candidates: Vec<(u32, u64)> = known_children.lock().unwrap().clone();
        for (pid, recorded_start_time) in candidates {
            // Re-verify identity before trusting the fd scan at all: if
            // the pid no longer exists, or now belongs to a process with
            // a different start time, it is not the child this test
            // spawned, regardless of what its `/proc/<pid>/fd` shows.
            if process_start_time(pid) != Some(recorded_start_time) {
                continue;
            }
            // Review 583: `FD_CLOEXEC` is honoured at `execve`, not at
            // `fork` -- a forked-but-not-yet-`exec`ed child's `/proc/
            // <pid>/fd` still lists a properly close-on-exec fd, because
            // nothing has reached the point where the kernel actually
            // acts on the flag yet. Checking fd presence alone cannot
            // tell "leaked" apart from "about to vanish, exactly as
            // intended" -- only whether the child has *already* exec'd
            // can. `/proc/<pid>/comm` says so directly: a child still
            // mid-fork carries this test binary's own name, not `sleep`.
            if process_comm(pid).as_deref() != Some("sleep") {
                continue;
            }
            let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
                continue; // already exited
            };
            for fd_entry in fds.flatten() {
                if std::fs::read_link(fd_entry.path())
                    .is_ok_and(|link| link.to_string_lossy() == "/dev/ptmx")
                {
                    leaks.push(format!(
                        "iteration {iterations}: child pid={pid} (start_time={recorded_start_time}, \
                         already exec'd into sleep) fd={:?} is /dev/ptmx -- a plain `sleep` binary \
                         never opens this itself",
                        fd_entry.file_name()
                    ));
                }
            }
        }
        drop(master);
    }

    keep_forking.store(false, Ordering::Relaxed);
    let total_forks = fork_count.load(Ordering::Relaxed);
    forker.join().unwrap();

    assert!(
        leaks.is_empty(),
        "{} forked sibling(s) inherited a PTY master fd across {iterations} iterations against \
         {total_forks} forks: {:?}",
        leaks.len(),
        &leaks[..leaks.len().min(5)]
    );
}

/// The kernel's own per-process creation tick (`/proc/<pid>/stat` field
/// 22, "starttime"), used as a cheap, reliable process identity check: a
/// pid can be recycled by the kernel, but two processes that ever
/// actually coexisted can never share both the same pid and the same
/// start time. Parsed past the last `)` rather than by a fixed field
/// index from the start -- `comm` (field 2) is the one field that can
/// itself contain spaces or parentheses, and `proc(5)` itself recommends
/// this as the robust way to skip past it.
fn process_start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    after_comm.split_whitespace().nth(19)?.parse().ok()
}

/// `/proc/<pid>/comm` -- the kernel's own record of the executable
/// currently running as `pid`, already trimmed of its own trailing
/// newline by the kernel. Review 583's own discriminator: a process that
/// has `fork`ed but not yet `exec`ed still carries its *parent's* comm,
/// not the name of whatever it is about to become.
fn process_comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|comm| comm.trim().to_string())
}

/// Review 625's own named experiment, decisive before any repair: review
/// 623's libproc datum (`proc_pidinfo`/`proc_listallpids` both blind to a
/// live, signalable PTY child, uniform across 13 real firings) could mean
/// libproc cannot see *this crate's PTY children specifically* (the
/// difference would then be inside `spawn_pty_child` -- `setsid`,
/// `TIOCSCTTY`, or review 614's own `fcntl` loop), or it could mean
/// libproc cannot see *any child this process spawns*, which is a
/// different repair (the enumeration mechanism itself would need
/// replacing, not debugging). A plain, PTY-less child
/// (`std::process::Command::spawn`, no `setsid`, no `TIOCSCTTY`) and a
/// normal PTY child are queried the identical way, side by side, in the
/// same real run -- not two separate runs that could each be measuring a
/// different moment of CI contention.
///
/// A third, non-libproc view of the same question:
/// `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_PID, pid)`. `CTL_KERN`,
/// `KERN_PROC` and `KERN_PROC_PID` are all in Apple's own `libc` crate
/// (confirmed against real Apple targets the standalone-crate way, the
/// same method this project already uses for every macOS-only syscall
/// this session added); `kinfo_proc`, the struct the real kernel would
/// fill in, is **not** -- confirmed the same way. Review 625 asked for it
/// declared by hand, the `ptsname_r` precedent (`pty.rs`) for a real
/// symbol `libc` omits. This experiment deliberately does **not** declare
/// it: `kinfo_proc`'s own real layout is a large, nested, version-stable
/// but entirely unverified-here struct, and getting a hand-reconstructed
/// field layout wrong (not merely a wrong numeric constant, `ptsname_r`'s
/// own rejected alternative) risks reading garbage silently on real
/// hardware this project cannot reach to check. The question this
/// experiment asks -- *does the kernel have a record of this pid at
/// all* -- does not need any field inside the struct: `sysctl` always
/// writes the same fixed, non-negotiable size for a pid it has a record
/// of, and reports exactly `0` bytes (or an error) when it does not. An
/// oversized, untyped byte buffer as `oldp` answers that question
/// completely, without parsing a single field of a struct this project
/// has no way to verify.
///
/// Proposes nothing. Prints both children's three-way answer; asserts
/// nothing about what they say.
#[test]
#[cfg(target_os = "macos")]
fn libproc_visibility_plain_child_vs_pty_child_and_a_non_libproc_view() {
    /// SAFETY: `kill` with signal `0` delivers nothing; `proc_pidinfo`'s
    /// `info` is a plain, fully-zeroed `proc_bsdinfo` the call writes
    /// into, `size` matches its own real size exactly; `sysctl`'s own
    /// `buffer` is valid and sized as declared for the duration of the
    /// call.
    ///
    /// Review 626's own two required additions: the written length is
    /// reported as its own datum (distinct from an error -- with an
    /// oversized buffer `ENOMEM` cannot occur, so "failed", "0 bytes"
    /// and "N bytes" are the whole outcome space, and a plausible N is
    /// itself a sanity check that this is really talking to the call it
    /// thinks it is); and, for `dump_hex_prefix` callers only, the first
    /// 64 raw bytes are printed as hex -- not parsed as any field, read
    /// back only as opaque bytes, so this is not the struct-layout risk
    /// this function's own doc comment above already declined. If
    /// `is_zombie` ever needs replacing, this dump lets a future reader
    /// learn `kinfo_proc`'s real layout **off the real hardware** instead
    /// of reconstructing it from headers -- the plain child only, which
    /// is expected to have a record; the PTY child may not.
    fn describe_pid_three_ways(pid: libc::pid_t, dump_hex_prefix: bool) -> String {
        let kill_result = unsafe { libc::kill(pid, 0) };
        let kill_label = if kill_result == 0 {
            "exists/signalable".to_owned()
        } else {
            format!("{:?}", std::io::Error::last_os_error())
        };

        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        let proc_pidinfo_result = unsafe {
            libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size)
        };
        let proc_pidinfo_label = if proc_pidinfo_result == size {
            "found".to_owned()
        } else {
            format!("failed: {:?}", std::io::Error::last_os_error())
        };

        let in_pid_list = list_all_pids().map(|pids| pids.contains(&pid));
        let in_pid_list_label = match in_pid_list {
            Some(true) => "yes".to_owned(),
            Some(false) => "no".to_owned(),
            None => "list_all_pids itself failed".to_owned(),
        };

        let mut mib: [libc::c_int; 4] = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
        // Generously over the real `struct kinfo_proc`'s own documented
        // size (a few hundred bytes on every Darwin release this
        // project has read about) -- see this function's own doc
        // comment for why the contents are never parsed.
        let mut buffer = vec![0_u8; 4096];
        let mut written_len: libc::size_t = buffer.len();
        let sysctl_result = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                buffer.as_mut_ptr().cast(),
                &raw mut written_len,
                std::ptr::null_mut(),
                0,
            )
        };
        let sysctl_label = if sysctl_result != 0 {
            format!("failed: {:?}", std::io::Error::last_os_error())
        } else if written_len == 0 {
            "0 bytes written (no record)".to_owned()
        } else {
            format!("{written_len} bytes written (found)")
        };

        let hex_prefix = if dump_hex_prefix && sysctl_result == 0 && written_len > 0 {
            let prefix_len = written_len.min(64);
            let hex = buffer[..prefix_len]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("; first {prefix_len} bytes (hex): {hex}")
        } else {
            String::new()
        };

        format!(
            "kill={kill_label}; proc_pidinfo={proc_pidinfo_label}; in_proc_listallpids=\
             {in_pid_list_label}; sysctl(KERN_PROC_PID)={sysctl_label}{hex_prefix}"
        )
    }

    let mut plain_child = std::process::Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .expect("a plain, PTY-less child should spawn");
    let plain_pid = plain_child.id() as libc::pid_t;

    let root = test_root("libproc-visibility");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = TerminalRuntime::new();
    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    wait_for_shell_ready(&mut runtime, &handle);
    let pty_pid = runtime
        .session(&handle)
        .expect("the just-launched session should still be findable")
        .child
        .id() as libc::pid_t;

    eprintln!(
        "[libproc-visibility] plain child (pid {plain_pid}): {}",
        describe_pid_three_ways(plain_pid, true)
    );
    eprintln!(
        "[libproc-visibility] pty child (pid {pty_pid}): {}",
        describe_pid_three_ways(pty_pid, false)
    );

    let _ = plain_child.kill();
    let _ = plain_child.wait();
    let _ = runtime.write_input(&handle, b"exit\n");
    let _ = runtime.wait_for_exit(&handle, Duration::from_secs(5));
    cleanup_root(root);
}
