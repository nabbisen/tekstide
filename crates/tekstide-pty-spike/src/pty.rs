use std::fs;
use std::io::{self, Read};
use std::os::fd::{FromRawFd, RawFd};
use std::time::{Duration, Instant};

pub struct OpenPty {
    pub master: fs::File,
    slave: Option<RawFd>,
}

impl OpenPty {
    pub fn new(rows: u16, cols: u16) -> Result<Self, String> {
        let mut master = -1;
        let mut slave = -1;
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
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
            return Err(format!(
                "failed to open Linux PTY: {}",
                io::Error::last_os_error()
            ));
        }

        set_nonblocking(master, "set PTY master nonblocking")?;

        Ok(Self {
            master: unsafe { fs::File::from_raw_fd(master) },
            slave: Some(slave),
        })
    }

    pub fn close_slave(&mut self) {
        if let Some(fd) = self.slave.take() {
            close_fd(fd);
        }
    }

    pub fn duplicate_slave(&self, context: &str) -> Result<RawFd, String> {
        let duplicated = unsafe { libc::dup(self.slave_fd()) };
        if duplicated == -1 {
            Err(format!("{context}: {}", io::Error::last_os_error()))
        } else {
            Ok(duplicated)
        }
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        let winsize = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let result = unsafe { libc::ioctl(self.master_fd(), libc::TIOCSWINSZ, &winsize) };
        if result == -1 {
            Err(format!(
                "failed to resize PTY to {rows}x{cols}: {}",
                io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    }

    fn master_fd(&self) -> RawFd {
        use std::os::fd::AsRawFd;
        self.master.as_raw_fd()
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

pub fn read_until_child_exits(
    master: &mut fs::File,
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let started = Instant::now();
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut child_exited = false;

    loop {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => output.extend_from_slice(&buffer[..bytes_read]),
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || error.raw_os_error() == Some(libc::EIO) =>
            {
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if child_exited {
                    break;
                }
            }
            Err(error) => return Err(format!("failed to read PTY output: {error}")),
        }

        if !child_exited {
            child_exited = child
                .try_wait()
                .map_err(|error| format!("failed to inspect PTY shell status: {error}"))?
                .is_some();
        }

        if started.elapsed() > timeout {
            let _ = child.kill();
            return Err(format!(
                "timed out waiting for scripted PTY shell after {timeout:?}"
            ));
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(output)
}

pub fn read_until_child_exits_capped(
    master: &mut fs::File,
    child: &mut std::process::Child,
    timeout: Duration,
    cap_bytes: usize,
) -> Result<CappedOutput, String> {
    let started = Instant::now();
    let mut output = CappedOutput::new(cap_bytes);
    let mut buffer = [0_u8; 4096];
    let mut child_exited = false;

    loop {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => output.push(&buffer[..bytes_read]),
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || error.raw_os_error() == Some(libc::EIO) =>
            {
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if child_exited {
                    break;
                }
            }
            Err(error) => return Err(format!("failed to read capped PTY output: {error}")),
        }

        if !child_exited {
            child_exited = child
                .try_wait()
                .map_err(|error| format!("failed to inspect PTY shell status: {error}"))?
                .is_some();
        }

        if started.elapsed() > timeout {
            let _ = child.kill();
            return Err(format!(
                "timed out waiting for capped PTY output after {timeout:?}"
            ));
        }

        std::thread::sleep(Duration::from_millis(5));
    }

    Ok(output)
}

pub fn read_until_contains(
    master: &mut fs::File,
    marker: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let started = Instant::now();
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => {
                output.extend_from_slice(&buffer[..bytes_read]);
                if contains_subsequence(&output, marker) {
                    break;
                }
            }
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || error.raw_os_error() == Some(libc::EIO) =>
            {
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(format!("failed to read PTY marker output: {error}")),
        }

        if started.elapsed() > timeout {
            return Err(format!(
                "timed out waiting for PTY marker after {timeout:?}"
            ));
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    if contains_subsequence(&output, marker) {
        Ok(output)
    } else {
        Err(format!(
            "PTY marker {:?} was not observed before timeout",
            String::from_utf8_lossy(marker)
        ))
    }
}

pub fn read_available_for(master: &mut fs::File, duration: Duration) -> Vec<u8> {
    let started = Instant::now();
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];

    while started.elapsed() < duration {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => output.extend_from_slice(&buffer[..bytes_read]),
            Err(error)
                if error.kind() == io::ErrorKind::Interrupted
                    || error.raw_os_error() == Some(libc::EIO) =>
            {
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }

    output
}

#[derive(Debug)]
pub struct CappedOutput {
    cap_bytes: usize,
    total_bytes: usize,
    stored: Vec<u8>,
}

impl CappedOutput {
    fn new(cap_bytes: usize) -> Self {
        Self {
            cap_bytes,
            total_bytes: 0,
            stored: Vec::with_capacity(cap_bytes.min(4096)),
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        self.total_bytes += bytes.len();

        if self.stored.len() >= self.cap_bytes {
            return;
        }

        let remaining = self.cap_bytes - self.stored.len();
        self.stored
            .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
    }

    pub fn cap_bytes(&self) -> usize {
        self.cap_bytes
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn stored_bytes(&self) -> usize {
        self.stored.len()
    }

    pub fn dropped_bytes(&self) -> usize {
        self.total_bytes.saturating_sub(self.stored.len())
    }

    pub fn was_truncated(&self) -> bool {
        self.dropped_bytes() > 0
    }

    pub fn rendered_with_marker(&self) -> String {
        let mut rendered = sanitize_pty_output(&self.stored);
        if self.was_truncated() {
            rendered.push_str(&format!(
                "\n<TRUNCATED {} BYTES AFTER {} BYTE CAP>\n",
                self.dropped_bytes(),
                self.cap_bytes
            ));
        }
        rendered
    }
}

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

pub fn close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

fn set_nonblocking(fd: RawFd, context: &str) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(format!("{context}: {}", io::Error::last_os_error()));
    }

    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result == -1 {
        Err(format!("{context}: {}", io::Error::last_os_error()))
    } else {
        Ok(())
    }
}

pub fn sanitize_pty_output(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let mut sanitized = String::new();

    for character in text.chars() {
        match character {
            '\n' => sanitized.push('\n'),
            '\r' => sanitized.push_str("<CR>\n"),
            '\t' => sanitized.push('\t'),
            '\u{1b}' => sanitized.push_str("<ESC>"),
            control if control.is_control() => {
                sanitized.push_str("<CTRL>");
            }
            printable => sanitized.push(printable),
        }
    }

    sanitized
}
