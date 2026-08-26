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

    let error = LinuxTerminalRuntime::new()
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

    let error = LinuxTerminalRuntime::new()
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

    let error = LinuxTerminalRuntime::new()
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

    let error = LinuxTerminalRuntime::new()
        .launch_project_shell(&project, spec)
        .expect_err("non-executable shell path must be rejected");

    assert!(matches!(
        error,
        TerminalLaunchError::ShellUnavailable { .. }
    ));
    cleanup_root(root);
}

#[test]
fn linux_runtime_launches_project_shell_and_reads_marker() {
    let root = test_root("launch-shell");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

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
    let root = test_root("cross-project-input");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

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

    runtime
        .write_input(&handle, b"exit\n")
        .expect("cleanup exit should write to PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    cleanup_root(root);
}

#[test]
fn linux_runtime_reads_output_through_bounded_buffer() {
    let root = test_root("bounded-output");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

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
    let mut runtime = LinuxTerminalRuntime::new();

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

#[test]
fn linux_runtime_terminates_session_leader_with_sighup() {
    let root = test_root("terminate-sighup");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Cat", &root, "/bin/cat");
    let mut runtime = LinuxTerminalRuntime::new();

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
    assert_eq!(
        events.last(),
        Some(&TerminalRuntimeEvent::Terminated {
            handle: handle.clone(),
            outcome: TerminationOutcome::TerminatedBySignal {
                signal: TerminationSignal::Sighup,
            },
        })
    );

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
    let root = test_root("cross-project-terminate");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

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

    runtime
        .write_input(&handle, b"exit\n")
        .expect("cleanup exit should write to PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
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
    let mut runtime = LinuxTerminalRuntime::new();

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
    let root = test_root("terminate-real-backgrounded-job");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    runtime
        .write_input(&handle, b"sleep 300 & echo BGPID=$!\n")
        .expect("backgrounded sleep command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"BGPID=");
    let background_pid = parse_bgpid(&output)
        .expect("real backgrounded job's own pid should be parseable from PTY output");
    assert!(
        crate::test_support::process_is_alive(background_pid),
        "test precondition: the real backgrounded job must be alive before the close"
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
    assert!(
        !crate::test_support::process_is_alive(background_pid),
        "an OS-level kill(pid, 0) on the real backgrounded job's own pid, after a real close, \
         must find it gone -- not inferred from the returned events"
    );
    cleanup_root(root);
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
    let root = test_root("terminate-setsid-survives");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    // `setsid --fork`, not a bare `setsid sleep 300 &`: this job is
    // already its own process group leader by the time it runs (bash's
    // own job-control backgrounding put it there), and `setsid(2)`
    // itself refuses to run on a process that already leads its own
    // group -- confirmed directly, not assumed (a first attempt without
    // `--fork` had the job die immediately with no visible error, since
    // `setsid`'s own failure message went to a stderr write that raced
    // past this test's own read). `--fork` is `setsid`'s own answer to
    // exactly this case. `$!` from the outer `setsid` invocation would
    // only be *its* pid, which exits the moment it forks -- `sh -c
    // 'echo ...; exec sleep 300'` inside the forked child reports its
    // own pid before `exec` replaces it with `sleep`, keeping the same
    // pid throughout, which is the one this test actually needs.
    runtime
        .write_input(
            &handle,
            b"setsid --fork sh -c 'echo DETACHEDPID=$$; exec sleep 300' &\n",
        )
        .expect("setsid-detached command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"DETACHEDPID=");
    let detached_pid = parse_pid_after(&output, "DETACHEDPID=")
        .expect("real setsid-detached job's own pid should be parseable from PTY output");
    assert!(
        crate::test_support::process_is_alive(detached_pid),
        "test precondition: the real detached job must be alive before the close"
    );

    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("setsid opt-out smoke"),
    };
    let _ = runtime
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
    // Not this test's own cleanup responsibility to kill -- proving the
    // survival is the point -- but leaving a real `sleep 300` running
    // for the rest of the test binary's life is its own kind of leak.
    unsafe {
        libc::kill(detached_pid as libc::pid_t, libc::SIGKILL);
    }
    cleanup_root(root);
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
    let mut runtime = LinuxTerminalRuntime::new();

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
/// session or `request_terminate` call at all: [`test_proc_root`] points
/// the real enumeration at a directory that cannot be listed, forcing
/// `processes_in_session` to return `None` deterministically -- the
/// *other* honest-`false` case `what-containment-must-not-become.md` §4
/// names explicitly ("a `/proc` read that failed"). Found dishonest
/// while writing this very test: an earlier version of
/// `session_confirmed_empty` defaulted a failed read to an empty `Vec`,
/// which is precisely the unearned "almost certainly empty" confidence
/// that document forbids.
#[test]
fn session_confirmed_empty_reports_false_when_its_own_enumeration_fails() {
    let unreadable_proc = test_root("session-confirmed-empty-unreadable-proc");
    std::fs::remove_dir(&unreadable_proc).expect("directory should be removable to force ENOENT");
    let _proc_root_guard = super::launch::test_proc_root(&unreadable_proc);

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
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

fn read_until_contains(
    runtime: &mut LinuxTerminalRuntime,
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
/// `LinuxTerminalRuntime`-launched terminal instead. `Drop::drop` runs
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
    let mut runtime = LinuxTerminalRuntime::new();
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
    let mut runtime = LinuxTerminalRuntime::new();

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
    let mut runtime = LinuxTerminalRuntime::new();

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
