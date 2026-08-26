mod change_detection;
mod content;
mod diff;
mod identity;
mod ignored_directories;
mod metadata;
pub mod recent;
pub mod root;
mod runtime;
mod session;

pub use change_detection::{
    ChangeLifecycle, ChangePathKind, ChangedPathValidationError, ChangedPathValidationErrorReason,
    DetectedChangedPath, DetectedChanges, GeneratedChangeDetectionPolicy, GeneratedChangeDetector,
    ReviewBaseline, ReviewBaselineEntry,
};
pub use content::{
    ProjectActiveFileLaunchAssessment, ProjectActiveFileLaunchBlockReason,
    ProjectActiveFileLaunchDecision, ProjectContentError, ProjectContentStatus,
    ProjectContentWorkspace, ProjectExplorerStatus, explorer_node_kind_label,
    explorer_node_state_label, explorer_symlink_status_label, text_document_state_label,
};
pub use diff::{
    ContentLifecycle, DEFAULT_MAX_DIFF_INPUT_BYTES, DEFAULT_MAX_DIFF_LINES, DiffContent,
    DiffContentError, DiffGateDecision, DiffGateRefusal, DiffPreviewPolicy, diff_content_is_stale,
    gate_diff_content_read, read_diff_content,
};
pub use identity::ProjectId;
pub use ignored_directories::IGNORED_DIRECTORY_NAMES;
pub use metadata::{
    ProjectFileState, ProjectGitDisplayStatus, ProjectGitSummary, ProjectMetadataCountStatus,
    ProjectMode, ProjectOpenSurface, ProjectProviderState, ProjectResourceLimits, ProjectWarning,
    ProjectWarningLevel, ProjectWarningState, WorkspaceTrust,
};
pub use runtime::ProjectRuntimeSummary;
pub use session::{
    ProjectAgentActiveFileLaunchError, ProjectAgentLaunchError, ProjectAgentRuntimeLaunchError,
    ProjectApprovalError, ProjectChangeSetError, ProjectSession, ProjectTerminalError,
    ProjectTranscriptError, ProjectTranscriptPurgeSummary, agent_run_status_is_active,
    terminal_status_is_active,
};

#[cfg(test)]
mod tests;
