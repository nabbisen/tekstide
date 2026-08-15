use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::domain::AgentRunId;
use crate::project::{ProjectId, ProjectSession};
use crate::transcript::{
    TranscriptCaptureMode, TranscriptPathRequest, TranscriptPathResolver,
    TranscriptRetentionLimits, TranscriptRetentionState, TranscriptStoragePath,
    TranscriptWriteSummary, TranscriptWriterConfig,
};

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

/// RFC-017 Amendment 1, PR-A1-C: the wake notifier must actually wake on
/// real reader-thread activity, not merely compile. Real PTY output,
/// real blocking wait, run on its own thread and joined with a bounded
/// timeout so a regression fails this test rather than hanging the
/// suite.
#[test]
fn the_wake_notifier_wakes_when_real_pty_output_arrives() {
    let root = test_root("reader-wake-data");
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
    let notifier = reader
        .try_clone_wake_notifier()
        .expect("wake notifier should clone against a live reader");

    runtime
        .write_input(&handle, b"printf 'tekstide-wake-ok\\n'\nexit\n")
        .expect("marker command should write to PTY");

    let (done_sender, done_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let more_coming = notifier.block_until_woken();
        let _ = done_sender.send(more_coming);
    });

    match done_receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(more_coming) => assert!(
            more_coming,
            "the first wake from real PTY output should report more wakes may still come, not \
             that the reader has already stopped"
        ),
        Err(_) => panic!(
            "block_until_woken did not return within 5s against real PTY output -- the wake \
             eventfd is not being signalled on a successful send"
        ),
    }

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    drop(reader);
    cleanup_root(root);
}

/// RFC-017 Amendment 1, PR-A1-C, response 205's second required
/// constraint: the wake must fire on EOF/termination, not only on a
/// successful send. A child that exits without producing any output of
/// its own must still wake a waiting caller exactly once more, or
/// `check_exit()` never runs for that pane and the session bar keeps
/// reporting a dead shell as running forever -- the exact regression
/// the terminal-launch-UX slice was written to fix.
#[test]
fn the_wake_notifier_delivers_a_final_wake_and_then_reports_no_more_are_coming() {
    let root = test_root("reader-wake-final");
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
    let notifier = reader
        .try_clone_wake_notifier()
        .expect("wake notifier should clone against a live reader");

    runtime
        .write_input(&handle, b"exit\n")
        .expect("exit command should write to PTY");

    let (done_sender, done_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        // Zero or more `true` wakes first (the echoed "exit" line's own
        // bytes), always followed by exactly one `false` once the child
        // is gone.
        loop {
            if !notifier.block_until_woken() {
                let _ = done_sender.send(());
                return;
            }
        }
    });

    if done_receiver.recv_timeout(Duration::from_secs(5)).is_err() {
        panic!(
            "the wake notifier never reported the reader had stopped within 5s after the \
             child exited -- EOF is not reaching the wake signal"
        );
    }

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    drop(reader);
    cleanup_root(root);
}

