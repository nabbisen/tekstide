//! RFC-026 D10, review 456 decision (a): one [`ProjectWatcher`] per open project. It owns
//! the platform backend, the watch scope, the batcher and the event receiver together, so
//! closing the project drops all of them and every watch with them (R6 by ownership, not by
//! a cleanup step someone could forget). A refusal stays inside the project that caused it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::backend::{NotifyBackend, WatchBackend, WatchRefusal};
use super::directory::{WatchAdmissionError, WatchedDirectory};
use super::scope::{ScopeChange, WatchScope, WatchState};
use super::{ScanBatcher, ScanRequest};

/// One thing the platform told us, already reduced to what the watcher needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchNotice {
    /// Paths that changed. Each is recorded against its parent directory, if that
    /// directory is watched.
    Changed(Vec<PathBuf>),
    /// The platform reported an error, so events may have been lost. Every watched
    /// directory is treated as changed, which is a full rescan of what we watch.
    Failed,
}

/// The platform's event receiver for one project. Not `Clone`, and it is only ever handed
/// out once, by [`WatchEventsSlot::take`], so exactly one thread can be waiting on it. A
/// second waiter could never be woken to no effect, so the type forbids one (review 457).
///
/// The guard against a future `#[derive(Clone)]` is a `compile_fail` doctest. It passes only
/// because `Clone` is missing, so it is paired with a companion that compiles over the same
/// path: a rename or a removed module breaks the pair instead of satisfying the first.
///
/// ```compile_fail
/// fn assert_clone<T: Clone>() {}
/// assert_clone::<tekstide_core::project::WatchEvents>();
/// ```
///
/// ```
/// fn assert_send<T: Send>() {}
/// assert_send::<tekstide_core::project::WatchEvents>();
/// ```
pub struct WatchEvents {
    receiver: Receiver<notify::Result<notify::Event>>,
}

impl WatchEvents {
    /// Blocks until the platform reports one thing, and returns it. `None` once the owning
    /// [`ProjectWatcher`] has been dropped, which is when the project closed, so the caller
    /// stops waiting for good.
    pub fn wait_for_notice(&mut self) -> Option<WatchNotice> {
        match self.receiver.recv().ok()? {
            Ok(event) => Some(WatchNotice::Changed(event.paths)),
            Err(_) => Some(WatchNotice::Failed),
        }
    }
}

/// Shared access to a project's one [`WatchEvents`]. Cloning shares the slot, not the
/// receiver: whichever clone calls [`Self::take`] first gets it, and every later call gets
/// `None` without blocking. A subscription rebuilt with the same identity therefore cannot
/// start a second waiter.
#[derive(Clone)]
pub struct WatchEventsSlot {
    slot: Arc<Mutex<Option<WatchEvents>>>,
}

impl WatchEventsSlot {
    /// The receiver, if nobody has taken it yet. Never blocks on a waiter: the lock is held
    /// only for the swap.
    pub fn take(&self) -> Option<WatchEvents> {
        self.slot.lock().ok()?.take()
    }
}

/// The watch owner for one open project (review 456, decision (a)).
pub struct ProjectWatcher {
    scope: WatchScope,
    platform: Result<NotifyBackend, WatchRefusal>,
    events: Option<WatchEventsSlot>,
    admission_refusals: Vec<(PathBuf, WatchAdmissionError)>,
    batcher: ScanBatcher,
}

impl ProjectWatcher {
    /// Starts the platform for this project. If it refuses to start, the watcher still
    /// exists and reconciles into [`WatchState::Stopped`] on its first call, so the
    /// refusal reaches the screen the same way every other refusal does.
    pub fn open() -> Self {
        let (sender, receiver) = channel();
        match NotifyBackend::new(sender) {
            Ok(backend) => Self {
                scope: WatchScope::new(),
                platform: Ok(backend),
                events: Some(WatchEventsSlot {
                    slot: Arc::new(Mutex::new(Some(WatchEvents { receiver }))),
                }),
                admission_refusals: Vec::new(),
                batcher: ScanBatcher::new(),
            },
            Err(refusal) => Self::unavailable(refusal),
        }
    }

