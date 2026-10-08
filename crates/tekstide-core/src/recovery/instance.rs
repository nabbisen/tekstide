use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

/// RFC-027 D12: markers live under `<state_root>/recovery/instances/`, one empty file per
/// running instance, named by its own pid. A sibling directory (`records/`, PR-027-B) will
/// hold the actual buffer content this slice does not write.
pub fn instances_dir(state_root: &Path) -> PathBuf {
    state_root.join("recovery").join("instances")
}

/// RFC-027 D1/D12: this instance's own marker -- written at startup, removed on clean
/// shutdown. Held for the whole application lifetime (a field on `shell::State`), so a
/// normal exit drops it as part of ordinary Rust teardown; [`Drop`] is the removal
/// mechanism itself, the same shape `test_support::KillOnDropChild` already uses for a
/// real child process, so a panic mid-session still cleans up rather than leaving a marker
/// a later startup's crash scan would misread as evidence of a crash that never happened.
///
/// **Measurement-mode exits are a known, narrow exception.** The two `std::process::exit`
/// calls in `shell.rs` (`Message::MeasurementTick`/`Message::MeasurementFrame`, reached
/// only under `TEKSTIDE_*` measurement env vars, never in ordinary use) skip every
/// destructor, this one included, and would leave a marker behind. Disclosed rather than
/// silently accepted: an ordinary user exiting the window never takes this path, since
/// closing the last window is a real `winit`/`iced` teardown with no `process::exit` in it
/// (read directly from `iced_winit` 0.14's own event loop before relying on it).
#[derive(Debug)]
pub struct InstanceMarker {
    path: PathBuf,
}

impl InstanceMarker {
    /// RFC-027 D12: named by [`std::process::id`], not a generated id -- the filename
    /// itself is the whole input the crash scan's liveness check needs.
    fn create(instances_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(instances_dir)?;
        let path = instances_dir.join(std::process::id().to_string());
        File::create(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for InstanceMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// RFC-027 D1/D12: a marker found at startup whose own pid is no longer alive -- the
/// previous session that wrote it did not exit cleanly. PR-027-A reports this internally
/// only; there is nothing to recover yet (no buffer content exists on disk), so there is
/// nothing user-facing here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectedCrash {
    pub pid: u32,
}

/// RFC-027 D1/D12: the result of starting a new instance -- this session's own marker,
/// and every crash the startup scan found before writing it.
#[derive(Debug)]
pub struct InstanceStartup {
    pub marker: InstanceMarker,
    pub detected_crashes: Vec<DetectedCrash>,
}

/// RFC-027 D1/D12: scans for stale markers *before* writing this instance's own, so a
/// freshly created marker is never mistaken for evidence against itself. Each stale marker
/// found is removed: a startup scan is the only consumer of the crash fact in this slice
/// (there is no recovery record yet to tie a longer lifecycle to), so there is nothing to
/// keep it for.
///
/// **Two concurrent instances do not make each other look crashed (D12).** A marker whose
/// own pid is still alive belongs to a sibling instance that is genuinely running right
/// now; it is left untouched, not removed and not reported.
///
/// **Disclosed, not fixed: pid reuse (D12).** A stale marker whose pid number has since
/// been reused by an unrelated live process reads as "still running" here, the same as a
/// real sibling instance would -- neither detected nor removed. That is the direction this
/// must fail in: not offering recovery costs the user a prompt; falsely offering stale text
/// as theirs is the failure this RFC exists to prevent. No cleverer check is built for it.
pub fn start_instance(state_root: &Path) -> io::Result<InstanceStartup> {
    let instances_dir = instances_dir(state_root);
    let detected_crashes = scan_and_clean_stale_markers(&instances_dir)?;
    let marker = InstanceMarker::create(&instances_dir)?;
    Ok(InstanceStartup {
        marker,
        detected_crashes,
    })
}

fn scan_and_clean_stale_markers(instances_dir: &Path) -> io::Result<Vec<DetectedCrash>> {
    let entries = match fs::read_dir(instances_dir) {
        Ok(entries) => entries,
        // A first run: nothing has ever written here, so there is nothing stale.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut detected = Vec::new();
    for entry in entries {
        let entry = entry?;
        // Not a filename this code ever wrote -- ignore rather than delete something this
        // module does not recognise.
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid_is_alive(pid) {
            continue;
        }
        if fs::remove_file(entry.path()).is_ok() {
            detected.push(DetectedCrash { pid });
        }
    }
    Ok(detected)
}

/// RFC-027 D12: production's own version of
/// `test_support::process_is_alive`'s `libc::kill(pid, 0)` technique -- signal `0` sends
/// nothing; the kernel only validates that the target exists and is signalable, which is
/// exactly "is this pid still in the process table." `test_support` is `#[cfg(test)]`, so
/// production code could never reach it regardless; this is the real implementation D12
/// asks for, not a weakened gate around the test one.
fn pid_is_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_is_alive_is_true_for_this_very_process() {
        assert!(pid_is_alive(std::process::id()));
    }

    #[test]
    fn pid_is_alive_is_false_for_a_pid_that_does_not_exist() {
        // RFC-027: not a magic number -- `pid_t` is a 32-bit signed int on Linux, so this
        // is the largest value the type can hold, chosen because a real system is
        // vanishingly unlikely to have ever allocated it.
        assert!(!pid_is_alive(i32::MAX as u32));
    }
}
