//! RFC-049 PR-049-B: selection and cleanup.
//!
//! **Liveness is driven the way production drives it.** Runs reach
//! `Running` through `AgentRun::transition_to` — the validated transition
//! the real launch path uses — and leave it through
//! `ProjectSession::apply_agent_terminal_outcome`, the call production
//! makes when a terminal exits. No test assigns `lifecycle_state`: doing
//! so is the defect RFC-049 D8′ exists to remove, and it would pass here
//! while the product selected nothing.
//!
//! **Transcripts are production-shaped**: `byte_count` stays `0` and
//! `last_write_at` stays `None`, because that is what every real
//! transcript looks like. The one test that sets `last_write_at` does so
//! to state an *age*, not a liveness, and says so.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunId, AgentRunStatus, DomainTimestamp, TerminalId,
    TerminalKind, TerminalSession, TerminalStatus, Transcript, TranscriptId,
    TranscriptLifecycleState, TranscriptOrigin, TruncationState,
};
use crate::project::{ProjectId, ProjectSession, ProjectTranscriptError};
use crate::runtime::terminal::{BoundedRuntimeSummary, TerminationOutcome};
use crate::transcript::TranscriptRetentionLimits;

const NOW: &str = "2026-03-01T00:00:00Z";
/// Fifty-nine days before `NOW` — past a thirty-day limit.
const LONG_AGO: &str = "2026-01-01T00:00:00Z";
/// One day before `NOW` — inside any limit used here.
const YESTERDAY: &str = "2026-02-28T00:00:00Z";

#[test]
fn a_live_writer_is_not_selected_when_it_is_the_only_candidate_and_the_app_wide_budget_is_exhausted()
 {
    let dirs = TestDirs::new("live-only-candidate");
    let mut project = project_session(&dirs);
    let live = attach_running_run(&mut project, &dirs, "live", &[b'x'; 10], YESTERDAY);

    let cleanup =
        project.apply_transcript_retention(limits(100, 1_000, 1_000, 30), 5_000, &at(NOW));

    assert!(
        live.path.exists(),
        "a transcript whose run is Running must never be deleted to relieve a budget, even when \
         it is the only thing that could be freed (RFC-049 §2)"
    );
    assert_eq!(
        state_of(&project, &live.transcript_id),
        TranscriptLifecycleState::Active
    );
    assert_eq!(cleanup.budget.purged_transcripts, 0);
    assert!(
        cleanup.app_budget_exhausted,
        "the exhaustion must be reported, so PR-049-C can start the run without capture and \
         say so (RFC-049 D4′) instead of deleting"
    );
}

#[test]
fn a_detached_runs_transcript_is_not_selected() {
    let dirs = TestDirs::new("detached");
    let mut project = project_session(&dirs);
    let detached = attach_running_run(&mut project, &dirs, "detached", b"detached", LONG_AGO);
    project
        .apply_agent_terminal_outcome(
            &detached.agent_run_id,
            &detached.terminal_id,
            &TerminationOutcome::OrphanedUnknown {
                summary: BoundedRuntimeSummary::new("supervision lost"),
            },
        )
        .unwrap();
    assert_eq!(
        status_of(&project, &detached.agent_run_id),
        AgentRunStatus::Detached
    );

    project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    assert!(
        detached.path.exists(),
        "a detached process may still be writing; its transcript is live (RFC-049 D8′)"
    );
    assert_ne!(
        state_of(&project, &detached.transcript_id),
        TranscriptLifecycleState::Purged
    );
}

#[test]
fn a_completed_runs_transcript_is_selected() {
    let dirs = TestDirs::new("completed");
    let mut project = project_session(&dirs);
    let completed = attach_running_run(&mut project, &dirs, "completed", b"done", LONG_AGO);
    complete(&mut project, &completed);

    project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    assert!(
        !completed.path.exists(),
        "a Completed run is not live, so its expired transcript is removed"
    );
}