/// RFC-017 Amendment 1, PR-A1-C: the shutdown path (`Drop` over a live,
/// silent child, not a child exiting on its own) must also deliver a
/// final wake -- otherwise a `WakeNotifier` cloned before the pane
/// closes would block forever with nothing left to ever signal it,
/// which is exactly the bridging-thread leak a caller on the other end
/// of this notifier needs to avoid.
///
/// The shell's own startup output (a real, visible prompt -- confirmed
/// separately by this session's own GUI evidence for PR-A1-B) produces
/// at least one ordinary wake before this test ever calls `Drop`.
/// Consumed synchronously and asserted `true` first, deliberately, so
/// that wake cannot land close enough in time to the shutdown-triggered
/// one to be mistaken for it -- eventfd's own counter accumulates
/// pending signals into one value, so a background thread that only
/// checks "did I eventually see `false`" could pass even with the
/// shutdown path's own signal ablated, if an unrelated earlier wake's
/// `read(2)` happened to land after `reader_alive` had already flipped.
/// Found by ablating this test's own predecessor and watching it pass
/// anyway.
#[test]
fn dropping_a_reader_over_a_live_silent_child_also_delivers_a_final_wake() {
    let root = test_root("reader-wake-on-drop");
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
    let notifier = reader
        .try_clone_wake_notifier()
        .expect("wake notifier should clone against a live reader");

    std::thread::sleep(Duration::from_millis(200));
    assert!(
        notifier.block_until_woken(),
        "the shell's own startup output should produce an ordinary wake reporting the reader \
         still alive, consumed here before Drop so it cannot be confused with the final one"
    );

    let (done_sender, done_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let more_coming = notifier.block_until_woken();
        let _ = done_sender.send(more_coming);
    });

    drop(reader);

    match done_receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(more_coming) => assert!(
            !more_coming,
            "the wake immediately after Drop should report the reader has stopped for good, \
             not that more wakes may still come"
        ),
        Err(_) => panic!(
            "the wake notifier never reported the reader had stopped within 5s after Drop -- \
             the shutdown path is not signalling the wake eventfd, which would leak the \
             bridging thread on the other end of any WakeNotifier for the pane's whole session"
        ),
    }

    runtime
        .request_terminate(
            &handle,
            TerminationRequest {
                source: TerminationRequestSource::TestHarness,
                reason: BoundedRuntimeSummary::new("cleanup after wake-on-drop test"),
            },
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .expect("termination should succeed");
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

/// RFC-011 Amendment 2, PR-A2-A: "a transcript written through the new
/// path is byte-identical to the PTY output, proven against a real
/// child process." Drains the reader's channel to completion (the same
/// bytes a real consumer would ever see) and compares it, byte for
/// byte, against what landed in the transcript file on disk -- both are
/// reading the *same* raw PTY stream the reader thread saw, so nothing
/// short of an actual bug (a chunk written but not sent, or vice versa)
/// could make them differ.
/// **Found live, under full-workspace-suite contention, and fixed**: the
/// first version of this test used `drain_until_contains` to collect
/// `drained`, which returns as soon as the marker text appears -- before
/// the reader thread has necessarily delivered *later* chunks (the
/// echoed `exit\r\n`, any trailing bytes) to the channel, since whether
/// those bytes land in the same read chunk as the marker or a later one
/// depends on real scheduling timing. Comparing against a channel drain
/// taken that early produced a genuine, reproducible one-line-short
/// mismatch once under contention (`drained` missing the shell's own
/// `exit\r\n` echo that the transcript file still had) -- disclosed
/// here rather than silently patched; see `qa-evidence.md`. Fixed by
/// waiting for the reader's own wake notifier to report no more wakes
/// are coming (the same "reader has permanently stopped" signal
/// `the_wake_notifier_delivers_a_final_wake_and_then_reports_no_more_are_coming`
/// already proves is accurate) before draining, so nothing sent after
/// the marker but before the reader's own completion can be missed.
#[test]
fn transcript_written_through_the_reader_thread_is_byte_identical_to_pty_output() {
    let (mut runtime, handle, storage, dirs) =
        launch_with_transcript_capture("byte-identical", TranscriptCaptureMode::LocalBounded);
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");
    let notifier = reader
        .try_clone_wake_notifier()
        .expect("wake notifier should clone against a live reader");

    runtime
        .write_input(&handle, b"printf 'tekstide-transcript-marker\\n'\nexit\n")
        .expect("marker command should write to PTY");

    let (done_sender, done_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            if !notifier.block_until_woken() {
                let _ = done_sender.send(());
                return;
            }
        }
    });
    if done_receiver.recv_timeout(Duration::from_secs(5)).is_err() {
        panic!(
            "the reader never reported it had permanently stopped within 5s after the marker \
             command exited -- EOF is not reaching the wake signal"
        );
    }

    let drained = reader.drain_available().into_bytes();
    assert!(
        contains_subsequence(&drained, b"tekstide-transcript-marker"),
        "the channel should carry the real marker output; drained: {}",
        String::from_utf8_lossy(&drained)
    );

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    drop(reader);

    let transcript_bytes =
        std::fs::read(storage.transcript_file()).expect("transcript file should be readable");
    assert_eq!(
        transcript_bytes, drained,
        "the transcript must be byte-identical to what the consumer actually saw -- any \
         difference here means the reader thread's write path and its channel-send path \
         disagree about what the PTY produced"
    );
    drop(dirs);
    cleanup_root(storage.transcript_dir().to_path_buf());
}

