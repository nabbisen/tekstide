use std::fmt;
use std::fs;
use std::path::PathBuf;

use super::{
    FileAccessBlockedReason, FileAccessError, FileAccessSymlinkStatus, FileAccessTarget,
    ProjectFileAccessPolicy, ProjectRootHandle,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileExplorerScanPolicy {
    pub max_children_per_directory: usize,
    pub collapsed_directory_names: Vec<String>,
}

impl FileExplorerScanPolicy {
    /// Change-detection-wiring handoff, D1: `collapsed_directory_names`
    /// used to be this policy's own hardcoded copy of
    /// `[".git", "node_modules", "target"]` -- now built from
    /// `super::super::IGNORED_DIRECTORY_NAMES`, the one list change
    /// detection's own policy also builds from, so the two cannot
    /// independently drift into disagreement.
    pub fn linux_mvp() -> Self {
        Self {
            max_children_per_directory: 256,
            collapsed_directory_names: super::super::IGNORED_DIRECTORY_NAMES
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        }
    }

    fn should_collapse(&self, name: &str) -> bool {
        self.collapsed_directory_names
            .iter()
            .any(|collapsed| collapsed == name)
    }
}

impl Default for FileExplorerScanPolicy {
    fn default() -> Self {
        Self::linux_mvp()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerNodeKind {
    File,
    Directory,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExplorerNodeState {
    Available,
    Collapsed,
    Blocked(FileAccessBlockedReason),
    Unreadable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerNode {
    pub name: String,
    pub relative_path: PathBuf,
    pub kind: ExplorerNodeKind,
    pub state: ExplorerNodeState,
    pub symlink_status: FileAccessSymlinkStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerDirectoryScan {
    pub directory: FileAccessTarget,
    pub nodes: Vec<ExplorerNode>,
    /// True when the scanner stopped at `max_children_per_directory`.
    ///
    /// Returned nodes are sorted for presentation, but a truncated scan is a
    /// filesystem-order subset. It must not be presented as the complete
    /// alphabetically-first contents of the directory.
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExplorerScanError {
    Access(FileAccessError),
    NotDirectory { target: Box<FileAccessTarget> },
    CannotReadDirectory { target: Box<FileAccessTarget> },
}

impl fmt::Display for ExplorerScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => write!(formatter, "file access blocked: {error}"),
            Self::NotDirectory { target } => write!(
                formatter,
                "not an explorer directory: {}",
                target.selected_relative_path.display()
            ),
            Self::CannotReadDirectory { target } => write!(
                formatter,
                "could not read directory: {}",
                target.selected_relative_path.display()
            ),
        }
    }
}

impl std::error::Error for ExplorerScanError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct FileExplorerScanner;

impl FileExplorerScanner {
    pub fn scan_directory(
        self,
        root: &ProjectRootHandle,
        selected_relative_path: impl Into<PathBuf>,
        policy: &FileExplorerScanPolicy,
    ) -> Result<ExplorerDirectoryScan, ExplorerScanError> {
        let selected_relative_path = selected_relative_path.into();
        let directory = ProjectFileAccessPolicy
            .resolve_existing(root, &selected_relative_path)
            .map_err(ExplorerScanError::Access)?;

        if !directory.canonical_path.is_dir() {
            return Err(ExplorerScanError::NotDirectory {
                target: Box::new(directory),
            });
        }

        let read_dir = fs::read_dir(&directory.canonical_path).map_err(|_| {
            ExplorerScanError::CannotReadDirectory {
                target: Box::new(directory.clone()),
            }
        })?;

        let base_relative_path = directory.selected_relative_path.clone();
        let mut nodes = Vec::new();
        let mut truncated = false;

        for entry_result in read_dir {
            if nodes.len() >= policy.max_children_per_directory {
                truncated = true;
                break;
            }

            let entry = match entry_result {
                Ok(entry) => entry,
                Err(_) => {
                    nodes.push(unreadable_node(
                        "<unreadable>",
                        selected_relative_path.clone(),
                    ));
                    continue;
                }
            };

            let raw_name = entry.file_name();
            let name = raw_name.to_string_lossy().into_owned();
            let relative_path = base_relative_path.join(&raw_name);

            nodes.push(node_for_entry(root, policy, name, relative_path, entry));
        }

        nodes.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(ExplorerDirectoryScan {
            directory,
            nodes,
            truncated,
        })
    }
}

fn node_for_entry(
    root: &ProjectRootHandle,
    policy: &FileExplorerScanPolicy,
    name: String,
    relative_path: PathBuf,
    entry: fs::DirEntry,
) -> ExplorerNode {
    let file_type = match entry.file_type() {
        Ok(file_type) => file_type,
        Err(_) => return unreadable_node(name, relative_path),
    };

    let entry_is_symlink = file_type.is_symlink();
    let kind = if file_type.is_dir() {
        ExplorerNodeKind::Directory
    } else if file_type.is_file() {
        ExplorerNodeKind::File
    } else {
        ExplorerNodeKind::Other
    };

    match ProjectFileAccessPolicy.resolve_existing(root, &relative_path) {
        Ok(target) => {
            let kind = if target.canonical_path.is_dir() {
                ExplorerNodeKind::Directory
            } else if target.canonical_path.is_file() {
                ExplorerNodeKind::File
            } else {
                kind
            };
            let state = if kind == ExplorerNodeKind::Directory && policy.should_collapse(&name) {
                ExplorerNodeState::Collapsed
            } else {
                ExplorerNodeState::Available
            };

            ExplorerNode {
                name,
                relative_path,
                kind,
                state,
                symlink_status: target.symlink_status,
            }
        }
        Err(error) => ExplorerNode {
            name,
            relative_path,
            kind,
            state: ExplorerNodeState::Blocked(error.reason),
            symlink_status: blocked_symlink_status(entry_is_symlink, error.reason),
        },
    }
}

fn blocked_symlink_status(
    entry_is_symlink: bool,
    reason: FileAccessBlockedReason,
) -> FileAccessSymlinkStatus {
    if reason == FileAccessBlockedReason::SymlinkEscape {
        FileAccessSymlinkStatus::EscapesRoot
    } else if entry_is_symlink {
        FileAccessSymlinkStatus::UnresolvedSymlink
    } else {
        FileAccessSymlinkStatus::NoSymlink
    }
}

fn unreadable_node(name: impl Into<String>, relative_path: PathBuf) -> ExplorerNode {
    ExplorerNode {
        name: name.into(),
        relative_path,
        kind: ExplorerNodeKind::Other,
        state: ExplorerNodeState::Unreadable,
        symlink_status: FileAccessSymlinkStatus::NoSymlink,
    }
}

/// RFC-038 PR-038-G: one directory, for the folder browser that chooses
/// a **new** project's root -- not [`FileExplorerScanner`]'s own
/// `ExplorerDirectoryScan`, deliberately. That type's `directory:
/// FileAccessTarget` carries a `project_id` and a containment/symlink
/// policy relative to an already-open project's fixed root; browsing to
/// *find* a root has no root yet to be contained within or escape from
/// -- there is nothing to enforce. Whatever the user ultimately picks
/// is re-validated independently, in full, by `add_project_from_path`'s
/// own `ProjectRootValidator` (`SymlinkPolicy::FailClosed`) the moment
/// they commit to it -- this type only has to get them there, the same
/// way an ordinary OS file-open dialog follows symlinks freely because
/// the program opening the chosen file validates it afterwards anyway.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowseNode {
    pub name: String,
    /// Absolute, not relative -- there is no fixed root here for a
    /// relative path to be relative *to*, unlike `ExplorerNode::
    /// relative_path`.
    pub path: PathBuf,
    pub state: BrowseNodeState,
}

/// Deliberately narrower than [`ExplorerNodeState`]: no `Blocked`
/// variant exists here because nothing in [`browse_directory`] ever
/// enforces the containment policy that produces one -- an
/// unrepresentable state is safer than one this function could only
/// ever leave unconstructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowseNodeState {
    Available,
    Collapsed,
    Unreadable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryBrowseScan {
    pub current_dir: PathBuf,
    /// `None` only at the filesystem root -- a browser may navigate
    /// anywhere upward, unlike the project explorer's own "parent
    /// row only until the project root" rule.
    pub parent_dir: Option<PathBuf>,
    /// Directories only. A folder browser is choosing a project root,
    /// not a file -- there is nothing for a file entry to do here, so
    /// none are collected in the first place rather than being
    /// collected and then never rendered.
    pub nodes: Vec<BrowseNode>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectoryBrowseError {
    NotDirectory { path: PathBuf },
    CannotReadDirectory { path: PathBuf },
}

impl fmt::Display for DirectoryBrowseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDirectory { path } => {
                write!(formatter, "not a directory: {}", path.display())
            }
            Self::CannotReadDirectory { path } => {
                write!(formatter, "could not read directory: {}", path.display())
            }
        }
    }
}

impl std::error::Error for DirectoryBrowseError {}

/// RFC-038 PR-038-G: scans an arbitrary, real filesystem directory for
/// the folder browser -- project-independent, unlike
/// [`FileExplorerScanner::scan_directory`] (see [`BrowseNode`]'s own
/// doc for why the two are genuinely different properties, not one
/// reimplemented as two). Still bounded by the same [`FileExplorerScanPolicy`]
/// (`max_children_per_directory`, `collapsed_directory_names`) the
/// project explorer uses, so this is one bounding policy shared by both
/// scanners, not a second one invented here.
///
/// `path` is canonicalised (symlinks resolved, following them --
/// deliberately, since there is no root for a symlink to "escape"; see
/// [`BrowseNode`]) before anything else, so `current_dir`/`parent_dir`
/// are always real, existing, canonical paths.
pub fn browse_directory(
    path: impl AsRef<std::path::Path>,
    policy: &FileExplorerScanPolicy,
) -> Result<DirectoryBrowseScan, DirectoryBrowseError> {
    let canonical =
        fs::canonicalize(path.as_ref()).map_err(|_| DirectoryBrowseError::CannotReadDirectory {
            path: path.as_ref().to_path_buf(),
        })?;
    if !canonical.is_dir() {
        return Err(DirectoryBrowseError::NotDirectory { path: canonical });
    }
    let read_dir =
        fs::read_dir(&canonical).map_err(|_| DirectoryBrowseError::CannotReadDirectory {
            path: canonical.clone(),
        })?;

    let mut nodes = Vec::new();
    let mut truncated = false;
    for entry_result in read_dir {
        if nodes.len() >= policy.max_children_per_directory {
            truncated = true;
            break;
        }
        let Ok(entry) = entry_result else {
            nodes.push(BrowseNode {
                name: "<unreadable>".to_owned(),
                path: canonical.clone(),
                state: BrowseNodeState::Unreadable,
            });
            continue;
        };
        let raw_name = entry.file_name();
        let name = raw_name.to_string_lossy().into_owned();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let entry_path = canonical.join(&raw_name);
        let state = if policy.should_collapse(&name) {
            BrowseNodeState::Collapsed
        } else if fs::read_dir(&entry_path).is_err() {
            BrowseNodeState::Unreadable
        } else {
            BrowseNodeState::Available
        };
        nodes.push(BrowseNode {
            name,
            path: entry_path,
            state,
        });
    }
    nodes.sort_by(|left, right| left.name.cmp(&right.name));

    let parent_dir = canonical.parent().map(std::path::Path::to_path_buf);

    Ok(DirectoryBrowseScan {
        current_dir: canonical,
        parent_dir,
        nodes,
        truncated,
    })
}

#[cfg(test)]
mod tests;
