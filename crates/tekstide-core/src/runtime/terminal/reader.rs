//! RFC-017 Amendment 1, PR-A1-A: a dedicated reader thread over the PTY
//! master, feeding a bounded channel, built **alongside**
//! `LinuxTerminalRuntime::read_available_bounded_for` rather than in
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
//! **RFC-017 Amendment 1, PR-A1-C: [`WakeNotifier`], a second `eventfd`
//! separate from the shutdown one.** `read_available_bounded_for`'s old
//! call site was reached by a fixed-interval poll tick; removing that
//! tick (PR-A1-C's own job) needs something to replace its role of
//! telling a caller "go check this terminal now" without reintroducing
//! a timer. The reader thread signals this `eventfd` whenever it
//! buffers new bytes, and one final time on every exit path (shutdown,
//! EOF, or a fatal read error) -- see [`WakeNotifier::block_until_woken`]'s
//! own doc for why "the reader is done" needs an explicit signal rather
//! than being inferred from the `eventfd`'s own semantics.

use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;

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
    // The write end of an `eventfd(2)` the reader thread also `poll(2)`s
    // alongside the PTY master. Response 201: dropping `receiver` alone
    // only unblocks a thread parked in `sender.send` on a full channel;
    // it does nothing for the far more common case of a thread parked in
    // `poll(2)` on a live, silent child (no data, PTY not hung up). This
    // is what makes that case interruptible too -- see `Drop`.
    shutdown: fs::File,
    // RFC-017 Amendment 1, PR-A1-C: a second `eventfd`, signalled by the
    // reader thread (not `Drop`) every time it either buffers new bytes
    // or stops for good -- what lets a caller replace polling with a
    // real wait. `TerminalReader` itself never reads this fd; it exists
    // to be duplicated out via [`Self::try_clone_wake_notifier`] for
    // whoever needs to wake up when this reader has something to report.
    wake: fs::File,
    // Set to `false` by the reader thread immediately before its last
    // ever wake signal, on every exit path -- shutdown, EOF, or a fatal
    // read error. `eventfd`'s own counter has no notion of "the writer
    // is done" (unlike a pipe's write end closing), so this is the
    // explicit signal a `WakeNotifier` checks to know no further wakes
    // are coming, rather than trying to encode that into the counter
    // value itself.
    reader_alive: Arc<AtomicBool>,
}

/// A duplicated handle onto one [`TerminalReader`]'s wake `eventfd`,
/// obtained via [`TerminalReader::try_clone_wake_notifier`]. Carries no
/// PTY data of its own -- see [`Self::block_until_woken`].
pub struct WakeNotifier {
    file: fs::File,
    reader_alive: Arc<AtomicBool>,
}

impl WakeNotifier {
    /// Blocks (a real `poll(2)` park, not a sleep) until the reader
    /// thread has either buffered new bytes or stopped for good, then
    /// drains the `eventfd`'s accumulated count. Returns `true` if more
    /// wakes may still arrive -- the caller should act on this wake and
    /// call again; `false` means this was the reader's last wake ever,
    /// on any exit path (shutdown, EOF, or a fatal read error) -- the
    /// caller should still act on it once (there may be a final exit to
    /// notice) and then stop waiting, since nothing will signal this
    /// notifier again.
    pub fn block_until_woken(&self) -> bool {
        if !block_on_eventfd(self.file.as_raw_fd()) {
            return false;
        }
        let mut buffer = [0_u8; 8];
        let _ = (&self.file).read(&mut buffer);
        self.reader_alive.load(Ordering::Acquire)
    }

