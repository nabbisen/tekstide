//! RFC-026 D10, review 456 decision (a): one [`ProjectWatcher`] per open project. It owns
//! the platform backend, the watch scope and the event receiver together, so closing the
//! project drops all three and every watch with them (R6 by ownership, not by a cleanup
//! step someone could forget). A refusal stays inside the project that caused it.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use super::backend::{NotifyBackend, WatchBackend, WatchRefusal};
use super::directory::WatchedDirectory;
use super::scope::{ScopeChange, WatchScope, WatchState};

/// The platform's event stream for one project. Cloning shares the one receiver, so the
/// app can hand a clone to the thread that waits on it while the owner keeps its own.
#[derive(Clone)]
pub struct WatchEvents {
    receiver: Arc<Mutex<Receiver<notify::Result<notify::Event>>>>,
}

impl WatchEvents {
    fn new(receiver: Receiver<notify::Result<notify::Event>>) -> Self {
        Self {
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    /// Blocks until the platform reports one event, then returns `true`. A failed event
    /// counts as one too: the watcher is alive, and deciding what a failure means is the
    /// caller's job. Returns `false` once the owning [`ProjectWatcher`] has been dropped,
    /// which is when the project closed, so the caller stops waiting for good.
    pub fn wait_for_event(&self) -> bool {
        let Ok(receiver) = self.receiver.lock() else {
            return false;
        };
        receiver.recv().is_ok()
    }
}

/// The watch owner for one open project (review 456, decision (a)).
pub struct ProjectWatcher {
    scope: WatchScope,
    platform: Result<NotifyBackend, WatchRefusal>,
    events: Option<WatchEvents>,
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
                events: Some(WatchEvents::new(receiver)),
            },
            Err(refusal) => Self {
                scope: WatchScope::new(),
                platform: Err(refusal),
                events: None,
            },
        }
    }

    /// Makes the watches equal `desired`. Call this on a trigger (a toggle, a scan
    /// finishing, a document opening or closing, a project opening), never on every
    /// frame: it asks the filesystem whether each desired directory still exists, so a
    /// hundred expanded folders is a hundred stat calls per call.
    pub fn reconcile(&mut self, desired: &BTreeSet<WatchedDirectory>) -> ScopeChange {
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
        }
    }

    pub fn state(&self) -> &WatchState {
        self.scope.state()
    }

    pub fn scope(&self) -> &WatchScope {
        &self.scope
    }

    /// The event stream to subscribe to, or `None` if the platform never started.
    pub fn events(&self) -> Option<WatchEvents> {
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
