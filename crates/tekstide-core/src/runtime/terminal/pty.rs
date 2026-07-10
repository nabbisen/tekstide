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

        set_nonblocking(master).map_err(BoundedRuntimeSummary::new)?;

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
