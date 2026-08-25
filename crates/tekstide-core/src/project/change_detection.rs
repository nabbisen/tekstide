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
    /// Change-detection-wiring handoff, D1: directory names a filesystem
    /// scan skips entirely -- not recorded as an entry, not recursed
    /// into, not counted against `max_entries`. Defaults to
    /// `super::IGNORED_DIRECTORY_NAMES`, the same list
    /// `FileExplorerScanPolicy::linux_mvp` builds its own
    /// `collapsed_directory_names` from -- one shared definition of
    /// project-wide scan noise, not two that could disagree. Matched by
    /// exact name against **directory** entries only, at any depth --
    /// the same directory-only rule the explorer already uses (a file or
    /// symlink named `.git`/`target`/`node_modules` is scanned
    /// normally).
    ///
    /// **`.git` is an exception to "skips entirely" as of RFC-035
    /// PR-035-A.** When this list names `.git` (the default), the
    /// directory is not fully skipped any more -- `scan_directory` gives
    /// it a narrow carve-out (`GIT_WATCHED_ENTRY_NAMES`: `hooks/` and
    /// `config`) instead of a hard skip, so an agent that installs or
    /// redirects a git hook is detected. Everything else under `.git/`
    /// (`refs/`, `objects/`, `index`, ...) is still skipped for the same
    /// churn reason the original exclusion existed. See
    /// `what-watching-dot-git-must-not-become.md`. The explorer's own
    /// `collapsed_directory_names` is untouched by this -- it is a
    /// different list built from the same shared *names*
    /// (`IGNORED_DIRECTORY_NAMES`), not the same list, and this
    /// carve-out lives only in this module's own scan, never in
    /// `is_ignored` itself.
    pub ignored_directory_names: &'static [&'static str],
}

impl Default for GeneratedChangeDetectionPolicy {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_CHANGE_DETECTOR_ENTRY_LIMIT,
            max_changed_paths: DEFAULT_CHANGED_PATH_LIMIT,
            ignored_directory_names: super::IGNORED_DIRECTORY_NAMES,
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
    /// RFC-035 PR-035-B: when a completed scan finds more changed paths
    /// than `GeneratedChangeDetectionPolicy::max_changed_paths`,
    /// `changed_paths` holds only the first `max_changed_paths` of them
    /// -- **kept, not discarded** (the pre-RFC-035 behaviour emptied
    /// `changed_paths` entirely and set `status` to `Partial`). This
    /// field carries the count of the rest, so nothing downstream has
    /// to guess it from a length comparison. Zero whenever the true
    /// count never exceeded the limit. `status` itself no longer
    /// changes for this case -- the scan genuinely completed; only the
    /// *list* was capped, which is a display-shaped fact
    /// (`ChangeSetSummary::omitted_changed_file_count`'s own kind), not
    /// a scan-completeness one (`ChangeDetectionStatus::Partial`'s
    /// kind). See `what-watching-dot-git-must-not-become.md` §5.
    pub changed_paths_omitted_by_limit: usize,
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
    pub lifecycle: ChangeLifecycle,
}

