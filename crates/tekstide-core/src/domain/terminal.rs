use super::{DomainTimestamp, TerminalId, TranscriptId};
use crate::project::ProjectId;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalKind {
    Plain,
    Supervised,
    Managed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalStatus {
    Starting,
    Running,
    Exited,
    Failed,
    Terminating,
    OrphanedUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisibleSlot {
    Hidden,
    Primary,
    Secondary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSession {
    // Persistent/reference metadata.
    pub id: TerminalId,
    pub project_id: ProjectId,
    pub kind: TerminalKind,
    pub title: String,
    pub cwd: PathBuf,
    pub command_line_summary: String,
    pub created_at: DomainTimestamp,
    pub last_output_at: Option<DomainTimestamp>,
    pub exit_status: Option<i32>,
    pub transcript_ref: Option<TranscriptId>,
    pub environment_policy_ref: Option<String>,
    // Runtime state summary. Future process handles/PIDs remain runtime-only and must not be
    // persisted through this metadata shape.
    status: TerminalStatus,
    visible_slot: VisibleSlot,
}

impl TerminalSession {
    pub fn new(
        project_id: ProjectId,
        kind: TerminalKind,
        title: impl Into<String>,
        cwd: impl Into<PathBuf>,
        command_line_summary: impl Into<String>,
    ) -> Self {
        Self {
            id: TerminalId::new_uuid(),
            project_id,
            kind,
            title: title.into(),
            cwd: cwd.into(),
            command_line_summary: command_line_summary.into(),
            created_at: DomainTimestamp::now_utc(),
            last_output_at: None,
            exit_status: None,
            transcript_ref: None,
            environment_policy_ref: None,
            status: TerminalStatus::Starting,
            visible_slot: VisibleSlot::Hidden,
        }
    }

    pub fn status(&self) -> TerminalStatus {
        self.status
    }

    pub fn visible_slot(&self) -> VisibleSlot {
        self.visible_slot
    }

    pub fn transition_to(&mut self, next: TerminalStatus) -> Result<(), TerminalTransitionError> {
        if can_transition_terminal(self.status, next) {
            self.status = next;
            Ok(())
        } else {
            Err(TerminalTransitionError {
                from: self.status,
                to: next,
            })
        }
    }

    pub fn assign_visible_slot(&mut self, visible_slot: VisibleSlot) {
        self.visible_slot = visible_slot;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTransitionError {
    pub from: TerminalStatus,
    pub to: TerminalStatus,
}

fn can_transition_terminal(from: TerminalStatus, to: TerminalStatus) -> bool {
    use TerminalStatus::{Exited, Failed, OrphanedUnknown, Running, Starting, Terminating};

    matches!(
        (from, to),
        (Starting, Running)
            | (Starting, Failed)
            | (Running, Terminating)
            | (Running, Exited)
            | (Running, Failed)
            | (Running, OrphanedUnknown)
            | (Terminating, Exited)
            | (Terminating, Failed)
            | (Terminating, OrphanedUnknown)
            | (OrphanedUnknown, Exited)
    )
}
