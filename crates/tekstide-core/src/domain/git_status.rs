use std::path::PathBuf;

/// RFC-077 Stage 9: vocabulary (T0), moved from `project::metadata` --
/// dependency-free, re-exported from there unchanged so no caller has to
/// move with it. The behavioral impls on `ProjectGitSummary`
/// (`display_status`, `file_status`) stay in `project::metadata`, since
/// they return `project`-owned types (`ProjectGitDisplayStatus`) --
/// Rust's own inherent-impl rules do not require an `impl` block to sit
/// in the same module as the type it is for, only the same crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectProviderState {
    Complete,
    Unavailable,
    NotImplemented,
    Unknown,
}

/// RFC-030 PR-030-C, REQ-GIT-003: a file's status as `git status`'s
/// porcelain v2 XY code would report it, collapsed to the one category a
/// per-file explorer badge needs. `Unmerged` covers every XY combination
/// porcelain v2's `u` record type can report (`DD`/`AU`/`UD`/`UA`/`DU`/
/// `AA`/`UU`) -- a merge conflict, not any one side's specific action;
/// the explorer badge for it is "this file has a conflict", not which
/// side changed what.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileGitStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Unmerged,
    /// **RFC-055 D5.** git says the entry is ignored -- the fifth category
    /// `REQ-FILE-002` names and the model had never carried. Never produced by
    /// [`crate::runtime::git::compute_summary`] (`git status` is not run with
    /// `--ignored`; RFC-055 measurement 5): it comes from the explorer's own
    /// `git check-ignore` query, carried on the node, and is *not* passed
    /// `--no-index`, so a tracked file that matches a pattern is tracked and is
    /// never this. Every `match` on this type must say what it does with it.
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGitSummary {
    pub provider_state: ProjectProviderState,
    pub branch_name: Option<String>,
    pub changed_file_count: Option<u32>,
    pub ahead_count: Option<u32>,
    pub behind_count: Option<u32>,
    /// REQ-GIT-003: per-file status, keyed by the path relative to the
    /// project root (the same convention `ExplorerNode::relative_path`
    /// already uses, in `project::root::explorer`). `Some` only
    /// alongside `changed_file_count: Some(_)`
    /// -- the one path (`read_status_summary`'s single `git status`
    /// call) that actually walks per-file entries; every other path
    /// (`branch_only_summary`, `Unavailable`) leaves it `None`, the same
    /// "not computed" convention the other `Option` fields already use.
    /// A file with no entry in the map (present but unmodified, or not
    /// tracked at all and not shown by a plain `git status`) carries no
    /// status -- absence is not itself a status.
    pub file_statuses: Option<std::collections::BTreeMap<PathBuf, FileGitStatus>>,
}