    /// A second, independent duplicate of this notifier's own `eventfd`
    /// handle, sharing the same underlying counter and `reader_alive`
    /// flag -- for a caller that only holds a borrow (`&WakeNotifier`,
    /// for example inside an API that hands out `&self` rather than
    /// ownership) but needs an owned, `'static` handle to move into a
    /// background thread. Failure here is the same resource-exhaustion
    /// case `try_clone_wake_notifier` can already fail on.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            reader_alive: Arc::clone(&self.reader_alive),
        })
    }
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
    /// Fallible only because `eventfd(2)` can fail (resource exhaustion)
    /// -- everything else here is infallible in practice.
    pub(super) fn spawn(master: fs::File) -> io::Result<Self> {
        // Owned by `TerminalReader` for the `write` side `Drop` uses to
        // signal shutdown; the reader thread below only needs the bare
        // fd number to `poll(2)` it, not ownership -- `shutdown` here
        // keeps the fd open for as long as `TerminalReader` (and
        // therefore the thread that polls it) exists.
        let shutdown_fd = create_eventfd()?;
        let shutdown = unsafe { fs::File::from_raw_fd(shutdown_fd) };
        // Same shape, second `eventfd`: kept alive here only so
        // `try_clone_wake_notifier` has something to duplicate from for
        // as long as this `TerminalReader` exists. Nothing on this
        // struct ever reads it.
        let wake_fd = create_eventfd()?;
        let wake = unsafe { fs::File::from_raw_fd(wake_fd) };
        let reader_alive = Arc::new(AtomicBool::new(true));

        let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        // A one-shot handshake so the caller can learn the reader
        // thread's kernel thread id -- used only for the idle-CPU
        // measurement in this module's own tests
        // (`reader_thread_does_not_busy_wait_while_idle`); harmless
        // overhead (one syscall at thread start) on the production path.
        let (tid_sender, tid_receiver) = mpsc::sync_channel(1);
        let thread_reader_alive = Arc::clone(&reader_alive);
        let join_handle = std::thread::spawn(move || {
            let _ = tid_sender.send(current_thread_id());
            reader_thread_loop(master, sender, shutdown_fd, wake_fd, thread_reader_alive);
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
        })
    }

    /// The reader thread's kernel thread id (`gettid(2)`), for
    /// diagnostics and for measuring its idle CPU usage directly against
    /// `/proc/self/task/<tid>/stat` rather than trusting the mechanism's
    /// description.
    pub fn os_thread_id(&self) -> i32 {
        self.os_thread_id
    }

    /// RFC-017 Amendment 1, PR-A1-C: a duplicate handle onto this
    /// reader's wake `eventfd`, for a caller that wants to wait for
    /// readiness instead of polling `drain_available` on a timer.
    /// `try_clone`'s failure mode is the same resource exhaustion
    /// `spawn` can already fail on.
    pub fn try_clone_wake_notifier(&self) -> io::Result<WakeNotifier> {
        Ok(WakeNotifier {
            file: self.wake.try_clone()?,
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

impl Drop for TerminalReader {
    fn drop(&mut self) {
        // Response 201: the previous version of this `drop` only had the
        // `receiver`-drop path below and could still hang forever --
        // that path unblocks a thread parked in `sender.send` on a full
        // channel, but does nothing for a thread parked in `poll(2)` on
        // a live, silent child (no data, PTY not hung up), which is the
        // common case once a real caller drops a reader for a terminal
        // that is simply not producing output right now. Writing to the
        // shutdown `eventfd` wakes `poll(2)` regardless of PTY state --
        // the fd shows readable, `reader_thread_loop` sees it ahead of
        // the PTY fd and returns immediately. A write failure here is
        // not actionable (`join` below is the only thing that must still
        // complete), so it is not propagated.
        let wakeup: u64 = 1;
        let _ = self.shutdown.write(&wakeup.to_ne_bytes());

        // Drop `receiver` next -- a custom `Drop::drop` body runs
        // *before* Rust's automatic field drops, not after, so this must
        // be explicit rather than relying on struct field order (an
        // earlier version of this comment claimed the latter and was
        // wrong). This is the second, independent unblock path: a thread
        // already past `poll(2)` and blocked inside `sender.send` on a
        // full channel is not reading `shutdown` at that moment, so the
        // write above does not reach it -- dropping `receiver` makes
        // that blocked (or any future) `send` return an error instead,
        // which is `reader_thread_loop`'s own signal to stop.
        self.receiver.take();

        // With both unblock paths in place, this join can no longer
        // depend on the child's own behaviour --
        // `dropping_a_reader_over_a_live_silent_child_completes_promptly`
        // proves it under a real timeout, so a regression here fails
        // that test rather than hanging the suite.
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

fn current_thread_id() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

/// RFC-017 Amendment 1, PR-A1-C: every exit path here does the same two
/// things in the same order before returning -- mark `reader_alive`
/// `false`, then signal `wake_fd` -- so a `WakeNotifier` can never
/// observe "still alive" after the reader has actually stopped, nor
/// miss the final wake that tells it so. Factored into one place rather
/// than repeated at each `return`, so that ordering cannot drift between
/// call sites.
fn reader_thread_loop(
    mut master: fs::File,
    sender: SyncSender<Vec<u8>>,
    shutdown_fd: RawFd,
    wake_fd: RawFd,
    reader_alive: Arc<AtomicBool>,
) {
    let pty_fd = master.as_raw_fd();
    let mut buffer = [0_u8; READ_CHUNK_BYTES];

    loop {
        match poll_for_readiness(pty_fd, shutdown_fd) {
            PollOutcome::ShutdownRequested | PollOutcome::Failed => {
                return stop_reading(&reader_alive, wake_fd);
            }
            PollOutcome::PtyReadable => {}
        }

        loop {
            match master.read(&mut buffer) {
                Ok(0) => return stop_reading(&reader_alive, wake_fd),
                Ok(bytes_read) => {
                    if sender.send(buffer[..bytes_read].to_vec()).is_err() {
                        return stop_reading(&reader_alive, wake_fd);
                    }
                    signal_wake(wake_fd);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return stop_reading(&reader_alive, wake_fd),
            }
        }
    }
}

/// The reader thread's last act on every exit path: mark itself dead,
/// then wake anyone waiting so they notice -- both the "the reader has
/// permanently stopped" state and the "something happened, go check"
/// signal reuse the same `eventfd`, ordered so a waiter reading `false`
/// off `reader_alive` can trust it.
fn stop_reading(reader_alive: &AtomicBool, wake_fd: RawFd) {
    reader_alive.store(false, Ordering::Release);
    signal_wake(wake_fd);
}

fn signal_wake(wake_fd: RawFd) {
    let value: u64 = 1;
    let bytes = value.to_ne_bytes();
    unsafe {
        libc::write(wake_fd, bytes.as_ptr().cast(), bytes.len());
    }
}

enum PollOutcome {
    PtyReadable,
    ShutdownRequested,
    Failed,
}

/// Blocks until the PTY master is readable, the shutdown `eventfd` is
/// signalled, or `poll(2)` itself fails unrecoverably. Checks the
/// shutdown fd first: if a `Drop` write and real PTY output race on the
/// same wakeup, exiting promptly is correct either way, since whatever
/// is still in the PTY's kernel buffer is not this module's job to keep
/// draining once the caller has asked to stop.
fn poll_for_readiness(pty_fd: RawFd, shutdown_fd: RawFd) -> PollOutcome {
    let mut fds = [
        libc::pollfd {
            fd: pty_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: shutdown_fd,
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

/// Blocks until `fd` is readable or `poll(2)` itself fails
/// unrecoverably. The single-fd counterpart to [`poll_for_readiness`],
/// used by [`WakeNotifier::block_until_woken`] -- that caller has only
/// one fd to wait on, not a PTY master plus a shutdown signal.
fn block_on_eventfd(fd: RawFd) -> bool {
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

fn create_eventfd() -> io::Result<RawFd> {
    let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
    if fd == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

#[cfg(test)]
mod tests;