/// RFC-011 Amendment 2, PR-A2-A, D2: "a test that observes the record
/// contains bytes the consumer has not yet drained" -- proven exactly,
/// per byte, not by racing wall-clock timing against a microsecond-scale
/// write (tried first; see below for why it does not work) and not by
/// an aggregate byte-count threshold either (also tried first; also
/// does not work, for a more interesting reason).
///
/// **Why a real PTY payload plus a byte-count threshold does not work.**
/// The first attempt wrote a large payload and asserted the transcript
/// exceeded the reader's own ~512 KiB channel bound
/// (`CHANNEL_CAPACITY * READ_CHUNK_BYTES`) before ever draining. In
/// practice the transcript plateaus far below that -- this environment's
/// real PTY throughput stalls *upstream* of the channel (kernel PTY/line-
/// discipline buffering, well under a single `READ_CHUNK_BYTES`
/// chunk's worth), so the channel itself never fills against a real
/// child process at all. Switching to a **named pipe (FIFO)** standing
/// in for the transcript file fixed that: its own kernel buffer (a real
/// pipe, ~64 KiB) reliably fills once nothing reads it, so
/// `self.writer.append(...)`'s `write_all` reliably blocks **inside
/// `record_write`**. But an aggregate "total bytes ever drained" bound
/// *still* does not distinguish orderings: the reader thread is single-
/// threaded and sequential, so a block *anywhere* inside one chunk's
/// processing halts every later chunk equally, regardless of which side
/// of the send it sits on -- a send-before-write reordering only ever
/// lets **one extra chunk** (at most `READ_CHUNK_BYTES`) through before
/// hitting the same wall, nowhere near the channel's own much larger
/// bound a threshold-based assertion was checking against.
///
/// **What actually distinguishes it**: `FIONREAD` (`fionread`, below)
/// reports exactly how many bytes are queued in the FIFO's kernel
/// buffer *without consuming them* -- precisely how many bytes have
/// really been committed to the transcript so far, independent of
/// anything this test drains from the channel. With the real ordering,
/// nothing can ever be sent that was not already committed first, so
/// `drained_total <= written_to_transcript` always holds, exactly, no
/// margin needed.
///
/// **Ablated**: swapping this call site to send *before* calling
/// `ReaderTranscriptState::record_write` (rather than after, as written)
/// made this test fail with a concrete, real violation: 142,148 bytes
/// drained against only 52,705 bytes ever committed to the transcript --
/// the channel visibly ahead of the record it is supposed to be a
/// subset of. Restored afterward.
#[test]
fn transcript_write_blocking_also_blocks_every_later_send() {
    let dirs = TestDirs::new("d2-ordering-fifo");
    let request = TranscriptPathRequest::new(
        &dirs.state_root,
        &dirs.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );
    let storage = TranscriptPathResolver
        .resolve_agent_run(request)
        .expect("test storage path should resolve");
    std::fs::create_dir_all(storage.transcript_dir()).expect("transcript dir should be creatable");
    make_fifo(storage.transcript_file());

    // Rendezvous with the writer's own blocking `open()` -- a FIFO's
    // write side does not unblock `open` until a reader is also open.
    // This reader thread then does nothing further: it never calls
    // `read`, so the FIFO's kernel buffer is the only thing absorbing
    // bytes from here on, exactly the deterministic small capacity this
    // test depends on.
    let fifo_path = storage.transcript_file().to_path_buf();
    let fifo_reader = std::thread::spawn(move || {
        fs::File::open(&fifo_path).expect("fifo should be openable for reading")
    });

    let project = project_session(ProjectId::for_test(1), &dirs.project_root);
    let mut spec = TerminalLaunchSpec::plain_shell(
        project.id().clone(),
        "Shell",
        &dirs.project_root,
        "/bin/sh",
    );
    spec.set_transcript_writer_config(Some(TranscriptWriterConfig::new(
        storage.clone(),
        TranscriptRetentionLimits::agent_run_default(),
        TranscriptCaptureMode::LocalBounded,
    )));
    let mut runtime = LinuxTerminalRuntime::new();
    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("transcript-capturing shell launch should succeed (fifo reader must be open)");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");
    let _fifo_reader_file = fifo_reader
        .join()
        .expect("fifo reader thread should not panic");

    // A real PTY payload well over the FIFO's own small kernel-buffer
    // capacity, but nowhere near the channel's own much larger one --
    // `yes` writes continuously and directly (no intermediate pipe stage
    // of its own to become an earlier bottleneck).
    runtime
        .write_input(
            &handle,
            b"yes aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        )
        .expect("payload command should write to PTY");

    // Deliberately never drain until the check below. Poll a single
    // `drain_available` repeatedly (each call itself never blocks, per
    // this module's own contract) until its cumulative total stops
    // growing for a sustained window -- the real stall signature, not a
    // guessed sleep duration.
    let mut drained_total = 0_usize;
    let mut stable_since = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(10);
    while stable_since.elapsed() < Duration::from_millis(500) && Instant::now() < deadline {
        let drain = reader.drain_available();
        if !drain.bytes().is_empty() {
            drained_total += drain.bytes().len();
            stable_since = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // `FIONREAD` reports how many bytes are currently queued in the
    // FIFO's own kernel buffer *without consuming them* -- exactly "how
    // much has actually been committed to the transcript, still
    // sitting there," independent of anything this test has drained
    // from the channel. Single-threaded, sequential execution inside
    // the reader thread means *any* block halts every later chunk
    // equally regardless of which side of it the block sits on, so the
    // aggregate totals alone do not distinguish orderings (tried first,
    // found not to -- see `qa-evidence.md`'s PR-A2-A section). This
    // exact, per-byte comparison does: with the real ordering, nothing
    // can ever be sent that was not already committed to the transcript
    // first.
    let written_to_transcript = fionread(&_fifo_reader_file);
    assert!(
        drained_total <= written_to_transcript,
        "the channel drained {drained_total} bytes but only {written_to_transcript} bytes were \
         ever actually committed to the transcript (queued in the FIFO's own kernel buffer) -- \
         the channel must never get ahead of the transcript it is supposed to be a subset of; \
         if it does, some byte was sent before its own write ever reached the transcript"
    );

    // Cleanup: `yes` never exits on its own, so kill it first (stops
    // further production). The drain and the drop must then run
    // *concurrently*, not one after the other: draining the FIFO to EOF
    // only completes once the reader thread's writer closes, which only
    // happens once the reader thread notices shutdown -- but it cannot
    // notice shutdown until its *current* blocked `write_all` unblocks,
    // which requires the drain to keep consuming. Sequencing "drain
    // fully, then drop" makes each side wait on the other under
    // scheduling contention: this thread's `read()` blocks (write end
    // still open, no more data yet) while the reader thread's `write()`
    // blocks (FIFO still full, nothing draining it) -- a real deadlock,
    // not a hang specific to this environment (found via a genuine hang
    // under concurrent build load, not reasoned about in the abstract).
    // Running the drop on its own thread, bounded by a timeout matching
    // `dropping_a_reader_over_a_live_silent_child_completes_promptly`'s
    // own established pattern, breaks the cycle: shutdown is signalled
    // immediately, so the reader thread proceeds as soon as this
    // thread's ongoing drain unblocks its current write.
    let request = TerminationRequest {
        source: TerminationRequestSource::TestHarness,
        reason: BoundedRuntimeSummary::new("d2 fifo ablation test cleanup"),
    };
    let _ = runtime.request_terminate(
        &handle,
        request,
        Duration::from_secs(2),
        Duration::from_secs(2),
    );

    let mut fifo_reader_file = _fifo_reader_file;
    let drain_thread = std::thread::spawn(move || {
        let mut sink = [0_u8; 4096];
        while fifo_reader_file.read(&mut sink).unwrap_or(0) > 0 {}
    });

    let (done_sender, done_receiver) = mpsc::channel();
    let drop_thread = std::thread::spawn(move || {
        drop(reader);
        let _ = done_sender.send(());
    });
    if done_receiver.recv_timeout(Duration::from_secs(10)).is_err() {
        panic!(
            "dropping the reader after the d2 ordering stall did not complete within 10s -- \
             the reader thread is stuck somewhere the shutdown signal and a concurrent FIFO \
             drain should both reach"
        );
    }
    let _ = drop_thread.join();
    drain_thread
        .join()
        .expect("fifo drain thread should not panic");
    drop(dirs);
}

/// `ioctl(fd, FIONREAD, ...)`: bytes currently queued and readable on
/// `file`, without consuming any of them -- the non-destructive way to
/// observe "how much has the writer actually committed so far."
fn fionread(file: &fs::File) -> usize {
    use std::os::fd::AsRawFd;
    let mut available: libc::c_int = 0;
    let result = unsafe { libc::ioctl(file.as_raw_fd(), libc::FIONREAD, &mut available) };
    assert_eq!(result, 0, "FIONREAD should succeed on a real fifo fd");
    available.max(0) as usize
}

fn make_fifo(path: &Path) {
    let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .expect("transcript path must not contain a NUL byte");
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    assert_eq!(
        result,
        0,
        "mkfifo should succeed for a fresh test path: {}",
        io::Error::last_os_error()
    );
}

/// RFC-011 Amendment 2, PR-A2-A, D1: the replacement for the old
/// `LinuxTerminalRuntime::transcript_write_summary` must not silently
/// return `None` and look like "no transcript configured" once the
/// writer moves into the reader thread. `None` here means capture was
/// genuinely never requested for this terminal -- the ordinary case for
/// every plain-shell launch in this crate today.
#[test]
fn transcript_write_summary_is_none_when_capture_was_never_configured() {
    let root = test_root("reader-transcript-summary-none");
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

    assert_eq!(
        reader.transcript_write_summary(),
        None,
        "a plain-shell launch never configures transcript capture, so the summary must stay \
         None -- not a zero-byte Some, which would look configured when it is not"
    );

    drop(reader);
    cleanup_root(root);
}

/// The positive case for the same replacement: once capture is
/// configured, `TerminalReader::transcript_write_summary` must reflect
/// real writes as they land, queried through the reader itself (D1's
/// chosen mechanism) rather than the runtime, which no longer has
/// anything to consult once the writer moves.
#[test]
fn transcript_write_summary_reflects_real_writes_through_the_reader() {
    let (mut runtime, handle, _storage, dirs) = launch_with_transcript_capture(
        "summary-reflects-writes",
        TranscriptCaptureMode::LocalBounded,
    );
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");

    assert_eq!(
        reader.transcript_write_summary(),
        Some(TranscriptWriteSummary {
            byte_count: 0,
            retention_state: TranscriptRetentionState::Active,
        }),
        "configured but nothing written yet must report Active at zero bytes, matching \
         BoundedTranscriptWriter::create's own initial state"
    );

    runtime
        .write_input(&handle, b"printf 'tekstide-summary-marker\\n'\nexit\n")
        .expect("marker command should write to PTY");
    let _ = drain_until_contains(&reader, b"tekstide-summary-marker", Duration::from_secs(5));

    let summary_started = Instant::now();
    let mut summary = reader.transcript_write_summary();
    while summary.map(|summary| summary.byte_count) == Some(0)
        && summary_started.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(5));
        summary = reader.transcript_write_summary();
    }
    assert!(
        summary.is_some_and(|summary| summary.byte_count > 0
            && summary.retention_state == TranscriptRetentionState::Active),
        "byte_count must have advanced past zero once real output was written: {summary:?}"
    );

    drop(reader);
    drop(dirs);
}

/// RFC-011 Amendment 2, PR-A2-A: "an enumeration names the production
/// transcript-write call site, so a future disappearance fails a test
/// rather than going unnoticed for two months" -- the exact defect that
/// created this amendment, named explicitly here so it cannot recur the
/// same way twice. Scans `.append(` calls specifically on a variable
/// named `writer` (`writer.append(`) rather than a bare `.append(` --
/// `Vec::append`/`AuditStore::append` elsewhere in this crate are a
/// different method entirely, and a bare-substring scan would wrongly
/// count them. Both named sites are legitimate and distinct:
/// `read_available_bounded_for`'s own writer (out of scope for this
/// amendment -- see `qa-evidence.md`'s PR-A2-A section for why re-homing
/// left it alone) and this amendment's own new reader-thread writer.
#[test]
fn only_two_named_production_call_sites_ever_append_to_a_transcript_writer() {
    let occurrences = count_occurrences_in_crate("writer.append(");
    assert_eq!(
        occurrences,
        vec![
            ("runtime/terminal/launch.rs".to_string(), 1),
            ("runtime/terminal/reader.rs".to_string(), 1),
        ],
        "exactly these two named files may ever call BoundedTranscriptWriter::append -- any \
         other file, or an unexpected count in either of these two, means a transcript-write \
         call site appeared or disappeared without this test being updated to say so: \
         {occurrences:?}"
    );
}

/// RFC-011 Amendment 2, D3: `LocalBounded`'s mid-stream failure policy,
/// exercised against a real, genuinely unwritable transcript -- not an
/// injected error value. `/dev/full` is a real kernel character device
/// that always fails `write(2)` with `ENOSPC`, the identical failure a
/// truly full filesystem produces, without needing root or any
/// filesystem setup of its own, and isolated to just this one path (no
/// other test or process is affected).
#[test]
fn local_bounded_marks_capture_failed_and_keeps_reading_when_the_transcript_is_genuinely_unwritable()
 {
    let (mut runtime, handle, _storage, dirs) = launch_with_unwritable_transcript_capture(
        "local-bounded-unwritable",
        TranscriptCaptureMode::LocalBounded,
    );
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");

    runtime
        .write_input(&handle, b"printf 'trigger-the-failed-write\\n'\n")
        .expect("trigger command should write to PTY");

    let summary = wait_for_summary_state(
        &reader,
        TranscriptRetentionState::CaptureFailed,
        Duration::from_secs(5),
    );
    assert_eq!(
        summary.retention_state,
        TranscriptRetentionState::CaptureFailed,
        "writing to a real, always-full device should mark the transcript CaptureFailed, not \
         silently succeed or stay Active"
    );

    // LocalBounded: reading resumes for later chunks even though
    // capture is now permanently off for this reader's remaining
    // lifetime -- the terminal must stay usable. Proven, not assumed,
    // by writing a second, distinct marker *after* the failure and
    // confirming it actually reaches the reader's channel.
    runtime
        .write_input(
            &handle,
            b"printf 'still-usable-after-capture-failed\\n'\nexit\n",
        )
        .expect("second marker command should write to PTY");
    let output = drain_until_contains(
        &reader,
        b"still-usable-after-capture-failed",
        Duration::from_secs(5),
    );
    assert!(
        contains_subsequence(&output, b"still-usable-after-capture-failed"),
        "LocalBounded must keep reading after a capture failure -- the terminal stays usable \
         even though its transcript stopped; captured: {}",
        String::from_utf8_lossy(&output)
    );

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("shell wait should not fail");
    assert_eq!(outcome, Some(TerminationOutcome::Exited { exit_status: 0 }));
    drop(reader);
    drop(dirs);
}

/// RFC-011 Amendment 2, D3: `RequiredLocalBounded`'s mid-stream failure
/// policy -- marks `CaptureFailed`, stops reading **altogether** (not
/// just the one failing chunk), and applies RFC-017 Amendment 1's
/// backpressure so the child stalls on its own `write(2)` rather than
/// being killed; termination stays the caller's decision. Exercised
/// separately from the `LocalBounded` case above, against the same
/// real `/dev/full` device, per the pack's own gate ("covering only
/// LocalBounded proves the easier half").
#[test]
fn required_local_bounded_marks_capture_failed_stops_reading_and_stalls_the_child_without_killing_it()
 {
    let (mut runtime, handle, _storage, dirs) = launch_with_unwritable_transcript_capture(
        "required-local-bounded-unwritable",
        TranscriptCaptureMode::RequiredLocalBounded,
    );
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against a real PTY master");

    // `yes` produces output continuously and directly, so once the
    // reader thread stops draining the PTY (below), `yes`'s own
    // `write(2)` calls fill the PTY's kernel buffer and then block --
    // the real backpressure D3 describes, not a synthesised stall.
    runtime
        .write_input(&handle, b"yes never-drained-because-reading-stopped\n")
        .expect("payload command should write to PTY");

    let summary = wait_for_summary_state(
        &reader,
        TranscriptRetentionState::CaptureFailed,
        Duration::from_secs(5),
    );
    assert_eq!(
        summary.retention_state,
        TranscriptRetentionState::CaptureFailed
    );

    // "Stops reading altogether": every chunk `yes` produces after the
    // first (failing) one must never reach the channel either -- not
    // just the one that failed. Poll for a sustained window rather than
    // a single snapshot, so a reader that resumed reading a moment
    // later would still be caught.
    let mut drained_total = 0_usize;
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        drained_total += reader.drain_available().bytes().len();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        drained_total, 0,
        "RequiredLocalBounded must stop reading entirely on capture failure -- any bytes \
         reaching the channel after CaptureFailed would be unrecorded output making it to the \
         display, exactly what D2's ordering guarantee exists to prevent"
    );

    // The child is stalled, not killed: still alive, blocked on its own
    // `write(2)` into a full PTY buffer, well past the point a killed
    // process would already have been reaped.
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_millis(300))
        .expect("inspecting the child's status should not fail");
    assert_eq!(
        outcome, None,
        "the child must still be alive and merely stalled, not exited or killed, while \
         RequiredLocalBounded has stopped reading"
    );

    // Termination stays the caller's decision, and still works even
    // though the reader stopped draining -- SIGTERM reaches a process
    // blocked in write(2) exactly as it would any other blocking call.
    let events = runtime
        .request_terminate(
            &handle,
            TerminationRequest {
                source: TerminationRequestSource::TestHarness,
                reason: BoundedRuntimeSummary::new("required-local-bounded stall cleanup"),
            },
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .expect("terminating a stalled RequiredLocalBounded child should still succeed");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, TerminalRuntimeEvent::Terminated { .. })),
        "termination must still succeed against a child stalled on write(2), not hang or \
         silently no-op: {events:?}"
    );

    drop(reader);
    drop(dirs);
}

