use super::{AgentRunId, DomainTimestamp, TerminalId, TranscriptId};
use crate::project::ProjectId;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transcript {
    // Persistent metadata only. Transcript bytes live in the local transcript store.
    pub id: TranscriptId,
    pub project_id: ProjectId,
    pub terminal_id: TerminalId,
    pub agent_run_id: Option<AgentRunId>,
    pub storage_path: PathBuf,
    pub byte_count: u64,
    pub truncation_state: TruncationState,
    pub lifecycle_state: TranscriptLifecycleState,
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
            lifecycle_state: TranscriptLifecycleState::Active,
            retention_policy: retention_policy.into(),
            created_at: DomainTimestamp::now_utc(),
            last_write_at: None,
        }
    }

    pub fn record_active_write(&mut self, byte_count: u64) {
        self.byte_count = byte_count;
        self.truncation_state = TruncationState::Complete;
        self.lifecycle_state = TranscriptLifecycleState::Active;
        self.last_write_at = Some(DomainTimestamp::now_utc());
    }

    pub fn record_truncated_write(&mut self, byte_count: u64) {
        self.byte_count = byte_count;
        self.truncation_state = TruncationState::Truncated;
        self.lifecycle_state = TranscriptLifecycleState::Truncated;
        self.last_write_at = Some(DomainTimestamp::now_utc());
    }

    pub fn record_lifecycle_state(&mut self, lifecycle_state: TranscriptLifecycleState) {
        self.lifecycle_state = lifecycle_state;
        if !lifecycle_state.has_retained_bytes() {
            self.byte_count = 0;
        }
    }

    pub fn mark_purged(&mut self) {
        self.storage_path = PathBuf::new();
        self.byte_count = 0;
        self.truncation_state = TruncationState::Complete;
        self.lifecycle_state = TranscriptLifecycleState::Purged;
    }

    pub fn has_retained_bytes(&self) -> bool {
        self.lifecycle_state.has_retained_bytes() && self.byte_count > 0
    }

    pub fn is_tombstone(&self) -> bool {
        self.lifecycle_state.is_tombstone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TruncationState {
    Complete,
    Truncated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptLifecycleState {
    Active,
    Truncated,
    Expired,
    DisabledByOptOut,
    CaptureFailed,
    Purged,
}

impl TranscriptLifecycleState {
    pub fn has_retained_bytes(self) -> bool {
        matches!(self, Self::Active | Self::Truncated | Self::Expired)
    }

    pub fn is_tombstone(self) -> bool {
        self == Self::Purged
    }
}