#[test]
fn a_live_transcript_past_its_age_limit_is_not_marked_expired() {
    let dirs = TestDirs::new("live-past-age");
    let mut project = project_session(&dirs);
    let live = attach_running_run(&mut project, &dirs, "live", b"still writing", LONG_AGO);

    let cleanup = project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    assert_eq!(
        state_of(&project, &live.transcript_id),
        TranscriptLifecycleState::Active,
        "production never records last_write_at, so a run writing for fifty-nine days reads as \
         fifty-nine days old; marking it Expired would be a false state on a durable record \
         (RFC-049 D8′-a)"
    );
    assert_eq!(cleanup.marked_expired, 0);
    assert!(live.path.exists());
}

#[test]
fn budget_selection_is_oldest_first_by_most_recent_activity() {
    let dirs = TestDirs::new("oldest-first");
    let mut project = project_session(&dirs);
    // Created earlier but written later: by `created_at` alone this is the
    // oldest, by most recent activity it is the newest. The timestamps
    // state an age; liveness still comes from the run's status.
    let written_recently = attach_running_run_last_written(
        &mut project,
        &dirs,
        "written",
        &[b'a'; 10],
        "2026-02-10T00:00:00Z",
        Some("2026-02-27T00:00:00Z"),
    );
    let idle_since_creation = attach_running_run(
        &mut project,
        &dirs,
        "idle",
        &[b'b'; 10],
        "2026-02-20T00:00:00Z",
    );
    complete(&mut project, &written_recently);
    complete(&mut project, &idle_since_creation);

    let cleanup = project.apply_transcript_retention(limits(15, 15, 10_000, 30), 0, &at(NOW));

    assert!(
        written_recently.path.exists(),
        "the transcript written on 02-27 is newer than the one idle since 02-20, whatever their \
         creation order"
    );
    assert!(!idle_since_creation.path.exists());
    assert_eq!(cleanup.budget.purged_transcripts, 1);
}

/// **Why this asserts attribution rather than survivors.** With one shared
/// notion of age, expiring first and relieving budgets first leave the
/// same transcripts behind in every case — an expired transcript is always
/// among the oldest, so a budget pass reaches it first either way. What the
/// order changes is **why** a transcript is recorded as removed, and that is
/// what PR-049-C writes down.
#[test]
fn a_transcript_past_its_age_is_removed_by_expiry_not_by_the_budget() {
    let dirs = TestDirs::new("expiry-before-budget");
    let mut project = project_session(&dirs);
    let expired = attach_running_run(&mut project, &dirs, "expired", &[b'e'; 10], LONG_AGO);
    let fresh = attach_running_run(&mut project, &dirs, "fresh", &[b'f'; 10], YESTERDAY);
    complete(&mut project, &expired);
    complete(&mut project, &fresh);

    let cleanup = project.apply_transcript_retention(limits(15, 15, 10_000, 30), 0, &at(NOW));

    assert_eq!(
        cleanup.expired.purged_transcripts, 1,
        "the expired transcript is removed by the expiry pass, which runs first"
    );
    assert_eq!(
        cleanup.budget.purged_transcripts, 0,
        "removing it already brought the project under budget, so the budget pass removes nothing"
    );
    assert!(fresh.path.exists());
}

#[test]
fn cleanup_goes_through_the_purge_and_leaves_a_tombstone() {
    let dirs = TestDirs::new("tombstone");
    let mut project = project_session(&dirs);
    let completed = attach_running_run(&mut project, &dirs, "completed", b"done", LONG_AGO);
    complete(&mut project, &completed);

    project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == completed.transcript_id)
        .expect(
            "RFC-033's purge keeps the record; a raw file deletion would too, but would not \
                 change it",
        );
    assert_eq!(transcript.lifecycle_state, TranscriptLifecycleState::Purged);
    assert!(transcript.is_tombstone());
    assert_eq!(
        transcript.storage_path,
        PathBuf::new(),
        "the tombstone is content-free: it no longer names where the bytes were"
    );
}

