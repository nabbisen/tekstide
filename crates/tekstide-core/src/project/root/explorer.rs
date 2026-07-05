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
    pub fn linux_mvp() -> Self {
        Self {
            max_children_per_directory: 256,
            collapsed_directory_names: vec![
                ".git".to_owned(),
                "node_modules".to_owned(),
                "target".to_owned(),
            ],
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

#[cfg(test)]
mod tests;
