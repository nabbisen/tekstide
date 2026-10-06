//! RFC-026 D1, D2, R6: which directories are watched, and what happens when the
//! platform refuses one.
//!
//! The watched set is always reconciled *to* a desired set, so the scope can shrink
//! as well as grow -- a scope that only grows is risk R3 arriving on its own. Any
//! refusal stops watching outright: every watch is dropped and the state becomes
//! [`WatchState::Stopped`], which is the state the screen says in words (D2). The
//! policy never inspects the refusal (review 452, C1); its detail is kept for logs.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::backend::{WatchBackend, WatchRefusal};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchState {
    /// Watches are placed for the desired set.
    Live,
    /// A refusal stopped watching. Nothing is watched until [`WatchScope::resume`].
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScopeChange {
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug)]
pub struct WatchScope {
    watched: BTreeSet<PathBuf>,
    state: WatchState,
    last_refusal: Option<WatchRefusal>,
}

impl Default for WatchScope {
    fn default() -> Self {
        Self {
            watched: BTreeSet::new(),
            state: WatchState::Live,
            last_refusal: None,
        }
    }
}

impl WatchScope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &WatchState {
        &self.state
    }

    pub fn watched_count(&self) -> usize {
        self.watched.len()
    }

    pub fn is_watched(&self, directory: &Path) -> bool {
        self.watched.contains(directory)
    }

    /// The refusal that stopped watching, for logs. Never shown to a user (C3).
    pub fn last_refusal(&self) -> Option<&WatchRefusal> {
        self.last_refusal.as_ref()
    }

    /// Makes the watched set equal the desired set **that still exists**. A directory
    /// that has gone is dropped here, before the platform is asked about it (review 453):
    /// that branches on a fact this policy checks itself, never on a library's error.
    ///
    /// The one race left is named, not engineered away: a directory that exists when
    /// checked and is gone when watched is still a refusal, and that stops watching.
    /// Removals run first, so a shrinking scope frees platform budget before a growing
    /// one needs it. The first refusal stops watching and ends the call.
    pub fn reconcile(
        &mut self,
        desired: &BTreeSet<PathBuf>,
        backend: &mut dyn WatchBackend,
    ) -> ScopeChange {
        let mut change = ScopeChange::default();
        if self.state == WatchState::Stopped {
            return change;
        }
        let desired: BTreeSet<PathBuf> = desired
            .iter()
            .filter(|directory| backend.directory_exists(directory))
            .cloned()
            .collect();
        let desired = &desired;

        let stale: Vec<PathBuf> = self.watched.difference(desired).cloned().collect();
        for directory in stale {
            backend.unwatch_directory(&directory);
            self.watched.remove(&directory);
            change.removed += 1;
        }

        let fresh: Vec<PathBuf> = desired.difference(&self.watched).cloned().collect();
        for directory in fresh {
            match backend.watch_directory(&directory) {
                Ok(()) => {
                    self.watched.insert(directory);
                    change.added += 1;
                }
                Err(refusal) => {
                    self.stop(backend, refusal);
                    break;
                }
            }
        }
        change
    }

    /// Clears a stop, so the next [`Self::reconcile`] may place watches again. Called
    /// by a user action (a reopen), never automatically: a refusal that resumed on its
    /// own would retry the platform on every frame.
    pub fn resume(&mut self) {
        self.state = WatchState::Live;
        self.last_refusal = None;
    }

    fn stop(&mut self, backend: &mut dyn WatchBackend, refusal: WatchRefusal) {
        for directory in std::mem::take(&mut self.watched) {
            backend.unwatch_directory(&directory);
        }
        self.state = WatchState::Stopped;
        self.last_refusal = Some(refusal);
    }
}

/// The directories that should be watched for one open project: its root, the
/// expanded folders, and the folders of open documents (D1). All relative paths are
/// joined onto `root`; the root is always included while the project is open.
pub fn desired_directories(
    root: &Path,
    expanded: &[PathBuf],
    open_document_directories: &[PathBuf],
) -> BTreeSet<PathBuf> {
    let mut desired = BTreeSet::new();
    desired.insert(root.to_path_buf());
    for relative in expanded.iter().chain(open_document_directories) {
        desired.insert(root.join(relative));
    }
    desired
}
