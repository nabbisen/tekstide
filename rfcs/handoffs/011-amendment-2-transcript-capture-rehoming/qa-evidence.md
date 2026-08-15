---
title: "RFC-011 Amendment 2: Re-homing transcript capture - QA Evidence"
rfc: "RFC-011 Amendment 2"
rfc_file: "../../done/011-transcript-retention-and-local-data-policy.md"
status: "Open - no slices implemented yet"
target_milestone: "M11 prerequisite"
created: "2026-08-15"
---

# QA Evidence

**This file holds results. The obligations live in
[`task-breakdown-pr-plan.md`](./task-breakdown-pr-plan.md).** Four obligations in this
project have been lost to that gap.

## Conventions

- **Ablation**: break the property, watch the *specific* named test fail, restore. **A
  green ablation is a defect in the ablation, not a pass.**
- **Positive control**: prove the check reaches real data before asserting what it does not
  find.
- **Real conditions, not synthesised values**: a real child process, a really unwritable
  file. The failure this amendment handles is a real-world one.
- State what each piece of evidence **does not** prove.

## Starting state, recorded before any change

- `LinuxTerminalRuntime::read_available_bounded_for` (`runtime/terminal/launch.rs:115`) is
  the **only** non-test writer of a `BoundedTranscriptWriter`: `.append(` at `:131-136`,
  `.flush(` at `:162-169`.
- It has **zero production callers** — the only non-test reference is its own definition.
- `runtime/terminal/reader.rs` contains the string "transcript" **zero times**.
- Nothing in production creates an `AgentRun`, so no transcript writer is ever configured
  and nothing fails today.

## PR-A2-A - Capture in the reader thread, and the ordering

**The writer lives in the reader thread.** `TerminalReader::spawn`
(`runtime/terminal/reader.rs`) now takes an `Option<TranscriptCapture>` (writer + capture
mode) and builds a private `ReaderTranscriptState` that lives inside `reader_thread_loop`'s
own stack, not in `poll()` and not on the UI thread. `spawn_output_reader`
(`runtime/terminal/launch.rs`) takes the writer out of the `RunningTerminal` session
(`session.transcript_writer.take()`) and moves it into the `TranscriptCapture` handed to
`TerminalReader::spawn` — after this call, the runtime no longer owns a writer for that
terminal at all.

**Byte-identical, against a real child process.**
`transcript_written_through_the_reader_thread_is_byte_identical_to_pty_output`
(`runtime/terminal/reader/tests.rs`) launches a real `/bin/sh` PTY child through
`launch_with_transcript_capture`, writes a marker line, drains the reader channel to EOF,
then reads the transcript file back and asserts it is byte-identical to the drained
channel output. Passed.

**D2 proven by ordering, not by inference.** `transcript_write_blocking_also_blocks_every_later_send`
(`runtime/terminal/reader/tests.rs`) proves write-before-send with a real, deterministic
stall: the transcript path is a named FIFO, so the writer's `write()` blocks once the FIFO's
kernel buffer fills, and the test compares — via `FIONREAD` on the FIFO's read end, which
reports bytes queued but not yet consumed, without side effects — the exact byte count ever
committed to the transcript against the exact byte count ever drained from the reader's
channel. Passed with the real write-before-send code
(`drained_total <= written_to_transcript` holds throughout). **Ablated**: a temporary,
unconditional `sender.send()` inserted before `record_write` in `reader_thread_loop` (send
first, write after) produced a real, concrete violation — 142,148 bytes drained from the
channel against only 52,705 bytes ever committed to the transcript — then reverted.

