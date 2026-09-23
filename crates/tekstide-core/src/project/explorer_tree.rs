//! RFC-052 PR-052-B: the explorer as a real tree.
//!
//! **Pure model, no `iced`, no filesystem walking of its own.** It holds the
//! directories that have been scanned, which of them are expanded, and
//! which scans are in flight, and flattens them into the rows the sidebar
//! draws. Every scan is a [`ExplorerScanRequest`] that runs *somewhere
//! else* and comes back as an [`ExplorerScanCompleted`]; nothing here scans
//! on the caller's thread (D3′: a scan is linear in path length -- 65 ms at
//! depth 1 500 -- so it must not run on the render thread).
//!
//! What it inherits, unchanged, from the one-level explorer it replaces:
//!
//! * [`FileExplorerScanner`] is still the only thing that reads a
//!   directory, so the project-root boundary, the escaping-symlink report,
//!   the 256-child cap (now **per level**) and the collapse list all hold
//!   because the same code enforces them. The tree adds no path handling.
//! * Names stay in [`ExplorerNode`], raw. Escaping happens where a name is
//!   drawn (`surface::explorer`), never here -- storing an escaped name
//!   would corrupt the path a row opens.
//!
//! **Nothing is hidden silently.** A directory the cap cut short gets an
//! [`ExplorerTreeRowKind::Omitted`] row naming how many entries were left
//! out; a tree larger than [`MAX_TREE_ROWS`] ends in an
//! [`ExplorerTreeRowKind::RowsNotShown`] row naming how many rows were.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::root::{
    ExplorerDirectoryScan, ExplorerNode, ExplorerNodeKind, ExplorerNodeState, ExplorerScanError,
    FileExplorerScanPolicy, FileExplorerScanner, ProjectRootHandle,
};

/// The most rows the flattened tree will hold. The sidebar draws only a
/// window of them (`surface::explorer`), so this bounds the model, not the
/// frame; it exists so a user who opens hundreds of full directories does
/// not make every render walk hundreds of thousands of entries.
///
/// Measured for RFC-052 PR-052-B: flattening 10 000 rows is well under a
/// millisecond (`explorer_tree::tests::flattening_the_row_bound_is_cheap`).
pub const MAX_TREE_ROWS: usize = 10_000;

/// One directory scan, ready to run anywhere. `Send + 'static`, so it can
/// be moved onto a worker thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerScanRequest {
    root: ProjectRootHandle,
    path: PathBuf,
    generation: u64,
    policy: FileExplorerScanPolicy,
}

impl ExplorerScanRequest {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Reads the directory. **Blocking**: call it off the render thread.
    pub fn run(self) -> ExplorerScanCompleted {
        let result = FileExplorerScanner.scan_directory(&self.root, &self.path, &self.policy);
        ExplorerScanCompleted {
            path: self.path,
            generation: self.generation,
            result,
        }
    }
}

/// What a finished scan sends back. `generation` lets the tree drop a
/// result that a newer request for the same directory has superseded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerScanCompleted {
    pub path: PathBuf,
    pub generation: u64,
    pub result: Result<ExplorerDirectoryScan, ExplorerScanError>,
}

/// What toggling a directory row did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerToggle {
    Collapsed,
    /// Expanded, and a fresh scan is now pending.
    Expanded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerRootState {
    NotRequested,
    Pending,
    Loaded,
    Failed,
}

#[derive(Clone, Copy, Debug)]
pub enum ExplorerTreeRowKind<'a> {
    Node {
        node: &'a ExplorerNode,
        expanded: bool,
        /// Whether Enter on this row toggles it. A file, a blocked or
        /// unreadable entry, and anything that is not a directory are not
        /// expandable; a directory on the collapse list **is** (it is
        /// labelled, and bounded by the same per-level cap).
        expandable: bool,
    },
    /// An expanded directory whose scan has not come back yet.
    Loading,
    /// An expanded directory that could not be read. The row carries no
    /// message: the error text embeds the path, which is attacker-chosen.
    CannotRead,
    /// An expanded directory with nothing in it.
    Empty,
    /// The per-level cap cut this directory short.
    Omitted { count: usize, at_least: bool },
    /// The whole tree passed [`MAX_TREE_ROWS`].
    RowsNotShown { count: usize },
}

#[derive(Clone, Copy, Debug)]
pub struct ExplorerTreeRow<'a> {
    pub depth: usize,
    pub kind: ExplorerTreeRowKind<'a>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExplorerTree {
    loaded: BTreeMap<PathBuf, ExplorerDirectoryScan>,
    failed: BTreeSet<PathBuf>,
    expanded: BTreeSet<PathBuf>,
    pending: BTreeMap<PathBuf, u64>,
    next_generation: u64,
}

impl ExplorerTree {
    pub fn root_scan(&self) -> Option<&ExplorerDirectoryScan> {
        self.loaded.get(Path::new(""))
    }

