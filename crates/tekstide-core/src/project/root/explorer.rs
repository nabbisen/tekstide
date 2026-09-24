use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::runtime::git::{IgnoreAnswer, IgnoreReport, IgnoreUnknown};

use super::{
    FileAccessBlockedReason, FileAccessError, FileAccessSymlinkStatus, FileAccessTarget,
    ProjectFileAccessPolicy, ProjectRootHandle,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileExplorerScanPolicy {
    pub max_children_per_directory: usize,
    /// How many entries beyond the cap the scanner will count before it
    /// reports "at least" (see [`OMITTED_COUNT_LIMIT`]).
    pub omitted_count_limit: usize,
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
            omitted_count_limit: OMITTED_COUNT_LIMIT,
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

/// **RFC-055 D4.** What git said about one entry -- three states, never two.
/// `NotIgnored` is a claim git made (the entry was in a batch that got an
/// answer); `Unknown` is the absence of one, and must not be drawn or counted
/// as either of the others. An entry the scan never asked about -- a blocked
/// or unreadable one (the query is not a second way into the filesystem), or
/// anything past the per-directory cap -- is `Unknown`, as is every entry when
/// git could not answer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExplorerIgnoreState {
    #[default]
    Unknown,
    NotIgnored,
    Ignored,
}

/// Where the repository that answered is, relative to the project. The status
/// bar reads only a `.git` at the project root; a project nested in a
/// repository, or a repository nested in a project, is answered by a
/// repository the status bar does not describe, and the sidebar says so
/// (review 431, ruling 4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerRepositoryPlacement {
    AtProjectRoot,
    /// The project is inside the repository.
    AboveProjectRoot,
    /// The directory is inside a repository that is itself inside the project.
    BelowProjectRoot,
}

/// Why git's answer was not used, so the floor is stated rather than guessed at.
/// Mirrors [`crate::runtime::git::IgnoreUnknown`] plus [`Self::NotAsked`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerFloorReason {
    /// The scan was never sent to git (the pure scanner on its own).
    NotAsked,
    /// No repository encloses the directory.
    NotARepository,
    /// A repository encloses it and was declined (rooted at or above `$HOME`).
    RepositoryDeclined,
    /// `git` is unavailable, too old, or the repository's configuration is
    /// not one the gate vouches for.
    GateRefused,
    /// `git` ran and did not answer (or a name could not be asked about).
    QueryFailed,
}

