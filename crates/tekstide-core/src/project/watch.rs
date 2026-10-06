//! RFC-026 PR-026-A: the batching, proved before any watcher exists (D11).
//!
//! A scan is a git subprocess (RFC-055; RFC-026 measurement 8), so an event storm --
//! a build writing a thousand files into one directory -- must cost one scan per
//! window, not a thousand. This is the part of the watcher Tekstide writes itself;
//! the dependency that feeds it (D4) is judged afterwards against the numbers this
//! module produces.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The batching window (D3). Stated, not tuned: at most one scan request per
/// directory per window, where a window opens at the first change in a directory
/// that has none open. A burst longer than the window yields one request per
/// window rather than starving until it ends.
pub const SCAN_WINDOW: Duration = Duration::from_millis(250);

/// Git subprocesses one scan request costs: the gate's configuration query, and one
/// `check-ignore` (RFC-026 measurement 8, measured at review 431).
pub const GIT_SUBPROCESSES_PER_SCAN: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanRequest {
    pub directory: PathBuf,
    /// The changes this one scan answers.
    pub coalesced_events: u32,
}

#[derive(Clone, Debug)]
struct PendingScan {
    due: Instant,
    coalesced_events: u32,
}

#[derive(Debug, Default)]
pub struct ScanBatcher {
    pending: BTreeMap<PathBuf, PendingScan>,
}

impl ScanBatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a change at `path`. Changes in a directory whose window is already
    /// open collapse into it; a directory with no open window gets one, closing
    /// [`SCAN_WINDOW`] after this change. Different directories never share a window.
    pub fn record(&mut self, path: &Path, at: Instant) {
        let directory = path.parent().unwrap_or(Path::new("")).to_path_buf();
        match self.pending.get_mut(&directory) {
            Some(pending) => pending.coalesced_events += 1,
            None => {
                self.pending.insert(
                    directory,
                    PendingScan {
                        due: at + SCAN_WINDOW,
                        coalesced_events: 1,
                    },
                );
            }
        }
    }

    /// Every scan whose window has closed by `now`, one per directory, in path order.
    pub fn drain_due(&mut self, now: Instant) -> Vec<ScanRequest> {
        let due: Vec<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.due <= now)
            .map(|(directory, _)| directory.clone())
            .collect();
        due.into_iter()
            .filter_map(|directory| {
                self.pending.remove(&directory).map(|pending| ScanRequest {
                    directory,
                    coalesced_events: pending.coalesced_events,
                })
            })
            .collect()
    }

    /// Directories with a window currently open.
    pub fn pending_directories(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests;
