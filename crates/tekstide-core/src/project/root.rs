use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use super::ProjectId;

mod explorer;

/// RFC-052: the explorer tree's own tests build on the same fixture.
#[cfg(test)]
pub(crate) use explorer::hostile_fixture;
pub use explorer::{
    BrowseNode, BrowseNodeState, DirectoryBrowseError, DirectoryBrowseScan, ExplorerDirectoryScan,
    ExplorerFloorReason, ExplorerIgnoreRule, ExplorerIgnoreState, ExplorerNode, ExplorerNodeKind,
    ExplorerNodeState, ExplorerRepositoryPlacement, ExplorerScanError, FileExplorerScanPolicy,
    FileExplorerScanner, OMITTED_COUNT_LIMIT, browse_directory,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymlinkPolicy {
    FailClosed,
    AllowCanonicalTarget,
}

/// RFC-077 D7.1 (review 696/697/698): fields are private, not `pub` --
/// the defect review 696 found (`ProjectRootHandle::from_project_session`
/// built one by hand from a `ProjectSession`'s own accessors, bypassing
/// `ProjectRootValidator`) is closed by making a struct literal
/// impossible outside this file, not merely discouraged. The two
/// construction sites that remain are both inside `root.rs`:
/// `ProjectRootValidator::validate`'s own sanctioned mint, and
/// `ProjectRootHandle::new` below, which builds one from values its
/// caller already holds -- the same values `from_project_session` used
/// to read through `&ProjectSession`, just no longer naming that type to
/// get them. Accessors below, plus `#[cfg(test)] for_test`, give every
/// other module the read (and test-only write) access it needs without
/// a public constructor -- the `VerifiedCwd::for_test` pattern
/// (`agent/launch.rs`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidProjectRoot {
    display_name: String,
    selected_path: PathBuf,
    canonical_path: PathBuf,
}

impl ValidProjectRoot {
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn selected_path(&self) -> &Path {
        &self.selected_path
    }

    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    /// By-value destructuring for a caller (`app.rs`'s own
    /// `add_validated_project_root`) that already owns `self` and would
    /// otherwise have to clone each field back out of the accessors
    /// above.
    pub fn into_parts(self) -> (String, PathBuf, PathBuf) {
        (self.display_name, self.selected_path, self.canonical_path)
    }

