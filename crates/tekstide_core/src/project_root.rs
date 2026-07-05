use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymlinkPolicy {
    FailClosed,
    AllowCanonicalTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidProjectRoot {
    pub display_name: String,
    pub selected_path: PathBuf,
    pub canonical_path: PathBuf,
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