**A genuine deadlock was found and fixed in this test's own cleanup**, not in production
code, disclosed here rather than silently patched. The test's first cleanup ordering drained
the FIFO to EOF, then called `drop(reader)`. Run once in isolation this passed; run again
shortly after under real, independent CPU contention on this machine (a concurrent,
unrelated `cargo build` in another project's tree), the test binary hung for 13+ minutes —
found and diagnosed live, not reasoned about in the abstract. Root cause: EOF on the FIFO's
read side requires the reader thread's writer to close, which requires the reader thread to
notice shutdown, which requires its *current* blocked `write_all` to unblock first — which
requires the drain to keep running. Sequencing "drain fully, then drop" makes the drain and
the shutdown signal wait on each other under scheduling contention: a real circular wait, not
merely slow. **Fixed** by running the drop and the drain concurrently (drop on its own
thread, bounded by a 10s timeout via `mpsc::recv_timeout`, mirroring
`dropping_a_reader_over_a_live_silent_child_completes_promptly`'s own established pattern one
test above it in the same file) — this breaks the cycle, since the shutdown signal reaches
the reader thread immediately and it proceeds as soon as the concurrent drain unblocks its
current write. Re-verified: 5 consecutive runs pass in isolation; 3 more consecutive runs
pass under artificial CPU contention (28 concurrent `yes` processes across a 32-core
machine) reproducing the same conditions that surfaced the original hang. Two orphaned `yes`
child processes left behind by killing the original hung test binary were found and cleaned
up separately (`kill -9`; routine, not evidence of an ongoing bug).

Two earlier test designs were tried and rejected, and are recorded here rather than
hidden, per this project's disclosure convention:

1. **Large real PTY payload + a channel-byte-count threshold.** Rejected because real PTY
   throughput in this environment stalls upstream of the reader's own channel (kernel
   PTY/line-discipline buffering), not at the reader's `CHANNEL_CAPACITY`/`READ_CHUNK_BYTES`
   bound. Across repeated real runs (`dd|tr`, then `yes`) the transcript byte count
   plateaued far below the theoretical channel bound (49339, 22715, 10427, then 1507
   bytes across different attempts), and temporary diagnostics showed `transcript_len ==
   drained_len` exactly every time — the reader was never actually caught mid-send. This is
   a genuine environmental finding, not a defect in the reader.
2. **FIFO-backed transcript + an aggregate threshold** (`drained_total < channel_only_bound
   / 4`). Rejected because a real ablation (the same send-before-write change described
   above) passed both with and without the bug present. Root cause: the reader thread is
   single-threaded and sequential, so a block during one chunk's processing halts every
   later chunk equally regardless of which side of the send the block sits on — an ablated
   reordering only lets one extra chunk (tens of KB) through before hitting the same wall,
   nowhere near a loose aggregate threshold. Replaced with the exact per-byte `FIONREAD`
   comparison above, which does distinguish the orderings.

**`transcript_write_summary`'s replacement, decided and named.** The runtime's old
`LinuxTerminalRuntime::transcript_write_summary` reads `session.transcript_writer.as_ref()`
and is deliberately left unchanged — it still correctly serves `read_available_bounded_for`'s
own callers (the agent-run subsystem, out of scope below), and now correctly returns `None`
once `spawn_output_reader` has taken the writer for a given terminal, rather than silently
reporting stale state. The reader thread's own writes are surfaced through a **new**
`TerminalReader::transcript_write_summary()` accessor, backed by an `Arc<Mutex<TranscriptWriteSummary>>`
shared between the reader thread and the caller. `transcript_write_summary_is_none_when_capture_was_never_configured`
and `transcript_write_summary_reflects_real_writes_through_the_reader` (both in
`runtime/terminal/reader/tests.rs`) prove the `None`-when-unconfigured and
reflects-real-writes cases respectively. Both passed.

**Enumeration names the production call site(s).**
`only_two_named_production_call_sites_ever_append_to_a_transcript_writer`
(`runtime/terminal/reader/tests.rs`) scans every `.rs` file under `tekstide-core/src/`
(excluding files literally named `tests.rs`) for the substring `writer.append(` and asserts
it appears in exactly two files: `runtime/terminal/reader.rs` (this amendment's new call
site) and `agent/launch.rs`'s companion module reached via `read_available_bounded_for`
(scoped out below — see `agent/tests.rs`). A future third call site, or the disappearance of
either named one, fails this test rather than going unnoticed. Passed.

**Scope: `read_available_bounded_for` is untouched.** `LinuxTerminalRuntime::read_available_bounded_for`
and its own transcript-writing logic (`runtime/terminal/launch.rs`) serve the agent-run
subsystem, not terminal-pane ingress — confirmed by `agent::tests::transcript_capture_retains_pty_bytes_dropped_from_ui_buffer`
(`crates/tekstide-core/src/agent/tests.rs`), which launches a real `AgentRun` via
`launch_agent_run_with_runtime` and depends on `read_available_bounded_for` directly as
its own, separate, still-load-bearing output-reading primitive. This mirrors the scoping
boundary RFC-017 Amendment 1 PR-A1-C's response 206 drew for an analogous question. This
amendment's enumeration test above names both legitimate call sites rather than eliminate
the older one.

**P1/P2 re-run, not assumed to transfer.** The writer is a new consumer *inside* the reader
thread's own sequential loop, not a second path out of it: `BoundedTranscriptWriter` is not
`Clone`, so once `spawn_output_reader` moves it into the `TranscriptCapture` handed to
`TerminalReader::spawn`, no other code holds a handle to it — the type system rules out a
second writer existing. The byte-identical proof above is this amendment's P1/P2 re-proof:
`real_pty_output_reaches_the_channel_end_to_end` (pre-existing, still passing) proves the
channel path is unchanged; `transcript_written_through_the_reader_thread_is_byte_identical_to_pty_output`
proves the new transcript path carries the identical bytes.