    /// Test-only escape hatch, matching this crate's established
    /// `for_test` convention (`VerifiedCwd::for_test`, `ProjectId::for_test`)
    /// -- a fixture that needs a `ValidProjectRoot` without running a
    /// real filesystem validation. `#[cfg(test)]`-gated, so this does not
    /// weaken the real guarantee in a release build.
    #[cfg(test)]
    pub fn for_test(
        display_name: impl Into<String>,
        selected_path: impl Into<PathBuf>,
        canonical_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            display_name: display_name.into(),
            selected_path: selected_path.into(),
            canonical_path: canonical_path.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRootHandle {
    project_id: ProjectId,
    valid_root: ValidProjectRoot,
}

impl ProjectRootHandle {
    /// RFC-077 D7.1: replaces `from_project_session(&ProjectSession)` --
    /// `root` no longer names `ProjectSession` at all. Takes exactly the
    /// values the old constructor read through that reference
    /// (`project.rs`'s own `ProjectSession::root_location`-adjacent
    /// accessors), so the one caller, `project::session`, supplies them
    /// directly instead of handing over the whole session.
    pub fn new(
        project_id: ProjectId,
        display_name: impl Into<String>,
        selected_path: impl Into<PathBuf>,
        canonical_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            project_id,
            valid_root: ValidProjectRoot {
                display_name: display_name.into(),
                selected_path: selected_path.into(),
                canonical_path: canonical_path.into(),
            },
        }
    }

    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    pub fn valid_root(&self) -> &ValidProjectRoot {
        &self.valid_root
    }
}

/// RFC-077 Stage 4: these five are vocabulary (T0), not orchestration
/// types -- they describe the shape of a resolved access attempt, with no
/// behavior of their own -- and now live in `domain::file_access`,
/// re-exported from here unchanged so no caller and no published-API
/// consumer has to move with them.
pub use crate::domain::{
    FileAccessBlockedReason, FileAccessContainmentStatus, FileAccessError, FileAccessSymlinkStatus,
    FileAccessTarget,
};

/// RFC-077 Stage 4, D5 move 2: `content` is the one consumer of this
/// policy outside `project` itself, and it no longer names
/// `ProjectFileAccessPolicy` or `ProjectRootHandle` directly -- it defines
/// [`crate::content::FileAccessResolver`] for the one question it needs
/// answered, and this impl is the provider orchestration supplies.
/// Every in-`project` call site (`diff.rs`, `watch/directory.rs`,
/// `root/explorer.rs`) keeps calling `ProjectFileAccessPolicy` directly,
/// unaffected -- this impl exists for the one caller that was reaching
/// out of the module, not to replace the policy's own API.
impl crate::content::FileAccessResolver for ProjectRootHandle {
    fn resolve_existing(
        &self,
        selected_relative_path: &Path,
    ) -> Result<FileAccessTarget, FileAccessError> {
        ProjectFileAccessPolicy.resolve_existing(self, selected_relative_path)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectFileAccessPolicy;

impl ProjectFileAccessPolicy {
    /// Resolves only existing explorer/open targets.
    ///
    /// New-file and save targets must use a separate save-time resolver that
    /// revalidates root containment and symlink safety.
    pub fn resolve_existing(
        self,
        root: &ProjectRootHandle,
        selected_relative_path: impl AsRef<Path>,
    ) -> Result<FileAccessTarget, FileAccessError> {
        let selected_path_raw = selected_relative_path.as_ref().to_path_buf();

        let selected_relative_path =
            normalize_project_relative_path(&selected_path_raw).map_err(|reason| {
                let selected_absolute_path = if selected_path_raw.is_absolute() {
                    selected_path_raw.clone()
                } else {
                    root.valid_root.selected_path.join(&selected_path_raw)
                };
                file_access_error(
                    root.project_id.clone(),
                    selected_path_raw.clone(),
                    selected_absolute_path,
                    root,
                    reason,
                )
            })?;

        let selected_absolute_path = root.valid_root.selected_path.join(&selected_relative_path);

        let canonical_path = fs::canonicalize(&selected_absolute_path).map_err(|error| {
            file_access_error(
                root.project_id.clone(),
                selected_relative_path.clone(),
                selected_absolute_path.clone(),
                root,
                map_file_access_error(error),
            )
        })?;

        let containment_status = if is_inside_root(&canonical_path, &root.valid_root.canonical_path)
        {
            FileAccessContainmentStatus::InsideRoot
        } else {
            FileAccessContainmentStatus::OutsideRoot
        };

        let has_symlink = contains_symlink_component(&selected_absolute_path).map_err(|error| {
            file_access_error(
                root.project_id.clone(),
                selected_relative_path.clone(),
                selected_absolute_path.clone(),
                root,
                map_file_access_error(error),
            )
        })?;

        let symlink_status = match (has_symlink, containment_status) {
            (false, _) => FileAccessSymlinkStatus::NoSymlink,
            (true, FileAccessContainmentStatus::InsideRoot) => {
                FileAccessSymlinkStatus::InRootSymlink
            }
            (true, FileAccessContainmentStatus::OutsideRoot) => {
                FileAccessSymlinkStatus::EscapesRoot
            }
        };

        match (containment_status, symlink_status) {
            (_, FileAccessSymlinkStatus::EscapesRoot) => Err(file_access_error(
                root.project_id.clone(),
                selected_relative_path,
                selected_absolute_path,
                root,
                FileAccessBlockedReason::SymlinkEscape,
            )),
            (FileAccessContainmentStatus::OutsideRoot, _) => Err(file_access_error(
                root.project_id.clone(),
                selected_relative_path,
                selected_absolute_path,
                root,
                FileAccessBlockedReason::RootEscape,
            )),
            (FileAccessContainmentStatus::InsideRoot, _) => Ok(FileAccessTarget {
                project_id: root.project_id.clone(),
                selected_relative_path,
                selected_absolute_path,
                canonical_path,
                root_canonical_path: root.valid_root.canonical_path.clone(),
                symlink_status,
                containment_status,
            }),
        }
    }
}

fn normalize_project_relative_path(
    selected_path: &Path,
) -> Result<PathBuf, FileAccessBlockedReason> {
    if selected_path.is_absolute() {
        return Err(FileAccessBlockedReason::AbsolutePathNotAllowed);
    }

    let mut normalized = PathBuf::new();

    for component in selected_path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(FileAccessBlockedReason::InvalidRelativePath);
            }
        }
    }

