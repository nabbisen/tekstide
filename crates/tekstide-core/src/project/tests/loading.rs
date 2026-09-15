//! RFC-050 PR-050-B: loading transcripts that earlier runs left on disk.
//!
//! Every fixture is a fresh temporary directory laid out exactly as the
//! product writes, `<state>/transcripts/<project_id>/agent-run-<uuid>/transcript.log`.
//! **No test here reads the real state root** (§5).

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{AgentRunId, TerminalKind, TerminalSession, Transcript, TranscriptOrigin};
use crate::project::{ProjectId, ProjectSession};
use crate::transcript::scan_transcript_disk_usage;

#[test]
fn a_found_transcript_whose_lock_is_held_loads_as_live_and_survives_purge() {
    let dirs = TestDirs::new("found-held");
    let mut project = project_session(&dirs, 1);
    let path = write_run(
        &dirs,
        project.id(),
        &AgentRunId::new_uuid(),
        b"still being written",
    );
    let holder = fs::File::open(&path).unwrap();
    holder.try_lock().unwrap();

    let loaded = project.load_transcripts_from_disk(&dirs.state_root);
    assert_eq!(loaded.loaded, 1);
    assert!(matches!(
        project.transcripts()[0].origin,
        TranscriptOrigin::FoundOnDisk {
            writer_held_lock_at_load: true
        }
    ));

    let purged = project.purge_project_transcripts().unwrap();

    assert!(
        path.exists(),
        "a file another handle is still writing must never be unlinked under it (RFC-050 D3)"
    );
    assert_eq!(purged.skipped_still_being_written, 1);
    assert_eq!(purged.purged_transcripts, 0);
    assert_eq!(project.transcripts_still_being_written_count(), 1);
    assert_eq!(
        project.purgeable_transcript_count(),
        0,
        "the dialog must not count what purge will skip"
    );
    drop(holder);
}

#[test]
fn the_load_probe_releases_the_lock_it_took() {
    let dirs = TestDirs::new("probe-releases");
    let mut project = project_session(&dirs, 1);
    let path = write_run(
        &dirs,
        project.id(),
        &AgentRunId::new_uuid(),
        b"left by an earlier run",
    );

    project.load_transcripts_from_disk(&dirs.state_root);

    assert!(
        fs::File::open(&path).unwrap().try_lock().is_ok(),
        "the probe takes the lock and drops it at once; a later lock must succeed"
    );
    assert!(matches!(
        project.transcripts()[0].origin,
        TranscriptOrigin::FoundOnDisk {
            writer_held_lock_at_load: false
        }
    ));
}

#[test]
fn a_found_transcript_takes_its_age_from_its_mtime() {
    let dirs = TestDirs::new("found-age");
    let mut project = project_session(&dirs, 1);
    let path = write_run(&dirs, project.id(), &AgentRunId::new_uuid(), b"aged");
    let mtime = fs::metadata(&path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    project.load_transcripts_from_disk(&dirs.state_root);

    let transcript = &project.transcripts()[0];
    assert_eq!(
        transcript
            .last_write_at
            .as_ref()
            .and_then(|at| at.unix_seconds()),
        Some(mtime)
    );
    assert_eq!(
        transcript.created_at.unix_seconds(),
        Some(0),
        "created_at is no later than the true creation time, so age comes from the mtime"
    );
}

#[test]
fn every_unrecognised_entry_is_skipped_and_still_present_after_purge() {
    let dirs = TestDirs::new("skipped");
    let mut project = project_session(&dirs, 1);
    let project_dir = dirs
        .state_root
        .join("transcripts")
        .join(project.id().as_str());

    // A symlinked run directory pointing at a file outside the state root.
    let outside = dirs.base.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("transcript.log");
    fs::write(
        &outside_file,
        b"the user's own file, outside the state root",
    )
    .unwrap();
    fs::create_dir_all(&project_dir).unwrap();
    std::os::unix::fs::symlink(&outside, project_dir.join(AgentRunId::new_uuid().as_str()))
        .unwrap();

    // A non-UUID run directory name.
    let not_uuid = project_dir
        .join("agent-run-not-a-uuid")
        .join("transcript.log");
    fs::create_dir_all(not_uuid.parent().unwrap()).unwrap();
    fs::write(&not_uuid, b"not ours").unwrap();

    // A stray file in the project directory.
    let stray = project_dir.join("stray.txt");
    fs::write(&stray, b"stray").unwrap();

    // A transcript.log one level too deep.
    let too_deep = project_dir
        .join(AgentRunId::new_uuid().as_str())
        .join("nested")
        .join("transcript.log");
    fs::create_dir_all(too_deep.parent().unwrap()).unwrap();
    fs::write(&too_deep, b"too deep").unwrap();

    let loaded = project.load_transcripts_from_disk(&dirs.state_root);
    project.purge_project_transcripts().unwrap();

    assert_eq!(
        loaded.loaded, 0,
        "nothing here is a transcript this product wrote"
    );
    assert!(loaded.skipped_entries >= 4);
    for survivor in [&outside_file, &not_uuid, &stray, &too_deep] {
        assert!(
            survivor.exists(),
            "skipped and never deleted: {}",
            survivor.display()
        );
    }
}

#[test]
fn run_directories_in_other_uuid_spellings_are_skipped_and_still_present() {
    let dirs = TestDirs::new("spellings");
    let mut project = project_session(&dirs, 1);
    let uuid = "0f8e4c2a-7b1d-4e3f-9a6b-2c5d8e1f4a7b";
    let project_dir = dirs
        .state_root
        .join("transcripts")
        .join(project.id().as_str());

    let spelled = |suffix: String| {
        let path = project_dir
            .join(format!("agent-run-{suffix}"))
            .join("transcript.log");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"a name this product never writes").unwrap();
        path
    };
    let others = [
        spelled(uuid.to_uppercase()),
        spelled(uuid.replace('-', "")),
        spelled(format!("{{{uuid}}}")),
        spelled(format!("urn:uuid:{uuid}")),
    ];
    let ours = spelled(uuid.to_owned());

    let loaded = project.load_transcripts_from_disk(&dirs.state_root);
    project.purge_project_transcripts().unwrap();

    assert_eq!(
        loaded.loaded, 1,
        "only the lowercase hyphenated spelling is this product's"
    );
    assert!(
        !ours.exists(),
        "the product's own spelling is loaded and purged"
    );
    for other in &others {
        assert!(
            other.exists(),
            "skipped and never deleted: {}",
            other.display()
        );
    }
}

