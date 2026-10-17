//! RFC-017 Amendment 1, PR-A1-A: a dedicated reader thread over the PTY
//! master, feeding a bounded channel, built **alongside**
//! `TerminalRuntime::read_available_bounded_for` rather than in
//! place of it -- both paths exist so they can be measured and compared
//! against the same tests. Nothing in `crates/tekstide` drains this
//! channel yet; wiring it into `TerminalPane` and re-proving P1/P2 for
//! the new shape is PR-A1-B's job, not this module's.
//!
//! **The two defects this replaces, and why fixing one without the
//! other is worse than fixing neither**: the old path's `WouldBlock`
//! sleep (10 ms against a caller-supplied 5 ms bound) caps throughput at
//! ~374 KB/s; its 64 KiB per-poll cap truncates mid-read and discards
//! the remainder. `dropped_bytes` is zero today only because the sleep
//! starves the reader before the cap is ever reached. This module has
//! no truncation logic and no dropped-bytes concept at all: every byte
//! `read(2)` returns is either handed to a blocking channel send, or the
//! send blocks until there is room. There is no code path that computes
//! a byte count and then discards part of it -- "unreachable" here is a
//! structural property of the type, not a runtime check.
//!
//! **RFC-017 Amendment 1, PR-A1-C: [`WakeNotifier`], a second wake
//! signal separate from the shutdown one.** `read_available_bounded_for`'s
//! old call site was reached by a fixed-interval poll tick; removing that
//! tick (PR-A1-C's own job) needs something to replace its role of
//! telling a caller "go check this terminal now" without reintroducing
//! a timer. The reader thread signals this whenever it buffers new
//! bytes, and one final time on every exit path (shutdown, EOF, or a
//! fatal read error) -- see [`WakeNotifier::block_until_woken`]'s own
//! doc for why "the reader is done" needs an explicit signal rather than
//! being inferred from the signal's own semantics.
//!
//! **RFC-028 D5 (review 593): [`WakeSignal`] is the cross-platform
//! primitive behind both the shutdown signal and `WakeNotifier`.**
//! `eventfd(2)` (Linux) does not exist on Apple targets, and no atomic-
//! `CLOEXEC` self-pipe construction exists there either (`pipe2` and
//! `SOCK_CLOEXEC` are both absent for `apple` in the vendored `libc`
//! crate, and `rustix` -- already a transitive dependency -- omits its
//! own flagged `pipe_with` entirely for Apple rather than ship a racy
//! fallback). A self-pipe's specific failure mode -- a concurrently
//! `exec`'d child inheriting the write end, so the read side never sees
//! EOF and shutdown hangs -- is also why a narrower `CLOEXEC` window is
//! not an adequate fix on its own: `WakeSignal` uses `kqueue(2)` with a
//! single `EVFILT_USER` registration on macOS instead, which has no
//! write end for anything to inherit. See [`WakeSignal`]'s own doc for
//! the exact contract this gives both backends.

use std::fs;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::transcript::{
    BoundedTranscriptWriter, TranscriptCaptureMode, TranscriptRetentionState,
    TranscriptWriteSummary,
};

/// One `read(2)` call reads at most this many bytes before the chunk is
/// handed to the channel. Matches the old path's 64 KiB per-poll cap in
/// size (not in kind -- there, it was a truncation bound; here, it is
/// only how much a single syscall is asked to return before the reader
/// loops back to draining or blocking).
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// The channel's capacity in *messages*, not bytes -- each message is at
/// most [`READ_CHUNK_BYTES`], so this bounds total buffered output at
/// roughly `CHANNEL_CAPACITY * READ_CHUNK_BYTES` (512 KiB). **This is a
/// starting point, not a measured figure**: PR-A1-A's own gate asks for
/// backpressure to be demonstrated correctly at *some* bound, not for
/// the bound to be right-sized yet. PR-A1-D re-measures per-pane cost
/// once the old tick is gone; this constant is deliberately not claimed
/// as tuned against `NFR-PERF-004` before that happens.
const CHANNEL_CAPACITY: usize = 8;

