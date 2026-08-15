use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::project::{ProjectId, ProjectSession};

use super::super::*;

#[test]
fn real_pty_output_reaches_the_channel_end_to_end() {
    let root = test_root("reader-basic");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");

    runtime
        .write_input(&handle, b"printf 'tekstide-reader-ok\\n'\nexit\n")
        .expect("marker command should write to PTY");

    let output = drain_until_contains(&reader, b"tekstide-reader-ok", Duration::from_secs(5));
    assert!(
        contains_subsequence(&output, b"tekstide-reader-ok"),
        "reader channel should carry real PTY output; captured: {}",
        String::from_utf8_lossy(&output)
    );

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    drop(reader);
    cleanup_root(root);
}

/// Response 201's required evidence: dropping a `TerminalReader` must
/// not depend on the child ever producing output or exiting. The child
/// here is real, alive, and deliberately never told to do anything --
/// the reader thread is parked in `poll(2)` on a live, silent PTY, the
/// exact case the `receiver`-drop path in `Drop` does not reach (that
/// path only unblocks a thread parked in `sender.send` on a full
/// channel). Runs the drop on its own thread and waits on it with a
/// real timeout, so a regression in `Drop` fails **this test** rather
/// than hanging the suite -- matching how the earlier `Drop`-ordering
/// deadlock was actually found (a test that hung for 30+ seconds).
#[test]
fn dropping_a_reader_over_a_live_silent_child_completes_promptly() {
    let root = test_root("reader-drop-live-child");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");

    // Let the reader thread settle into poll(2) with nothing to read --
    // the shell is alive but has not been sent any command, so it
    // produces no output on its own.
    std::thread::sleep(Duration::from_millis(50));

    let (done_sender, done_receiver) = mpsc::channel();
    let drop_thread = std::thread::spawn(move || {
        drop(reader);
        let _ = done_sender.send(());
    });

    if done_receiver.recv_timeout(Duration::from_secs(5)).is_err() {
        panic!(
            "dropping TerminalReader over a live, silent child did not complete within 5s -- \
             Drop is blocked, most likely joining a reader thread parked in poll(2) that \
             nothing woke up"
        );
    }
    let _ = drop_thread.join();

    // The child is still alive -- dropping the reader stops the reader
    // thread, not the terminal session. Terminate it directly rather
    // than via the (now reader-less) session's own output stream.
    runtime
        .request_terminate(
            &handle,
            TerminationRequest {
                source: TerminationRequestSource::TestHarness,
                reason: BoundedRuntimeSummary::new(
                    "cleanup after drop-over-live-silent-child liveness test",
                ),
            },
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .expect("termination should succeed");
    cleanup_root(root);
}

/// PR-A1-A's own gate: "a dedicated thread blocks on PTY readiness; no
/// sleep, no busy-wait." A busy-wait would burn CPU while nothing is
/// happening on the PTY; a real `poll(2)`-parked thread would not.
/// Measured, not asserted from the mechanism's description alone.
#[test]
fn reader_thread_does_not_busy_wait_while_idle() {
    let root = test_root("reader-idle-cpu");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");
    let tid = reader.os_thread_id();

    // Let the thread settle into its first poll(2) call before measuring.
    std::thread::sleep(Duration::from_millis(50));
    let before_ticks = thread_cpu_ticks(tid);
    std::thread::sleep(Duration::from_millis(300));
    let after_ticks = thread_cpu_ticks(tid);

    let delta_ticks = after_ticks.saturating_sub(before_ticks);
    assert!(
        delta_ticks <= 2,
        "reader thread accumulated {delta_ticks} CPU clock ticks while the PTY was idle for \
         300ms -- a thread genuinely parked in poll(2) should accumulate close to none; a \
         busy-wait would burn most of that window as CPU time"
    );

    runtime
        .write_input(&handle, b"exit\n")
        .expect("cleanup exit should write to PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    drop(reader);
    cleanup_root(root);
}

/// PR-A1-A's own gate: "the UI thread never blocks. Show it, do not
/// assert it." Calls `drain_available` repeatedly while a real shell
/// produces output continuously and faster than we drain, and measures
/// each individual call's wall time rather than trusting
/// `mpsc::Receiver::try_recv`'s documented non-blocking behaviour.
#[test]
fn drain_available_never_blocks_the_caller_even_under_sustained_production() {
    let root = test_root("reader-nonblocking-drain");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");

    runtime
        .write_input(
            &handle,
            b"while true; do printf 'tekstide-flood-line-abcdefghijklmnopqrstuvwxyz\\n'; done\n",
        )
        .expect("flood command should write to PTY");

    // Give the flood a head start so the channel is genuinely under
    // production pressure (and likely full) while we measure drains.
    std::thread::sleep(Duration::from_millis(100));

    let mut max_call_duration = Duration::ZERO;
    for _ in 0..200 {
        let started = Instant::now();
        let _ = reader.drain_available();
        let elapsed = started.elapsed();
        if elapsed > max_call_duration {
            max_call_duration = elapsed;
        }
    }

    assert!(
        max_call_duration < Duration::from_millis(20),
        "drain_available took up to {max_call_duration:?} under sustained production -- the UI \
         thread must never wait for the reader thread to produce more"
    );

    // Stop the flood by killing the process group directly; the reader
    // thread unblocks once the PTY reports end-of-file.
    runtime
        .request_terminate(
            &handle,
            TerminationRequest {
                source: TerminationRequestSource::TestHarness,
                reason: BoundedRuntimeSummary::new(
                    "stop the flood after non-blocking drain measurement",
                ),
            },
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .expect("termination should succeed");
    drop(reader);
    cleanup_root(root);
}

/// PR-A1-A's own gate: "backpressure demonstrated end to end: a
/// producer faster than the consumer stalls on write() and resumes
/// correctly, with no byte loss across the stall." This is also the
/// evidence for `dropped_bytes` being unreachable: the reader module
/// has no truncation logic at all, so the only way to show that
/// honestly is to prove fidelity across a real stall, not to grep for
/// an absent field name.
///
/// Isolates the measured payload with `START`/`END` markers so PTY echo
/// of the command line itself (which happens before `START`) cannot
/// contaminate the byte count. Both markers are matched with their real,
/// `ONLCR`-translated `\r\n` line endings -- see the comment above the
/// first marker search below for why a bare `\n` (let alone a bare
/// `END`) is wrong here.
#[test]
fn backpressure_stalls_the_producer_and_resumes_with_no_byte_loss_across_the_stall() {
    let root = test_root("reader-backpressure");
    let project = project_session(ProjectId::for_test(1), &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");

    const PAYLOAD_KIB: usize = 2048; // 2 MiB -- well over the ~512 KiB channel bound.
    let expected_payload_len = PAYLOAD_KIB * 1024;
    let command = format!(
        "printf 'START\\n'; dd if=/dev/zero bs=1024 count={PAYLOAD_KIB} 2>/dev/null | tr '\\0' 'a'; printf '\\nEND\\n'\n"
    );
    runtime
        .write_input(&handle, command.as_bytes())
        .expect("payload command should write to PTY");

    // Deliberately do not drain. If backpressure works, the channel and
    // the kernel PTY buffer both fill, the reader thread stops calling
    // read(2), and dd/tr block on write(2) -- so a real `\r\nEND` must
    // not appear yet.
    //
    // Two things make a naive `contains_subsequence(_, b"END")` the
    // wrong check, both found by this test itself failing in ways that
    // pointed at a test bug rather than a real one:
    //
    // 1. The shell's local echo repeats the raw *command line* back
    //    before anything runs, and that command line's own source text
    //    contains the literal characters `\`, `n`, `E`, `N`, `D` (the
    //    `printf '\nEND\n'` argument, unevaluated) almost immediately --
    //    a bare `END` search matches that echoed argument text, well
    //    before the real `printf` output has any chance to run.
    // 2. The real `printf` output's `\n` is not delivered as a bare LF:
    //    `ONLCR` (on by default) translates outgoing LF to CRLF, so the
    //    actual bytes are `START\r\n` and `\r\nEND\r\n`, never a bare
    //    `\n`-only newline. A marker search using `b"START\n"` never
    //    matches the real output either, for the same reason in reverse.
    std::thread::sleep(Duration::from_millis(300));
    let undrained = reader.drain_available();
    assert!(
        !contains_subsequence(undrained.bytes(), b"\r\nEND"),
        "the producer should still be stalled on write() after 300ms with nothing draining the \
         channel; seeing a real \\r\\nEND this early means backpressure did not actually stall it"
    );

    let mut output = undrained.into_bytes();
    let drain_started = Instant::now();
    while !contains_subsequence(&output, b"\r\nEND")
        && drain_started.elapsed() < Duration::from_secs(10)
    {
        let drain = reader.drain_available();
        output.extend_from_slice(drain.bytes());
        if drain.bytes().is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    let payload = extract_between(&output, b"START\r\n", b"\r\nEND").unwrap_or_else(|| {
        panic!(
            "captured output should contain START/END markers; captured: {}",
            String::from_utf8_lossy(&output)
        )
    });
    assert_eq!(
        payload.len(),
        expected_payload_len,
        "payload length should survive the stall exactly -- a shorter length means bytes were \
         dropped, a longer one means something duplicated"
    );
    assert!(
        payload.iter().all(|&byte| byte == b'a'),
        "payload should be entirely the 'a' fill byte with no corruption"
    );

    runtime
        .write_input(&handle, b"exit\n")
        .expect("cleanup exit should write to PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    drop(reader);
    cleanup_root(root);
}

fn thread_cpu_ticks(tid: i32) -> u64 {
    let path = format!("/proc/self/task/{tid}/stat");
    let content = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("reader thread's own /proc stat should be readable at {path}: {error}")
    });
    let after_comm = content
        .rsplit_once(')')
        .expect("stat should contain the comm field in parentheses")
        .1;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // Fields after `comm)` start at field 3 (state) as index 0, so
    // utime (field 14) is index 11 and stime (field 15) is index 12.
    let utime: u64 = fields[11].parse().expect("utime field should be numeric");
    let stime: u64 = fields[12].parse().expect("stime field should be numeric");
    utime + stime
}

fn drain_until_contains(reader: &TerminalReader, marker: &[u8], timeout: Duration) -> Vec<u8> {
    let started = Instant::now();
    let mut output = Vec::new();

    while started.elapsed() < timeout {
        let drain = reader.drain_available();
        output.extend_from_slice(drain.bytes());
        if contains_subsequence(&output, marker) {
            return output;
        }
        if drain.bytes().is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    output
}

fn extract_between(haystack: &[u8], start_marker: &[u8], end_marker: &[u8]) -> Option<Vec<u8>> {
    let start_index = find_subsequence(haystack, start_marker)? + start_marker.len();
    let end_index = find_subsequence(&haystack[start_index..], end_marker)? + start_index;
    Some(haystack[start_index..end_index].to_vec())
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    find_subsequence(haystack, needle).is_some()
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