/// **Composition, disclosed — three tests, not one.** Ablating the byte
/// source to `Transcript.byte_count` fails this test,
/// `budget_selection_is_oldest_first_by_most_recent_activity`, and
/// `a_failed_budget_deletion_stops_budget_cleanup_rather_than_deleting_something_newer`.
/// Every budget test here is production-shaped, and every real
/// transcript's `byte_count` is `0`, so with the tracked count no budget is
/// ever over its limit and budget cleanup never runs at all. Isolating this
/// test would mean giving the others a `byte_count` production never
/// writes. This comment first named only the oldest-first test; the
/// ablation found the third.
#[test]
fn budget_pressure_is_measured_from_the_files_not_the_tracked_byte_count() {
    let dirs = TestDirs::new("real-bytes");
    let mut project = project_session(&dirs);
    let completed = attach_running_run(&mut project, &dirs, "big", &[b'z'; 100], YESTERDAY);
    complete(&mut project, &completed);
    assert_eq!(byte_count_of(&project, &completed.transcript_id), 0);

    let cleanup = project.apply_transcript_retention(limits(50, 50, 10_000, 30), 0, &at(NOW));

    assert!(
        !completed.path.exists(),
        "100 real bytes are over a 50-byte project budget, even though byte_count reads 0 (D8′-b)"
    );
    assert_eq!(cleanup.budget.bytes_removed, 100);
}

#[test]
fn a_transcript_with_no_agent_run_is_never_selected() {
    let dirs = TestDirs::new("no-run");
    let mut project = project_session(&dirs);
    let orphan = attach_transcript_without_run(&mut project, &dirs, "orphan", b"who", LONG_AGO);

    project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    assert!(
        orphan.exists(),
        "liveness that cannot be determined is live (§1); production always links a run, which \
         is exactly why the unlinked case must not be the one that deletes"
    );
}

#[test]
fn a_failed_budget_deletion_stops_budget_cleanup_rather_than_deleting_something_newer() {
    let dirs = TestDirs::new("stop-on-failure");
    let mut project = project_session(&dirs);
    let undeletable = attach_running_run(
        &mut project,
        &dirs,
        "undeletable",
        &[b'u'; 10],
        "2026-02-20T00:00:00Z",
    );
    // A directory where the file should be: RFC-033's purge refuses it.
    fs::remove_file(&undeletable.path).unwrap();
    fs::create_dir_all(undeletable.path.join("not-a-transcript")).unwrap();
    let newer = attach_running_run(&mut project, &dirs, "newer", &[b'n'; 10], YESTERDAY);
    complete(&mut project, &undeletable);
    complete(&mut project, &newer);

    let cleanup = project.apply_transcript_retention(limits(1, 1, 10_000, 30), 0, &at(NOW));

    assert!(matches!(
        cleanup.failures.as_slice(),
        [ProjectTranscriptError::StoragePathIsDirectory { .. }]
    ));
    assert!(
        newer.path.exists(),
        "an older transcript that cannot be removed does not make a newer one eligible in its place"
    );
    assert!(cleanup.project_budget_exhausted);
}

