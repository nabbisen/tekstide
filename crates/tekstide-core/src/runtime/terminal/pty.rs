use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};

use super::{BoundedRuntimeSummary, TerminalDimensions};

pub(super) struct OpenPty {
    master: Option<fs::File>,
    slave: Option<RawFd>,
}

impl OpenPty {
    /// Review 579: this used to call `libc::openpty` (master and slave
    /// both opened without `O_CLOEXEC`), then apply `FD_CLOEXEC` to each
    /// via a separate `fcntl` afterward. That left a real window -- two
    /// syscalls wide (`set_nonblocking`, then the `fcntl` pair) -- during
    /// which the fds existed without `CLOEXEC`; a `fork` on *any other
    /// thread in this process* landing in that window took a copy whose
    /// `CLOEXEC` was never set and never could be (the flag is copied at
    /// fork time; setting it afterward in this process cannot reach a
    /// copy already taken), which then survived the forked child's own
    /// `exec` and persisted for that child's entire life -- demonstrated
    /// directly: a bounded, 3-second experiment (max 4 concurrent
    /// children, scanning only its own tracked pids, never the whole
    /// process table) found 7,470 such leaked descriptors across 46,143
    /// checks against 236 forked children. This rebuilds the open with
    /// `O_CLOEXEC` requested atomically at each `open()` call instead --
    /// the same reason `std::os::unix::net::UnixListener::bind` (which
    /// uses `SOCK_CLOEXEC` at its own `socket()` call) was never
    /// susceptible to this class of race at all.
    pub(super) fn new(dimensions: TerminalDimensions) -> Result<Self, BoundedRuntimeSummary> {
        let master = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) };
        if master == -1 {
            return Err(BoundedRuntimeSummary::new(format!(
                "failed to open PTY master: {}",
                io::Error::last_os_error()
            )));
        }

        if let Err(error) = grant_and_unlock(master) {
            close_fd(master);
            return Err(BoundedRuntimeSummary::new(error));
        }

        let slave_name = match slave_path(master) {
            Ok(name) => name,
            Err(error) => {
                close_fd(master);
                return Err(BoundedRuntimeSummary::new(error));
            }
        };

        // SAFETY: `slave_name` is a NUL-terminated buffer filled by
        // `ptsname_r` above, valid for the duration of this call.
        let slave = unsafe {
            libc::open(
                slave_name.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
            )
        };
        if slave == -1 {
            close_fd(master);
            return Err(BoundedRuntimeSummary::new(format!(
                "failed to open PTY slave: {}",
                io::Error::last_os_error()
            )));
        }

        if let Err(error) = set_nonblocking(master) {
            close_fd(master);
            close_fd(slave);
            return Err(BoundedRuntimeSummary::new(error));
        }

        // Postcondition, not mechanism: `O_CLOEXEC` above is what
        // actually closes the race (see this function's own doc comment).
        // This only reads the flag back and errors loudly if it is
        // somehow unset, so a future refactor that drops `O_CLOEXEC` from
        // either `open()` call fails here instead of silently reopening
        // the window. `duplicate_slave` is unaffected either way: `dup(2)`
        // never copies `FD_CLOEXEC` onto the new descriptor, by design --
        // that is what lets the stdin/stdout/stderr/ctty copies still
        // reach the child.
        if let Err(error) = assert_cloexec(master).and_then(|()| assert_cloexec(slave)) {
            close_fd(master);
            close_fd(slave);
            return Err(BoundedRuntimeSummary::new(error));
        }

        let winsize = libc::winsize {
            ws_row: dimensions.rows,
            ws_col: dimensions.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: `master` is a valid, just-opened fd; `TIOCSWINSZ` only
        // reads `winsize` and performs no action that could invalidate it.
        if unsafe { libc::ioctl(master, libc::TIOCSWINSZ, &winsize) } == -1 {
            close_fd(master);
            close_fd(slave);
            return Err(BoundedRuntimeSummary::new(format!(
                "failed to set initial PTY size: {}",
                io::Error::last_os_error()
            )));
        }

        Ok(Self {
            master: Some(unsafe { fs::File::from_raw_fd(master) }),
            slave: Some(slave),
        })
    }

    pub(super) fn into_master(mut self) -> fs::File {
        self.master
            .take()
            .expect("PTY master remains available until runtime owns it")
    }

    pub(super) fn close_slave(&mut self) {
        if let Some(fd) = self.slave.take() {
            close_fd(fd);
        }
    }

    pub(super) fn duplicate_slave(&self, context: &str) -> Result<RawFd, BoundedRuntimeSummary> {
        let duplicated = unsafe { libc::dup(self.slave_fd()) };
        if duplicated == -1 {
            Err(BoundedRuntimeSummary::new(format!(
                "{context}: {}",
                io::Error::last_os_error()
            )))
        } else {
            Ok(duplicated)
        }
    }

    fn slave_fd(&self) -> RawFd {
        self.slave
            .expect("PTY slave fd remains open until child is spawned")
    }
}