/// A dedicated OS thread reading one PTY master, and the bounded channel
/// it feeds. `Receiver<Vec<u8>>` is not `Clone` -- by construction, at
/// most one consumer of this channel can ever exist. That is P2's
/// "exactly one consumer" made unrepresentable by the type rather than
/// checked by a test, per the ingress re-proof document's own
/// preference for that discipline. When PR-A1-B re-enumerates P2 against
/// the new shape, the shutdown `eventfd` below is a second channel this
/// module owns and must be accounted for in that enumeration too -- it
/// carries no PTY data, but it is a channel.
pub struct TerminalReader {
    // `Option` so `Drop` can explicitly drop this *before* joining the
    // thread -- a custom `Drop::drop` body runs before Rust's automatic
    // field drops, not after, so a plain `Receiver<Vec<u8>>` field would
    // still be alive for the whole body of `drop()`, and a join while it
    // is still alive can deadlock (see `Drop for TerminalReader`).
    receiver: Option<Receiver<Vec<u8>>>,
    join_handle: Option<JoinHandle<()>>,
    os_thread_id: i32,
    // Signalled by `Self::shutdown`/`Drop`, polled by the reader thread
    // alongside the PTY master. Response 201: dropping `receiver` alone
    // only unblocks a thread parked in `sender.send` on a full channel;
    // it does nothing for the far more common case of a thread parked in
    // `poll(2)` on a live, silent child (no data, PTY not hung up). This
    // is what makes that case interruptible too -- see `Drop`.
    shutdown: WakeSignal,
    // RFC-017 Amendment 1, PR-A1-C: a second `WakeSignal`, signalled by
    // the reader thread (not `Drop`) every time it either buffers new
    // bytes or stops for good -- what lets a caller replace polling with
    // a real wait. `TerminalReader` itself never waits on this; it
    // exists to be duplicated out via [`Self::try_clone_wake_notifier`]
    // for whoever needs to wake up when this reader has something to
    // report.
    wake: WakeSignal,
    // Set to `false` by the reader thread immediately before its last
    // ever wake signal, on every exit path -- shutdown, EOF, or a fatal
    // read error. `eventfd`'s own counter has no notion of "the writer
    // is done" (unlike a pipe's write end closing), so this is the
    // explicit signal a `WakeNotifier` checks to know no further wakes
    // are coming, rather than trying to encode that into the counter
    // value itself.
    reader_alive: Arc<AtomicBool>,
    // RFC-011 Amendment 2, D1: `None` iff no `TranscriptCapture` was
    // handed to `spawn` (capture not configured for this terminal at
    // all) -- distinct from "configured, zero bytes written yet",
    // which is `Some(TranscriptWriteSummary::active(0))`, matching the
    // exact `Option` semantics the runtime-level accessor this replaces
    // (`TerminalRuntime::transcript_write_summary`) used to have.
    // The reader thread is the sole writer of the lock's contents; this
    // handle only ever reads it.
    transcript_write_summary: Option<Arc<Mutex<TranscriptWriteSummary>>>,
}

/// A duplicated handle onto one [`TerminalReader`]'s wake [`WakeSignal`],
/// obtained via [`TerminalReader::try_clone_wake_notifier`]. Carries no
/// PTY data of its own -- see [`Self::block_until_woken`].
pub struct WakeNotifier {
    signal: WakeSignal,
    reader_alive: Arc<AtomicBool>,
}

impl WakeNotifier {
    /// Blocks (a real wait, not a sleep) until the reader thread has
    /// either buffered new bytes or stopped for good. Returns `true` if
    /// more wakes may still arrive -- the caller should act on this wake
    /// and call again; `false` means this was the reader's last wake
    /// ever, on any exit path (shutdown, EOF, or a fatal read error) --
    /// the caller should still act on it once (there may be a final exit
    /// to notice) and then stop waiting, since nothing will signal this
    /// notifier again.
    ///
    /// **Does not promise a wake count, and does not promise one wake
    /// per signal** -- see [`WakeSignal::wait`]'s own doc for exactly
    /// what is and is not guaranteed across the two backends. Callers
    /// must already be correct for spurious and coalesced wakes; this
    /// one is, since it never infers "how many chunks were sent" from
    /// "how many times I was woken."
    pub fn block_until_woken(&self) -> bool {
        if !self.signal.wait() {
            return false;
        }
        self.reader_alive.load(Ordering::Acquire)
    }

    /// A second, independent duplicate of this notifier's own
    /// [`WakeSignal`], sharing the same underlying signal and
    /// `reader_alive` flag -- for a caller that only holds a borrow
    /// (`&WakeNotifier`, for example inside an API that hands out
    /// `&self` rather than ownership) but needs an owned, `'static`
    /// handle to move into a background thread. Failure here is the
    /// same resource-exhaustion case `try_clone_wake_notifier` can
    /// already fail on.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            signal: self.signal.try_clone()?,
            reader_alive: Arc::clone(&self.reader_alive),
        })
    }
}

/// RFC-011 Amendment 2, D1: what [`TerminalReader::spawn`] needs to move
/// transcript capture into the reader thread -- the writer itself, plus
/// the capture mode D3's mid-stream failure policy is keyed on. Built by
/// the caller (`TerminalRuntime::spawn_output_reader`) from
/// whatever `RunningTerminal` already holds; this module has no opinion
/// on where either value came from.
pub struct TranscriptCapture {
    pub writer: BoundedTranscriptWriter,
    pub mode: TranscriptCaptureMode,
}

