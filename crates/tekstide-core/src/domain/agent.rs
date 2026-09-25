use super::{
    AgentRunId, ApprovalId, AuditEventId, ChangeSet, ChangeSetId, DomainTimestamp, OwnershipError,
    TerminalId, TerminalSession, TranscriptId,
};
use crate::domain::ownership::ensure_same_project;
use crate::project::ProjectId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentCompatibilityLevel {
    Plain,
    Supervised,
    Managed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunStatus {
    Draft,
    Ready,
    Preparing,
    Running,
    AwaitingApproval,
    ReviewReady,
    Completed,
    Failed,
    Cancelled,
    Detached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRun {
    // Persistent/reference metadata.
    pub id: AgentRunId,
    pub project_id: ProjectId,
    pub profile_id: String,
    pub terminal_id: Option<TerminalId>,
    pub prompt_summary: String,
    pub full_prompt_ref: Option<String>,
    pub compatibility_level: AgentCompatibilityLevel,
    pub started_at: Option<DomainTimestamp>,
    pub ended_at: Option<DomainTimestamp>,
    pub transcript_ref: Option<TranscriptId>,
    /// Why this run has no transcript, when the product knows. `None` covers
    /// a run that has one, and the cases no reason is recorded for yet.
    pub transcript_absence: Option<TranscriptAbsence>,
    pub approval_ids: Vec<ApprovalId>,
    pub change_set_ids: Vec<ChangeSetId>,
    pub artifact_refs: Vec<String>,
    pub audit_event_ids: Vec<AuditEventId>,
    // Runtime lifecycle summary. It records Tekstide's known lifecycle state, not a process
    // handle or proof of supervision after `Detached`.
    pub status: AgentRunStatus,
    /// RFC-056 D6/D12: whether the run's ending is known. **Separate from
    /// `ended_at`**, which nothing sets and the change-attribution overlap
    /// rule reads as "no end recorded, so conservatively overlapping":
    /// setting `ended_at` at a terminal transition would change which
    /// changes are attributed strongly, and this slice does not change
    /// attribution.
    pub ending: RunEnding,
    /// RFC-056 D3: the user's classification, `None` until they choose one.
    pub classification: Option<RunClassification>,
    /// RFC-056 D4: the user's own notes, and nothing but the user writes
    /// them. `None` when there are none.
    pub notes: Option<String>,
    /// RFC-056 D8: what the run's record could not hold. Set by the setters
    /// here and by [`RunRecord`](crate::transcript::RunRecord) restoration,
    /// never by the run's own content.
    pub record_bounds: RecordBounds,
    /// RFC-056 D6: how this run came to be in the project.
    pub origin: AgentRunOrigin,
}

/// RFC-056 D8's caps. A run directory must not grow without limit and a
/// runaway run must not turn the state directory into a heap; the transcript
/// stays the only place run *content* lives.
pub const RUN_NOTES_MAX_CHARS: usize = 4_000;
pub const RUN_CUSTOM_CLASSIFICATION_MAX_CHARS: usize = 64;
pub const RUN_PROMPT_SUMMARY_MAX_CHARS: usize = 1_000;
/// Per kind: approvals, change sets, audit events and artifact references.
pub const RUN_RECORD_MAX_IDS_PER_KIND: usize = 200;

/// RFC-056 D6/D12: `Unknown` is genuinely unknown, the opposite of the defect
/// RFC-053 fixed (`unknown` printed where zero was known). A run whose process
/// was killed with the app has no ending, and neither its start, zero, nor the
/// moment its record was read is one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunEnding {
    /// The run this process launched has not ended yet.
    NotEnded,
    /// Tekstide saw the run end, at this instant.
    Ended(DomainTimestamp),
    /// The run began and Tekstide never saw it end: it was detached, or the
    /// app closed while it was running.
    Unknown,
}

/// RFC-056 D3: the seven values `REQ-AGENT-015` names, and nothing more.
/// `Custom` carries a user string, which every surface renders through
/// `quote_untrusted` like any other untrusted text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunClassification {
    Coding,
    Review,
    Documentation,
    Testing,
    Refactoring,
    Release,
    Custom(String),
}

/// RFC-056 D6: a run this process launched has a lifecycle; a run restored
/// from its record is a record and has none. Every lifecycle consumer —
/// the launch limit, change attribution, the running and failed counts, the
/// close prompt — reads the launched collection only, so a record can never
/// look like a process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunOrigin {
    LaunchedHere,
    RestoredFromRecord,
}

/// RFC-056 D8: everything a run's record left out or shortened, so a bounded
/// record says it is bounded rather than reading as complete.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecordBounds {
    pub notes_truncated: bool,
    pub classification_label_truncated: bool,
    pub prompt_summary_truncated: bool,
    pub omitted_approval_ids: u64,
    pub omitted_change_set_ids: u64,
    pub omitted_audit_event_ids: u64,
    pub omitted_artifact_refs: u64,
}

impl RecordBounds {
    pub fn is_bounded(&self) -> bool {
        self.notes_truncated
            || self.classification_label_truncated
            || self.prompt_summary_truncated
            || self.omitted_approval_ids > 0
            || self.omitted_change_set_ids > 0
            || self.omitted_audit_event_ids > 0
            || self.omitted_artifact_refs > 0
    }
}

/// A custom classification with no text says nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlankClassificationLabel;