impl Drop for OpenPty {
    fn drop(&mut self) {
        self.close_slave();
    }
}

pub(super) fn close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

pub(super) fn resize_master(
    master: &fs::File,
    dimensions: TerminalDimensions,
) -> Result<(), BoundedRuntimeSummary> {
    let winsize = libc::winsize {
        ws_row: dimensions.rows,
        ws_col: dimensions.cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let result = unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) };

    if result == -1 {
        Err(BoundedRuntimeSummary::new(format!(
            "failed to resize PTY: {}",
            io::Error::last_os_error()
        )))
    } else {
        Ok(())
    }
}

fn set_nonblocking(fd: RawFd) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(format!(
            "failed to read fd flags: {}",
            io::Error::last_os_error()
        ));
    }

    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result == -1 {
        Err(format!(
            "failed to set PTY master nonblocking: {}",
            io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

/// `F_GETFD`, not `F_GETFL` (`set_nonblocking`, above) -- close-on-exec is
/// a *descriptor* flag, not a file-status flag, and the two live in
/// disjoint `fcntl` command spaces. Review 579: no longer sets
/// `FD_CLOEXEC` -- `O_CLOEXEC` at each `open()` call in `OpenPty::new`
/// does that atomically now, closing a fork/exec race a post-hoc
/// `fcntl` could never close. This only reads the flag back as a
/// postcondition check.
///
/// Review 580: a failure here means `OpenPty::new` returns `Err` and no
/// terminal opens at all -- deliberately. If some platform's `open()`
/// ever accepted `O_CLOEXEC` without honouring it, failing silently here
/// would let every terminal on that platform start up with the exact
/// fork/exec leak this function exists to rule out. Refusing to open
/// rather than opening unprotected is the correct posture for a security
/// property; do not turn this into a warning that lets `OpenPty::new`
/// keep going.
fn assert_cloexec(fd: RawFd) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        return Err(format!(
            "failed to read fd descriptor flags: {}",
            io::Error::last_os_error()
        ));
    }
    if flags & libc::FD_CLOEXEC == 0 {
        return Err(
            "PTY fd was not close-on-exec immediately after creation -- O_CLOEXEC did not take \
             effect at open()"
                .to_string(),
        );
    }
    Ok(())
}

fn grant_and_unlock(master_fd: RawFd) -> Result<(), String> {
    // SAFETY: `master_fd` is a valid, just-opened `posix_openpt` fd.
    if unsafe { libc::grantpt(master_fd) } == -1 {
        return Err(format!("grantpt failed: {}", io::Error::last_os_error()));
    }
    // SAFETY: same fd, still valid.
    if unsafe { libc::unlockpt(master_fd) } == -1 {
        return Err(format!("unlockpt failed: {}", io::Error::last_os_error()));
    }
    Ok(())
}

/// `ptsname_r`, not `ptsname` -- the latter writes into a single
/// process-wide static buffer with no thread-safety guarantee at all,
/// which concurrent terminal creation across threads (exactly what
/// surfaced this file's own fork/exec race) would race on directly.
fn slave_path(master_fd: RawFd) -> Result<[libc::c_char; 128], String> {
    let mut buf = [0 as libc::c_char; 128];
    // SAFETY: `buf` is valid for `buf.len()` bytes for the duration of
    // this call; `master_fd` is a valid, just-opened, granted-and-
    // unlocked `posix_openpt` fd.
    let result = unsafe { libc::ptsname_r(master_fd, buf.as_mut_ptr(), buf.len()) };
    if result != 0 {
        return Err(format!(
            "ptsname_r failed: {}",
            io::Error::from_raw_os_error(result)
        ));
    }
    Ok(buf)
}
