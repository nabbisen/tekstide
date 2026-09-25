//! RFC-056 PR-056-B: the record a run leaves, and what a later session makes
//! of it.
//!
//! Every fixture is a fresh temporary state root laid out as the product
//! writes it. **No test here reads the real state root.** "The app was
//! killed" is a session dropped without any closing step: nothing in these
//! tests calls a shutdown path, because the product has none to call.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunId, AgentRunOrigin, AgentRunStatus, ApprovalId,
    AuditEventId, ChangeSetId, RUN_NOTES_MAX_CHARS, RUN_RECORD_MAX_IDS_PER_KIND, RunClassification,
    RunEnding, TerminalKind, TerminalSession, Transcript,
};
use crate::project::{ProjectId, ProjectSession, RunAnnotationError, RunRecordWrite};
use crate::transcript::{RUN_RECORD_FILE_NAME, RUN_RECORD_VERSION, scan_transcript_disk_usage};

#[test]
fn a_launched_run_gets_a_record_beside_its_transcript_holding_references_only() {
    let dirs = TestDirs::new("beside");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);

    let summary = project.persist_agent_run_records();

    assert_eq!(summary.written, 1);
    let record = read_json(&run_dir.join(RUN_RECORD_FILE_NAME));
    assert_eq!(record["version"], RUN_RECORD_VERSION);
    assert_eq!(record["run_id"], run_id.as_str());
    assert_eq!(record["project_id"], project.id().as_str());
    assert_eq!(record["profile_id"], "profile");
    assert_eq!(record["prompt_summary"], "a summary");
    assert!(
        run_dir.join("transcript.log").exists(),
        "the record sits beside the transcript, in the directory the run already owns"
    );
    // References and the user's words only: nothing of the transcript's bytes.
    let text = fs::read_to_string(run_dir.join(RUN_RECORD_FILE_NAME)).unwrap();
    assert!(
        !text.contains("SECRET TRANSCRIPT CONTENT"),
        "run content lives in the transcript and nowhere else (D8)"
    );
}

#[test]
fn a_pass_over_an_unchanged_run_writes_nothing() {
    let dirs = TestDirs::new("unchanged");
    let (mut project, _, _) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();

    let second = project.persist_agent_run_records();

    assert_eq!(second.written, 0);
    assert_eq!(second.unchanged, 1);
}

/// D9, ablated: the setter persists at once. A run killed a moment after the
/// user classified it, and before any periodic pass, keeps the classification.
#[test]
fn a_classification_set_mid_run_survives_a_kill() {
    let dirs = TestDirs::new("kill-survives");
    let (mut project, run_id, _) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();

    let write = project
        .set_agent_run_classification(&run_id, Some(RunClassification::Testing))
        .unwrap();
    project
        .set_agent_run_notes(&run_id, "watched this one closely")
        .unwrap();
    assert_eq!(write, RunRecordWrite::Written);
    drop(project); // killed: no periodic pass, no shutdown step

    let mut reopened = project_session(&dirs, 1);
    let summary = reopened.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(summary.run_records_restored, 1);
    let restored = &reopened.restored_agent_runs()[0];
    assert_eq!(restored.classification, Some(RunClassification::Testing));
    assert_eq!(restored.notes.as_deref(), Some("watched this one closely"));
    assert_eq!(restored.id, run_id);
}

#[test]
fn a_restored_run_carries_its_prompt_profile_ids_and_its_transcript() {
    let dirs = TestDirs::new("restored-fields");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    let approval = ApprovalId::new_uuid();
    let change_set = ChangeSetId::new_uuid();
    let audit = AuditEventId::new_uuid();
    {
        let run = project.agent_run_mut_for_test(&run_id);
        run.approval_ids.push(approval.clone());
        run.change_set_ids.push(change_set.clone());
        run.audit_event_ids.push(audit.clone());
    }
    project.persist_agent_run_records();
    drop(project);

    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    let restored = &reopened.restored_agent_runs()[0];
    assert_eq!(restored.profile_id, "profile");
    assert_eq!(restored.prompt_summary, "a summary");
    assert_eq!(restored.approval_ids, vec![approval]);
    assert_eq!(restored.change_set_ids, vec![change_set]);
    assert_eq!(restored.audit_event_ids, vec![audit]);
    assert_eq!(restored.origin, AgentRunOrigin::RestoredFromRecord);
    let transcript_id = restored
        .transcript_ref
        .as_ref()
        .expect("the transcript is attached to the run, not orphaned");
    let transcript = reopened
        .transcripts()
        .iter()
        .find(|transcript| &transcript.id == transcript_id)
        .unwrap();
    assert_eq!(transcript.storage_path, run_dir.join("transcript.log"));
    assert_eq!(reopened.transcripts().len(), 1);
}

