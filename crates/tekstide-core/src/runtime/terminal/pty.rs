use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};

use super::{BoundedRuntimeSummary, TerminalDimensions};

pub(super) struct OpenPty {
    master: Option<fs::File>,
    slave: Option<RawFd>,
}

impl OpenPty {
    pub(super) fn new(dimensions: TerminalDimensions) -> Result<Self, BoundedRuntimeSummary> {
        let mut master = -1;
        let mut slave = -1;
        let winsize = libc::winsize {
            ws_row: dimensions.rows,
            ws_col: dimensions.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let result = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &winsize,
            )
        };

        if result == -1 {
            return Err(BoundedRuntimeSummary::new(format!(
                "failed to open Linux PTY: {}",
                io::Error::last_os_error()
            )));
        }

        if let Err(error) = set_nonblocking(master) {
            close_fd(master);
            close_fd(slave);
            return Err(BoundedRuntimeSummary::new(error));
        }

        // pty-master-fd-inheritance handoff: glibc's `openpty` does not set
        // `O_CLOEXEC` on either fd it returns, and nothing downstream of
        // this call did either -- every child this process ever spawns
        // (every terminal, every agent run) inherited every PTY master
        // already open at that moment, crossing RFC-009's terminal
        // security boundary. Both fds get it, not only the master:
        // `duplicate_slave` is unaffected (`dup(2)` never copies
        // `FD_CLOEXEC` onto the new descriptor, by design -- that is what
        // makes the stdin/stdout/stderr/ctty copies still reach the
        // child), so this cannot break the slave's own intended use.
        if let Err(error) = set_cloexec(master).and_then(|()| set_cloexec(slave)) {
            close_fd(master);
            close_fd(slave);
            return Err(BoundedRuntimeSummary::new(error));
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

/// `F_SETFD`/`FD_CLOEXEC`, not `F_SETFL`/`O_NONBLOCK` (`set_nonblocking`,
/// above) -- close-on-exec is a *descriptor* flag, not a file-status
/// flag, and the two live in disjoint `fcntl` command spaces. Getting
/// this wrong (e.g. reusing `F_GETFL`/`F_SETFL`) would silently no-op:
/// `F_SETFL` ignores bits it does not recognise rather than erroring, so
/// a mixed-up call would report success without setting anything.
fn set_cloexec(fd: RawFd) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        return Err(format!(
            "failed to read fd descriptor flags: {}",
            io::Error::last_os_error()
        ));
    }

    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    if result == -1 {
        Err(format!(
            "failed to set PTY fd close-on-exec: {}",
            io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}
