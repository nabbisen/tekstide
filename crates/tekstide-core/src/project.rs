mod identity;
mod metadata;
pub mod recent;
pub mod root;
mod runtime;
mod session;

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
