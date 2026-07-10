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
fn linux_runtime_terminates_process_group_with_sigterm() {
    let root = test_root("terminate-sigterm");
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
            signal: TerminationSignal::Sigterm,
        })
    );
    assert_eq!(
        events.last(),
        Some(&TerminalRuntimeEvent::Terminated {
            handle: handle.clone(),
            outcome: TerminationOutcome::TerminatedBySignal {
                signal: TerminationSignal::Sigterm,
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

#[test]
fn linux_runtime_uses_sigkill_fallback_for_foreground_child_after_sigterm_timeout() {
    let root = test_root("terminate-sigkill-fallback");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    runtime
        .write_input(&handle, b"printf 'sleep-started\\n'; sleep 30\n")
        .expect("foreground child command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"sleep-started");
    assert!(
        contains_subsequence(&output, b"sleep-started"),
        "PTY output should contain foreground-child marker; captured: {}",
        String::from_utf8_lossy(&output)
    );

    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("force foreground child fallback smoke"),
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
            signal: TerminationSignal::Sigterm,
        })
    );
    assert!(events.contains(&TerminalRuntimeEvent::TerminationTimedOut {
        handle: handle.clone(),
        after_signal: TerminationSignal::Sigterm,
    }));
    assert!(
        events.contains(&TerminalRuntimeEvent::TerminationSignalSent {
            handle: handle.clone(),
            signal: TerminationSignal::Sigkill,
        })
    );
    assert_eq!(
        events.last(),
        Some(&TerminalRuntimeEvent::Terminated {
            handle,
            outcome: TerminationOutcome::KilledAfterTimeout {
                initial_signal: TerminationSignal::Sigterm,
                fallback_signal: TerminationSignal::Sigkill,
            },
        })
    );
    cleanup_root(root);
}

#[test]
fn linux_runtime_does_not_overclaim_when_child_outlives_direct_shell_after_sigterm() {
    let root = test_root("terminate-descendant-outlives-shell");
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
            b"(trap '' TERM; printf 'descendant-ready\\n'; while :; do sleep 1; done) & wait\n",
        )
        .expect("SIGTERM-ignoring descendant command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"descendant-ready");
    assert!(
        contains_subsequence(&output, b"descendant-ready"),
        "PTY output should contain descendant marker; captured: {}",
        String::from_utf8_lossy(&output)
    );

    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("descendant outlives direct shell smoke"),
    };
    let events = runtime
        .request_terminate(
            &handle,
            request,
            Duration::from_millis(100),
            Duration::from_secs(2),
        )
        .expect("termination request should continue to SIGKILL fallback");

    assert!(events.contains(&TerminalRuntimeEvent::TerminationTimedOut {
        handle: handle.clone(),
        after_signal: TerminationSignal::Sigterm,
    }));
    assert!(
        events.contains(&TerminalRuntimeEvent::TerminationSignalSent {
            handle: handle.clone(),
            signal: TerminationSignal::Sigkill,
        })
    );
    assert_eq!(
        events.last(),
        Some(&TerminalRuntimeEvent::Terminated {
            handle,
            outcome: TerminationOutcome::KilledAfterTimeout {
                initial_signal: TerminationSignal::Sigterm,
                fallback_signal: TerminationSignal::Sigkill,
            },
        })
    );
    cleanup_root(root);
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
