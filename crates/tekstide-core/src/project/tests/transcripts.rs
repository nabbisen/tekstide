use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{
    AgentCompatibilityLevel, AgentRun, TerminalKind, TerminalSession, Transcript,
    TranscriptLifecycleState, TruncationState,
};
use crate::project::{
    ProjectId, ProjectSession, ProjectTranscriptError, ProjectTranscriptPurgeSummary,
};
use crate::transcript::{
    TranscriptBudgetScope, TranscriptRetentionLimits, TranscriptRetentionState,
    TranscriptWriteSummary,
};

#[test]
fn terminal_writer_summary_updates_transcript_metadata_without_content() {
    let temp = TestDirs::new("metadata-summary");
    let mut project = real_project_session(1, &temp);
    let (terminal_id, _agent_run_id, transcript_id, _path) =
        attach_agent_run_transcript(&mut project, &temp, "summary.log", b"secret bytes");

    project
        .record_terminal_transcript_write_summary(
            &terminal_id,
            TranscriptWriteSummary {
                byte_count: 42,
                retention_state: TranscriptRetentionState::Truncated {
                    scope: TranscriptBudgetScope::Transcript,
                },
            },
        )
        .unwrap();

    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == transcript_id)
        .unwrap();
    assert_eq!(transcript.byte_count, 42);
    assert_eq!(transcript.truncation_state, TruncationState::Truncated);
    assert_eq!(
        transcript.lifecycle_state,
        TranscriptLifecycleState::Truncated
    );
    assert!(transcript.last_write_at.is_some());
    assert!(!format!("{transcript:?}").contains("secret bytes"));
}

#[test]
fn transcript_purge_removes_bytes_and_preserves_path_free_tombstone_references() {
    let temp = TestDirs::new("purge-one");
    let mut project = real_project_session(1, &temp);
    let (terminal_id, agent_run_id, transcript_id, transcript_path) =
        attach_agent_run_transcript(&mut project, &temp, "run.log", b"secret transcript");

    let summary = project.purge_transcript(&transcript_id).unwrap();

    assert_eq!(
        summary,
        ProjectTranscriptPurgeSummary {
            requested_transcripts: 1,
            purged_transcripts: 1,
            bytes_removed: 17,
            tombstones_preserved: 1,
        }
    );
    assert!(!transcript_path.exists());

    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == transcript_id)
        .unwrap();
    assert!(transcript.is_tombstone());
    assert!(transcript.storage_path.as_os_str().is_empty());
    assert_eq!(transcript.byte_count, 0);
    assert_eq!(
        project
            .terminal_session(&terminal_id)
            .unwrap()
            .transcript_ref,
        Some(transcript_id.clone())
    );
    assert_eq!(
        project
            .agent_runs()
            .iter()
            .find(|run| run.id == agent_run_id)
            .unwrap()
            .transcript_ref,
        Some(transcript_id)
    );
}

#[test]
fn transcript_purge_is_idempotent_when_bytes_are_absent() {
    let temp = TestDirs::new("purge-idempotent");
    let mut project = real_project_session(1, &temp);
    let (_terminal_id, _agent_run_id, transcript_id, transcript_path) =
        attach_agent_run_transcript(&mut project, &temp, "run.log", b"secret transcript");
    fs::remove_file(&transcript_path).unwrap();

    let first = project.purge_transcript(&transcript_id).unwrap();
    let second = project.purge_transcript(&transcript_id).unwrap();

    assert_eq!(first.requested_transcripts, 1);
    assert_eq!(first.purged_transcripts, 1);
    assert_eq!(first.bytes_removed, 0);
    assert_eq!(second.requested_transcripts, 1);
    assert_eq!(second.purged_transcripts, 0);
    assert_eq!(second.tombstones_preserved, 1);
}

#[test]
fn agent_run_purge_removes_only_related_transcripts() {
    let temp = TestDirs::new("purge-agent");
    let mut project = real_project_session(1, &temp);
    let (_terminal_id, agent_run_id, first_id, first_path) =
        attach_agent_run_transcript(&mut project, &temp, "first.log", b"first");
    let (_terminal_id, _other_agent_run_id, second_id, second_path) =
        attach_agent_run_transcript(&mut project, &temp, "second.log", b"second");

    let summary = project.purge_agent_run_transcripts(&agent_run_id).unwrap();

    assert_eq!(summary.purged_transcripts, 1);
    assert!(!first_path.exists());
    assert!(second_path.exists());
    assert!(
        project
            .transcripts()
            .iter()
            .find(|transcript| transcript.id == first_id)
            .unwrap()
            .is_tombstone()
    );
    assert!(
        !project
            .transcripts()
            .iter()
            .find(|transcript| transcript.id == second_id)
            .unwrap()
            .is_tombstone()
    );
}