    /// A watcher whose platform never started. Every reconcile stops it with `refusal`, the
    /// same stop a runtime refusal gives, so nothing downstream needs a second path. Also
    /// how a test reaches the stopped state without the kernel refusing anything.
    pub fn unavailable(refusal: WatchRefusal) -> Self {
        Self {
            scope: WatchScope::new(),
            platform: Err(refusal),
            events: None,
            admission_refusals: Vec::new(),
            batcher: ScanBatcher::new(),
        }
    }

    /// Makes the watches equal `desired`, and keeps `refused` (the paths the access policy
    /// refused, with their reasons) on the owner: nothing reads them yet, but a log would
    /// read them here, so they are not thrown away at the call site. Call this on a trigger
    /// (a toggle, a scan finishing, a document opening or closing, a project opening), never
    /// on every frame: it asks the filesystem whether each desired directory still exists,
    /// so a hundred expanded folders is a hundred stat calls per call.
    pub fn reconcile(
        &mut self,
        desired: &BTreeSet<WatchedDirectory>,
        refused: Vec<(PathBuf, WatchAdmissionError)>,
    ) -> ScopeChange {
        self.admission_refusals = refused;
        match &mut self.platform {
            Ok(backend) => self.scope.reconcile(desired, backend),
            Err(refusal) => self
                .scope
                .reconcile(desired, &mut Unavailable(refusal.clone())),
        }
    }

    /// The feed into the batcher (D3). Each changed path is recorded against its parent
    /// directory, **only if that directory is one we watch**: a non-recursive watch reports
    /// changes in its own directory, so the parent of a real event is always watched, and
    /// anything else (a path with no parent, or a directory we do not watch) is dropped
    /// rather than recorded. A directory this feed never names as empty cannot reach the
    /// batcher.
    pub fn record_changed_paths(&mut self, paths: &[PathBuf], at: Instant) {
        for path in paths {
            let Some(parent) = path.parent() else {
                continue;
            };
            if self.scope.is_watched(parent) {
                self.batcher.record(parent.to_path_buf(), at);
            }
        }
    }

    /// The fallback for [`WatchNotice::Failed`]: every watched directory is recorded as
    /// changed, so each is rescanned rather than trusted to have been quiet.
    pub fn record_all_watched(&mut self, at: Instant) {
        let watched: Vec<PathBuf> = self.scope.watched_paths().map(Path::to_path_buf).collect();
        for directory in watched {
            self.batcher.record(directory, at);
        }
    }

    /// The scan requests whose window has closed by `now`.
    pub fn drain_due(&mut self, now: Instant) -> Vec<ScanRequest> {
        self.batcher.drain_due(now)
    }

    /// Whether any directory is waiting on a window to close. The app polls for drains only
    /// while this is true.
    pub fn has_pending_scans(&self) -> bool {
        self.batcher.pending_directories() > 0
    }

    pub fn state(&self) -> &WatchState {
        self.scope.state()
    }

    pub fn scope(&self) -> &WatchScope {
        &self.scope
    }

    /// The paths the access policy refused at the last reconcile, with their reasons.
    pub fn admission_refusals(&self) -> &[(PathBuf, WatchAdmissionError)] {
        &self.admission_refusals
    }

    /// The slot holding this project's event receiver, or `None` if the platform never
    /// started.
    pub fn events(&self) -> Option<WatchEventsSlot> {
        self.events.clone()
    }
}

/// Stands in for a platform that never started: every watch is refused with the reason
/// it failed to start, so the scope records the same stop a runtime refusal would.
struct Unavailable(WatchRefusal);

impl WatchBackend for Unavailable {
    fn watch_directory(&mut self, _directory: &Path) -> Result<(), WatchRefusal> {
        Err(self.0.clone())
    }

    fn unwatch_directory(&mut self, _directory: &Path) {}

    fn directory_exists(&self, directory: &Path) -> bool {
        directory.is_dir()
    }
}