/// Why a run has no transcript (RFC-050 D3; RFC-049 D4′ adds its own case).
///
/// Kept **on the run** because a run without a transcript has no
/// `Transcript` record to hold a state, and neither existing lifecycle state
/// is true of these cases: `DisabledByOptOut` says the user declined, and
/// `CaptureFailed` says a write failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptAbsence {
    /// The transcript file's exclusive lock was held by another handle when
    /// the run started, so this run never wrote to it.
    WriterLockUnavailable,
    /// RFC-049 D4′: a retained-byte budget was still over its limit after the
    /// cleanup that ran at this launch's preflight, so the run started with
    /// capture disabled rather than deleting a transcript a live writer holds
    /// (§2). Distinct from the user declining capture, which is not true here.
    BudgetExhausted,
}

impl AgentRun {
    pub fn draft(
        project_id: ProjectId,
        profile_id: impl Into<String>,
        prompt_summary: impl Into<String>,
        compatibility_level: AgentCompatibilityLevel,
    ) -> Self {
        Self {
            id: AgentRunId::new_uuid(),
            project_id,
            profile_id: profile_id.into(),
            terminal_id: None,
            prompt_summary: prompt_summary.into(),
            full_prompt_ref: None,
            compatibility_level,
            started_at: None,
            ended_at: None,
            transcript_ref: None,
            transcript_absence: None,
            approval_ids: Vec::new(),
            change_set_ids: Vec::new(),
            artifact_refs: Vec::new(),
            audit_event_ids: Vec::new(),
            status: AgentRunStatus::Draft,
            ending: RunEnding::NotEnded,
            classification: None,
            notes: None,
            record_bounds: RecordBounds::default(),
            origin: AgentRunOrigin::LaunchedHere,
        }
    }

    pub fn transition_to(&mut self, next: AgentRunStatus) -> Result<(), AgentRunTransitionError> {
        if can_transition_agent_run(self.status, next) {
            self.status = next;
            match next {
                AgentRunStatus::Running if self.started_at.is_none() => {
                    self.started_at = Some(DomainTimestamp::now_utc());
                }
                AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Cancelled => {
                    self.ending = RunEnding::Ended(DomainTimestamp::now_utc());
                }
                AgentRunStatus::Detached => self.ending = RunEnding::Unknown,
                _ => {}
            }
            Ok(())
        } else {
            Err(AgentRunTransitionError {
                from: self.status,
                to: next,
            })
        }
    }

    /// RFC-056 D3. A custom label is trimmed and bounded, and a blank one is
    /// refused rather than stored as a classification that says nothing.
    pub fn set_classification(
        &mut self,
        classification: Option<RunClassification>,
    ) -> Result<(), BlankClassificationLabel> {
        let (classification, truncated) = match classification {
            Some(RunClassification::Custom(label)) => {
                let label = label.trim();
                if label.is_empty() {
                    return Err(BlankClassificationLabel);
                }
                let truncated = label.chars().count() > RUN_CUSTOM_CLASSIFICATION_MAX_CHARS;
                let bounded = label
                    .chars()
                    .take(RUN_CUSTOM_CLASSIFICATION_MAX_CHARS)
                    .collect();
                (Some(RunClassification::Custom(bounded)), truncated)
            }
            other => (other, false),
        };
        self.classification = classification;
        self.record_bounds.classification_label_truncated = truncated;
        Ok(())
    }

    /// RFC-056 D4/D8: the user's notes, bounded. Blank notes clear them. The
    /// truncation flag follows the notes as they are now, so shortening a note
    /// that was cut clears it.
    pub fn set_notes(&mut self, notes: &str) {
        if notes.trim().is_empty() {
            self.notes = None;
            self.record_bounds.notes_truncated = false;
            return;
        }
        let truncated = notes.chars().count() > RUN_NOTES_MAX_CHARS;
        self.notes = Some(notes.chars().take(RUN_NOTES_MAX_CHARS).collect());
        self.record_bounds.notes_truncated = truncated;
    }

    pub fn attach_terminal(&mut self, terminal: &TerminalSession) -> Result<(), OwnershipError> {
        ensure_same_project(&self.project_id, &terminal.project_id)?;
        if self.terminal_id.as_ref() == Some(&terminal.id) {
            return Ok(());
        }
        if self.terminal_id.is_some() {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.terminal_id = Some(terminal.id.clone());
        Ok(())
    }

    pub fn add_change_set(&mut self, change_set: &ChangeSet) -> Result<(), OwnershipError> {
        ensure_same_project(&self.project_id, &change_set.project_id)?;
        if change_set.agent_run_id.as_ref() != Some(&self.id) {
            return Err(OwnershipError::WrongAgentRun);
        }
        if self.change_set_ids.contains(&change_set.id) {
            return Err(OwnershipError::DuplicateAttachment);
        }
        self.change_set_ids.push(change_set.id.clone());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentRunTransitionError {
    pub from: AgentRunStatus,
    pub to: AgentRunStatus,
}

fn can_transition_agent_run(from: AgentRunStatus, to: AgentRunStatus) -> bool {
    use AgentRunStatus::{
        AwaitingApproval, Cancelled, Completed, Detached, Draft, Failed, Preparing, Ready,
        ReviewReady, Running,
    };

    matches!(
        (from, to),
        (Draft, Ready)
            | (Draft, Cancelled)
            | (Ready, Preparing)
            | (Ready, Cancelled)
            | (Preparing, Running)
            | (Preparing, Failed)
            | (Preparing, Cancelled)
            | (Running, AwaitingApproval)
            | (Running, ReviewReady)
            | (Running, Completed)
            | (Running, Failed)
            | (Running, Cancelled)
            | (Running, Detached)
            | (AwaitingApproval, Running)
            | (AwaitingApproval, Failed)
            | (AwaitingApproval, Cancelled)
            | (ReviewReady, Completed)
            | (ReviewReady, Failed)
            | (ReviewReady, Cancelled)
    )
}
