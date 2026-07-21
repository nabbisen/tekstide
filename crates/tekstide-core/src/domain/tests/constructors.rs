use crate::domain::{
    AgentRunId, ChangeAssociationConfidence, ChangeDetectionSource, ChangeDetectionStatus,
    ChangeSet, ReviewState, ReviewStateTransitionError, TerminalId, Transcript,
    TranscriptLifecycleState, TruncationState,
};
use crate::project::ProjectId;

#[test]
fn transcript_metadata_constructor_sets_safe_defaults_without_writing_bytes() {
    let transcript = Transcript::metadata(
        ProjectId::for_test(1),
        TerminalId::for_test(1),
        Some(AgentRunId::for_test(1)),
        "/state/transcripts/run.log",
        "bounded-default",
    );

    assert_eq!(transcript.byte_count, 0);
    assert_eq!(transcript.truncation_state, TruncationState::Complete);
    assert_eq!(transcript.lifecycle_state, TranscriptLifecycleState::Active);
    assert_eq!(transcript.retention_policy, "bounded-default");
    assert!(transcript.last_write_at.is_none());
}

#[test]
fn changeset_constructor_starts_unreviewed() {
    let changeset = ChangeSet::unreviewed(
        ProjectId::for_test(1),
        Some(AgentRunId::for_test(1)),
        vec!["src/main.rs".into()],
        "one generated change",
    );

    assert_eq!(changeset.review_state, ReviewState::Unreviewed);
    assert_eq!(
        changeset.detection_source,
        ChangeDetectionSource::ExplicitPaths
    );
    assert_eq!(changeset.detection_status, ChangeDetectionStatus::Complete);
    assert_eq!(
        changeset.association_confidence,
        ChangeAssociationConfidence::Unlinked
    );
    assert!(changeset.artifact_refs.is_empty());
    assert_eq!(changeset.changed_files.len(), 1);
    assert!(changeset.created_at.as_str().ends_with('Z'));
    assert_eq!(changeset.updated_at, changeset.created_at);
}

#[test]
fn changeset_agent_run_constructor_records_strong_association() {
    let agent_run_id = AgentRunId::for_test(1);
    let changeset = ChangeSet::agent_run_detected(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        "baseline-1",
        vec!["src/main.rs".into()],
        "one generated change",
    )
    .with_detection(
        ChangeDetectionSource::FilesystemSnapshot,
        ChangeDetectionStatus::Partial { limit: 64 },
    )
    .with_artifact_ref("artifact:report");

    assert_eq!(changeset.agent_run_id, Some(agent_run_id));
    assert_eq!(
        changeset.baseline_snapshot_ref,
        Some("baseline-1".to_owned())
    );
    assert_eq!(
        changeset.association_confidence,
        ChangeAssociationConfidence::Strong
    );
    assert_eq!(
        changeset.detection_source,
        ChangeDetectionSource::FilesystemSnapshot
    );
    assert_eq!(
        changeset.detection_status,
        ChangeDetectionStatus::Partial { limit: 64 }
    );
    assert_eq!(changeset.artifact_refs, vec!["artifact:report".to_owned()]);
}

#[test]
fn changeset_bounded_summary_reports_metadata_without_content() {
    let changeset = ChangeSet::unreviewed(
        ProjectId::for_test(1),
        Some(AgentRunId::for_test(1)),
        vec![
            "src/main.rs".into(),
            "src/lib.rs".into(),
            "README.md".into(),
        ],
        "secret file contents in caller summary",
    )
    .with_artifact_ref("artifact:secret-reference");

    let summary = changeset.bounded_summary(2);

    assert_eq!(summary.changed_file_count, 3);
    assert_eq!(summary.shown_changed_files.len(), 2);
    assert_eq!(summary.omitted_changed_file_count, 1);
    assert_eq!(summary.artifact_ref_count, 1);
    let debug_summary = format!("{summary:?}");
    assert!(!debug_summary.contains("secret file contents"));
    assert!(!debug_summary.contains("artifact:secret-reference"));
}

#[test]
fn changeset_review_transitions_are_explicit_and_non_destructive() {
    let mut changeset = ChangeSet::unreviewed(
        ProjectId::for_test(1),
        Some(AgentRunId::for_test(1)),
        vec!["src/main.rs".into()],
        "one generated change",
    );
    let created_at = changeset.created_at.clone();

    changeset
        .transition_review_to(ReviewState::PartiallyAccepted)
        .unwrap();
    assert_eq!(changeset.review_state, ReviewState::PartiallyAccepted);
    assert!(changeset.updated_at.as_str() >= created_at.as_str());

    changeset
        .transition_review_to(ReviewState::Accepted)
        .unwrap();
    assert_eq!(changeset.review_state, ReviewState::Accepted);

    assert_eq!(
        changeset
            .transition_review_to(ReviewState::Unreviewed)
            .unwrap_err(),
        ReviewStateTransitionError {
            from: ReviewState::Accepted,
            to: ReviewState::Unreviewed,
        }
    );

    changeset
        .transition_review_to(ReviewState::Superseded)
        .unwrap();
    assert_eq!(changeset.review_state, ReviewState::Superseded);
}

#[test]
fn terminal_review_outcomes_can_be_superseded_by_later_detection() {
    let mut accepted = ChangeSet::unreviewed(
        ProjectId::for_test(1),
        Some(AgentRunId::for_test(1)),
        vec!["src/main.rs".into()],
        "accepted change",
    );
    accepted
        .transition_review_to(ReviewState::Accepted)
        .unwrap();
    accepted
        .transition_review_to(ReviewState::Superseded)
        .unwrap();
    assert_eq!(accepted.review_state, ReviewState::Superseded);

    let mut rejected = ChangeSet::unreviewed(
        ProjectId::for_test(1),
        Some(AgentRunId::for_test(1)),
        vec!["src/lib.rs".into()],
        "rejected change",
    );
    rejected
        .transition_review_to(ReviewState::Rejected)
        .unwrap();
    rejected
        .transition_review_to(ReviewState::Superseded)
        .unwrap();
    assert_eq!(rejected.review_state, ReviewState::Superseded);
}