#[test]
fn an_unclaimed_project_directory_is_not_loaded_or_deleted_and_is_counted() {
    let dirs = TestDirs::new("unclaimed");
    let mut project = project_session(&dirs, 1);
    let ours = write_run(&dirs, project.id(), &AgentRunId::new_uuid(), b"ours");
    let unclaimed_id = ProjectId::for_test(9);
    let unclaimed = write_run(
        &dirs,
        &unclaimed_id,
        &AgentRunId::new_uuid(),
        b"no project owns this",
    );

    let loaded = project.load_transcripts_from_disk(&dirs.state_root);
    project.purge_project_transcripts().unwrap();
    let usage = scan_transcript_disk_usage(&dirs.state_root, &[project.id().clone()]);

    assert_eq!(loaded.loaded, 1);
    assert!(!ours.exists());
    assert!(
        unclaimed.exists(),
        "a directory no project claims is never deleted in-app (D6)"
    );
    assert_eq!(usage.unclaimed_bytes, b"no project owns this".len() as u64);
}

#[test]
fn the_app_wide_figure_includes_a_closed_project() {
    let dirs = TestDirs::new("closed-project");
    let open = ProjectId::for_test(1);
    let closed = ProjectId::for_test(2);
    write_run(&dirs, &open, &AgentRunId::new_uuid(), b"open project");
    write_run(
        &dirs,
        &closed,
        &AgentRunId::new_uuid(),
        b"closed but still in the recent list",
    );
    fs::write(
        dirs.state_root.join("transcripts").join("stray-at-root"),
        b"stray",
    )
    .unwrap();

    let usage = scan_transcript_disk_usage(&dirs.state_root, &[open, closed]);

    assert_eq!(
        usage.total_bytes,
        (b"open project".len() + b"closed but still in the recent list".len() + b"stray".len())
            as u64,
        "closed projects count: the app-wide budget must not reset when a project closes (D5)"
    );
    assert_eq!(usage.unclaimed_bytes, b"stray".len() as u64);
}

#[test]
fn a_transcript_this_session_already_has_is_not_loaded_twice() {
    let dirs = TestDirs::new("already-known");
    let mut project = project_session(&dirs, 1);
    let run = AgentRunId::new_uuid();
    let path = write_run(&dirs, project.id(), &run, b"launched this session");
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Supervised,
        "Agent",
        &dirs.project_root,
        "agent-cli",
    );
    let transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        None,
        fs::canonicalize(&path).unwrap(),
        "local-bounded-agent-run",
    );
    project.add_terminal_session(terminal).unwrap();
    project.add_transcript(transcript).unwrap();

    let loaded = project.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(loaded.loaded, 0);
    assert_eq!(loaded.already_known, 1);
    assert_eq!(project.transcripts().len(), 1);
}

fn write_run(dirs: &TestDirs, project_id: &ProjectId, run: &AgentRunId, bytes: &[u8]) -> PathBuf {
    let path = dirs
        .state_root
        .join("transcripts")
        .join(project_id.as_str())
        .join(run.as_str())
        .join("transcript.log");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    fs::canonicalize(&path).unwrap()
}

fn project_session(dirs: &TestDirs, sequence: u64) -> ProjectSession {
    ProjectSession::new(
        ProjectId::for_test(sequence),
        "Loading",
        &dirs.project_root,
        fs::canonicalize(&dirs.project_root).unwrap(),
    )
}

struct TestDirs {
    base: PathBuf,
    project_root: PathBuf,
    state_root: PathBuf,
}

impl TestDirs {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("tekstide-loading-{name}-{nanos}"));
        let project_root = base.join("project");
        let state_root = base.join("state");
        fs::create_dir_all(&project_root).unwrap();
        fs::create_dir_all(&state_root).unwrap();
        Self {
            base,
            project_root,
            state_root,
        }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}