/// RFC-049 PR-049-C (response 385, decision 5): PR-049-B established that a
/// transcript saved by a raised limit survives. What nothing held is that it
/// stops claiming to be expired: production never moved a transcript **out of**
/// `Expired`, so a durable, false state survived every later trigger.
///
/// The only way to reach a stale mark is the one this test builds: a deletion
/// that failed, then a limit raised past the transcript's age.
#[test]
fn a_stale_expired_mark_is_cleared_when_the_limit_is_raised() {
    let dirs = TestDirs::new("stale-expired-mark");
    let mut project = project_session(&dirs);
    let undeletable = attach_running_run(&mut project, &dirs, "undeletable", &[b'u'; 10], LONG_AGO);
    // A directory where the file should be: RFC-033's purge refuses it, so the
    // mark outlives the pass that made it.
    fs::remove_file(&undeletable.path).unwrap();
    fs::create_dir_all(undeletable.path.join("not-a-transcript")).unwrap();
    complete(&mut project, &undeletable);

    let marked = project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    assert_eq!(marked.marked_expired, 1);
    // The precondition is asserted on `failures` itself, not through
    // `a_deletion_failed()`: that predicate has its own test, and reading it
    // here would make an ablation of it fail this test too.
    assert!(!marked.failures.is_empty(), "the deletion must have failed");
    assert_eq!(
        state_of(&project, &undeletable.transcript_id),
        TranscriptLifecycleState::Expired,
        "a transcript whose deletion failed stays marked"
    );

    // The user raises the limit past this transcript's age.
    let cleared =
        project.apply_transcript_retention(limits(100, 1_000, 10_000, 10_000), 0, &at(NOW));

    assert_eq!(cleared.cleared_expired_marks, 1);
    assert_eq!(
        state_of(&project, &undeletable.transcript_id),
        TranscriptLifecycleState::Active,
        "a mark that is no longer true is cleared, not merely ignored"
    );
    assert!(
        undeletable.path.exists(),
        "clearing a mark deletes nothing (PR-049-B's decision 5: it survives)"
    );
    assert_eq!(
        cleared.marked_expired, 0,
        "and it is not marked again in the same pass"
    );
}

/// RFC-049 PR-049-C (response 385): the budget stays exhausted in two different
/// situations, and a user's remedy differs. This is the half with a remedy —
/// **a deletion failed**, and the file can be looked at.
#[test]
fn a_budget_left_exhausted_by_a_failed_deletion_says_a_deletion_failed() {
    let dirs = TestDirs::new("exhausted-by-failure");
    let mut project = project_session(&dirs);
    let undeletable = attach_running_run(
        &mut project,
        &dirs,
        "undeletable",
        &[b'u'; 100],
        "2026-02-20T00:00:00Z",
    );
    fs::remove_file(&undeletable.path).unwrap();
    fs::create_dir_all(undeletable.path.join("not-a-transcript")).unwrap();
    complete(&mut project, &undeletable);

    let cleanup = project.apply_transcript_retention(limits(1, 1, 10_000, 10_000), 0, &at(NOW));

    assert!(cleanup.budget_still_exhausted());
    assert!(
        cleanup.a_deletion_failed(),
        "one undeletable file keeps the budget exhausted on every trigger, and that has a remedy"
    );
}

/// The other half: **nothing is deletable**, because the only candidate is a
/// live writer (§2). The budget is equally exhausted, and there is nothing the
/// user can do but wait — so the disclosure must not say a deletion failed.
#[test]
fn a_budget_left_exhausted_by_a_live_writer_says_no_deletion_failed() {
    let dirs = TestDirs::new("exhausted-by-live-writer");
    let mut project = project_session(&dirs);
    let _running = attach_running_run(&mut project, &dirs, "running", &[b'r'; 100], YESTERDAY);

    let cleanup = project.apply_transcript_retention(limits(1, 1, 10_000, 10_000), 0, &at(NOW));

    assert!(cleanup.budget_still_exhausted());
    assert!(
        !cleanup.a_deletion_failed(),
        "a live writer that was never selected is not a failed deletion"
    );
}

/// RFC-049 §4's measurement: marking and clearing are not removals, so a pass
/// that only did those writes no audit record.
#[test]
fn a_cleanup_that_only_marked_a_transcript_removed_nothing() {
    let dirs = TestDirs::new("marked-only");
    let mut project = project_session(&dirs);
    let undeletable = attach_running_run(&mut project, &dirs, "undeletable", &[b'u'; 10], LONG_AGO);
    fs::remove_file(&undeletable.path).unwrap();
    fs::create_dir_all(undeletable.path.join("not-a-transcript")).unwrap();
    complete(&mut project, &undeletable);

    let cleanup = project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    assert_eq!(cleanup.marked_expired, 1);
    assert!(
        !cleanup.removed_anything(),
        "a mark is not a removal, and §4's record must not be written for one"
    );
}

