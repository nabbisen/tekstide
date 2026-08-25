use super::{AgentRunId, ChangeSetId, DomainTimestamp};
use crate::project::ProjectId;
use std::path::PathBuf;

pub const DEFAULT_CHANGESET_PATH_SUMMARY_LIMIT: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeSet {
    // Persistent/reference metadata for detected changes. It does not snapshot file contents.
    pub id: ChangeSetId,
    pub project_id: ProjectId,
    pub agent_run_id: Option<AgentRunId>,
    pub baseline_snapshot_ref: Option<String>,
    pub changed_files: Vec<PathBuf>,
    /// RFC-035 PR-035-B: how many additional changed paths detection
    /// found but did not include in `changed_files`, because the true
    /// count exceeded `GeneratedChangeDetectionPolicy::max_changed_paths`
    /// (`DetectedChanges::changed_paths_omitted_by_limit`, carried
    /// forward unchanged). Zero whenever detection never hit that limit.
    /// Distinct from `bounded_summary`'s own *display*-level omission (a
    /// `path_limit` like `DEFAULT_CHANGESET_PATH_SUMMARY_LIMIT`) -- this
    /// is a *detection*-level one. `bounded_summary` sums both into one
    /// `ChangeSetSummary::omitted_changed_file_count`, since both are
    /// the same fact from a reader's perspective ("not everything that
    /// changed is listed here"); `detection_status`'s own
    /// `Partial { limit }` stays the separate, scan-completeness fact it
    /// always was.
    pub changed_files_omitted_by_detection: usize,
    pub artifact_refs: Vec<String>,
    pub summary: String,
    pub detection_source: ChangeDetectionSource,
    pub detection_status: ChangeDetectionStatus,
    pub association_confidence: ChangeAssociationConfidence,
    pub review_state: ReviewState,
    pub created_at: DomainTimestamp,
    pub updated_at: DomainTimestamp,
}

impl ChangeSet {
    pub fn unreviewed(
        project_id: ProjectId,
        agent_run_id: Option<AgentRunId>,
        changed_files: Vec<PathBuf>,
        summary: impl Into<String>,
    ) -> Self {
        let created_at = DomainTimestamp::now_utc();
        Self {
            id: ChangeSetId::new_uuid(),
            project_id,
            agent_run_id,
            baseline_snapshot_ref: None,
            changed_files,
            changed_files_omitted_by_detection: 0,
            artifact_refs: Vec::new(),
            summary: summary.into(),
            detection_source: ChangeDetectionSource::ExplicitPaths,
            detection_status: ChangeDetectionStatus::Complete,
            association_confidence: ChangeAssociationConfidence::Unlinked,
            review_state: ReviewState::Unreviewed,
            created_at: created_at.clone(),
            updated_at: created_at,
        }
    }

    pub fn agent_run_detected(
        project_id: ProjectId,
        agent_run_id: AgentRunId,
        baseline_snapshot_ref: impl Into<String>,
        changed_files: Vec<PathBuf>,
        summary: impl Into<String>,
    ) -> Self {
        let mut change_set =
            Self::unreviewed(project_id, Some(agent_run_id), changed_files, summary);
        change_set.baseline_snapshot_ref = Some(baseline_snapshot_ref.into());
        change_set.association_confidence = ChangeAssociationConfidence::Strong;
        change_set
    }

    pub fn with_detection(
        mut self,
        source: ChangeDetectionSource,
        status: ChangeDetectionStatus,
    ) -> Self {
        self.detection_source = source;
        self.detection_status = status;
        self
    }

    pub fn with_association_confidence(
        mut self,
        association_confidence: ChangeAssociationConfidence,
    ) -> Self {
        self.association_confidence = association_confidence;
        if self.agent_run_id.is_none()
            && matches!(
                self.association_confidence,
                ChangeAssociationConfidence::Strong | ChangeAssociationConfidence::Weak
            )
        {
            self.association_confidence = ChangeAssociationConfidence::Unlinked;
        }
        self
    }

