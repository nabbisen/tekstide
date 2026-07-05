mod content;
mod identity;
mod metadata;
pub mod recent;
pub mod root;
mod runtime;
mod session;

pub use content::{
    ProjectContentError, ProjectContentStatus, ProjectContentWorkspace, text_document_state_label,
};
pub use identity::ProjectId;
pub use metadata::{
    ProjectFileState, ProjectGitDisplayStatus, ProjectGitSummary, ProjectMetadataCountStatus,
    ProjectMode, ProjectOpenSurface, ProjectProviderState, ProjectResourceLimits, ProjectWarning,
    ProjectWarningLevel, ProjectWarningState, WorkspaceTrust,
};
pub use runtime::ProjectRuntimeSummary;
pub use session::ProjectSession;

#[cfg(test)]
mod tests;
