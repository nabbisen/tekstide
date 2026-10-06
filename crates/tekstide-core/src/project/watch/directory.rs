//! RFC-026 D7 (review 455): the only directories the watch scope accepts are ones the
//! project's own access policy has admitted. The type cannot be built any other way in
//! production, so the guarantee is structural rather than something the caller must
//! remember: a path the policy would refuse never reaches the platform.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::project::root::{
    FileAccessContainmentStatus, FileAccessError, FileAccessSymlinkStatus, ProjectFileAccessPolicy,
    ProjectRootHandle,
};

/// A directory inside the project root, admitted by [`ProjectFileAccessPolicy`]. Watched
/// by its canonical path, so the events the platform reports name the same paths the
/// explorer does.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct WatchedDirectory {
    canonical: PathBuf,
}

/// Why a path was not admitted. Logged and skipped; never a reason to stop watching.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchAdmissionError {
    /// The access policy refused the path outright (escape, missing, unreadable).
    Refused(FileAccessError),
    /// The policy resolved it, but the path is not a directory.
    NotADirectory,
    /// The policy resolved it, but it lands outside the root.
    OutsideRoot,
    /// The policy resolved it, but it is a symlink it cannot fully resolve.
    UnresolvedSymlink,
}

impl WatchedDirectory {
    /// Admits `relative` (a project-relative path) through the access policy. Only
    /// this path, and `for_test` in tests, produce a `WatchedDirectory`.
    pub fn admit(
        root: &ProjectRootHandle,
        relative: impl AsRef<Path>,
    ) -> Result<Self, WatchAdmissionError> {
        let target = ProjectFileAccessPolicy
            .resolve_existing(root, relative)
            .map_err(WatchAdmissionError::Refused)?;
        if target.containment_status != FileAccessContainmentStatus::InsideRoot {
            return Err(WatchAdmissionError::OutsideRoot);
        }
        if target.symlink_status == FileAccessSymlinkStatus::UnresolvedSymlink {
            return Err(WatchAdmissionError::UnresolvedSymlink);
        }
        if !target.canonical_path.is_dir() {
            return Err(WatchAdmissionError::NotADirectory);
        }
        Ok(Self {
            canonical: target.canonical_path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.canonical
    }

    /// Test-only: a directory without the policy. Production code cannot reach this, so
    /// the scope's own logic can be tested without a real project tree.
    #[cfg(test)]
    pub(crate) fn for_test(path: impl Into<PathBuf>) -> Self {
        Self {
            canonical: path.into(),
        }
    }
}

/// The directories that should be watched for one open project: the root, the expanded
/// folders and the folders of open documents (D1), each admitted by the access policy.
/// Anything the policy refuses is left out and reported by its reason, not silently.
pub fn desired_directories(
    root: &ProjectRootHandle,
    expanded: &[PathBuf],
    open_document_directories: &[PathBuf],
) -> (
    BTreeSet<WatchedDirectory>,
    Vec<(PathBuf, WatchAdmissionError)>,
) {
    let mut admitted = BTreeSet::new();
    let mut refused = Vec::new();
    for relative in std::iter::once(PathBuf::new())
        .chain(expanded.iter().cloned())
        .chain(open_document_directories.iter().cloned())
    {
        match WatchedDirectory::admit(root, &relative) {
            Ok(directory) => {
                admitted.insert(directory);
            }
            Err(reason) => refused.push((relative, reason)),
        }
    }
    (admitted, refused)
}