/// **Which rule decided the ignore state and the collapsed directories of one
/// scan -- a value the scan carries**, because the sidebar has to say it (D6) and
/// a render-time guess would drift from what the scan did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplorerIgnoreRule {
    /// Git's answer governs: a directory is collapsed when git says it is
    /// ignored, and an entry git did not name is an ordinary one.
    Git {
        repository: ExplorerRepositoryPlacement,
    },
    /// The fixed list (`IGNORED_DIRECTORY_NAMES`) governs, and this says why git
    /// did not.
    Floor(ExplorerFloorReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerNode {
    pub name: String,
    pub relative_path: PathBuf,
    pub kind: ExplorerNodeKind,
    pub state: ExplorerNodeState,
    pub symlink_status: FileAccessSymlinkStatus,
    /// RFC-055. `Unknown` until a scan has asked git (see
    /// [`ExplorerDirectoryScan::ask_git`]).
    pub ignore: ExplorerIgnoreState,
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
    /// How many entries the cap left out (RFC-052: **nothing is hidden
    /// silently** -- a row names how many are not shown). Counted by
    /// draining the rest of the directory *without* stat-ing anything, on
    /// the scanning thread, so the render thread never pays for it. `0`
    /// unless `truncated`.
    pub omitted_entries: usize,
    /// True when counting stopped at [`OMITTED_COUNT_LIMIT`]: the real
    /// number is at least `omitted_entries`, and the row must say "at
    /// least" rather than state a number it did not finish counting.
    pub omitted_is_lower_bound: bool,
    /// RFC-055 D6: which rule decided this scan's collapsed directories and
    /// ignore states. [`ExplorerFloorReason::NotAsked`] straight from
    /// [`FileExplorerScanner::scan_directory`]; set by [`Self::ask_git`].
    pub ignore_rule: ExplorerIgnoreRule,
}

impl ExplorerDirectoryScan {
    /// **RFC-055: asks git about the entries this scan is about to return.**
    /// Blocking (a `git` subprocess or two): call it where the scan itself is
    /// called, off the render thread -- `ExplorerScanRequest::run` is the caller.
    ///
    /// Only entries the access policy admitted are asked about (`Available` or
    /// `Collapsed`) -- the query is not a second way into the filesystem -- and
    /// only those already in `nodes`, so at most `max_children_per_directory` of
    /// them: **the omitted tail was never in the batch and keeps unknown ignore
    /// state**, and nothing here counts or describes it as ignored or as not.
    /// Names come from `relative_path`, which holds the raw filename; `name` is a
    /// lossy display string and would ask about a different file.
    pub fn ask_git(
        &mut self,
        root: &ProjectRootHandle,
        oracle: &dyn Fn(&std::path::Path, &[std::ffi::OsString]) -> IgnoreReport,
    ) {
        let asked: Vec<(usize, std::ffi::OsString)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                matches!(
                    node.state,
                    ExplorerNodeState::Available | ExplorerNodeState::Collapsed
                )
            })
            .filter_map(|(index, node)| {
                node.relative_path
                    .file_name()
                    .map(|name| (index, name.to_owned()))
            })
            .collect();
        let names: Vec<std::ffi::OsString> = asked.iter().map(|(_, name)| name.clone()).collect();
        let report = oracle(&self.directory.canonical_path, &names);
        self.apply_ignore_report(&root.valid_root().canonical_path, &asked, report);
    }

    /// The pure half of [`Self::ask_git`]: what an answer does to the nodes and
    /// to the rule. `asked` pairs each asked node's index with its raw name.
    fn apply_ignore_report(
        &mut self,
        project_root: &std::path::Path,
        asked: &[(usize, std::ffi::OsString)],
        report: IgnoreReport,
    ) {
        let placement = report.repository_root.as_deref().map(|repository| {
            if repository == project_root {
                ExplorerRepositoryPlacement::AtProjectRoot
            } else if project_root.starts_with(repository) {
                ExplorerRepositoryPlacement::AboveProjectRoot
            } else {
                ExplorerRepositoryPlacement::BelowProjectRoot
            }
        });
        let ignored = match report.answer {
            IgnoreAnswer::Ignored(set) => Some(set),
            IgnoreAnswer::NoneIgnored => Some(std::collections::BTreeSet::new()),
            IgnoreAnswer::Unknown(unknown) => {
                self.ignore_rule = ExplorerIgnoreRule::Floor(match unknown {
                    IgnoreUnknown::NotARepository => ExplorerFloorReason::NotARepository,
                    IgnoreUnknown::RepositoryDeclined => ExplorerFloorReason::RepositoryDeclined,
                    IgnoreUnknown::GateRefused => ExplorerFloorReason::GateRefused,
                    IgnoreUnknown::QueryFailed | IgnoreUnknown::UnusableName => {
                        ExplorerFloorReason::QueryFailed
                    }
                });
                None
            }
        };
        let Some(ignored) = ignored else {
            return;
        };
        // git answered, so a repository was found; `placement` is `Some`.
        let Some(repository) = placement else {
            self.ignore_rule = ExplorerIgnoreRule::Floor(ExplorerFloorReason::QueryFailed);
            return;
        };
        self.ignore_rule = ExplorerIgnoreRule::Git { repository };
        for (index, name) in asked {
            let node = &mut self.nodes[*index];
            node.ignore = if ignored.contains(name) {
                ExplorerIgnoreState::Ignored
            } else {
                ExplorerIgnoreState::NotIgnored
            };
            // D6: git's answer governs what is collapsed. A directory git says
            // is ignored is collapsed; one it does not name is an ordinary,
            // expandable one even if it is called `target`. `.git` is version-
            // control metadata, not something a `.gitignore` names, and stays
            // collapsed under either rule.
            if node.kind == ExplorerNodeKind::Directory {
                node.state = if node.ignore == ExplorerIgnoreState::Ignored || node.name == ".git" {
                    ExplorerNodeState::Collapsed
                } else {
                    ExplorerNodeState::Available
                };
            }
        }
    }
}

/// The most entries the scanner will count beyond the per-directory cap.
/// A directory with more than this many hidden entries reports "at least
/// this many": counting must stay bounded even for a hostile directory.
pub const OMITTED_COUNT_LIMIT: usize = 1_000_000;

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
        let mut omitted_entries = 0;
        let mut omitted_is_lower_bound = false;
        let mut read_dir = read_dir;

        while let Some(entry_result) = read_dir.next() {
            if nodes.len() >= policy.max_children_per_directory {
                truncated = true;
                // The entry just pulled is the first one left out, and
                // every entry after it is too. Count without stat-ing.
                let counted = read_dir
                    .by_ref()
                    .take(policy.omitted_count_limit + 1)
                    .filter(|entry| entry.is_ok())
                    .count();
                omitted_is_lower_bound = counted > policy.omitted_count_limit;
                omitted_entries = 1 + counted.min(policy.omitted_count_limit);
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

        // RFC-052 PR-052-C: folders first, then files, each by name compared
        // without regard to case (so `README.md` does not sort apart from
        // `docs`), with the exact name as the tie-break so the order is total
        // and stable. A capped scan sorts the subset it kept, as before.
        nodes.sort_by(|left, right| {
            (left.kind != ExplorerNodeKind::Directory)
                .cmp(&(right.kind != ExplorerNodeKind::Directory))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.name.cmp(&right.name))
        });

        Ok(ExplorerDirectoryScan {
            directory,
            nodes,
            truncated,
            omitted_entries,
            omitted_is_lower_bound,
            ignore_rule: ExplorerIgnoreRule::Floor(ExplorerFloorReason::NotAsked),
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
                ignore: ExplorerIgnoreState::Unknown,
            }
        }
        Err(error) => ExplorerNode {
            name,
            relative_path,
            kind,
            state: ExplorerNodeState::Blocked(error.reason),
            symlink_status: blocked_symlink_status(entry_is_symlink, error.reason),
            ignore: ExplorerIgnoreState::Unknown,
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
        ignore: ExplorerIgnoreState::Unknown,
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
pub(crate) mod hostile_fixture;
#[cfg(test)]
mod hostile_tests;
#[cfg(test)]
mod ignore_tests;
#[cfg(test)]
mod tests;
