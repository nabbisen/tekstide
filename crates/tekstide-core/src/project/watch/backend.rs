//! RFC-026 D4: the seam between watch policy and the platform's watch API. Every
//! platform call goes through [`WatchBackend`]; the production implementation is
//! [`NotifyBackend`], and the policy's tests use a fake that refuses on demand (H2 --
//! the real kernel's limit cannot be forced on a development machine).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::mpsc::Sender;

use notify::{RecursiveMode, Watcher};

/// Why a watch could not be placed. One shape for every cause (review 452, C1): the
/// policy never branches on it, so a refusal is a refusal whatever the platform said.
/// `detail` is for logs and tests; it is never product text (C3).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchRefusal {
    detail: String,
}

impl WatchRefusal {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Places and removes non-recursive watches on single directories.
pub trait WatchBackend {
    fn watch_directory(&mut self, directory: &Path) -> Result<(), WatchRefusal>;
    fn unwatch_directory(&mut self, directory: &Path);
}

/// The production backend: `notify`'s recommended watcher, non-recursive only.
pub struct NotifyBackend {
    watcher: notify::RecommendedWatcher,
}

impl NotifyBackend {
    /// Events are forwarded into `events` and nothing else. The handler runs on
    /// notify's own thread and has no panic path: a send error is dropped, because
    /// a dead receiver means the project is closed and there is nothing to tell.
    pub fn new(events: Sender<notify::Result<notify::Event>>) -> Result<Self, WatchRefusal> {
        let watcher = notify::recommended_watcher(move |event| {
            let _ = events.send(event);
        })
        .map_err(|error| WatchRefusal::new(error.to_string()))?;
        Ok(Self { watcher })
    }
}

impl WatchBackend for NotifyBackend {
    fn watch_directory(&mut self, directory: &Path) -> Result<(), WatchRefusal> {
        guard_watch_call(|| self.watcher.watch(directory, RecursiveMode::NonRecursive))
    }

    fn unwatch_directory(&mut self, directory: &Path) {
        let _ = guard_watch_call(|| self.watcher.unwatch(directory));
    }
}

/// Runs one watch call and turns every non-success into a [`WatchRefusal`], including
/// a panic. notify's event thread has no `catch_unwind`, so after a handler panic or a
/// `poll(2)` failure every later call panics in our thread at notify's own `unwrap()`
/// (RFC-026 H1). Catching it here makes that the same degradation as any other failure.
pub(crate) fn guard_watch_call<T>(
    call: impl FnOnce() -> notify::Result<T>,
) -> Result<T, WatchRefusal> {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(WatchRefusal::new(error.to_string())),
        Err(_) => Err(WatchRefusal::new("the watcher's thread panicked")),
    }
}