/// Polls [`TerminalReader::transcript_write_summary`] until it reports
/// `expected`, or panics after `timeout` -- the observability half of
/// D3's "the failure must be observable" requirement: this is the exact
/// mechanism (D1's chosen replacement, `TerminalReader`'s own accessor)
/// a real caller would use to notice a capture failure.
fn wait_for_summary_state(
    reader: &TerminalReader,
    expected: TranscriptRetentionState,
    timeout: Duration,
) -> TranscriptWriteSummary {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(summary) = reader.transcript_write_summary()
            && summary.retention_state == expected
        {
            return summary;
        }
        if Instant::now() >= deadline {
            panic!(
                "transcript_write_summary did not reach {expected:?} within {timeout:?} \
                 (last seen: {:?})",
                reader.transcript_write_summary()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Same shape as [`launch_with_transcript_capture`], except the
/// resolved transcript path is replaced on disk with a symlink to
/// `/dev/full` before launch. `TranscriptStoragePath::is_safe_for_write`
/// checks containment with a lexical `Path::starts_with`, not a
/// symlink-resolving one, so the resolved path still passes it; opening
/// it (`OpenOptions::open`, which follows symlinks) reaches the real
/// device instead. `BoundedTranscriptWriter::create`'s own `open()`
/// call succeeds against `/dev/full` (only writes to it fail), so the
/// shell launch itself succeeds -- the failure genuinely happens
/// mid-stream, inside the reader thread's first write attempt, not at
/// preflight.
fn launch_with_unwritable_transcript_capture(
    label: &str,
    mode: TranscriptCaptureMode,
) -> (
    LinuxTerminalRuntime,
    TerminalRuntimeHandle,
    TranscriptStoragePath,
    TestDirs,
) {
    let dirs = TestDirs::new(label);
    let project = project_session(ProjectId::for_test(1), &dirs.project_root);
    let mut spec = TerminalLaunchSpec::plain_shell(
        project.id().clone(),
        "Shell",
        &dirs.project_root,
        "/bin/sh",
    );
    let request = TranscriptPathRequest::new(
        &dirs.state_root,
        &dirs.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );
    let storage_path = TranscriptPathResolver
        .resolve_agent_run(request)
        .expect("test storage path should resolve");
    std::fs::create_dir_all(storage_path.transcript_dir())
        .expect("transcript dir should be creatable");
    std::os::unix::fs::symlink("/dev/full", storage_path.transcript_file())
        .expect("symlinking the resolved transcript path to /dev/full should succeed");
    spec.set_transcript_writer_config(Some(TranscriptWriterConfig::new(
        storage_path.clone(),
        TranscriptRetentionLimits::agent_run_default(),
        mode,
    )));

    let mut runtime = LinuxTerminalRuntime::new();
    let (terminal, _) = runtime.launch_project_shell(&project, spec).expect(
        "shell launch should succeed even though the transcript path is unwritable -- \
         opening /dev/full for write succeeds; only later writes to it fail",
    );
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    (runtime, handle, storage_path, dirs)
}

fn launch_with_transcript_capture(
    label: &str,
    mode: TranscriptCaptureMode,
) -> (
    LinuxTerminalRuntime,
    TerminalRuntimeHandle,
    TranscriptStoragePath,
    TestDirs,
) {
    let dirs = TestDirs::new(label);
    let project = project_session(ProjectId::for_test(1), &dirs.project_root);
    let mut spec = TerminalLaunchSpec::plain_shell(
        project.id().clone(),
        "Shell",
        &dirs.project_root,
        "/bin/sh",
    );
    let request = TranscriptPathRequest::new(
        &dirs.state_root,
        &dirs.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );
    let storage_path = TranscriptPathResolver
        .resolve_agent_run(request)
        .expect("test storage path should resolve");
    spec.set_transcript_writer_config(Some(TranscriptWriterConfig::new(
        storage_path.clone(),
        TranscriptRetentionLimits::agent_run_default(),
        mode,
    )));

    let mut runtime = LinuxTerminalRuntime::new();
    let (terminal, _) = runtime
        .launch_project_shell(&project, spec)
        .expect("transcript-capturing shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    (runtime, handle, storage_path, dirs)
}

struct TestDirs {
    base: PathBuf,
    state_root: PathBuf,
    project_root: PathBuf,
}

impl TestDirs {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-reader-transcript-{label}-{}-{unique}",
            std::process::id()
        ));
        let state_root = base.join("state");
        let project_root = base.join("project");
        std::fs::create_dir_all(&state_root).unwrap();
        std::fs::create_dir_all(&project_root).unwrap();
        Self {
            base,
            state_root,
            project_root,
        }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Total occurrences of `needle` across `tekstide-core`'s production
/// `.rs` files (any file literally named `tests.rs` excluded) --
/// mirrors `crates/tekstide`'s own `count_occurrences_in_crate`
/// (`surface/terminal/tests.rs`), the same shape this project already
/// uses for `.advance(`/`.drain_available(`/`write_terminal_input`.
fn count_occurrences_in_crate(needle: &str) -> Vec<(String, usize)> {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    let mut occurrences: Vec<(String, usize)> = files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("tests.rs"))
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).expect("readable source file");
            let count = content.matches(needle).count();
            (count > 0).then(|| (relative_to_src(path), count))
        })
        .collect();
    occurrences.sort();
    occurrences
}

fn crate_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn relative_to_src(path: &Path) -> String {
    path.strip_prefix(crate_src_dir())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
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
