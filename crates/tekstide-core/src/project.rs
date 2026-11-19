mod change_detection;
mod content;
mod diff;
mod metadata;
mod runtime;
mod session;

/// RFC-077 D7.2: `recent` is already detached from `project` in both
/// directions (zero internal references, measured before this move) --
/// the cheapest possible first capability departure. Re-exported from
/// here unchanged so no caller and no published-API consumer has to
/// move with it: `project::recent::X` still resolves, exactly as it did
/// when this was `pub mod recent;` rather than a re-export of the new
/// top-level `crate::recent`.
pub use crate::recent;

/// RFC-077 D7.3: `root` and `explorer_tree` out, together -- `root` is
/// the internal base every other departing capability still needs
/// (in-degree 4 before this), so splitting it from `explorer_tree`
/// (nine references, one direction) would only double the churn.
/// Re-exported unchanged so `project::root::X` still resolves.
pub use crate::root;

/// RFC-074 D3, Stage 1: `ProjectId` is vocabulary (T0), not an orchestration
/// type, and now lives in `domain::identity` -- re-exported from here
/// unchanged so no caller and no published-API consumer has to move with it.
pub use crate::domain::ProjectId;

/// RFC-077 Stage 3: `WorkspaceTrust` is vocabulary (T0), not an
/// orchestration type, and now lives in `domain::trust` -- re-exported
/// from here unchanged so no caller and no published-API consumer has to
/// move with it.
pub use crate::domain::WorkspaceTrust;

/// RFC-077 Stage 9: `FileGitStatus`, `ProjectGitSummary` and
/// `ProjectProviderState` are vocabulary (T0), not orchestration types,
/// and now live in `domain::git_status` -- re-exported from here
/// unchanged so no caller and no published-API consumer has to move
/// with them.
pub use crate::domain::{FileGitStatus, ProjectGitSummary, ProjectProviderState};

/// RFC-077 D7.3: moved to `domain` ahead of `root` leaving `project` --
/// `root::explorer`'s own read of it would otherwise become a
/// `root -> project` edge the moment `root` is a sibling, cycling
/// against the `project -> root` façade above. Re-exported unchanged.
pub use crate::domain::IGNORED_DIRECTORY_NAMES;

pub use crate::explorer_tree::{
    ExplorerRootState, ExplorerScanCompleted, ExplorerScanRequest, ExplorerToggle, ExplorerTree,
    ExplorerTreeRow, ExplorerTreeRowKind, MAX_TREE_ROWS, is_expandable,
};
/// RFC-077 D7.4: `watch` out -- its only external reference
/// (`watch/directory.rs`'s own `crate::root::{...}`) already targets a
/// departed capability, so this move holds `project`'s genuine-reach
/// weight at 28 rather than lowering it (review 704's own correction:
/// not every departure moves that bound). **Not in-degree 0 as first
/// assumed**: `project/session.rs`'s own `watched_directories` named
/// three `super::watch::X` items (invisible to the gate, the same
/// previously-unseen-self-reference class every prior D7 stage has
/// found at least one of) -- repointed to `crate::watch::X`, the same
/// choice made for every other departing capability's own in-crate
/// callers, rather than left to inflate a later stage's own cut cost.
pub use crate::watch::{
    GIT_SUBPROCESSES_PER_SCAN, NotifyBackend, ProjectWatcher, SCAN_WINDOW, ScanBatcher,
    ScanRequest, ScopeChange, WatchAdmissionError, WatchBackend, WatchEvents, WatchEventsSlot,
    WatchNotice, WatchRefusal, WatchScope, WatchState, WatchedDirectory, desired_directories,
};
pub use change_detection::{
    ChangeLifecycle, ChangePathKind, ChangedPathValidationError, ChangedPathValidationErrorReason,
    DetectedChangedPath, DetectedChanges, GeneratedChangeDetectionPolicy, GeneratedChangeDetector,
    ReviewBaseline, ReviewBaselineEntry,
};
pub use content::{
    DocumentSaveOutcome, ProjectActiveFileLaunchAssessment, ProjectActiveFileLaunchBlockReason,
    ProjectActiveFileLaunchDecision, ProjectContentError, ProjectContentStatus,
    ProjectContentWorkspace, ProjectExplorerStatus, SaveAllOutcome, explorer_node_kind_label,
    explorer_node_state_label, explorer_symlink_status_label, text_document_state_label,
};
pub use diff::{
    ContentLifecycle, DEFAULT_MAX_DIFF_INPUT_BYTES, DEFAULT_MAX_DIFF_LINES, DiffContent,
    DiffContentError, DiffGateDecision, DiffGateRefusal, DiffPreviewPolicy, diff_content_is_stale,
    gate_diff_content_read, read_diff_content,
};
pub use metadata::{
    ProjectFileState, ProjectGitDisplayStatus, ProjectMetadataCountStatus, ProjectMode,
    ProjectOpenSurface, ProjectResourceLimits, ProjectWarning, ProjectWarningLevel,
    ProjectWarningState,
};
pub use runtime::ProjectRuntimeSummary;
/// RFC-077 Stage 8: `#[cfg(test)]`, not merely `pub(crate)` -- this is
/// the one place that can name both `agent` and `runtime`, and its only
/// caller outside this module is `agent::tests`'s own end-to-end
/// authority-mutation test, not production code or an API this crate
/// ships.
#[cfg(test)]
pub(crate) use session::assemble_terminal_launch_spec;
pub use session::{
    AgentAdapterApprovalError, ProjectAgentActiveFileLaunchError, ProjectAgentLaunchError,
    ProjectAgentRuntimeLaunchError, ProjectApprovalError, ProjectChangeSetError, ProjectSession,
    ProjectTerminalError, ProjectTranscriptError, ProjectTranscriptPurgeSummary,
    RunAnnotationError, RunRecordPersistSummary, RunRecordWrite, RunRecordsSetAside,
    TranscriptLoadSummary, TranscriptRetentionCleanup, agent_run_status_is_active,
    terminal_status_is_active,
};

/// RFC-077 Stage 7: the crate's one public entry point for reading a
/// transcript window. `transcript::reader::read_window` takes the
/// boundary-scanning function as a parameter rather than naming
/// `runtime` itself (see that module's own doc comment); this is the
/// one place that supplies the real one, `runtime::terminal::next_token_len`,
/// because `project` is the module with an edge to both `transcript` and
/// `runtime` already -- adding this wiring here names neither module for
/// the first time. Callers outside this crate used to reach
/// `transcript::read_window` directly; they now come through here instead
/// (a published-API change, not a behavior change -- the same file, same
/// bytes, same resynchronization).
pub fn read_transcript_window(
    storage_path: &crate::transcript::TranscriptStoragePath,
    policy: crate::transcript::TranscriptReadPolicy,
    still_being_written: bool,
) -> Result<crate::transcript::TranscriptWindow, crate::transcript::TranscriptReadError> {
    crate::transcript::read_window(
        storage_path,
        policy,
        still_being_written,
        crate::runtime::terminal::next_token_len,
    )
}

#[cfg(test)]
mod tests;
