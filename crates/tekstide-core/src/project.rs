mod content;
mod identity;
mod metadata;
pub mod recent;
pub mod root;
mod runtime;
mod session;

pub use content::{
    ProjectActiveFileLaunchAssessment, ProjectActiveFileLaunchBlockReason,
    ProjectActiveFileLaunchDecision, ProjectContentError, ProjectContentStatus,
    ProjectContentWorkspace, ProjectExplorerStatus, explorer_node_kind_label,
    explorer_node_state_label, explorer_symlink_status_label, text_document_state_label,
};
pub use identity::ProjectId;
pub use metadata::{
    ProjectFileState, ProjectGitDisplayStatus, ProjectGitSummary, ProjectMetadataCountStatus,
    ProjectMode, ProjectOpenSurface, ProjectProviderState, ProjectResourceLimits, ProjectWarning,
    ProjectWarningLevel, ProjectWarningState, WorkspaceTrust,
};
pub use runtime::ProjectRuntimeSummary;
pub use session::{
    ProjectAgentActiveFileLaunchError, ProjectAgentLaunchError, ProjectAgentRuntimeLaunchError,
    ProjectSession, ProjectTerminalError,
};

#[cfg(test)]
mod tests;
