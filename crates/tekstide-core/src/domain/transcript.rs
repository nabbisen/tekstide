use super::{AgentRunId, DomainTimestamp, TerminalId, TranscriptId};
use crate::project::ProjectId;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transcript {
    // Persistent metadata only. Transcript bytes are not written by RFC-002.
    pub id: TranscriptId,
    pub project_id: ProjectId,
    pub terminal_id: TerminalId,
    pub agent_run_id: Option<AgentRunId>,
    pub storage_path: PathBuf,
    pub byte_count: u64,
    pub truncation_state: TruncationState,
    pub retention_policy: String,
    pub created_at: DomainTimestamp,
    pub last_write_at: Option<DomainTimestamp>,
}

impl Transcript {
    pub fn metadata(
        project_id: ProjectId,
        terminal_id: TerminalId,
        agent_run_id: Option<AgentRunId>,
        storage_path: impl Into<PathBuf>,
        retention_policy: impl Into<String>,
    ) -> Self {
        Self {
            id: TranscriptId::new_uuid(),
            project_id,
            terminal_id,
            agent_run_id,
            storage_path: storage_path.into(),
            byte_count: 0,
            truncation_state: TruncationState::Complete,
            retention_policy: retention_policy.into(),
            created_at: DomainTimestamp::now_utc(),
            last_write_at: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TruncationState {
    Complete,
    Truncated,
}
