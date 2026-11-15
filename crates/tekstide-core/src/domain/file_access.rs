use std::fmt;
use std::path::PathBuf;

use super::ProjectId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAccessSymlinkStatus {
    NoSymlink,
    /// Unix symlink handling is covered for the Linux MVP. Windows
    /// symlink/junction/reparse-point behavior must be reviewed before
    /// Windows support is claimed.
    InRootSymlink,
    UnresolvedSymlink,
    EscapesRoot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAccessContainmentStatus {
    InsideRoot,
    OutsideRoot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileAccessTarget {
    pub project_id: ProjectId,
    pub selected_relative_path: PathBuf,
    pub selected_absolute_path: PathBuf,
    pub canonical_path: PathBuf,
    pub root_canonical_path: PathBuf,
    pub symlink_status: FileAccessSymlinkStatus,
    pub containment_status: FileAccessContainmentStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAccessBlockedReason {
    AbsolutePathNotAllowed,
    InvalidRelativePath,
    MissingPath,
    PermissionDenied,
    CannotReadPath,
    RootEscape,
    SymlinkEscape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileAccessError {
    pub project_id: ProjectId,
    pub selected_relative_path: PathBuf,
    pub selected_absolute_path: PathBuf,
    pub root_canonical_path: PathBuf,
    pub reason: FileAccessBlockedReason,
}

impl fmt::Display for FileAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "blocked file access for project {} at {}: {:?}",
            self.project_id,
            self.selected_relative_path.display(),
            self.reason
        )
    }
}

impl std::error::Error for FileAccessError {}
