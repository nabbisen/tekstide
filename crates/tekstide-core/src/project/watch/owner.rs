//! RFC-026 D10, review 456 decision (a): one [`ProjectWatcher`] per open project. It owns
//! the platform backend, the watch scope and the event receiver together, so closing the
//! project drops all three and every watch with them (R6 by ownership, not by a cleanup
//! step someone could forget). A refusal stays inside the project that caused it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use super::backend::{NotifyBackend, WatchBackend, WatchRefusal};
use super::directory::{WatchAdmissionError, WatchedDirectory};
use super::scope::{ScopeChange, WatchScope, WatchState};

/// The platform's event receiver for one project. Not `Clone`, and it is only ever handed
/// out once, by [`WatchEventsSlot::take`], so exactly one thread can be waiting on it. A
/// second waiter could never be woken to no effect, so the type forbids one (review 457).
pub struct WatchEvents {
    receiver: Receiver<notify::Result<notify::Event>>,
}

impl WatchEvents {
    /// Blocks until the platform reports one event, then returns `true`. A failed event
    /// counts as one too: the watcher is alive, and deciding what a failure means is the
    /// caller's job. Returns `false` once the owning [`ProjectWatcher`] has been dropped,
    /// which is when the project closed, so the caller stops waiting for good.
    pub fn wait_for_event(&mut self) -> bool {
        self.receiver.recv().is_ok()
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
            },
            Err(refusal) => Self {
                scope: WatchScope::new(),
                platform: Err(refusal),
                events: None,
                admission_refusals: Vec::new(),
            },
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

    /// Test-only: an owner whose platform never started, so the refusal path is reachable
    /// without the kernel refusing anything.
    #[cfg(test)]
    pub(crate) fn refused_for_test(refusal: WatchRefusal) -> Self {
        Self {
            scope: WatchScope::new(),
            platform: Err(refusal),
            events: None,
            admission_refusals: Vec::new(),
        }
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