/// RFC-050 PR-050-A: liveness matches on **origin** first. Nothing loads a
/// found record yet (PR-050-B does); these are built the way the loader will
/// build them, with no terminal and no run.
#[test]
fn a_found_transcript_whose_lock_was_held_at_load_is_never_touched() {
    let dirs = TestDirs::new("found-locked");
    let mut project = project_session(&dirs);
    let path = write_transcript_file(&dirs, "found-locked", b"still being written elsewhere");
    project
        .add_transcript(found_on_disk(&project, &path, LONG_AGO, true))
        .unwrap();

    let cleanup = project.apply_transcript_retention(limits(1, 1, 10_000, 30), 0, &at(NOW));

    assert!(
        path.exists(),
        "a lock held at load means some process may still be writing (RFC-050 D3)"
    );
    assert_eq!(
        cleanup.expired.purged_transcripts + cleanup.budget.purged_transcripts,
        0
    );
}

#[test]
fn a_found_transcript_whose_lock_was_free_at_load_is_retained_like_a_finished_one() {
    let dirs = TestDirs::new("found-free");
    let mut project = project_session(&dirs);
    let path = write_transcript_file(&dirs, "found-free", b"left by an earlier process");
    project
        .add_transcript(found_on_disk(&project, &path, LONG_AGO, false))
        .unwrap();

    project.apply_transcript_retention(limits(100, 1_000, 10_000, 30), 0, &at(NOW));

    assert!(
        !path.exists(),
        "a file an earlier process left has no writer in this one, so its age applies"
    );
}

/// A record as PR-050-B's loader will build it: its own id, its project,
/// its path and mtime, and **no terminal or run** — the origin carries none.
fn found_on_disk(
    project: &ProjectSession,
    path: &Path,
    last_write_at: &str,
    writer_held_lock_at_load: bool,
) -> Transcript {
    Transcript {
        id: TranscriptId::new_uuid(),
        project_id: project.id().clone(),
        origin: TranscriptOrigin::FoundOnDisk {
            writer_held_lock_at_load,
        },
        storage_path: path.to_path_buf(),
        byte_count: 0,
        truncation_state: TruncationState::Complete,
        lifecycle_state: TranscriptLifecycleState::Active,
        retention_policy: "local-bounded-agent-run".to_owned(),
        created_at: at(last_write_at),
        last_write_at: Some(at(last_write_at)),
    }
}

/// RFC-050 D3′: the purge dialog says a transcript it would delete may belong to
/// a run still in progress. The count comes from the run's real status, through
/// the same conservative predicate deletion uses (response 394, F3).
#[test]
fn a_purgeable_transcript_of_a_running_run_is_counted() {
    let dirs = TestDirs::new("purge-running-run");
    let mut project = project_session(&dirs);
    attach_running_run(&mut project, &dirs, "running", b"still writing", YESTERDAY);

    assert_eq!(project.purgeable_transcripts_of_running_runs_count(), 1);
}

#[test]
fn a_completed_runs_transcript_is_not_counted_as_running() {
    let dirs = TestDirs::new("purge-completed-run");
    let mut project = project_session(&dirs);
    let run = attach_running_run(&mut project, &dirs, "completed", b"done", YESTERDAY);
    complete(&mut project, &run);

    assert_eq!(project.purgeable_transcripts_of_running_runs_count(), 0);
}

struct AttachedRun {
    terminal_id: TerminalId,
    agent_run_id: AgentRunId,
    transcript_id: TranscriptId,
    path: PathBuf,
}

fn attach_running_run(
    project: &mut ProjectSession,
    dirs: &TestDirs,
    name: &str,
    bytes: &[u8],
    created_at: &str,
) -> AttachedRun {
    attach_running_run_last_written(project, dirs, name, bytes, created_at, None)
}

