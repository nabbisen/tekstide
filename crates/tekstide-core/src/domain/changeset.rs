use super::{AgentRunId, ChangeSetId, DomainTimestamp};
use crate::project::ProjectId;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeSet {
    // Persistent/reference metadata for detected changes. It does not snapshot file contents.
    pub id: ChangeSetId,
    pub project_id: ProjectId,
    pub agent_run_id: Option<AgentRunId>,
    pub baseline_snapshot_ref: Option<String>,
    pub changed_files: Vec<PathBuf>,
    pub summary: String,
    pub review_state: ReviewState,
    pub created_at: DomainTimestamp,
}

impl ChangeSet {
    pub fn unreviewed(
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        changed_files: Vec<PathBuf>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            id: ChangeSetId::new_uuid(),
            project_id,
            agent_run_id,
            baseline_snapshot_ref: None,
            changed_files,
            summary: summary.into(),
            review_state: ReviewState::Unreviewed,
            created_at: DomainTimestamp::now_utc(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewState {
    Unreviewed,
    Accepted,
    PartiallyAccepted,
    Rejected,
    Superseded,
}