impl TranscriptCapture {
    pub fn new(writer: BoundedTranscriptWriter, mode: TranscriptCaptureMode) -> Self {
        Self { writer, mode }
    }
}

/// RFC-011 Amendment 2, D3: the reader thread's own mutable transcript
/// state -- the writer it owns exclusively (nothing else can reach it
/// once moved here), the mode deciding what a write failure means, the
/// shared snapshot `TerminalReader::transcript_write_summary` reads, and
/// whether capture has already failed once (so a `LocalBounded` reader
/// does not keep re-attempting a write that is not going to start
/// succeeding again mid-stream).
struct ReaderTranscriptState {
    writer: BoundedTranscriptWriter,
    mode: TranscriptCaptureMode,
    shared_summary: Arc<Mutex<TranscriptWriteSummary>>,
    failed: bool,
}

/// What the reader thread's main loop must do with the chunk it just
/// read, after [`ReaderTranscriptState::record_write`] -- see that
/// method's own doc for why a chunk whose own write failed is never
/// `Send`.
enum TranscriptCaptureOutcome {
    Send,
    Suppress { stop_reading: bool },
}

impl ReaderTranscriptState {
    /// **D2, held exactly, per chunk, with no exception**: the write
    /// happens before the bytes would enter the channel, so if the write
    /// fails, the bytes must not enter the channel either -- the
    /// ordering guarantee was not actually satisfied for this chunk, and
    /// showing it anyway would mean the display contains something the
    /// durable record does not, the one outcome D2 exists to prevent.
    /// Once `failed` is set (a prior chunk's write already failed under
    /// `LocalBounded`), later chunks skip the write attempt entirely and
    /// are always `Send` -- capture is permanently off for this reader's
    /// remaining lifetime, not retried per chunk.
    ///
    /// **D3's policy split lives entirely in the return value**:
    /// `LocalBounded` resumes normal reading from the *next* chunk
    /// (`stop_reading: false`) with capture now off; `RequiredLocalBounded`
    /// tells the caller to stop reading altogether
    /// (`stop_reading: true`) -- "no further unrecorded progress," held
    /// at the byte level, not just "no further reads attempted."
    fn record_write(&mut self, bytes: &[u8]) -> TranscriptCaptureOutcome {
        if self.failed {
            return TranscriptCaptureOutcome::Send;
        }
        let result = self.writer.append(bytes).and_then(|_| self.writer.flush());
        match result {
            Ok(summary) => {
                *lock_summary(&self.shared_summary) = summary;
                TranscriptCaptureOutcome::Send
            }
            Err(error) => {
                self.failed = true;
                *lock_summary(&self.shared_summary) = TranscriptWriteSummary {
                    byte_count: error.byte_count,
                    retention_state: TranscriptRetentionState::CaptureFailed,
                };
                TranscriptCaptureOutcome::Suppress {
                    stop_reading: self.mode == TranscriptCaptureMode::RequiredLocalBounded,
                }
            }
        }
    }
}

fn lock_summary(
    shared: &Mutex<TranscriptWriteSummary>,
) -> std::sync::MutexGuard<'_, TranscriptWriteSummary> {
    shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What one non-blocking drain call collected. `ended` means the reader
/// thread has stopped producing -- either the PTY reported end-of-file
/// (the child exited and every slave-side fd closed) or the underlying
/// read failed. **This module does not distinguish the two**: both
/// leave nothing more to read, and a caller that needs to tell them
/// apart (for example, to report an unexpected I/O failure) is a
/// wiring concern for whoever drains this in production, not something
/// this checkpoint's own gate requires.
pub struct TerminalReaderDrain {
    bytes: Vec<u8>,
    ended: bool,
}

impl TerminalReaderDrain {
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn ended(&self) -> bool {
        self.ended
    }
}