    pub fn root_state(&self) -> ExplorerRootState {
        let root = Path::new("");
        if self.loaded.contains_key(root) {
            ExplorerRootState::Loaded
        } else if self.pending.contains_key(root) {
            ExplorerRootState::Pending
        } else if self.failed.contains(root) {
            ExplorerRootState::Failed
        } else {
            ExplorerRootState::NotRequested
        }
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    /// Directories whose scan is in flight, with the generation to hand
    /// back. Stable order, so a caller building one worker per entry builds
    /// them in the same order every time.
    pub fn pending(&self) -> impl Iterator<Item = (&Path, u64)> {
        self.pending
            .iter()
            .map(|(path, generation)| (path.as_path(), *generation))
    }

    /// Marks `path` as needing a scan. A no-op if one is already in
    /// flight: a second request would only produce a result the first
    /// makes stale.
    pub fn request_scan(&mut self, path: &Path) {
        if self.pending.contains_key(path) {
            return;
        }
        self.next_generation += 1;
        self.pending
            .insert(path.to_path_buf(), self.next_generation);
        self.failed.remove(path);
    }

    /// Builds the runnable requests for every pending scan.
    pub fn scan_requests(&self, root: &ProjectRootHandle) -> Vec<ExplorerScanRequest> {
        self.pending()
            .map(|(path, generation)| ExplorerScanRequest {
                root: root.clone(),
                path: path.to_path_buf(),
                generation,
                policy: FileExplorerScanPolicy::linux_mvp(),
            })
            .collect()
    }

    /// Records the outcome of a scan that ran **synchronously on the
    /// caller's thread**, superseding anything pending for that directory.
    /// For tests and headless callers only: the GUI goes through
    /// [`ExplorerTree::request_scan`] and [`ExplorerTree::apply`], and the
    /// shell has a test that it never scans on the render thread.
    pub fn set_scan(
        &mut self,
        path: &Path,
        result: Result<ExplorerDirectoryScan, ExplorerScanError>,
    ) {
        self.pending.remove(path);
        match result {
            Ok(scan) => {
                self.failed.remove(path);
                self.loaded.insert(path.to_path_buf(), scan);
            }
            Err(_) => {
                self.loaded.remove(path);
                self.failed.insert(path.to_path_buf());
            }
        }
    }

    /// Applies a finished scan. Returns `false`, changing nothing, if the
    /// result is stale (a newer request for that directory is pending, or
    /// none is: the directory was collapsed and forgotten).
    pub fn apply(&mut self, completed: ExplorerScanCompleted) -> bool {
        if self.pending.get(&completed.path) != Some(&completed.generation) {
            return false;
        }
        self.pending.remove(&completed.path);
        match completed.result {
            Ok(scan) => {
                self.failed.remove(&completed.path);
                self.loaded.insert(completed.path, scan);
            }
            Err(_) => {
                // A refresh that fails leaves nothing trustworthy to show.
                self.loaded.remove(&completed.path);
                self.failed.insert(completed.path);
            }
        }
        true
    }

    /// Expands a collapsed directory (and asks for a fresh scan of it) or
    /// collapses an expanded one. Cached rows for a collapsed directory are
    /// kept, so expanding it again shows them at once while the refresh
    /// runs.
    pub fn toggle(&mut self, path: &Path) -> ExplorerToggle {
        if self.expanded.remove(path) {
            ExplorerToggle::Collapsed
        } else {
            self.expanded.insert(path.to_path_buf());
            self.request_scan(path);
            ExplorerToggle::Expanded
        }
    }

    /// Every row the tree currently has, in display order, bounded by
    /// [`MAX_TREE_ROWS`].
    pub fn rows(&self) -> Vec<ExplorerTreeRow<'_>> {
        let mut flatten = Flatten {
            out: Vec::new(),
            skipped: 0,
        };
        self.push_directory(Path::new(""), 0, &mut flatten);
        if flatten.skipped > 0 {
            flatten.out.push(ExplorerTreeRow {
                depth: 0,
                kind: ExplorerTreeRowKind::RowsNotShown {
                    count: flatten.skipped,
                },
            });
        }
        flatten.out
    }

    fn push_directory<'a>(&'a self, dir: &Path, depth: usize, flatten: &mut Flatten<'a>) {
        if let Some(scan) = self.loaded.get(dir) {
            if scan.nodes.is_empty() && !scan.truncated {
                flatten.push(depth, ExplorerTreeRowKind::Empty);
            }
            for node in &scan.nodes {
                let expandable = is_expandable(node);
                let expanded = expandable && self.expanded.contains(&node.relative_path);
                flatten.push(
                    depth,
                    ExplorerTreeRowKind::Node {
                        node,
                        expanded,
                        expandable,
                    },
                );
                if expanded {
                    self.push_directory(&node.relative_path, depth + 1, flatten);
                }
            }
            if scan.truncated {
                flatten.push(
                    depth,
                    ExplorerTreeRowKind::Omitted {
                        count: scan.omitted_entries,
                        at_least: scan.omitted_is_lower_bound,
                    },
                );
            }
        } else if self.pending.contains_key(dir) {
            flatten.push(depth, ExplorerTreeRowKind::Loading);
        } else if self.failed.contains(dir) {
            flatten.push(depth, ExplorerTreeRowKind::CannotRead);
        }
    }
}

/// Whether Enter on `node` toggles it.
pub fn is_expandable(node: &ExplorerNode) -> bool {
    node.kind == ExplorerNodeKind::Directory
        && matches!(
            node.state,
            ExplorerNodeState::Available | ExplorerNodeState::Collapsed
        )
}

struct Flatten<'a> {
    out: Vec<ExplorerTreeRow<'a>>,
    skipped: usize,
}

impl<'a> Flatten<'a> {
    fn push(&mut self, depth: usize, kind: ExplorerTreeRowKind<'a>) {
        if self.out.len() < MAX_TREE_ROWS {
            self.out.push(ExplorerTreeRow { depth, kind });
        } else {
            self.skipped += 1;
        }
    }
}

#[cfg(test)]
mod tests;