/// D6/D12, ablated: a run whose process was killed with the app has no
/// ending. Not its start, not zero, not the moment its record was read.
#[test]
fn a_run_killed_with_the_app_says_its_ending_is_unknown() {
    let dirs = TestDirs::new("unknown-ending");
    let (mut project, _, _) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);

    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    let restored = &reopened.restored_agent_runs()[0];
    assert_eq!(
        restored.ending,
        RunEnding::Unknown,
        "unknown is unknown: neither the start time, zero, nor the read time"
    );
    assert_ne!(restored.status, AgentRunStatus::Running);
    assert_eq!(restored.status, AgentRunStatus::Detached);
    assert!(restored.ended_at.is_none());
}

#[test]
fn a_run_that_ended_keeps_the_ending_it_was_seen_to_have() {
    let dirs = TestDirs::new("known-ending");
    let (mut project, run_id, _) = project_with_running_run(&dirs, 1);
    project
        .agent_run_mut_for_test(&run_id)
        .transition_to(AgentRunStatus::Completed)
        .unwrap();
    let ended = project.agent_runs()[0].ending.clone();
    assert!(matches!(ended, RunEnding::Ended(_)));
    // The status change is picked up by the periodic pass, not a setter.
    project.persist_agent_run_records();
    drop(project);

    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    let restored = &reopened.restored_agent_runs()[0];
    assert_eq!(restored.status, AgentRunStatus::Completed);
    assert_eq!(restored.ending, ended);
}

/// D7, ablated: a record that cannot be read is moved aside and counted, the
/// run appears as a transcript with no run, and nothing is invented.
#[test]
fn a_corrupt_record_is_moved_aside_and_the_run_is_a_transcript_with_no_run() {
    let dirs = TestDirs::new("corrupt");
    let (project, _, run_dir) = project_with_running_run(&dirs, 1);
    drop(project);
    let record = run_dir.join(RUN_RECORD_FILE_NAME);
    fs::write(&record, b"{ this is not json, the app was killed mid-").unwrap();

    let mut reopened = project_session(&dirs, 1);
    let summary = reopened.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(summary.run_records_set_aside_unreadable, 1);
    assert_eq!(summary.run_records_restored, 0);
    assert!(reopened.restored_agent_runs().is_empty());
    assert_eq!(reopened.agent_runs().len(), 0);
    assert_eq!(
        reopened.transcripts().len(),
        1,
        "the run appears as it does today: a transcript with no run"
    );
    assert!(!record.exists(), "moved, so it is not read again");
    let aside = run_dir.join("run.json.corrupt");
    assert_eq!(
        fs::read(&aside).unwrap(),
        b"{ this is not json, the app was killed mid-",
        "moved aside intact, never deleted and never rewritten"
    );

    // A second one does not overwrite the first.
    fs::write(&record, b"also not json").unwrap();
    let mut again = project_session(&dirs, 1);
    again.load_transcripts_from_disk(&dirs.state_root);
    assert_eq!(
        fs::read(&aside).unwrap(),
        b"{ this is not json, the app was killed mid-"
    );
    assert_eq!(
        fs::read(run_dir.join("run.json.corrupt-1")).unwrap(),
        b"also not json"
    );
}