fn attach_running_run_last_written(
    project: &mut ProjectSession,
    dirs: &TestDirs,
    name: &str,
    bytes: &[u8],
    created_at: &str,
    last_write_at: Option<&str>,
) -> AttachedRun {
    let path = write_transcript_file(dirs, name, bytes);

    let mut terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Supervised,
        "Agent",
        &dirs.project_root,
        "agent-cli",
    );
    terminal.transition_to(TerminalStatus::Running).unwrap();

    let mut run = AgentRun::draft(
        project.id().clone(),
        "agent",
        "prompt summary",
        AgentCompatibilityLevel::Supervised,
    );
    for status in [
        AgentRunStatus::Ready,
        AgentRunStatus::Preparing,
        AgentRunStatus::Running,
    ] {
        run.transition_to(status).unwrap();
    }
    run.attach_terminal(&terminal).unwrap();

    let mut transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        Some(run.id.clone()),
        &path,
        "local-bounded-agent-run",
    );
    transcript.created_at = at(created_at);
    transcript.last_write_at = last_write_at.map(at);
    terminal.transcript_ref = Some(transcript.id.clone());
    run.transcript_ref = Some(transcript.id.clone());

    let attached = AttachedRun {
        terminal_id: terminal.id.clone(),
        agent_run_id: run.id.clone(),
        transcript_id: transcript.id.clone(),
        path,
    };
    project.add_terminal_session(terminal).unwrap();
    project.add_agent_run(run).unwrap();
    project.add_transcript(transcript).unwrap();
    attached
}

fn attach_transcript_without_run(
    project: &mut ProjectSession,
    dirs: &TestDirs,
    name: &str,
    bytes: &[u8],
    created_at: &str,
) -> PathBuf {
    let path = write_transcript_file(dirs, name, bytes);
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Supervised,
        "Agent",
        &dirs.project_root,
        "agent-cli",
    );
    let mut transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        None,
        &path,
        "local-bounded-agent-run",
    );
    transcript.created_at = at(created_at);
    project.add_terminal_session(terminal).unwrap();
    project.add_transcript(transcript).unwrap();
    path
}

/// The terminal exiting cleanly, applied the way production applies it.
fn complete(project: &mut ProjectSession, attached: &AttachedRun) {
    project
        .apply_agent_terminal_outcome(
            &attached.agent_run_id,
            &attached.terminal_id,
            &TerminationOutcome::Exited { exit_status: 0 },
        )
        .unwrap();
    assert_eq!(
        status_of(project, &attached.agent_run_id),
        AgentRunStatus::Completed
    );
}

fn state_of(project: &ProjectSession, id: &TranscriptId) -> TranscriptLifecycleState {
    project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == *id)
        .unwrap()
        .lifecycle_state
}

fn byte_count_of(project: &ProjectSession, id: &TranscriptId) -> u64 {
    project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == *id)
        .unwrap()
        .byte_count
}

fn status_of(project: &ProjectSession, id: &AgentRunId) -> AgentRunStatus {
    project
        .agent_runs()
        .iter()
        .find(|run| run.id == *id)
        .unwrap()
        .status
}

fn limits(transcript: u64, project: u64, app: u64, days: u32) -> TranscriptRetentionLimits {
    let limits = TranscriptRetentionLimits::new(transcript, project, app, days);
    assert!(
        limits.is_bounded(),
        "a fixture limit that is not bounded would clean nothing"
    );
    limits
}

fn at(value: &str) -> DomainTimestamp {
    DomainTimestamp::from_utc_string(value).unwrap()
}

fn write_transcript_file(dirs: &TestDirs, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dirs
        .state_root
        .join("transcripts")
        .join(name)
        .join("transcript.log");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

fn project_session(dirs: &TestDirs) -> ProjectSession {
    ProjectSession::new(
        ProjectId::for_test(1),
        "Retention",
        &dirs.project_root,
        fs::canonicalize(&dirs.project_root).unwrap(),
    )
}

struct TestDirs {
    project_root: PathBuf,
    state_root: PathBuf,
}

impl TestDirs {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("tekstide-retention-{name}-{nanos}"));
        let base = crate::test_support::remove_when_this_test_ends(base);
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
