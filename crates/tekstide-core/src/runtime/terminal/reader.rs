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

use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
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
    /// thread calls `poll(2)` with an infinite timeout on `master`'s fd
    /// and a shutdown `eventfd`. The kernel parks the thread until
    /// either is ready, consuming no CPU while parked -- a real blocking
    /// primitive, not a fixed delay guessed to be short enough.
    ///
    /// Fallible only because `eventfd(2)` can fail (resource exhaustion)
    /// -- everything else here is infallible in practice.
    pub(super) fn spawn(master: fs::File) -> io::Result<Self> {
        let shutdown_fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
        if shutdown_fd == -1 {
            return Err(io::Error::last_os_error());
        }
        // Owned by `TerminalReader` for the `write` side `Drop` uses to
        // signal shutdown; the reader thread below only needs the bare
        // fd number to `poll(2)` it, not ownership -- `shutdown` here
        // keeps the fd open for as long as `TerminalReader` (and
        // therefore the thread that polls it) exists.
        let shutdown = unsafe { fs::File::from_raw_fd(shutdown_fd) };

        let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        // A one-shot handshake so the caller can learn the reader
        // thread's kernel thread id -- used only for the idle-CPU
        // measurement in this module's own tests
        // (`reader_thread_does_not_busy_wait_while_idle`); harmless
        // overhead (one syscall at thread start) on the production path.
        let (tid_sender, tid_receiver) = mpsc::sync_channel(1);
        let join_handle = std::thread::spawn(move || {
            let _ = tid_sender.send(current_thread_id());
            reader_thread_loop(master, sender, shutdown_fd);
        });
        let os_thread_id = tid_receiver
            .recv()
            .expect("reader thread should report its id before spawn returns");
        Ok(Self {
            receiver: Some(receiver),
            join_handle: Some(join_handle),
            os_thread_id,
            shutdown,
        })
    }

    /// The reader thread's kernel thread id (`gettid(2)`), for
    /// diagnostics and for measuring its idle CPU usage directly against
    /// `/proc/self/task/<tid>/stat` rather than trusting the mechanism's
    /// description.
    pub fn os_thread_id(&self) -> i32 {
        self.os_thread_id
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

fn reader_thread_loop(mut master: fs::File, sender: SyncSender<Vec<u8>>, shutdown_fd: RawFd) {
    let pty_fd = master.as_raw_fd();
    let mut buffer = [0_u8; READ_CHUNK_BYTES];

    loop {
        match poll_for_readiness(pty_fd, shutdown_fd) {
            PollOutcome::ShutdownRequested | PollOutcome::Failed => return,
            PollOutcome::PtyReadable => {}
        }

        loop {
            match master.read(&mut buffer) {
                Ok(0) => return,
                Ok(bytes_read) => {
                    if sender.send(buffer[..bytes_read].to_vec()).is_err() {
                        return;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return,
            }
        }
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

#[cfg(test)]
mod tests;