#[test]
fn project_purge_removes_all_project_transcript_bytes() {
    let temp = TestDirs::new("purge-project");
    let mut project = real_project_session(1, &temp);
    let (_terminal_id, _agent_run_id, _first_id, first_path) =
        attach_agent_run_transcript(&mut project, &temp, "first.log", b"first");
    let (_terminal_id, _agent_run_id, _second_id, second_path) =
        attach_agent_run_transcript(&mut project, &temp, "second.log", b"second");

    let summary = project.purge_project_transcripts().unwrap();

    assert_eq!(summary.requested_transcripts, 2);
    assert_eq!(summary.purged_transcripts, 2);
    assert_eq!(summary.bytes_removed, 11);
    assert!(!first_path.exists());
    assert!(!second_path.exists());
    assert!(project.transcripts().iter().all(Transcript::is_tombstone));
}

#[test]
fn transcript_purge_never_deletes_project_files() {
    let temp = TestDirs::new("purge-project-file");
    let mut project = real_project_session(1, &temp);
    let project_file = temp.project_root.join("src.log");
    fs::write(&project_file, b"must stay").unwrap();

    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Plain,
        "Shell",
        &temp.project_root,
        "bash",
    );
    let transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        None,
        project_file.clone(),
        "local-bounded-agent-run",
    );
    let transcript_id = transcript.id.clone();
    project.add_terminal_session(terminal).unwrap();
    project.add_transcript(transcript).unwrap();

    let error = project
        .purge_transcript(&transcript_id)
        .expect_err("project-local transcript path must not be deleted");

    assert!(matches!(
        error,
        ProjectTranscriptError::UnsafeProjectPath { .. }
    ));
    assert_eq!(fs::read(&project_file).unwrap(), b"must stay");
    assert!(
        !project
            .transcripts()
            .iter()
            .find(|transcript| transcript.id == transcript_id)
            .unwrap()
            .is_tombstone()
    );
}

#[test]
fn local_data_summary_counts_retained_bytes_without_transcript_content() {
    let temp = TestDirs::new("local-data-summary");
    let mut project = real_project_session(1, &temp);
    let (_terminal_id, _agent_run_id, first_id, _first_path) =
        attach_agent_run_transcript(&mut project, &temp, "first.log", b"secret-a");
    let (_terminal_id, _agent_run_id, _second_id, _second_path) =
        attach_agent_run_transcript(&mut project, &temp, "second.log", b"secret-bb");
    project
        .record_transcript_write_summary(
            &first_id,
            TranscriptWriteSummary {
                byte_count: 8,
                retention_state: TranscriptRetentionState::Active,
            },
        )
        .unwrap();
    project.purge_transcript(&first_id).unwrap();

    let summary =
        project.transcript_local_data_summary(128, TranscriptRetentionLimits::new(64, 64, 127, 30));

    assert_eq!(summary.project_retained_bytes, 9);
    assert_eq!(summary.project_transcript_count, 2);
    assert_eq!(summary.budget_pressure, Some(TranscriptBudgetScope::App));
    assert!(!format!("{summary:?}").contains("secret"));
}

fn attach_agent_run_transcript(
    project: &mut ProjectSession,
    temp: &TestDirs,
    filename: &str,
    bytes: &[u8],
) -> (
    crate::domain::TerminalId,
    crate::domain::AgentRunId,
    crate::domain::TranscriptId,
    PathBuf,
) {
    let transcript_path = temp.state_root.join("transcripts").join(filename);
    fs::create_dir_all(transcript_path.parent().unwrap()).unwrap();
    fs::write(&transcript_path, bytes).unwrap();

    let mut terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Supervised,
        "Agent",
        &temp.project_root,
        "agent-cli",
    );
    let mut run = AgentRun::draft(
        project.id().clone(),
        "agent",
        "prompt summary",
        AgentCompatibilityLevel::Supervised,
    );
    run.attach_terminal(&terminal).unwrap();
    let mut transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        Some(run.id.clone()),
        &transcript_path,
        "local-bounded-agent-run",
    );
    transcript.record_active_write(bytes.len() as u64);
    let transcript_id = transcript.id.clone();
    terminal.transcript_ref = Some(transcript_id.clone());
    run.transcript_ref = Some(transcript_id.clone());
    let terminal_id = terminal.id.clone();
    let agent_run_id = run.id.clone();

    project.add_terminal_session(terminal).unwrap();
    project.add_agent_run(run).unwrap();
    project.add_transcript(transcript).unwrap();

    (terminal_id, agent_run_id, transcript_id, transcript_path)
}

fn real_project_session(sequence: u64, temp: &TestDirs) -> ProjectSession {
    ProjectSession::new(
        ProjectId::for_test(sequence),
        format!("Project {sequence}"),
        &temp.project_root,
        fs::canonicalize(&temp.project_root).unwrap(),
    )
}

struct TestDirs {
    project_root: PathBuf,
    state_root: PathBuf,
}

impl TestDirs {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "tekstide-project-transcripts-{name}-{}",
            unique_suffix()
        ));
        let project_root = base.join("project");
        let state_root = base.join("state");
        fs::create_dir_all(&project_root).unwrap();
        fs::create_dir_all(&state_root).unwrap();
        Self {
            project_root,
            state_root,
        }
    }
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