    pub fn with_artifact_ref(mut self, artifact_ref: impl Into<String>) -> Self {
        self.artifact_refs.push(artifact_ref.into());
        self
    }

    pub fn with_baseline_snapshot_ref(mut self, baseline_snapshot_ref: impl Into<String>) -> Self {
        self.baseline_snapshot_ref = Some(baseline_snapshot_ref.into());
        self
    }

    pub fn with_changed_files_omitted_by_detection(mut self, count: usize) -> Self {
        self.changed_files_omitted_by_detection = count;
        self
    }

    pub fn transition_review_to(
        &mut self,
        next: ReviewState,
    ) -> Result<(), ReviewStateTransitionError> {
        if can_transition_review_state(self.review_state, next) {
            self.review_state = next;
            self.updated_at = DomainTimestamp::now_utc();
            Ok(())
        } else {
            Err(ReviewStateTransitionError {
                from: self.review_state,
                to: next,
            })
        }
    }

    pub fn bounded_summary(&self, path_limit: usize) -> ChangeSetSummary {
        let path_limit = path_limit.min(self.changed_files.len());
        // RFC-035 PR-035-B: the true total, and the true omission, both
        // sum the display-level cap (`path_limit`, applied to what is
        // actually stored) with the detection-level one
        // (`changed_files_omitted_by_detection`, already excluded from
        // `changed_files` before this ever runs) -- a reader must never
        // see a lower total than what detection genuinely found.
        ChangeSetSummary {
            id: self.id.clone(),
            project_id: self.project_id.clone(),
            agent_run_id: self.agent_run_id.clone(),
            changed_file_count: self.changed_files.len() + self.changed_files_omitted_by_detection,
            shown_changed_files: self
                .changed_files
                .iter()
                .take(path_limit)
                .cloned()
                .collect(),
            omitted_changed_file_count: (self.changed_files.len() - path_limit)
                + self.changed_files_omitted_by_detection,
            artifact_ref_count: self.artifact_refs.len(),
            detection_source: self.detection_source,
            detection_status: self.detection_status,
            association_confidence: self.association_confidence,
            review_state: self.review_state,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    pub fn default_summary(&self) -> ChangeSetSummary {
        self.bounded_summary(DEFAULT_CHANGESET_PATH_SUMMARY_LIMIT)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeDetectionSource {
    GitStatus,
    FilesystemSnapshot,
    ExplicitPaths,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeDetectionStatus {
    Complete,
    Unavailable,
    Unsupported,
    Partial {
        limit: usize,
    },
    Failed {
        reason: ChangeDetectionFailureReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeDetectionFailureReason {
    CrossProjectBaseline,
    MetadataReadFailed,
    PathOutsideRoot,
    RootUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeAssociationConfidence {
    Strong,
    Weak,
    Ambiguous,
    Unlinked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeSetSummary {
    pub id: ChangeSetId,
    pub project_id: ProjectId,
    pub agent_run_id: Option<AgentRunId>,
    pub changed_file_count: usize,
    pub shown_changed_files: Vec<PathBuf>,
    pub omitted_changed_file_count: usize,
    pub artifact_ref_count: usize,
    pub detection_source: ChangeDetectionSource,
    pub detection_status: ChangeDetectionStatus,
    pub association_confidence: ChangeAssociationConfidence,
    pub review_state: ReviewState,
    pub created_at: DomainTimestamp,
    pub updated_at: DomainTimestamp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewStateTransitionError {
    pub from: ReviewState,
    pub to: ReviewState,
}

fn can_transition_review_state(from: ReviewState, to: ReviewState) -> bool {
    use ReviewState::{Accepted, PartiallyAccepted, Rejected, Superseded, Unreviewed};

    from == to
        || matches!(
            (from, to),
            (Unreviewed, Accepted)
                | (Unreviewed, PartiallyAccepted)
                | (Unreviewed, Rejected)
                | (Unreviewed, Superseded)
                | (PartiallyAccepted, Accepted)
                | (PartiallyAccepted, Rejected)
                | (PartiallyAccepted, Superseded)
                | (Accepted, Superseded)
                | (Rejected, Superseded)
        )
}
