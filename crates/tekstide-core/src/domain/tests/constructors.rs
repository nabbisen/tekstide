use crate::domain::{AgentRunId, ChangeSet, ReviewState, TerminalId, Transcript};
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
    assert_eq!(changeset.changed_files.len(), 1);
    assert!(changeset.created_at.as_str().ends_with('Z'));
}
