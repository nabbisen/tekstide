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
use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
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
/// preference for that discipline.
pub struct TerminalReader {
    // `Option` so `Drop` can explicitly drop this *before* joining the
    // thread -- a custom `Drop::drop` body runs before Rust's automatic
    // field drops, not after, so a plain `Receiver<Vec<u8>>` field would
    // still be alive for the whole body of `drop()`, and a join while it
    // is still alive can deadlock (see `Drop for TerminalReader`).
    receiver: Option<Receiver<Vec<u8>>>,
    join_handle: Option<JoinHandle<()>>,
    os_thread_id: i32,
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
    /// thread calls `poll(2)` with an infinite timeout on `master`'s
    /// fd. The kernel parks the thread until the fd is readable (or
    /// hung up), consuming no CPU while parked -- a real blocking
    /// primitive, not a fixed delay guessed to be short enough.
    pub(super) fn spawn(master: fs::File) -> Self {
        let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        // A one-shot handshake so the caller can learn the reader
        // thread's kernel thread id -- used only for the idle-CPU
        // measurement in this module's own tests
        // (`reader_thread_does_not_busy_wait_while_idle`); harmless
        // overhead (one syscall at thread start) on the production path.
        let (tid_sender, tid_receiver) = mpsc::sync_channel(1);
        let join_handle = std::thread::spawn(move || {
            let _ = tid_sender.send(current_thread_id());
            reader_thread_loop(master, sender);
        });
        let os_thread_id = tid_receiver
            .recv()
            .expect("reader thread should report its id before spawn returns");
        Self {
            receiver: Some(receiver),
            join_handle: Some(join_handle),
            os_thread_id,
        }
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
        // Explicitly drop `receiver` first, before joining. A custom
        // `Drop::drop` body runs *before* Rust's automatic field drops,
        // not after -- relying on struct field order here (as an earlier
        // version of this comment claimed) is wrong, and would leave
        // `receiver` alive for this entire function body. If the channel
        // was full and nothing was draining it, the reader thread is
        // blocked inside `sender.send`, and joining while its matching
        // `receiver` is still alive waits for a send that can now never
        // succeed and can never fail either -- a real deadlock, found via
        // a test that panicked mid-drain and then hung forever instead of
        // reporting its failure. Dropping `receiver` here makes that
        // blocked (or any future) `send` return an error immediately,
        // which is `reader_thread_loop`'s own signal to stop.
        self.receiver.take();

        // If the thread is instead parked in `poll(2)` waiting for PTY
        // data that never comes, this join blocks until the PTY reports
        // end-of-file or an error -- acceptable here because every
        // production caller of `spawn` already terminates the child (and
        // therefore closes every slave-side fd) before dropping its
        // `RunningTerminal`, per `termination.rs`'s own sequencing. A
        // `TerminalReader` created over a PTY whose child is deliberately
        // kept alive and silent would hang this join -- not a concern
        // this checkpoint's tests create, and flagged here rather than
        // hidden for whoever wires this into session teardown in
        // PR-A1-B/C.
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

fn current_thread_id() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

fn reader_thread_loop(mut master: fs::File, sender: SyncSender<Vec<u8>>) {
    let fd = master.as_raw_fd();
    let mut buffer = [0_u8; READ_CHUNK_BYTES];

    loop {
        if !block_until_readable(fd) {
            return;
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

/// Blocks until `fd` is readable or hung up. Returns `false` only when
/// `poll(2)` itself fails unrecoverably -- the reader thread's own
/// signal to stop rather than loop forever on a broken fd.
fn block_until_readable(fd: RawFd) -> bool {
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
        return true;
    }
}

#[cfg(test)]
mod tests;