/// RFC-012 §Detection Sources answers *what kind of thing is this* --
/// deliberately no longer conflated with *what happened to it*
/// (`ChangeLifecycle`, below). For a `Deleted` path, `kind` reports what
/// the entry was in the baseline (`before.kind` in `changed_paths_between`),
/// not what it currently is, since it no longer exists to classify.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangePathKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// RFC-012 Amendment 1: *what happened to a path*, orthogonal to
/// `ChangePathKind`'s *what kind of thing is this*. `changed_paths_between`
/// already computes this distinction (present in baseline vs. current) one
/// line before it used to be discarded; this preserves it rather than
/// deriving anything new.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeLifecycle {
    Added,
    Modified,
    Deleted,
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
        let scan = scan_filesystem(project, &self.policy);
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
                changed_paths_omitted_by_limit: 0,
            };
        }

        let scan = scan_filesystem(project, &self.policy);
        let status = combine_status(baseline.status, scan.status);
        let scanned_entry_count = scan.entries.len();
        let changed_paths = if status == ChangeDetectionStatus::Complete {
            changed_paths_between(&baseline.entries, &scan.entries)
        } else {
            Vec::new()
        };

        // RFC-035 PR-035-B: keep the first `max_changed_paths`, report the
        // rest as a count -- the scan completed and the paths are known,
        // so discarding them (the pre-RFC-035 behaviour, which also
        // overwrote `status` to `Partial`) threw away real information.
        // `status` is untouched here: a capped *list* is not an
        // incomplete *scan* -- see `changed_paths_omitted_by_limit`'s own
        // doc.
        let (changed_paths, changed_paths_omitted_by_limit) =
            if changed_paths.len() > self.policy.max_changed_paths {
                let omitted = changed_paths.len() - self.policy.max_changed_paths;
                let mut changed_paths = changed_paths;
                changed_paths.truncate(self.policy.max_changed_paths);
                (changed_paths, omitted)
            } else {
                (changed_paths, 0)
            };

        DetectedChanges {
            project_id: project.id().clone(),
            source: ChangeDetectionSource::FilesystemSnapshot,
            baseline_snapshot_ref: Some(baseline.baseline_snapshot_ref.clone()),
            changed_paths,
            status,
            scanned_entry_count,
            changed_paths_omitted_by_limit,
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
            changed_paths_omitted_by_limit: 0,
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
            changed_paths_omitted_by_limit: 0,
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

fn scan_filesystem(
    project: &ProjectSession,
    policy: &GeneratedChangeDetectionPolicy,
) -> FilesystemScan {
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
    let status = match scan_directory(root, Path::new(""), &mut entries, policy) {
        Ok(true) => ChangeDetectionStatus::Partial {
            limit: policy.max_entries,
        },
        Ok(false) => ChangeDetectionStatus::Complete,
        Err(reason) => ChangeDetectionStatus::Failed { reason },
    };

    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    FilesystemScan { entries, status }
}

/// Change-detection-wiring handoff, D1, corrected per review response 251
/// finding 1: an entry is ignored only when it is a **directory** whose
/// name is in `policy.ignored_directory_names` -- not any entry of that
/// name. A file or symlink named `target`, `.git`, or `node_modules` is
/// scanned normally; only a directory by that name is skipped entirely
/// (not pushed as a `ReviewBaselineEntry`, not recursed into). This is
/// the same directory-only rule
/// `FileExplorerScanPolicy::should_collapse` already applies, both
/// sourced from `IGNORED_DIRECTORY_NAMES` -- the list was already
/// shared, this makes the *rule* for applying it shared too.
///
/// Checked against `file_type` from the one `symlink_metadata` call the
/// scan already needs to classify the entry, so telling a directory
/// apart from a same-named file costs nothing extra; the canonicalize
/// call in `ensure_canonical_inside_root` and any recursion are still
/// skipped for a matched directory.
fn is_ignored(
    file_name: &std::ffi::OsStr,
    file_type: &fs::FileType,
    policy: &GeneratedChangeDetectionPolicy,
) -> bool {
    file_type.is_dir()
        && file_name
            .to_str()
            .is_some_and(|name| policy.ignored_directory_names.contains(&name))
}

/// RFC-035 PR-035-A, "what watching .git/ must not become" §1: the two
/// paths that can install or redirect code that runs on this machine --
/// `hooks/` and `config` -- and nothing else. `refs/`, `objects/`,
/// `index`, and everything else under `.git/` churn on ordinary git
/// operations and carry no code; excluding them is the same
/// churn-avoidance reasoning that justified excluding all of `.git/` in
/// the first place. Not a `GeneratedChangeDetectionPolicy` field and not
/// configurable -- deliberately: RFC-035 D1 rejects configurability for
/// this specific default because "a security-relevant default should not
/// arrive as a setting first."
const GIT_WATCHED_ENTRY_NAMES: &[&str] = &["hooks", "config"];

/// True only for a real `.git` **directory** that `policy` would
/// otherwise fully skip -- i.e. `.git` is a directory-only name in
/// `policy.ignored_directory_names`. When a policy deliberately omits
/// `.git` from that list (the cost-measurement benchmark's "no
/// exclusions at all" policy, or a test proving the mechanism itself),
/// this returns `false` and `.git` falls through to the ordinary,
/// unrestricted recursive scan every other directory gets -- the narrow
/// carve-out below only ever *replaces* a skip, it never adds a
/// restriction that was not there before.
fn is_narrowly_watched_git_directory(
    file_name: &std::ffi::OsStr,
    file_type: &fs::FileType,
    policy: &GeneratedChangeDetectionPolicy,
) -> bool {
    file_type.is_dir() && file_name == ".git" && policy.ignored_directory_names.contains(&".git")
}

/// The classification and `ReviewBaselineEntry` construction shared by
/// [`scan_directory`] and [`scan_git_directory`] -- identical for both,
/// since a watched `.git/hooks/pre-commit` or `.git/config` is an
/// ordinary changed path like any other (§3 of the security doc: no
/// separate shape, no severity, no icon). Returns the classified `kind`
/// so the caller can decide whether to recurse.
fn classify_and_push_entry(
    root: &Path,
    relative_path: PathBuf,
    absolute_path: &Path,
    entries: &mut Vec<ReviewBaselineEntry>,
) -> Result<ChangePathKind, ChangeDetectionFailureReason> {
    let metadata = fs::symlink_metadata(absolute_path)
        .map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
    let file_type = metadata.file_type();

    let kind = if file_type.is_symlink() {
        ChangePathKind::Symlink
    } else if file_type.is_file() {
        ensure_canonical_inside_root(root, absolute_path)?;
        ChangePathKind::File
    } else if file_type.is_dir() {
        ensure_canonical_inside_root(root, absolute_path)?;
        ChangePathKind::Directory
    } else {
        ensure_canonical_inside_root(root, absolute_path)?;
        ChangePathKind::Other
    };

    entries.push(ReviewBaselineEntry {
        relative_path,
        kind,
        len: if kind == ChangePathKind::File {
            Some(metadata.len())
        } else {
            None
        },
        modified_unix_nanos: modified_unix_nanos(kind, &metadata),
    });

    Ok(kind)
}

fn scan_directory(
    root: &Path,
    relative_directory: &Path,
    entries: &mut Vec<ReviewBaselineEntry>,
    policy: &GeneratedChangeDetectionPolicy,
) -> Result<bool, ChangeDetectionFailureReason> {
    let absolute_directory = root.join(relative_directory);
    let read_dir = fs::read_dir(&absolute_directory)
        .map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
    let mut partial = false;

    for entry in read_dir {
        if entries.len() >= policy.max_entries {
            return Ok(true);
        }

        let entry = entry.map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
        let file_name = entry.file_name();
        let relative_path = relative_directory.join(&file_name);
        let absolute_path = entry.path();
        let metadata = fs::symlink_metadata(&absolute_path)
            .map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
        let file_type = metadata.file_type();

        if is_narrowly_watched_git_directory(&file_name, &file_type, policy) {
            // RFC-035: not a hard skip any more -- `.git` itself is not
            // pushed as an entry (consistent with every other name in
            // `ignored_directory_names` never becoming one), only its
            // watched children (`hooks/`, `config`) are, via the narrow
            // scan below rather than the ordinary unrestricted one.
            partial |= scan_git_directory(root, &relative_path, entries, policy)?;
            continue;
        }

        if is_ignored(&file_name, &file_type, policy) {
            continue;
        }

        let kind = classify_and_push_entry(root, relative_path.clone(), &absolute_path, entries)?;

        if kind == ChangePathKind::Directory {
            partial |= scan_directory(root, &relative_path, entries, policy)?;
        }
    }

    Ok(partial)
}

/// RFC-035 PR-035-A, "what watching .git/ must not become" §1, §2: scans
/// only `.git/`'s immediate children named in [`GIT_WATCHED_ENTRY_NAMES`]
/// -- everything else (`refs/`, `objects/`, `index`, ...) is skipped
/// without being pushed as an entry, the same churn-avoidance the full
/// exclusion used to provide for all of `.git/`. `hooks/`, once selected,
/// is recursed into with the ordinary [`scan_directory`] -- not a second,
/// narrower scanner -- so anything under it (not only a flat `.sample`
/// list) is watched exactly the way any other ordinary directory is.
///
/// **`core.hooksPath` is deliberately not followed to wherever it
/// points** (§2): watching `.git/config` itself already reports that the
/// hook location changed, which is the fact that matters. Resolving a
/// `hooksPath` redirect is real, separate scope -- reading config,
/// resolving a path that may be anywhere, watching a second location
/// that changes as config changes -- that this slice does not take on.
fn scan_git_directory(
    root: &Path,
    relative_git_directory: &Path,
    entries: &mut Vec<ReviewBaselineEntry>,
    policy: &GeneratedChangeDetectionPolicy,
) -> Result<bool, ChangeDetectionFailureReason> {
    let absolute_git_directory = root.join(relative_git_directory);
    let read_dir = fs::read_dir(&absolute_git_directory)
        .map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
    let mut partial = false;

    for entry in read_dir {
        if entries.len() >= policy.max_entries {
            return Ok(true);
        }

        let entry = entry.map_err(|_| ChangeDetectionFailureReason::MetadataReadFailed)?;
        let file_name = entry.file_name();
        if !GIT_WATCHED_ENTRY_NAMES
            .iter()
            .any(|watched| file_name == std::ffi::OsStr::new(watched))
        {
            continue;
        }

        let relative_path = relative_git_directory.join(&file_name);
        let absolute_path = entry.path();

        let kind = classify_and_push_entry(root, relative_path.clone(), &absolute_path, entries)?;

        if kind == ChangePathKind::Directory {
            partial |= scan_directory(root, &relative_path, entries, policy)?;
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
                    lifecycle: ChangeLifecycle::Modified,
                }),
                (Some(before), None) => Some(DetectedChangedPath {
                    relative_path,
                    kind: before.kind,
                    lifecycle: ChangeLifecycle::Deleted,
                }),
                (None, Some(after)) => Some(DetectedChangedPath {
                    relative_path,
                    kind: after.kind,
                    lifecycle: ChangeLifecycle::Added,
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
