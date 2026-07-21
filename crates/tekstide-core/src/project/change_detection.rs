use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::domain::{
    AgentRunId, ChangeDetectionFailureReason, ChangeDetectionSource, ChangeDetectionStatus,
    DomainTimestamp,
};

use super::{ProjectId, ProjectSession};

pub const DEFAULT_CHANGE_DETECTOR_ENTRY_LIMIT: usize = 4096;
pub const DEFAULT_CHANGED_PATH_LIMIT: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedChangeDetectionPolicy {
    pub max_entries: usize,
    pub max_changed_paths: usize,
}

impl Default for GeneratedChangeDetectionPolicy {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_CHANGE_DETECTOR_ENTRY_LIMIT,
            max_changed_paths: DEFAULT_CHANGED_PATH_LIMIT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewBaseline {
    pub project_id: ProjectId,
    pub agent_run_id: Option<AgentRunId>,
    pub captured_at: DomainTimestamp,
    pub source: ChangeDetectionSource,
    pub baseline_snapshot_ref: String,
    pub entries: Vec<ReviewBaselineEntry>,
    pub status: ChangeDetectionStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewBaselineEntry {
    pub relative_path: PathBuf,
    pub kind: ChangePathKind,
    pub len: Option<u64>,
    pub modified_unix_nanos: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectedChanges {
    pub project_id: ProjectId,
    pub source: ChangeDetectionSource,
    pub baseline_snapshot_ref: Option<String>,
    pub changed_paths: Vec<DetectedChangedPath>,
    pub status: ChangeDetectionStatus,
    pub scanned_entry_count: usize,
}

impl DetectedChanges {
    pub fn changed_files(&self) -> Vec<PathBuf> {
        self.changed_paths
            .iter()
            .map(|changed_path| changed_path.relative_path.clone())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectedChangedPath {
    pub relative_path: PathBuf,
    pub kind: ChangePathKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangePathKind {
    File,
    Directory,
    Symlink,
    Deleted,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangedPathValidationError {
    pub project_id: ProjectId,
    pub selected_path: PathBuf,
    pub reason: ChangedPathValidationErrorReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangedPathValidationErrorReason {
    AbsolutePathMissing,
    InvalidRelativePath,
    MetadataReadFailed,
    RootEscape,
    SymlinkEscape,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneratedChangeDetector {
    policy: GeneratedChangeDetectionPolicy,
}

impl GeneratedChangeDetector {
    pub fn new(policy: GeneratedChangeDetectionPolicy) -> Self {
        Self { policy }
    }

    pub fn capture_filesystem_baseline(&self, project: &ProjectSession) -> ReviewBaseline {
        self.capture_filesystem_baseline_for_agent_run(project, None)
    }

    pub fn capture_agent_run_filesystem_baseline(
        &self,
        project: &ProjectSession,
        agent_run_id: AgentRunId,
    ) -> ReviewBaseline {
        self.capture_filesystem_baseline_for_agent_run(project, Some(agent_run_id))
    }

    fn capture_filesystem_baseline_for_agent_run(
        &self,
        project: &ProjectSession,
        agent_run_id: Option<AgentRunId>,
    ) -> ReviewBaseline {
        let captured_at = DomainTimestamp::now_utc();
        let scan = scan_filesystem(project, self.policy.max_entries);
        let entry_count = scan.entries.len();
        ReviewBaseline {
            project_id: project.id().clone(),
            agent_run_id,
            captured_at: captured_at.clone(),
            source: ChangeDetectionSource::FilesystemSnapshot,
            baseline_snapshot_ref: format!(
                "filesystem-snapshot:{}:{}",
                captured_at.as_str(),
                entry_count
            ),
            entries: scan.entries,
            status: scan.status,
        }
    }

    pub fn detect_filesystem_changes(
        &self,
        project: &ProjectSession,
        baseline: &ReviewBaseline,
    ) -> DetectedChanges {
        if baseline.project_id != *project.id() {
            return DetectedChanges {
                project_id: project.id().clone(),
                source: ChangeDetectionSource::FilesystemSnapshot,
                baseline_snapshot_ref: Some(baseline.baseline_snapshot_ref.clone()),
                changed_paths: Vec::new(),
                status: ChangeDetectionStatus::Failed {
                    reason: ChangeDetectionFailureReason::CrossProjectBaseline,
                },
                scanned_entry_count: 0,
            };
        }

        let scan = scan_filesystem(project, self.policy.max_entries);
        let mut status = combine_status(baseline.status, scan.status);
        let scanned_entry_count = scan.entries.len();
        let changed_paths = if status == ChangeDetectionStatus::Complete {
            changed_paths_between(&baseline.entries, &scan.entries)
        } else {
            Vec::new()
        };

        let changed_paths = if status == ChangeDetectionStatus::Complete
            && changed_paths.len() > self.policy.max_changed_paths
        {
            status = ChangeDetectionStatus::Partial {
                limit: self.policy.max_changed_paths,
            };
            Vec::new()
        } else {
            changed_paths
        };

        DetectedChanges {
            project_id: project.id().clone(),
            source: ChangeDetectionSource::FilesystemSnapshot,
            baseline_snapshot_ref: Some(baseline.baseline_snapshot_ref.clone()),
            changed_paths,
            status,
            scanned_entry_count,
        }
    }

    pub fn detect_git_status_unavailable(&self, project: &ProjectSession) -> DetectedChanges {
        DetectedChanges {
            project_id: project.id().clone(),
            source: ChangeDetectionSource::GitStatus,
            baseline_snapshot_ref: None,
            changed_paths: Vec::new(),
            status: ChangeDetectionStatus::Unavailable,
            scanned_entry_count: 0,
        }
    }

    pub fn detect_git_status_unsupported(&self, project: &ProjectSession) -> DetectedChanges {
        DetectedChanges {
            project_id: project.id().clone(),
            source: ChangeDetectionSource::GitStatus,
            baseline_snapshot_ref: None,
            changed_paths: Vec::new(),
            status: ChangeDetectionStatus::Unsupported,
            scanned_entry_count: 0,
        }
    }

    pub fn validate_changed_path(
        &self,
        project: &ProjectSession,
        selected_path: impl AsRef<Path>,
    ) -> Result<PathBuf, ChangedPathValidationError> {
        validate_changed_path(project, selected_path.as_ref())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FilesystemScan {
    entries: Vec<ReviewBaselineEntry>,
    status: ChangeDetectionStatus,
}

fn scan_filesystem(project: &ProjectSession, max_entries: usize) -> FilesystemScan {
    let root = project.canonical_root_path();
    if fs::metadata(root).is_err() {
        return FilesystemScan {
            entries: Vec::new(),
            status: ChangeDetectionStatus::Failed {
                reason: ChangeDetectionFailureReason::RootUnavailable,
            },
        };
    }

    let mut entries = Vec::new();
    let status = match scan_directory(root, Path::new(""), &mut entries, max_entries) {
        Ok(true) => ChangeDetectionStatus::Partial { limit: max_entries },
        Ok(false) => ChangeDetectionStatus::Complete,
        Err(reason) => ChangeDetectionStatus::Failed { reason },
    };

    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    FilesystemScan { entries, status }
}

fn scan_directory(
    root: &Path,
    relative_directory: &Path,
    entries: &mut Vec<ReviewBaselineEntry>,
    max_entries: usize,
) -> Result<bool, ChangeDetectionFailureReason> {
    let absolute_directory = root.join(relative_directory);
    let read_dir = fs::read_dir(&absolute_directory)
        .map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
    let mut partial = false;

    for entry in read_dir {
        if entries.len() >= max_entries {
            return Ok(true);
        }

        let entry = entry.map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
        let relative_path = relative_directory.join(entry.file_name());
        let absolute_path = entry.path();
        let metadata = fs::symlink_metadata(&absolute_path)
            .map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
        let file_type = metadata.file_type();
        let kind = if file_type.is_symlink() {
            ChangePathKind::Symlink
        } else if file_type.is_file() {
            ensure_canonical_inside_root(root, &absolute_path)?;
            ChangePathKind::File
        } else if file_type.is_dir() {
            ensure_canonical_inside_root(root, &absolute_path)?;
            ChangePathKind::Directory
        } else {
            ensure_canonical_inside_root(root, &absolute_path)?;
            ChangePathKind::Other
        };

        entries.push(ReviewBaselineEntry {
            relative_path: relative_path.clone(),
            kind,
            len: if kind == ChangePathKind::File {
                Some(metadata.len())
            } else {
                None
            },
            modified_unix_nanos: modified_unix_nanos(kind, &metadata),
        });

        if kind == ChangePathKind::Directory {
            partial |= scan_directory(root, &relative_path, entries, max_entries)?;
        }
    }

    Ok(partial)
}

fn modified_unix_nanos(kind: ChangePathKind, metadata: &fs::Metadata) -> Option<u128> {
    if kind == ChangePathKind::Directory {
        return None;
    }

    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

fn ensure_canonical_inside_root(
    root: &Path,
    selected_path: &Path,
) -> Result<(), ChangeDetectionFailureReason> {
    let canonical_path = fs::canonicalize(selected_path)
        .map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
    if canonical_path == root || canonical_path.strip_prefix(root).is_ok() {
        Ok(())
    } else {
        Err(ChangeDetectionFailureReason::PathOutsideRoot)
    }
}

fn changed_paths_between(
    baseline_entries: &[ReviewBaselineEntry],
    current_entries: &[ReviewBaselineEntry],
) -> Vec<DetectedChangedPath> {
    let baseline_by_path = entries_by_path(baseline_entries);
    let current_by_path = entries_by_path(current_entries);
    let paths = baseline_by_path
        .keys()
        .chain(current_by_path.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    paths
        .into_iter()
        .filter_map(|relative_path| {
            match (
                baseline_by_path.get(&relative_path),
                current_by_path.get(&relative_path),
            ) {
                (Some(before), Some(after)) if before == after => None,
                (Some(_), Some(after)) => Some(DetectedChangedPath {
                    relative_path,
                    kind: after.kind,
                }),
                (Some(_), None) => Some(DetectedChangedPath {
                    relative_path,
                    kind: ChangePathKind::Deleted,
                }),
                (None, Some(after)) => Some(DetectedChangedPath {
                    relative_path,
                    kind: after.kind,
                }),
                (None, None) => None,
            }
        })
        .collect()
}

fn entries_by_path(entries: &[ReviewBaselineEntry]) -> BTreeMap<PathBuf, &ReviewBaselineEntry> {
    entries
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry))
        .collect()
}

fn combine_status(
    baseline_status: ChangeDetectionStatus,
    current_status: ChangeDetectionStatus,
) -> ChangeDetectionStatus {
    match (baseline_status, current_status) {
        (ChangeDetectionStatus::Failed { reason }, _)
        | (_, ChangeDetectionStatus::Failed { reason }) => ChangeDetectionStatus::Failed { reason },
        (ChangeDetectionStatus::Partial { limit }, _)
        | (_, ChangeDetectionStatus::Partial { limit }) => ChangeDetectionStatus::Partial { limit },
        (ChangeDetectionStatus::Unavailable, _) | (_, ChangeDetectionStatus::Unavailable) => {
            ChangeDetectionStatus::Unavailable
        }
        (ChangeDetectionStatus::Unsupported, _) | (_, ChangeDetectionStatus::Unsupported) => {
            ChangeDetectionStatus::Unsupported
        }
        (ChangeDetectionStatus::Complete, ChangeDetectionStatus::Complete) => {
            ChangeDetectionStatus::Complete
        }
    }
}

fn validate_changed_path(
    project: &ProjectSession,
    selected_path: &Path,
) -> Result<PathBuf, ChangedPathValidationError> {
    let normalized = if selected_path.is_absolute() {
        let canonical_path = fs::canonicalize(selected_path).map_err(|error| {
            changed_path_error(
                project,
                selected_path,
                if error.kind() == io::ErrorKind::NotFound {
                    ChangedPathValidationErrorReason::AbsolutePathMissing
                } else {
                    ChangedPathValidationErrorReason::MetadataReadFailed
                },
            )
        })?;

        if canonical_path == *project.canonical_root_path()
            || canonical_path
                .strip_prefix(project.canonical_root_path())
                .is_ok()
        {
            canonical_path
                .strip_prefix(project.canonical_root_path())
                .unwrap_or_else(|_| Path::new(""))
                .to_path_buf()
        } else if contains_existing_symlink_component(selected_path).unwrap_or(false) {
            return Err(changed_path_error(
                project,
                selected_path,
                ChangedPathValidationErrorReason::SymlinkEscape,
            ));
        } else {
            return Err(changed_path_error(
                project,
                selected_path,
                ChangedPathValidationErrorReason::RootEscape,
            ));
        }
    } else {
        normalize_relative_changed_path(selected_path)
            .map_err(|reason| changed_path_error(project, selected_path, reason))?
    };

    let selected_canonical_anchor_path = project.canonical_root_path().join(&normalized);
    if let Some(reason) = escaping_symlink_reason(project, &selected_canonical_anchor_path) {
        return Err(changed_path_error(project, selected_path, reason));
    }

    if selected_canonical_anchor_path.exists() {
        let canonical_path = fs::canonicalize(&selected_canonical_anchor_path).map_err(|_| {
            changed_path_error(
                project,
                selected_path,
                ChangedPathValidationErrorReason::MetadataReadFailed,
            )
        })?;
        if canonical_path != *project.canonical_root_path()
            && canonical_path
                .strip_prefix(project.canonical_root_path())
                .is_err()
        {
            return Err(changed_path_error(
                project,
                selected_path,
                ChangedPathValidationErrorReason::RootEscape,
            ));
        }
    }

    Ok(normalized)
}

fn normalize_relative_changed_path(
    selected_path: &Path,
) -> Result<PathBuf, ChangedPathValidationErrorReason> {
    let mut normalized = PathBuf::new();

    for component in selected_path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ChangedPathValidationErrorReason::InvalidRelativePath);
            }
        }
    }

    Ok(normalized)
}

fn escaping_symlink_reason(
    project: &ProjectSession,
    selected_absolute_path: &Path,
) -> Option<ChangedPathValidationErrorReason> {
    let relative_path = selected_absolute_path
        .strip_prefix(project.canonical_root_path())
        .ok()?;
    let mut current = project.canonical_root_path().clone();

    for component in relative_path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => current.push(component.as_os_str()),
            Component::Normal(part) => {
                current.push(part);
                let Ok(metadata) = fs::symlink_metadata(&current) else {
                    return None;
                };
                if metadata.file_type().is_symlink() {
                    let Ok(canonical_path) = fs::canonicalize(&current) else {
                        return Some(ChangedPathValidationErrorReason::MetadataReadFailed);
                    };
                    if canonical_path != *project.canonical_root_path()
                        && canonical_path
                            .strip_prefix(project.canonical_root_path())
                            .is_err()
                    {
                        return Some(ChangedPathValidationErrorReason::SymlinkEscape);
                    }
                }
            }
        }
    }

    None
}

fn contains_existing_symlink_component(path: &Path) -> io::Result<bool> {
    let mut current = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => current.push(component.as_os_str()),
            Component::Normal(part) => {
                current.push(part);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                    Err(error) => return Err(error),
                }
            }
        }
    }

    Ok(false)
}

fn changed_path_error(
    project: &ProjectSession,
    selected_path: &Path,
    reason: ChangedPathValidationErrorReason,
) -> ChangedPathValidationError {
    ChangedPathValidationError {
        project_id: project.id().clone(),
        selected_path: selected_path.to_path_buf(),
        reason,
    }
}