**Correction (response 211): P2's substance changed, not merely "re-ran."** P2 says no path
carries terminal bytes anywhere except the one ingress. After this slice, terminal bytes have
a **second destination inside `tekstide-core`**: the transcript file, written from inside the
reader thread. This is authorised, not an exposure — RFC-011 governs it, with its own
retention limits, path-resolution policy (`TranscriptPathResolver`), and purge — and the
destination existed before RFC-017 Amendment 1 temporarily removed it (this amendment's whole
purpose is putting it back). But it is a real change to what P2 describes, not a re-proof of
an unchanged property, and is recorded here explicitly so the next person enumerating where
terminal bytes go finds the transcript named rather than has to rediscover it — the exact gap
that created this amendment in the first place.

All 5 new tests, plus the full pre-existing `runtime::terminal::reader` suite, pass. Full
gate (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings`, `cargo test --workspace --all-targets --all-features`, `git diff --check`) run
clean against the final state.

## PR-A2-B - The failure policy

**A genuinely unwritable transcript, not an injected error value.** Both new tests
(`local_bounded_marks_capture_failed_and_keeps_reading_when_the_transcript_is_genuinely_unwritable`,
`required_local_bounded_marks_capture_failed_stops_reading_and_stalls_the_child_without_killing_it`,
both in `runtime/terminal/reader/tests.rs`) point the resolved transcript path at a real
symlink to `/dev/full` — a kernel character device that always fails `write(2)` with
`ENOSPC`, the identical failure a truly full filesystem produces, without root and without
mounting anything. `TranscriptStoragePath::is_safe_for_write`'s containment check is a
lexical `Path::starts_with`, not symlink-resolving, so the resolved, in-policy path still
passes it; `OpenOptions::open` (which follows symlinks) reaches the real device.
`BoundedTranscriptWriter::create`'s own `open()` succeeds against it — only writes fail —
so the shell launch itself succeeds and the failure genuinely happens mid-stream, inside
the reader thread's first write attempt, not at preflight.

**`LocalBounded`**: the trigger write fails, `transcript_write_summary()` reports
`CaptureFailed` (polled via a real accessor call, not asserted from the writer's own
internal state), and a second, later marker written *after* the failure still reaches the
channel — proven by draining for it, not assumed from the mode's name. Terminal stays
usable.

**`RequiredLocalBounded`**: exercised separately, against a continuous `yes` producer so a
real, sustained backpressure scenario exists to observe. The very first chunk read fails
(same `/dev/full` device), so **zero bytes ever reach the channel for this reader's entire
lifetime** — polled for a sustained 500ms window to catch a reader that resumed reading a
moment later, not just a single snapshot. The child is confirmed alive and merely stalled
(`wait_for_exit(..., 300ms)` returns `None`, not `Some(Exited/Terminated)`), not killed.
`request_terminate` still succeeds against it within its normal timeout — SIGTERM reaches a
process blocked in `write(2)` exactly as it would any other blocking call, so termination
stays available to the caller as D3 requires.

**`CaptureFailed` is the existing state** (`domain/transcript.rs:93`) — no new state
introduced, per the pack's own instruction.

**The failure is observable, and where it surfaces is stated.** `TerminalReader::transcript_write_summary()`
(D1's mechanism, PR-A2-A) is the observation point for both modes; both new tests poll it
directly rather than inferring the failure from a side effect.

**Ablated**: removed the `CaptureFailed`-marking write in `ReaderTranscriptState::record_write`'s
`Err` branch (kept `self.failed = true` and the `stop_reading` policy split, since ablating
*those* would test a different property). Both new tests failed with the same concrete
wrong value: `TranscriptWriteSummary { byte_count: 0, retention_state: Active }` — the
transcript looks untouched and healthy even though the write genuinely failed, exactly the
silent-success class of defect this ablation exists to catch. Reverted; both tests pass
again.

**A second, unrelated bug was found and fixed while re-running the full gate to close this
out**: `transcript_written_through_the_reader_thread_is_byte_identical_to_pty_output`
(PR-A2-A) compared a channel drain taken via `drain_until_contains` — which returns as soon
as the marker text appears — against the full transcript file. Under real scheduling
variance the shell's own `exit\r\n` echo can land in a read chunk *after* the one containing
the marker, so the comparison raced; reproduced live with a concrete byte-exact mismatch
(`drained` missing the trailing `exit\r\n` the transcript file still had). Fixed by waiting
for the reader's own wake notifier to report no more wakes are coming (the same signal
`the_wake_notifier_delivers_a_final_wake_and_then_reports_no_more_are_coming` already proves
accurate) before taking the final drain. Unrelated to D3; disclosed here because it surfaced
during this slice's own gate runs.

**A separate, pre-existing test-concurrency finding is recorded here as a test-infrastructure
fix, not as D3 evidence** (response 212's third condition) — a test-harness change is not
evidence for a capture property, so it is disclosed on its own rather than folded into the
record above.

Response 212 accepted PR-A2-B's own evidence and directed Option A (a file-wide concurrency
limiter), independently reproducing the flake and finding the *identity* of the failing test
changed between repeats — the signature of contention, not any one test being wrong.
Building it surfaced two more things, both corrected before the limiter was finalized:

- **A second, genuine, standalone bug**, found while implementing the limiter, unrelated to
  concurrency: `transcript_written_through_the_reader_thread_is_byte_identical_to_pty_output`
  waited for the reader's final wake *without draining the channel in between*. With
  transcript capture on, each chunk costs a real disk write inside the reader thread before
  it is ever sent; a real shell delivers its output in several separate bursts, and if
  enough land as separate chunks before anything drains, `sender.send` inside the reader
  thread blocks on the channel's bounded capacity (`CHANNEL_CAPACITY = 8`) — the reader can
  then never reach EOF or signal a final wake. Reproduced with `--test-threads=1` and only
  this one test running, no other concurrency involved, ruling out contention as *this*
  failure's cause: 4/15 failures alone before the fix, 0/20 after. Fixed by draining on
  every wake, not just the final one — the correct way to consume `TerminalReader` in the
  first place. This confounded the *first* concurrency measurement in review request 212
  (its 4-threads-clean number was really "4 threads clean of a channel-starvation bug that
  had nothing to do with thread count"), which is why the corrected table below differs from
  request 212's own.
- **A third, narrower, genuinely contention-sensitive race**, found once the above was fixed
  and the corrected measurement was underway: `transcript_write_summary_reflects_real_writes_through_the_reader`
  asserted `byte_count == 0` immediately after spawn, assuming nothing could have been
  written yet. A shell can legitimately emit its own unprompted startup bytes before any
  input is sent; under real contention this reproduced live as `byte_count: 8` where `0` was
  hard-coded as expected. Fixed by capturing whatever the summary reports as a baseline
  (asserting only that its retention state is `Active`) and asserting the later summary
  advances *past that baseline*, not past a hard-coded zero — the property the test actually
  needs (`transcript_write_summary` reflects real writes) without assuming a race-free
  window that was never guaranteed.

**Corrected concurrency measurement**, both bugs above fixed, `cargo test -p tekstide-core
--lib runtime::terminal::reader::tests::`, isolated:

| `--test-threads` | failures |
|---|---|
| 2 | 0/8 |
| 4 | 0/8 |
| 8 | 0/8, then 0/20 on a larger sample |
| 16 (this machine's default) | 6/8 |

A `RealProcessLimiter` (`runtime/terminal/reader/tests.rs`) caps how many of this file's 14
real-PTY-spawning tests run their spawn-through-cleanup critical section at once, at **6**
(below the clean 8-thread measurement, for margin), applied to all 14 — including the 10
from PR-A1-C/D this amendment does not otherwise touch, per response 212's own reasoning
that the wrong assumption ("5 seconds is enough margin") belongs to the file, not to any one
test. Confirmed clean: 0/30 runs of the full reader module at default concurrency, 0/25 runs
of the full `cargo test --workspace --all-targets --all-features` gate (one unrelated
failure across those 25 runs, on `approval::tests::channel::bind_recovers_from_a_stale_socket_file`
— the pre-existing, already-tracked RFC-021 socket flake `future-work.md` already connects
to the same fork-window pressure, not a reader-suite regression).

Per response 212's second condition: no test in this file spawns more than one
`TerminalReader`, so the limiter removes only wall-clock overlap *between* test functions,
not any coverage of concurrent readers within one. The only place multiple simultaneous
terminals are exercised at all is PR-A1-D's N-pane throughput measurement
(`crates/tekstide/src/measurement.rs`), a manual tool run against the live app, unaffected
by this limiter.

## PR-A2-C - Closeout

*Not started.*

## Known limitations going in

- **Correct before reachable.** Nothing creates an `AgentRun`, so this cannot be
  demonstrated end-to-end through a real agent run. That is deliberate: the capability must
  be right *before* adapter-spawn depends on it, not proven *through* it.
- **Backpressure now includes the disk** (D4). A stalled disk stalls the child whenever
  capture is on.