impl TerminalReader {
    /// Spawns the reader thread over `master`. `master` must be a
    /// dup'd handle distinct from the one `write_input`/`resize` use --
    /// reads and writes on separate `File`s over the same open file
    /// description are independent syscalls and need no synchronization
    /// between them, but this reader takes ownership of the handle it
    /// is given and will hold it open for as long as the thread runs.
    ///
    /// **Blocks on readiness, never sleeps, never busy-waits**: the
    /// thread calls `poll(2)` with an infinite timeout on `master`'s fd,
    /// a shutdown `eventfd`, and (implicitly, via its own writes) signals
    /// the wake `eventfd`. The kernel parks the thread until something is
    /// ready, consuming no CPU while parked -- a real blocking primitive,
    /// not a fixed delay guessed to be short enough.
    ///
    /// Fallible because [`WakeSignal::create`] can fail -- resource
    /// exhaustion on both platforms, plus (macOS only) the `CLOEXEC`
    /// postcondition it checks after construction -- everything else
    /// here is infallible in practice.
    ///
    /// **RFC-011 Amendment 2, D1**: `transcript_capture`, when `Some`,
    /// moves a real `BoundedTranscriptWriter` into this reader thread --
    /// the thread already blocks on `poll(2)`, which is the correct
    /// place for a blocking write (`UI thread` file I/O is the exact
    /// defect RFC-017 Amendment 1 removed; re-introducing it as a
    /// transcript write under a different name would undo that). `None`
    /// means capture is not configured for this terminal at all, the
    /// common case today since nothing in `crates/tekstide` yet
    /// requests it.
    pub(super) fn spawn(
        master: fs::File,
        transcript_capture: Option<TranscriptCapture>,
    ) -> io::Result<Self> {
        // Owned by `TerminalReader` for `Drop` to signal shutdown
        // through; the reader thread gets its own clone below (see
        // `thread_shutdown`) rather than reaching into this one, so
        // ownership of each side is ordinary and `Send`-safe instead of
        // resting on "the other field happens to keep the fd open."
        let shutdown = WakeSignal::create()?;
        // Same shape, second signal: kept alive here only so
        // `try_clone_wake_notifier` has something to duplicate from for
        // as long as this `TerminalReader` exists. Nothing on this
        // struct ever waits on it.
        let wake = WakeSignal::create()?;
        let thread_shutdown = shutdown.try_clone()?;
        let thread_wake = wake.try_clone()?;
        let reader_alive = Arc::new(AtomicBool::new(true));

        let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        // A one-shot handshake so the caller can learn the reader
        // thread's kernel thread id -- used only for the idle-CPU
        // measurement in this module's own tests
        // (`reader_thread_does_not_busy_wait_while_idle`); harmless
        // overhead (one syscall at thread start) on the production path.
        let (tid_sender, tid_receiver) = mpsc::sync_channel(1);
        let thread_reader_alive = Arc::clone(&reader_alive);
        let transcript_write_summary = transcript_capture
            .as_ref()
            .map(|capture| Arc::new(Mutex::new(capture.writer.summary())));
        let transcript_state = transcript_capture
            .zip(transcript_write_summary.clone())
            .map(|(capture, shared_summary)| ReaderTranscriptState {
                writer: capture.writer,
                mode: capture.mode,
                shared_summary,
                failed: false,
            });
        let join_handle = std::thread::spawn(move || {
            let _ = tid_sender.send(current_thread_id());
            reader_thread_loop(
                master,
                sender,
                thread_shutdown,
                thread_wake,
                thread_reader_alive,
                transcript_state,
            );
        });
        let os_thread_id = tid_receiver
            .recv()
            .expect("reader thread should report its id before spawn returns");
        Ok(Self {
            receiver: Some(receiver),
            join_handle: Some(join_handle),
            os_thread_id,
            shutdown,
            wake,
            reader_alive,
            transcript_write_summary,
        })
    }

    /// RFC-011 Amendment 2, D1's chosen replacement for the old
    /// `TerminalRuntime::transcript_write_summary` -- that method
    /// read a writer the runtime owned; once the writer moves into this
    /// thread (`spawn`, above), the runtime has nothing left to consult,
    /// so the summary is queried from the reader itself instead, backed
    /// by the shared, lock-protected snapshot the reader thread updates
    /// after every write attempt (success or failure). `None` iff
    /// capture was never configured for this terminal; `Some` thereafter
    /// for the lifetime of this reader, even after `CaptureFailed`.
    pub fn transcript_write_summary(&self) -> Option<TranscriptWriteSummary> {
        self.transcript_write_summary
            .as_ref()
            .map(|shared| *lock_summary(shared))
    }

    /// The reader thread's kernel thread id (`gettid(2)`), for
    /// diagnostics and for measuring its idle CPU usage directly against
    /// `/proc/self/task/<tid>/stat` rather than trusting the mechanism's
    /// description.
    pub fn os_thread_id(&self) -> i32 {
        self.os_thread_id
    }

    /// RFC-017 Amendment 1, PR-A1-C: a duplicate handle onto this
    /// reader's wake [`WakeSignal`], for a caller that wants to wait for
    /// readiness instead of polling `drain_available` on a timer.
    /// `try_clone`'s failure mode is the same resource exhaustion
    /// `spawn` can already fail on.
    pub fn try_clone_wake_notifier(&self) -> io::Result<WakeNotifier> {
        Ok(WakeNotifier {
            signal: self.wake.try_clone()?,
            reader_alive: Arc::clone(&self.reader_alive),
        })
    }