    Ok(normalized)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectRootValidationError {
    DoesNotExist {
        path: PathBuf,
    },
    NotDirectory {
        path: PathBuf,
    },
    PermissionDenied {
        path: PathBuf,
    },
    CannotReadFolder {
        path: PathBuf,
    },
    SymlinkAmbiguous {
        selected_path: PathBuf,
        canonical_path: PathBuf,
    },
}

impl fmt::Display for ProjectRootValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DoesNotExist { path } => {
                write!(formatter, "folder does not exist: {}", path.display())
            }
            Self::NotDirectory { path } => {
                write!(formatter, "path is not a folder: {}", path.display())
            }
            Self::PermissionDenied { path } => {
                write!(formatter, "permission denied: {}", path.display())
            }
            Self::CannotReadFolder { path } => {
                write!(formatter, "cannot read folder: {}", path.display())
            }
            Self::SymlinkAmbiguous {
                selected_path,
                canonical_path,
            } => write!(
                formatter,
                "path changed: {} resolves to {}",
                selected_path.display(),
                canonical_path.display()
            ),
        }
    }
}

impl std::error::Error for ProjectRootValidationError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectRootValidator;

impl ProjectRootValidator {
    pub fn validate(
        self,
        selected_path: impl AsRef<Path>,
        symlink_policy: SymlinkPolicy,
    ) -> Result<ValidProjectRoot, ProjectRootValidationError> {
        let selected_path = selected_path.as_ref().to_path_buf();
        let metadata = fs::metadata(&selected_path)
            .map_err(|error| map_metadata_error(error, selected_path.clone()))?;

        if !metadata.is_dir() {
            return Err(ProjectRootValidationError::NotDirectory {
                path: selected_path,
            });
        }

        let canonical_path = fs::canonicalize(&selected_path)
            .map_err(|error| map_metadata_error(error, selected_path.clone()))?;

        if contains_symlink_component(&selected_path).map_err(|_| {
            ProjectRootValidationError::CannotReadFolder {
                path: selected_path.clone(),
            }
        })? && symlink_policy == SymlinkPolicy::FailClosed
        {
            return Err(ProjectRootValidationError::SymlinkAmbiguous {
                selected_path,
                canonical_path,
            });
        }

        fs::read_dir(&canonical_path).map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied {
                ProjectRootValidationError::PermissionDenied {
                    path: canonical_path.clone(),
                }
            } else {
                ProjectRootValidationError::CannotReadFolder {
                    path: canonical_path.clone(),
                }
            }
        })?;

        let display_name = canonical_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Project")
            .to_owned();

        Ok(ValidProjectRoot {
            display_name,
            selected_path,
            canonical_path,
        })
    }
}

fn map_metadata_error(error: io::Error, path: PathBuf) -> ProjectRootValidationError {
    match error.kind() {
        io::ErrorKind::NotFound => ProjectRootValidationError::DoesNotExist { path },
        io::ErrorKind::PermissionDenied => ProjectRootValidationError::PermissionDenied { path },
        _ => ProjectRootValidationError::CannotReadFolder { path },
    }
}

fn map_file_access_error(error: io::Error) -> FileAccessBlockedReason {
    match error.kind() {
        io::ErrorKind::NotFound => FileAccessBlockedReason::MissingPath,
        io::ErrorKind::PermissionDenied => FileAccessBlockedReason::PermissionDenied,
        _ => FileAccessBlockedReason::CannotReadPath,
    }
}

fn file_access_error(
    project_id: ProjectId,
    selected_relative_path: PathBuf,
    selected_absolute_path: PathBuf,
    root: &ProjectRootHandle,
    reason: FileAccessBlockedReason,
) -> FileAccessError {
    FileAccessError {
        project_id,
        selected_relative_path,
        selected_absolute_path,
        root_canonical_path: root.valid_root.canonical_path.clone(),
        reason,
    }
}

fn is_inside_root(path: &Path, root: &Path) -> bool {
    path == root || path.strip_prefix(root).is_ok()
}

fn contains_symlink_component(path: &Path) -> io::Result<bool> {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    let mut current = PathBuf::new();

    for component in absolute_path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => current.push(component.as_os_str()),
            Component::Normal(part) => {
                current.push(part);
                if fs::symlink_metadata(&current)?.file_type().is_symlink() {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests;