#[test]
fn a_record_of_an_unknown_version_is_set_aside_and_named_as_such() {
    let dirs = TestDirs::new("unknown-version");
    let (project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    drop(project);
    let newer = format!(
        r#"{{"version": {}, "run_id": "{}", "a_field_this_build_has_never_heard_of": [1, 2, 3]}}"#,
        RUN_RECORD_VERSION + 1,
        run_id.as_str()
    );
    fs::write(run_dir.join(RUN_RECORD_FILE_NAME), &newer).unwrap();

    let mut reopened = project_session(&dirs, 1);
    let summary = reopened.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(summary.run_records_set_aside_unknown_version, 1);
    assert_eq!(summary.run_records_set_aside_unreadable, 0);
    assert!(reopened.restored_agent_runs().is_empty());
    assert_eq!(
        fs::read_to_string(run_dir.join("run.json.corrupt")).unwrap(),
        newer,
        "a newer Tekstide's record is kept whole, for that Tekstide"
    );
}

#[test]
fn records_that_are_not_this_runs_are_set_aside_and_never_guessed_at() {
    let dirs = TestDirs::new("not-this-run");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    let good = fs::read_to_string(run_dir.join(RUN_RECORD_FILE_NAME)).unwrap();
    drop(project);

    let other_run = good.replace(run_id.as_str(), AgentRunId::new_uuid().as_str());
    let other_project = good.replace(
        ProjectId::for_test(1).as_str(),
        ProjectId::for_test(2).as_str(),
    );
    let wrong_shapes: [(&str, String); 6] = [
        ("another run's", other_run),
        ("another project's", other_project),
        ("no version", r#"{"run_id": "x"}"#.to_owned()),
        ("a string version", r#"{"version": "1"}"#.to_owned()),
        ("an empty file", String::new()),
        ("valid json, wrong shape", r#"{"version": 1}"#.to_owned()),
    ];
    for (label, content) in wrong_shapes {
        fs::write(run_dir.join(RUN_RECORD_FILE_NAME), &content).unwrap();
        let mut reopened = project_session(&dirs, 1);
        let summary = reopened.load_transcripts_from_disk(&dirs.state_root);
        assert_eq!(
            summary.run_records_set_aside_unreadable, 1,
            "{label} is unreadable, not a crash and not a run"
        );
        assert!(reopened.restored_agent_runs().is_empty(), "{label}");
    }
}

#[test]
fn a_record_over_the_size_cap_is_set_aside_unread() {
    let dirs = TestDirs::new("too-big");
    let (project, _, run_dir) = project_with_running_run(&dirs, 1);
    drop(project);
    fs::write(run_dir.join(RUN_RECORD_FILE_NAME), vec![b' '; 300 * 1024]).unwrap();

    let mut reopened = project_session(&dirs, 1);
    let summary = reopened.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(summary.run_records_set_aside_unreadable, 1);
}

/// D8, ablated: more ids than the cap and an over-long note produce a bounded
/// record, and the record says it was bounded — and keeps saying so after a
/// restore and a rewrite.
#[test]
fn the_caps_hold_and_the_record_says_it_was_bounded() {
    let dirs = TestDirs::new("caps");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    let extra = 50;
    let total = RUN_RECORD_MAX_IDS_PER_KIND + extra;
    {
        let run = project.agent_run_mut_for_test(&run_id);
        run.approval_ids = (0..total).map(|_| ApprovalId::new_uuid()).collect();
        run.change_set_ids = (0..total).map(|_| ChangeSetId::new_uuid()).collect();
        run.audit_event_ids = (0..total).map(|_| AuditEventId::new_uuid()).collect();
    }
    project
        .set_agent_run_notes(&run_id, &"n".repeat(RUN_NOTES_MAX_CHARS + 100))
        .unwrap();
    project.persist_agent_run_records();

    let record = read_json(&run_dir.join(RUN_RECORD_FILE_NAME));
    for key in ["approval_ids", "change_set_ids", "audit_event_ids"] {
        assert_eq!(
            record[key].as_array().unwrap().len(),
            RUN_RECORD_MAX_IDS_PER_KIND,
            "{key}"
        );
    }
    assert_eq!(
        record["notes"].as_str().unwrap().chars().count(),
        RUN_NOTES_MAX_CHARS
    );
    assert_eq!(record["bounds"]["notes_truncated"], true);
    assert_eq!(record["bounds"]["omitted_approval_ids"], extra as u64);
    assert_eq!(record["bounds"]["omitted_change_set_ids"], extra as u64);
    assert_eq!(record["bounds"]["omitted_audit_event_ids"], extra as u64);
    drop(project);

    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);
    let restored = reopened.restored_agent_runs()[0].clone();
    assert_eq!(restored.approval_ids.len(), RUN_RECORD_MAX_IDS_PER_KIND);
    assert!(restored.record_bounds.is_bounded());
    assert_eq!(restored.record_bounds.omitted_approval_ids, extra as u64);

    // A later change rewrites the record, and the bound is not forgotten.
    reopened
        .set_agent_run_classification(&restored.id, Some(RunClassification::Review))
        .unwrap();
    let rewritten = read_json(&run_dir.join(RUN_RECORD_FILE_NAME));
    assert_eq!(rewritten["bounds"]["omitted_approval_ids"], extra as u64);
    assert_eq!(rewritten["bounds"]["notes_truncated"], true);
}

#[test]
fn a_hand_edited_record_beyond_the_caps_is_bounded_on_the_way_in() {
    let dirs = TestDirs::new("caps-on-read");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let path = run_dir.join(RUN_RECORD_FILE_NAME);
    let mut record = read_json(&path);
    record["approval_ids"] = (0..RUN_RECORD_MAX_IDS_PER_KIND + 7)
        .map(|_| serde_json::Value::String(ApprovalId::new_uuid().as_str().to_owned()))
        .collect::<Vec<_>>()
        .into();
    record["notes"] = "x".repeat(RUN_NOTES_MAX_CHARS * 2).into();
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();

    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    let restored = &reopened.restored_agent_runs()[0];
    assert_eq!(restored.id, run_id);
    assert_eq!(restored.approval_ids.len(), RUN_RECORD_MAX_IDS_PER_KIND);
    assert_eq!(restored.record_bounds.omitted_approval_ids, 7);
    assert!(restored.record_bounds.notes_truncated);
    assert_eq!(
        restored.notes.as_ref().unwrap().chars().count(),
        RUN_NOTES_MAX_CHARS
    );
}

/// R1, ablated: the record is the product's own bytes. A record, the file a
/// write in progress leaves, and a record set aside are never unclaimed; a
/// stray file beside them still is.
#[test]
fn the_disk_usage_figure_counts_the_record_as_the_products_own() {
    let dirs = TestDirs::new("usage");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    fs::write(run_dir.join("run.json.tmp"), b"a write a kill interrupted").unwrap();
    fs::write(run_dir.join("run.json.corrupt"), b"a record set aside").unwrap();
    let stray = b"not ours";
    fs::write(run_dir.join("stray.txt"), stray).unwrap();

    let usage = scan_transcript_disk_usage(&dirs.state_root, &[project.id().clone()]);

    assert!(usage.total_bytes > stray.len() as u64);
    assert_eq!(
        usage.unclaimed_bytes,
        stray.len() as u64,
        "only what this product did not write is unclaimed"
    );
}

#[test]
fn a_record_that_is_a_symlink_or_a_directory_is_not_a_record_and_is_left_alone() {
    let dirs = TestDirs::new("not-a-file");
    let (project, _, run_dir) = project_with_running_run(&dirs, 1);
    drop(project);
    let outside = dirs.base.join("outside.json");
    fs::write(&outside, b"the user's own file").unwrap();
    let record = run_dir.join(RUN_RECORD_FILE_NAME);
    std::os::unix::fs::symlink(&outside, &record).unwrap();

    let mut reopened = project_session(&dirs, 1);
    let summary = reopened.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(summary.run_records_restored, 0);
    assert_eq!(summary.run_records_set_aside_unreadable, 0);
    assert!(record.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(fs::read(&outside).unwrap(), b"the user's own file");
}

#[test]
fn a_record_is_never_written_through_a_planted_symlink() {
    let dirs = TestDirs::new("planted-tmp");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    let outside = dirs.base.join("victim.txt");
    fs::write(&outside, b"the user's own file").unwrap();
    std::os::unix::fs::symlink(&outside, run_dir.join("run.json.tmp")).unwrap();

    let summary = project.persist_agent_run_records();

    assert_eq!(summary.failed, 1, "refused rather than written through");
    assert_eq!(fs::read(&outside).unwrap(), b"the user's own file");
}

#[test]
fn a_write_replaces_a_temporary_file_an_earlier_kill_left() {
    let dirs = TestDirs::new("stale-tmp");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    fs::write(run_dir.join("run.json.tmp"), b"half a rec").unwrap();

    let summary = project.persist_agent_run_records();

    assert_eq!(summary.written, 1);
    assert!(
        !run_dir.join("run.json.tmp").exists(),
        "no temporary file survives a write"
    );
    assert_eq!(
        read_json(&run_dir.join(RUN_RECORD_FILE_NAME))["version"],
        RUN_RECORD_VERSION
    );
}

#[test]
fn a_run_without_a_run_directory_gets_no_record_and_no_directory() {
    let dirs = TestDirs::new("no-dir");
    let mut project = project_session(&dirs, 1);
    let run = AgentRun::draft(
        project.id().clone(),
        "profile",
        "no transcript was captured",
        AgentCompatibilityLevel::Plain,
    );
    let run_id = run.id.clone();
    project.add_agent_run(run).unwrap();

    let write = project
        .set_agent_run_classification(&run_id, Some(RunClassification::Coding))
        .unwrap();

    assert_eq!(write, RunRecordWrite::NoRunDirectory);
    assert!(
        !dirs.state_root.join("transcripts").exists(),
        "a record never creates a run directory"
    );
    assert_eq!(
        project.agent_runs()[0].classification,
        Some(RunClassification::Coding),
        "the classification still holds in memory"
    );
}

#[test]
fn a_purged_run_is_not_brought_back_by_a_late_annotation() {
    let dirs = TestDirs::new("late-annotation");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    fs::remove_file(run_dir.join(RUN_RECORD_FILE_NAME)).unwrap();
    project.purge_project_transcripts().unwrap();
    fs::remove_dir(&run_dir).unwrap();

    let write = project
        .set_agent_run_notes(&run_id, "written after the purge")
        .unwrap();

    assert_eq!(write, RunRecordWrite::NoRunDirectory);
    assert!(!run_dir.exists());
}

#[test]
fn a_purged_restored_run_is_not_written_back_by_a_late_annotation() {
    let dirs = TestDirs::new("late-annotation-restored");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);
    reopened.purge_project_transcripts().unwrap();
    // The record is what a later slice's purge removes; here it is removed by
    // hand, so the only thing left to prove is that nothing writes it back.
    fs::remove_file(run_dir.join(RUN_RECORD_FILE_NAME)).unwrap();

    let write = reopened
        .set_agent_run_notes(&run_id, "written after the purge")
        .unwrap();

    assert_eq!(write, RunRecordWrite::NoRunDirectory);
    assert!(!run_dir.join(RUN_RECORD_FILE_NAME).exists());
}

/// D6, ablated: a restored run is a record. It is in no collection a
/// lifecycle consumer reads, so it cannot look like a process.
#[test]
fn a_restored_run_is_never_running_never_failed_and_never_counts_against_a_limit() {
    let dirs = TestDirs::new("isolation");
    let (mut project, run_id, _) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);

    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(
        reopened.agent_runs().len(),
        0,
        "the launched collection stays empty"
    );
    assert_eq!(reopened.restored_agent_runs().len(), 1);
    let summary = reopened.runtime_summary();
    assert_eq!(
        summary.agent_run_count,
        Some(1),
        "the board counts what exists"
    );
    assert_eq!(summary.running_processes, 0, "a record is not a process");
    assert_eq!(summary.failed_processes, 0);
    assert_eq!(
        reopened.latest_agent_run_for_display().map(|run| &run.id),
        Some(&run_id)
    );
}

#[test]
fn a_run_this_session_launched_is_not_restored_over_itself() {
    let dirs = TestDirs::new("no-duplicate");
    let (mut project, _, _) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();

    let summary = project.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(summary.run_records_restored, 0);
    assert!(project.restored_agent_runs().is_empty());
    assert_eq!(project.agent_runs().len(), 1);
}

#[test]
fn a_record_beside_no_transcript_still_restores_its_run() {
    let dirs = TestDirs::new("no-transcript");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    fs::remove_file(run_dir.join("transcript.log")).unwrap();

    let mut reopened = project_session(&dirs, 1);
    let summary = reopened.load_transcripts_from_disk(&dirs.state_root);

    assert_eq!(summary.run_records_restored, 1);
    let restored = &reopened.restored_agent_runs()[0];
    assert_eq!(restored.id, run_id);
    assert!(restored.transcript_ref.is_none());
}

#[test]
fn the_seven_classifications_and_a_bounded_custom_one_round_trip() {
    let dirs = TestDirs::new("classifications");
    let (mut project, run_id, _) = project_with_running_run(&dirs, 1);
    let all = [
        RunClassification::Coding,
        RunClassification::Review,
        RunClassification::Documentation,
        RunClassification::Testing,
        RunClassification::Refactoring,
        RunClassification::Release,
        RunClassification::Custom("spike\u{202e} with\nnewline".to_owned()),
    ];
    for classification in all {
        project
            .set_agent_run_classification(&run_id, Some(classification.clone()))
            .unwrap();
        let mut reopened = project_session(&dirs, 1);
        reopened.load_transcripts_from_disk(&dirs.state_root);
        assert_eq!(
            reopened.restored_agent_runs()[0].classification,
            Some(classification)
        );
    }

    let too_long = "l".repeat(200);
    project
        .set_agent_run_classification(&run_id, Some(RunClassification::Custom(too_long)))
        .unwrap();
    assert!(
        project.agent_runs()[0]
            .record_bounds
            .classification_label_truncated
    );
    assert_eq!(
        project.set_agent_run_classification(&run_id, Some(RunClassification::Custom("  ".into()))),
        Err(RunAnnotationError::BlankClassificationLabel)
    );
    project.set_agent_run_classification(&run_id, None).unwrap();
    assert_eq!(project.agent_runs()[0].classification, None);
}

#[test]
fn a_restored_run_can_be_annotated_and_the_record_follows() {
    let dirs = TestDirs::new("annotate-restored");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    reopened
        .set_agent_run_notes(&run_id, "added after the restart")
        .unwrap();

    let record = read_json(&run_dir.join(RUN_RECORD_FILE_NAME));
    assert_eq!(record["notes"], "added after the restart");
    assert_eq!(record["ending"]["kind"], "unknown");
}

// ---- fixtures ---------------------------------------------------------------

fn project_with_running_run(
    dirs: &TestDirs,
    sequence: u64,
) -> (ProjectSession, AgentRunId, PathBuf) {
    let mut project = project_session(dirs, sequence);
    let mut run = AgentRun::draft(
        project.id().clone(),
        "profile",
        "a summary",
        AgentCompatibilityLevel::Supervised,
    );
    let run_id = run.id.clone();
    let terminal = TerminalSession::new(
        project.id().clone(),
        TerminalKind::Supervised,
        "Agent",
        &dirs.project_root,
        "agent-cli",
    );
    run.attach_terminal(&terminal).unwrap();
    for status in [
        AgentRunStatus::Ready,
        AgentRunStatus::Preparing,
        AgentRunStatus::Running,
    ] {
        run.transition_to(status).unwrap();
    }
    let transcript_path = dirs
        .state_root
        .join("transcripts")
        .join(project.id().as_str())
        .join(run_id.as_str())
        .join("transcript.log");
    fs::create_dir_all(transcript_path.parent().unwrap()).unwrap();
    fs::write(&transcript_path, b"SECRET TRANSCRIPT CONTENT").unwrap();
    let transcript_path = fs::canonicalize(&transcript_path).unwrap();
    let run_dir = transcript_path.parent().unwrap().to_path_buf();
    let transcript = Transcript::metadata(
        project.id().clone(),
        terminal.id.clone(),
        Some(run_id.clone()),
        transcript_path,
        "local-bounded-agent-run",
    );
    let transcript_id = transcript.id.clone();
    project.add_terminal_session(terminal).unwrap();
    project.add_agent_run(run).unwrap();
    project.add_transcript(transcript).unwrap();
    project.agent_run_mut_for_test(&run_id).transcript_ref = Some(transcript_id);
    (project, run_id, run_dir)
}

fn project_session(dirs: &TestDirs, sequence: u64) -> ProjectSession {
    ProjectSession::new(
        ProjectId::for_test(sequence),
        "Records",
        &dirs.project_root,
        fs::canonicalize(&dirs.project_root).unwrap(),
    )
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
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
        let base = std::env::temp_dir().join(format!("tekstide-runrec-{name}-{nanos}"));
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
