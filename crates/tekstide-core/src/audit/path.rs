use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditPathRequest {
    pub state_root: PathBuf,
    pub project_roots: Vec<PathBuf>,
}

impl AuditPathRequest {
    pub fn new(state_root: impl Into<PathBuf>, project_roots: Vec<PathBuf>) -> Self {
        Self {
            state_root: state_root.into(),
            project_roots,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditStoragePath {
    state_root: PathBuf,
    audit_dir: PathBuf,
    database_file: PathBuf,
    recovery_dir: PathBuf,
}

impl AuditStoragePath {
    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn audit_dir(&self) -> &Path {
        &self.audit_dir
    }

    pub fn database_file(&self) -> &Path {
        &self.database_file
    }

    pub fn recovery_dir(&self) -> &Path {
        &self.recovery_dir
    }

    pub fn journal_file(&self) -> PathBuf {
        sqlite_companion_path(&self.database_file, "-journal")
    }

    pub fn wal_file(&self) -> PathBuf {
        sqlite_companion_path(&self.database_file, "-wal")
    }

    pub fn shared_memory_file(&self) -> PathBuf {
        sqlite_companion_path(&self.database_file, "-shm")
    }

    pub fn ensure_project_root_compatible(
        &self,
        project_root: &Path,
    ) -> Result<(), AuditPathError> {
        let project_root =
            canonicalize_dir(project_root, AuditPathErrorReason::InvalidProjectRoot)?;
        if self.database_file.starts_with(&project_root)
            || self.state_root == project_root
            || self.state_root.starts_with(&project_root)
        {
            return Err(AuditPathError::new(
                AuditPathErrorReason::ProjectContainsAuditState,
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_before_open(&self) -> Result<(), AuditPathError> {
        let state_root =
            canonicalize_dir(&self.state_root, AuditPathErrorReason::InvalidStateRoot)?;
        if state_root != self.state_root {
            return Err(AuditPathError::new(
                AuditPathErrorReason::AuditPathEscapesStateRoot,
            ));
        }
        validate_existing_audit_paths(&state_root, &self.audit_dir, &self.database_file)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditPathErrorReason {
    StateRootNotAbsolute,
    InvalidStateRoot,
    InvalidProjectRoot,
    ProjectContainsAuditState,
    AuditPathEscapesStateRoot,
    AuditPathIsSymlink,
    AuditPathTypeInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditPathError {
    pub reason: AuditPathErrorReason,
}

impl AuditPathError {
    fn new(reason: AuditPathErrorReason) -> Self {
        Self { reason }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuditPathResolver;

impl AuditPathResolver {
    pub fn resolve(self, request: AuditPathRequest) -> Result<AuditStoragePath, AuditPathError> {
        if !request.state_root.is_absolute() {
            return Err(AuditPathError::new(
                AuditPathErrorReason::StateRootNotAbsolute,
            ));
        }

        let state_root =
            canonicalize_dir(&request.state_root, AuditPathErrorReason::InvalidStateRoot)?;
        let audit_dir = state_root.join("audit");
        let database_file = audit_dir.join("audit.sqlite3");
        let recovery_dir = audit_dir.join("recovery");

        validate_existing_audit_paths(&state_root, &audit_dir, &database_file)?;

        let resolved = AuditStoragePath {
            state_root,
            audit_dir,
            database_file,
            recovery_dir,
        };
        for project_root in request.project_roots {
            resolved.ensure_project_root_compatible(&project_root)?;
        }
        Ok(resolved)
    }
}

fn canonicalize_dir(path: &Path, reason: AuditPathErrorReason) -> Result<PathBuf, AuditPathError> {
    match fs::canonicalize(path) {
        Ok(path) if path.is_dir() => Ok(path),
        Ok(_) => Err(AuditPathError::new(reason)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(AuditPathError::new(reason)),
        Err(_) => Err(AuditPathError::new(reason)),
    }
}

fn validate_existing_audit_paths(
    state_root: &Path,
    audit_dir: &Path,
    database_file: &Path,
) -> Result<(), AuditPathError> {
    if !audit_dir.starts_with(state_root) || !database_file.starts_with(state_root) {
        return Err(AuditPathError::new(
            AuditPathErrorReason::AuditPathEscapesStateRoot,
        ));
    }

    if let Ok(metadata) = fs::symlink_metadata(audit_dir) {
        if metadata.file_type().is_symlink() {
            return Err(AuditPathError::new(
                AuditPathErrorReason::AuditPathIsSymlink,
            ));
        }
        if !metadata.is_dir() {
            return Err(AuditPathError::new(
                AuditPathErrorReason::AuditPathTypeInvalid,
            ));
        }
        let canonical = fs::canonicalize(audit_dir)
            .map_err(|_| AuditPathError::new(AuditPathErrorReason::AuditPathEscapesStateRoot))?;
        if !canonical.starts_with(state_root) {
            return Err(AuditPathError::new(
                AuditPathErrorReason::AuditPathEscapesStateRoot,
            ));
        }
    }

    if let Ok(metadata) = fs::symlink_metadata(database_file) {
        if metadata.file_type().is_symlink() {
            return Err(AuditPathError::new(
                AuditPathErrorReason::AuditPathIsSymlink,
            ));
        }
        if !metadata.is_file() {
            return Err(AuditPathError::new(
                AuditPathErrorReason::AuditPathTypeInvalid,
            ));
        }
    }

    for companion in [
        sqlite_companion_path(database_file, "-journal"),
        sqlite_companion_path(database_file, "-wal"),
        sqlite_companion_path(database_file, "-shm"),
    ] {
        if let Ok(metadata) = fs::symlink_metadata(companion) {
            if metadata.file_type().is_symlink() {
                return Err(AuditPathError::new(
                    AuditPathErrorReason::AuditPathIsSymlink,
                ));
            }
            if !metadata.is_file() {
                return Err(AuditPathError::new(
                    AuditPathErrorReason::AuditPathTypeInvalid,
                ));
            }
        }
    }

    Ok(())
}

fn sqlite_companion_path(database_file: &Path, suffix: &str) -> PathBuf {
    let mut path = database_file.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}