    /// Drains everything currently buffered in the channel **without
    /// blocking the caller** -- `mpsc::Receiver::try_recv` never waits
    /// for a message that has not arrived, so this returns immediately
    /// whether or not the reader thread has anything ready. This is
    /// the property PR-A1-A's gate calls "the UI thread never blocks";
    /// `drain_available_never_blocks_the_caller_even_under_sustained_production`
    /// in this module's own tests measures it under load rather than
    /// asserting on the channel type's documented behaviour alone.
    pub fn drain_available(&self) -> TerminalReaderDrain {
        let mut bytes = Vec::new();
        let mut ended = false;
        let receiver = self
            .receiver
            .as_ref()
            .expect("receiver is only taken during drop, after which nothing can call this");

        loop {
            match receiver.try_recv() {
                Ok(chunk) => bytes.extend_from_slice(&chunk),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    ended = true;
                    break;
                }
            }
        }

        TerminalReaderDrain { bytes, ended }
    }
}

impl TerminalReader {
    /// Signals shutdown and joins the reader thread synchronously,
    /// closing this reader's own `try_clone()`d duplicate of the PTY
    /// master in the process (that duplicate lives only in the thread
    /// closure's local `master: fs::File`, so it drops when the thread
    /// function returns). `Drop::drop` calls this; it is exposed here
    /// too, `pub`, so a caller (`TerminalPane::request_terminate`, RFC-043
    /// D1's own disjunction, response 342) can force this reader's
    /// duplicate closed *before* this value would otherwise drop --
    /// needed because `RunningTerminal`'s own close of its copy of the
    /// same master is only the pty's *last* reference, and so only
    /// actually triggers a hangup, once this one is already gone too.
    ///
    /// Idempotent: every step is itself idempotent (`Option::take` on an
    /// already-`None` field is a no-op; a second signal is harmless), so
    /// calling this explicitly and then letting `Drop` run anyway --
    /// which every caller's own drop still does -- performs no second
    /// shutdown, just two calls that agree the first one already
    /// finished. **After this returns, [`Self::drain_available`] panics**
    /// -- there is no more reader thread left to have buffered anything,
    /// and this reader will never receive real PTY output again.
    ///
    /// Response 201: the shutdown signal and the `receiver` drop are two
    /// independent unblock paths, both needed. Signalling `shutdown`
    /// wakes a thread parked in `poll(2)` on a live, silent child (no
    /// data, pty not hung up) -- the fd shows readable, `reader_thread_loop`
    /// sees it ahead of the pty fd and returns immediately; a failure
    /// here is not actionable (the join below is the only thing that
    /// must still complete), so it is not propagated. Dropping
    /// `receiver` is what unblocks a thread already past `poll(2)` and
    /// parked in `sender.send` on a full channel instead, which is not
    /// reading `shutdown` at that moment: it makes that blocked (or any
    /// future) `send` return an error, which is `reader_thread_loop`'s
    /// own signal to stop. With both unblock paths in place, the join
    /// below can no longer depend on the child's own behaviour --
    /// `dropping_a_reader_over_a_live_silent_child_completes_promptly`
    /// proves it under a real timeout, so a regression here fails that
    /// test rather than hanging the suite.
    pub fn shutdown(&mut self) {
        self.shutdown.signal();
        self.receiver.take();
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

impl Drop for TerminalReader {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// RFC-028 D5: the only consumer of this value, [`TerminalReader::
/// os_thread_id`], feeds it into a `/proc/self/task/<tid>/stat` read in
/// `reader/tests.rs` -- genuinely Linux-only, not merely untested
/// elsewhere, so the macOS arm here only needs to produce *some*
/// process-local, comparison-stable identifier, not a kernel tid in the
/// Linux sense. That test gets its own, Linux-only `#[cfg]`; this
/// function's job is only to compile and return something real on both
/// platforms.
#[cfg(target_os = "linux")]
fn current_thread_id() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

/// `pthread_threadid_np`, not a `0`/`NULL` sentinel for "current thread"
/// -- that behaviour is documented for Apple's own API, but
/// `libc::pthread_self()` is unambiguous and costs nothing extra.
/// Truncated to `i32` from the real `u64` id: every consumer treats this
/// as an opaque, process-local comparison key, never as an id fed to a
/// further kernel call the way Linux's own tid is elsewhere in this
/// module.
#[cfg(target_os = "macos")]
fn current_thread_id() -> i32 {
    let mut tid: u64 = 0;
    // SAFETY: `tid` is a valid, aligned `u64` the call writes through;
    // `pthread_self()` always returns the calling thread's own, valid
    // handle.
    unsafe {
        libc::pthread_threadid_np(libc::pthread_self(), &mut tid);
    }
    tid as i32
}

/// RFC-017 Amendment 1, PR-A1-C: every exit path here does the same two
/// things in the same order before returning -- mark `reader_alive`
/// `false`, then signal `wake` -- so a `WakeNotifier` can never observe
/// "still alive" after the reader has actually stopped, nor miss the
/// final wake that tells it so. Factored into one place rather than
/// repeated at each `return`, so that ordering cannot drift between
/// call sites.
fn reader_thread_loop(
    mut master: fs::File,
    sender: SyncSender<Vec<u8>>,
    shutdown: WakeSignal,
    wake: WakeSignal,
    reader_alive: Arc<AtomicBool>,
    mut transcript: Option<ReaderTranscriptState>,
) {
    let pty_fd = master.as_raw_fd();
    let mut buffer = [0_u8; READ_CHUNK_BYTES];

    loop {
        match poll_for_readiness(pty_fd, &shutdown) {
            PollOutcome::ShutdownRequested | PollOutcome::Failed => {
                return stop_reading(&reader_alive, &wake);
            }
            PollOutcome::PtyReadable => {}
        }

        loop {
            match master.read(&mut buffer) {
                Ok(0) => return stop_reading(&reader_alive, &wake),
                Ok(bytes_read) => {
                    let chunk = &buffer[..bytes_read];
                    let outcome = transcript
                        .as_mut()
                        .map(|state| state.record_write(chunk))
                        .unwrap_or(TranscriptCaptureOutcome::Send);
                    match outcome {
                        TranscriptCaptureOutcome::Send => {
                            if sender.send(chunk.to_vec()).is_err() {
                                return stop_reading(&reader_alive, &wake);
                            }
                            wake.signal();
                        }
                        TranscriptCaptureOutcome::Suppress { stop_reading: true } => {
                            // RFC-011 Amendment 2, D3: `RequiredLocalBounded`.
                            // This chunk is discarded, not displayed
                            // (see `ReaderTranscriptState::record_write`'s
                            // own doc for why), and no further reading
                            // happens -- the child stalls on its own
                            // `write()` once the pty's kernel buffer
                            // fills, since nothing drains it from here
                            // on. Not killed: the caller still decides
                            // termination, and this thread stays alive
                            // and joinable, responsive only to shutdown
                            // from now on.
                            wake.signal();
                            return wait_for_shutdown_only(&reader_alive, &shutdown, &wake);
                        }
                        TranscriptCaptureOutcome::Suppress {
                            stop_reading: false,
                        } => {
                            // `LocalBounded`: this one chunk is
                            // discarded, but reading resumes immediately
                            // -- capture is now permanently off for the
                            // rest of this reader's life, and the
                            // *next* chunk read is `Send` again.
                            wake.signal();
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return stop_reading(&reader_alive, &wake),
            }
        }
    }
}

/// The reader thread's last act on every exit path: mark itself dead,
/// then wake anyone waiting so they notice -- both the "the reader has
/// permanently stopped" state and the "something happened, go check"
/// signal reuse the same [`WakeSignal`], ordered so a waiter reading
/// `false` off `reader_alive` can trust it.
fn stop_reading(reader_alive: &AtomicBool, wake: &WakeSignal) {
    reader_alive.store(false, Ordering::Release);
    wake.signal();
}

/// RFC-011 Amendment 2, D3: `RequiredLocalBounded`'s "stop reading"
/// landing state -- waits on exactly the shutdown signal and nothing
/// else, which is exactly what this needs. **Deliberately does not call
/// `stop_reading` (mark dead) on entry**: the reader has not
/// permanently ended -- the child is still alive, just blocked -- so
/// `reader_alive` must stay `true` for as long as this thread is merely
/// stalled rather than actually gone. Only once real shutdown arrives
/// (`Drop`, or any other future termination path that reuses the same
/// signal) does this proceed to the normal exit, at which point
/// `reader_alive` finally becomes `false` and one last wake fires --
/// the same guarantee every other exit path in this module already
/// gives a `WakeNotifier`.
fn wait_for_shutdown_only(reader_alive: &AtomicBool, shutdown: &WakeSignal, wake: &WakeSignal) {
    shutdown.wait();
    stop_reading(reader_alive, wake);
}

enum PollOutcome {
    PtyReadable,
    ShutdownRequested,
    Failed,
}

/// Blocks until the PTY master is readable, `shutdown` is signalled, or
/// `poll(2)` itself fails unrecoverably. Checks the shutdown fd first:
/// if a `Drop` signal and real PTY output race on the same wakeup,
/// exiting promptly is correct either way, since whatever is still in
/// the PTY's kernel buffer is not this module's job to keep draining
/// once the caller has asked to stop.
///
/// **Identical on both platforms, deliberately**: this relies on a
/// `WakeSignal`'s underlying fd being directly `poll(2)`-able, which is
/// true for Linux's `eventfd` by construction and, for macOS's `kqueue`,
/// is documented BSD/Darwin behaviour rather than something this module
/// invented. `dropping_a_reader_over_a_live_silent_child_completes_promptly`
/// (this module's own bounded-timeout test) is what would catch a
/// regression in that assumption on the real runner -- a wrong answer
/// here is a hang, not a compile error, which is exactly why that test
/// exists under a hard timeout instead of asserting on documentation.
fn poll_for_readiness(pty_fd: RawFd, shutdown: &WakeSignal) -> PollOutcome {
    let mut fds = [
        libc::pollfd {
            fd: pty_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: shutdown.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
    ];

    loop {
        let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return PollOutcome::Failed;
        }
        if fds[1].revents != 0 {
            return PollOutcome::ShutdownRequested;
        }
        if fds[0].revents != 0 {
            return PollOutcome::PtyReadable;
        }
        // Spurious wakeup with nothing actually ready; poll again.
    }
}

/// RFC-028 D5 (review 593): the cross-platform primitive behind both
/// `TerminalReader`'s shutdown signal and [`WakeNotifier`]. Exposes only
/// the two operations a caller needs -- [`Self::signal`] and
/// [`Self::wait`] -- never a raw file descriptor whose *kind* (counter
/// vs. kqueue) a caller could structure around.
///
/// **The contract, stated exactly because the two backends implement it
/// differently**: after `signal` returns, at least one subsequent
/// `wait` call (on this handle or any of its [`Self::try_clone`]s)
/// observes a wake. This type does **not** promise a wake count, and
/// does **not** promise one wake per signal -- `eventfd` counts (N
/// signals before any wait can produce a count of N, drained by
/// whichever wait drains it); `EVFILT_USER` with `EV_CLEAR` coalesces (N
/// signals before any wait can still produce exactly one observed
/// wake). Every caller in this module already tolerates spurious and
/// coalesced wakes -- none of them infer "how many chunks were sent"
/// from "how many times I was woken."
///
/// Linux: one `eventfd(2)`, `EFD_CLOEXEC` set atomically at creation.
/// macOS: one `kqueue(2)` with a single `EVFILT_USER` registration,
/// triggered via `kevent(..., NOTE_TRIGGER)` -- chosen over a self-pipe
/// specifically because it has no write end for a concurrently-exec'd
/// child to inherit (review 593, Decision 1): the self-pipe failure mode
/// (a leaked write end means `wait` never sees EOF, so shutdown hangs)
/// does not exist for this backend at all, regardless of `CLOEXEC`
/// timing. The precedent is not this module's own invention: `polling`
/// (already a transitive dependency) uses the same filter for the same
/// reason, by its own account, "to avoid allocating a pipe."
struct WakeSignal {
    #[cfg(target_os = "linux")]
    file: fs::File,
    #[cfg(target_os = "macos")]
    kq: Arc<std::os::fd::OwnedFd>,
}

#[cfg(target_os = "linux")]
impl WakeSignal {
    fn create() -> io::Result<Self> {
        // SAFETY: no preconditions beyond kernel resource limits.
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
        if fd == -1 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` was just returned by `eventfd(2)` above, valid
        // and owned by nothing else yet.
        Ok(Self {
            file: unsafe { fs::File::from_raw_fd(fd) },
        })
    }

    fn signal(&self) {
        let value: u64 = 1;
        let bytes = value.to_ne_bytes();
        // SAFETY: `self.file`'s fd is a valid, live `eventfd`; `bytes`
        // is a valid 8-byte buffer for the duration of this call.
        // Best-effort: a write failure here is not actionable by any
        // caller (`Drop`, or the reader thread's own exit path), and a
        // stuck signal would surface as a hang in `wait` instead.
        unsafe {
            libc::write(self.file.as_raw_fd(), bytes.as_ptr().cast(), bytes.len());
        }
    }

    fn wait(&self) -> bool {
        if !poll_single_fd(self.file.as_raw_fd()) {
            return false;
        }
        let mut buffer = [0_u8; 8];
        let _ = (&self.file).read(&mut buffer);
        true
    }

    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
        })
    }

    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

#[cfg(target_os = "macos")]
impl WakeSignal {
    /// `kqueue(2)` has no atomic-`CLOEXEC` constructor on Apple targets
    /// (checked directly against the vendored `libc` crate and against
    /// `rustix`, which omits its own flagged `pipe_with` entirely for
    /// `apple` rather than ship a racy fallback -- review 593). `fcntl`
    /// here is the best available, matching what `polling` itself does
    /// for the same reason. `assert_cloexec_macos` below is the
    /// postcondition that actually catches a regression -- same shape
    /// as `pty.rs`'s own `assert_cloexec`, refusing rather than
    /// continuing if it is somehow unset.
    fn create() -> io::Result<Self> {
        // SAFETY: no preconditions beyond kernel resource limits.
        let fd = unsafe { libc::kqueue() };
        if fd == -1 {
            return Err(io::Error::last_os_error());
        }
        if let Err(error) = Self::make_cloexec_and_register(fd) {
            // SAFETY: `fd` is a valid, still-owned fd this function
            // created and is refusing to hand back.
            unsafe {
                libc::close(fd);
            }
            return Err(error);
        }
        // SAFETY: `fd` is a valid, fully set-up kqueue fd, owned by
        // nothing else yet.
        let owned = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        Ok(Self {
            kq: Arc::new(owned),
        })
    }

    fn make_cloexec_and_register(fd: RawFd) -> io::Result<()> {
        // SAFETY: `fd` is the just-created, valid kqueue fd.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(io::Error::last_os_error());
        }
        assert_cloexec_macos(fd)?;

        let setup = libc::kevent {
            ident: WAKE_SIGNAL_IDENT,
            filter: libc::EVFILT_USER,
            flags: libc::EV_ADD | libc::EV_CLEAR,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: `fd` is the just-created, valid kqueue fd; `setup` is
        // a single, fully-initialized `kevent` registering this
        // kqueue's own one-shot wake event; no output buffer is
        // requested.
        let result =
            unsafe { libc::kevent(fd, &setup, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn signal(&self) {
        let trigger = libc::kevent {
            ident: WAKE_SIGNAL_IDENT,
            filter: libc::EVFILT_USER,
            flags: 0,
            fflags: libc::NOTE_TRIGGER,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: `self.kq` is a valid, live kqueue fd, already
        // registered (at construction) for this exact `ident`/
        // `EVFILT_USER` pair; no output buffer requested. Best-effort,
        // matching this module's `eventfd` write convention above: not
        // actionable by any caller, and a stuck signal would surface as
        // a hang in `wait` instead.
        unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                &trigger,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            );
        }
    }

    fn wait(&self) -> bool {
        loop {
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            // SAFETY: `self.kq` is a valid, live kqueue fd; `event` is a
            // single, valid output slot; a `NULL` timeout blocks
            // indefinitely, the same "real wait, not a sleep" contract
            // this module's own `poll(2)` calls already give.
            let result = unsafe {
                libc::kevent(
                    self.kq.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    &mut event,
                    1,
                    std::ptr::null(),
                )
            };
            if result == -1 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return false;
            }
            if result > 0 {
                return true;
            }
            // `result == 0`: no event despite a `NULL`/infinite timeout
            // -- not supposed to happen, but retrying costs nothing and
            // keeps this loop's only exit paths "woken" or "the kqueue
            // itself failed," matching the Linux backend's own shape.
        }
    }

    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            kq: Arc::clone(&self.kq),
        })
    }

    fn as_raw_fd(&self) -> RawFd {
        self.kq.as_raw_fd()
    }
}

/// Arbitrary and private to this module: `EVFILT_USER` idents are not
/// file descriptors and need no relationship to anything outside the
/// one kqueue `WakeSignal::create` registers them against, so any fixed
/// value distinguishes this registration from itself consistently.
#[cfg(target_os = "macos")]
const WAKE_SIGNAL_IDENT: libc::uintptr_t = 1;

/// Same shape as `pty.rs`'s own `assert_cloexec`: a postcondition, not
/// the mechanism -- `make_cloexec_and_register`'s `fcntl` call above is
/// what actually sets the flag. This only reads it back and refuses
/// loudly if it is somehow unset, so a future refactor that drops that
/// call fails here instead of silently reopening the window `WakeSignal`
/// exists to close.
#[cfg(target_os = "macos")]
fn assert_cloexec_macos(fd: RawFd) -> io::Result<()> {
    // SAFETY: `fd` is a valid, live fd for the duration of this call.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        return Err(io::Error::last_os_error());
    }
    if flags & libc::FD_CLOEXEC == 0 {
        return Err(io::Error::other(
            "WakeSignal's kqueue fd was not close-on-exec after fcntl(F_SETFD, FD_CLOEXEC)",
        ));
    }
    Ok(())
}

/// Blocks until `fd` is readable or `poll(2)` itself fails
/// unrecoverably. Used only by [`WakeSignal`]'s own Linux `wait`
/// implementation -- the macOS backend waits via `kevent(2)` directly
/// instead, since consuming the event and waiting for it are one call
/// there.
#[cfg(target_os = "linux")]
fn poll_single_fd(fd: RawFd) -> bool {
    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };

    loop {
        let result = unsafe { libc::poll(&mut pollfd, 1, -1) };
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return false;
        }
        if pollfd.revents != 0 {
            return true;
        }
        // Spurious wakeup with nothing actually ready; poll again.
    }
}

#[cfg(test)]
mod tests;
