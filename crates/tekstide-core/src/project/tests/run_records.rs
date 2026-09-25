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
use crate::transcript::{
    RUN_RECORD_FILE_NAME, RUN_RECORD_VERSION, is_run_record_file_name, scan_transcript_disk_usage,
};

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

#[test]
fn a_finished_run_whose_record_holds_no_ending_says_unknown_not_the_read_time() {
    let dirs = TestDirs::new("finished-no-ending");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let path = run_dir.join(RUN_RECORD_FILE_NAME);
    let mut record = read_json(&path);
    record["status"] = "completed".into();
    record["ending"] = serde_json::json!({ "kind": "not_ended" });
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();

    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    let restored = &reopened.restored_agent_runs()[0];
    assert_eq!(restored.status, AgentRunStatus::Completed);
    assert_eq!(restored.ending, RunEnding::Unknown);
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
    assert_eq!(reopened.run_records_set_aside().unreadable, 1);
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
fn a_record_that_cannot_be_moved_is_named_as_left_in_place_not_as_set_aside() {
    use std::os::unix::fs::PermissionsExt;
    let dirs = TestDirs::new("cannot-move");
    let (project, _, run_dir) = project_with_running_run(&dirs, 1);
    drop(project);
    fs::write(run_dir.join(RUN_RECORD_FILE_NAME), b"not json").unwrap();
    fs::set_permissions(&run_dir, fs::Permissions::from_mode(0o555)).unwrap();

    let mut reopened = project_session(&dirs, 1);
    let summary = reopened.load_transcripts_from_disk(&dirs.state_root);
    fs::set_permissions(&run_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(summary.run_records_left_in_place, 1);
    assert_eq!(summary.run_records_set_aside_unreadable, 0);
    assert_eq!(reopened.run_records_set_aside().left_in_place, 1);
    assert!(run_dir.join(RUN_RECORD_FILE_NAME).exists());
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

/// Review 437 Q1: the matcher is **exact**, because PR-056-C deletes by it.
/// `move_aside` produces `run.json.corrupt` and `run.json.corrupt-<N>` and
/// nothing else; a name that merely *begins* like one is not ours.
#[test]
fn only_the_names_this_product_writes_are_its_record_files() {
    for ours in [
        "run.json",
        "run.json.tmp",
        "run.json.corrupt",
        "run.json.corrupt-1",
        "run.json.corrupt-2",
        "run.json.corrupt-999",
    ] {
        assert!(
            is_run_record_file_name(ours),
            "{ours} is written by this product"
        );
    }
    for not_ours in [
        "run.json.corruption-notes",
        "run.json.corruptXYZ",
        "run.json.corrupt-",
        "run.json.corrupt-1a",
        "run.json.corrupt--1",
        "run.json.corrupt-1.bak",
        "run.json.corrupt.bak",
        "run.json.tmp2",
        "run.json.bak",
        "run.jsonx",
        "Run.json",
        "run.json ",
        " run.json",
        "notes.txt",
        "transcript.log",
        "",
    ] {
        assert!(
            !is_run_record_file_name(not_ours),
            "{not_ours:?} is a name this product never writes"
        );
    }
}

/// The same property where it costs something today: a file that only looks
/// like a set-aside record is **unclaimed** bytes, not the product's own.
#[test]
fn a_file_that_only_begins_like_a_set_aside_record_is_counted_as_unclaimed() {
    let dirs = TestDirs::new("prefix-lookalike");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    let lookalike = b"someone's own notes about this run";
    fs::write(run_dir.join("run.json.corruption-notes"), lookalike).unwrap();

    let usage = scan_transcript_disk_usage(&dirs.state_root, &[project.id().clone()]);

    assert_eq!(usage.unclaimed_bytes, lookalike.len() as u64);
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
    project.purge_project_transcripts().unwrap();
    assert!(
        !run_dir.exists(),
        "the purge took the record and the empty directory"
    );

    let write = project
        .set_agent_run_notes(&run_id, "written after the purge")
        .unwrap();

    assert_eq!(write, RunRecordWrite::NoRunDirectory);
    assert!(!run_dir.exists());
}

#[test]
fn a_purged_restored_run_cannot_be_annotated_back_into_existence() {
    let dirs = TestDirs::new("late-annotation-restored");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);
    reopened.purge_project_transcripts().unwrap();

    let late = reopened.set_agent_run_notes(&run_id, "written after the purge");

    assert!(late.is_err(), "the run left the session with its record");
    assert!(!run_dir.exists(), "and nothing wrote the directory back");
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

// ---- PR-056-C: purge takes the record, and never a directory it did not find empty ----

/// **The box that matters** (RFC-056 R2), written before any code that deletes
/// a record or a directory existed. The third file is `run.json.corruption-notes`
/// — a name a prefix match on `run.json.corrupt` would eat and an exact match
/// does not — beside a plain unrelated file. After a purge both must survive,
/// and so must the directory that holds them.
#[test]
fn purge_never_removes_a_directory_it_did_not_find_empty_or_a_file_it_did_not_write() {
    let dirs = TestDirs::new("third-file");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    let lookalike = run_dir.join("run.json.corruption-notes");
    let unrelated = run_dir.join("notes-from-a-human.txt");
    fs::write(&lookalike, b"a person's own file, named like ours").unwrap();
    fs::write(&unrelated, b"another one").unwrap();

    let purged = project.purge_project_transcripts().unwrap();

    assert_eq!(
        purged.purged_transcripts, 1,
        "positive control: the purge ran"
    );
    assert!(!run_dir.join("transcript.log").exists());
    assert!(
        lookalike.exists(),
        "a file this product never wrote is not deleted"
    );
    assert!(unrelated.exists());
    assert!(
        run_dir.is_dir(),
        "the directory held files that are not ours, so it is not removed"
    );
}

#[test]
fn purge_removes_the_transcript_every_record_file_and_then_the_empty_directory() {
    let dirs = TestDirs::new("purge-all");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    for name in [
        "run.json.tmp",
        "run.json.corrupt",
        "run.json.corrupt-1",
        "run.json.corrupt-42",
    ] {
        fs::write(run_dir.join(name), b"a set-aside or interrupted record").unwrap();
    }
    let on_disk: u64 = fs::read_dir(&run_dir)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum();
    assert_eq!(
        project.purgeable_transcript_bytes(),
        on_disk,
        "the dialog counts the record files, so it never promises less than it removes"
    );

    let purged = project.purge_project_transcripts().unwrap();

    assert_eq!(purged.purged_transcripts, 1);
    assert_eq!(
        purged.bytes_removed, on_disk,
        "what the purge reports removing is what was on disk"
    );
    assert!(
        !run_dir.exists(),
        "nothing was left in it, so the directory goes too"
    );
}

/// D2's acceptance criterion, asserted against the disk, not against the code:
/// after a purge, nothing under the state directory names the run's prompt or
/// the user's notes.
#[test]
fn after_a_purge_nothing_on_disk_names_the_prompt_or_the_notes() {
    let dirs = TestDirs::new("purge-search");
    let (mut project, run_id, _) = project_with_running_run(&dirs, 1);
    project.agent_run_mut_for_test(&run_id).prompt_summary =
        "PROMPT-NEEDLE-4f1c refactor the billing module".to_owned();
    project
        .set_agent_run_notes(&run_id, "NOTES-NEEDLE-9b2e the user's own words")
        .unwrap();
    project.persist_agent_run_records();
    assert!(
        tree_contains(&dirs.state_root, b"PROMPT-NEEDLE-4f1c")
            && tree_contains(&dirs.state_root, b"NOTES-NEEDLE-9b2e"),
        "positive control: before the purge the record does name them"
    );

    project.purge_project_transcripts().unwrap();

    assert!(!tree_contains(&dirs.state_root, b"PROMPT-NEEDLE-4f1c"));
    assert!(!tree_contains(&dirs.state_root, b"NOTES-NEEDLE-9b2e"));
    assert!(!tree_contains(&dirs.state_root, run_id.as_str().as_bytes()));
}

#[test]
fn purge_deletes_only_regular_files_by_exact_name_and_leaves_the_rest() {
    let dirs = TestDirs::new("purge-exact");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    // A directory with a record's name, holding a file.
    let dir_named_like_ours = run_dir.join("run.json.corrupt-3");
    fs::create_dir(&dir_named_like_ours).unwrap();
    fs::write(dir_named_like_ours.join("inside"), b"inside").unwrap();
    // A symlink with a record's name, pointing outside the state root.
    let outside = dirs.base.join("victim.txt");
    fs::write(&outside, b"the user's own file").unwrap();
    std::os::unix::fs::symlink(&outside, run_dir.join("run.json.corrupt")).unwrap();

    project.purge_project_transcripts().unwrap();

    assert!(dir_named_like_ours.join("inside").exists());
    assert!(
        run_dir
            .join("run.json.corrupt")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&outside).unwrap(), b"the user's own file");
    assert!(run_dir.exists(), "not empty, so not removed");
    assert!(
        !run_dir.join("run.json").exists(),
        "the real record still went"
    );
}

/// Only the layout the product writes is looked at. A transcript at any other
/// path gets the old behaviour: its file, and nothing beside it.
#[test]
fn a_transcript_outside_the_products_layout_is_purged_alone() {
    let dirs = TestDirs::new("purge-foreign-layout");
    let mut project = project_session(&dirs, 1);
    let folder = dirs.state_root.join("not-a-run-directory");
    fs::create_dir_all(&folder).unwrap();
    let transcript_path = fs::canonicalize(&folder).unwrap().join("transcript.log");
    fs::write(&transcript_path, b"captured").unwrap();
    fs::write(folder.join("run.json"), b"looks like a record").unwrap();
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
        transcript_path.clone(),
        "local-bounded-agent-run",
    );
    project.add_terminal_session(terminal).unwrap();
    project.add_transcript(transcript).unwrap();

    project.purge_project_transcripts().unwrap();

    assert!(!transcript_path.exists());
    assert!(folder.join("run.json").exists());
    assert!(folder.exists());
}

#[test]
fn a_purged_restored_run_leaves_the_session_and_the_board_count() {
    let dirs = TestDirs::new("purge-restored");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);
    assert_eq!(reopened.restored_agent_runs().len(), 1);
    assert_eq!(reopened.runtime_summary().agent_run_count, Some(1));

    reopened.purge_project_transcripts().unwrap();

    assert!(reopened.restored_agent_runs().is_empty());
    assert_eq!(reopened.runtime_summary().agent_run_count, Some(0));
    assert!(!run_dir.exists());
}

#[test]
fn a_purge_that_cannot_remove_the_record_leaves_the_transcript_and_can_be_retried() {
    use std::os::unix::fs::PermissionsExt;
    let dirs = TestDirs::new("purge-retry");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    fs::set_permissions(&run_dir, fs::Permissions::from_mode(0o555)).unwrap();

    let refused = project.purge_project_transcripts();
    fs::set_permissions(&run_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        refused.is_err(),
        "a record that could not be removed is an error"
    );
    assert!(
        run_dir.join("transcript.log").exists() && run_dir.join("run.json").exists(),
        "nothing was half-deleted: the transcript is still there beside its record"
    );
    let retried = project.purge_project_transcripts().unwrap();
    assert_eq!(retried.purged_transcripts, 1);
    assert!(!run_dir.exists());
}

fn expire_everything(project: &mut ProjectSession) -> crate::project::TranscriptRetentionCleanup {
    let far_future =
        crate::domain::DomainTimestamp::from_utc_string("2099-01-01T00:00:00Z").unwrap();
    project.apply_transcript_retention(
        crate::transcript::TranscriptRetentionLimits::new(1 << 20, 1 << 20, 1 << 20, 30),
        0,
        &far_future,
    )
}

/// Review 438: **retention expires transcripts, not the user's writing.**
/// `transcript_retention_days` cannot be expected to delete a note, so an
/// expired transcript leaves its run's record — and the run — in place.
#[test]
fn a_transcript_expired_by_retention_leaves_the_record_and_the_run() {
    let dirs = TestDirs::new("retention-keeps-record");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let mut reopened = project_session(&dirs, 1);
    reopened.load_transcripts_from_disk(&dirs.state_root);

    let cleanup = expire_everything(&mut reopened);

    assert_eq!(
        cleanup.expired.purged_transcripts, 1,
        "positive control: it expired"
    );
    assert!(!run_dir.join("transcript.log").exists());
    assert!(
        run_dir.join(RUN_RECORD_FILE_NAME).exists(),
        "the record survives an expiry"
    );
    assert_eq!(reopened.restored_agent_runs().len(), 1);
    let write = reopened
        .set_agent_run_notes(&run_id, "written after the transcript expired")
        .unwrap();
    assert_eq!(
        write,
        RunRecordWrite::Written,
        "and it can still be annotated"
    );
    assert_eq!(
        read_json(&run_dir.join(RUN_RECORD_FILE_NAME))["notes"],
        "written after the transcript expired"
    );
}

/// The same for a run this session launched: its tombstone's own path is
/// empty, so the session remembers where the record is.
#[test]
fn a_launched_runs_record_survives_its_transcripts_expiry_and_can_still_be_written() {
    let dirs = TestDirs::new("retention-launched");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project
        .agent_run_mut_for_test(&run_id)
        .transition_to(AgentRunStatus::Completed)
        .unwrap();
    project.persist_agent_run_records();

    let cleanup = expire_everything(&mut project);

    assert_eq!(cleanup.expired.purged_transcripts, 1, "positive control");
    assert!(run_dir.join(RUN_RECORD_FILE_NAME).exists());
    let write = project
        .set_agent_run_notes(&run_id, "after expiry")
        .unwrap();
    assert_eq!(write, RunRecordWrite::Written);
    assert_eq!(
        read_json(&run_dir.join(RUN_RECORD_FILE_NAME))["notes"],
        "after expiry"
    );
}

/// Review 438's hole, closed: a tombstone used to return early from purge, so
/// a record retention left could never be removed by anything the user does.
#[test]
fn a_purge_after_retention_still_takes_the_record_and_the_directory() {
    let dirs = TestDirs::new("purge-after-retention");
    let (mut project, run_id, run_dir) = project_with_running_run(&dirs, 1);
    project
        .agent_run_mut_for_test(&run_id)
        .transition_to(AgentRunStatus::Completed)
        .unwrap();
    project.persist_agent_run_records();
    expire_everything(&mut project);
    assert!(run_dir.join(RUN_RECORD_FILE_NAME).exists(), "precondition");
    let record_bytes = fs::metadata(run_dir.join(RUN_RECORD_FILE_NAME))
        .unwrap()
        .len();
    assert_eq!(
        project.purgeable_run_data(),
        (1, record_bytes),
        "the dialog counts a run whose transcript is already gone"
    );

    let purged = project.purge_project_transcripts().unwrap();

    assert_eq!(purged.bytes_removed, record_bytes);
    assert!(!run_dir.exists());
    assert_eq!(project.purgeable_run_data(), (0, 0));
}

/// The record with no transcript beside it — which retention now makes an
/// ordinary state, not a corner: the next session finds the record alone.
#[test]
fn a_purge_takes_a_record_whose_transcript_retention_removed_in_an_earlier_session() {
    let dirs = TestDirs::new("record-only");
    let (mut project, _, run_dir) = project_with_running_run(&dirs, 1);
    project.persist_agent_run_records();
    drop(project);
    let mut middle = project_session(&dirs, 1);
    middle.load_transcripts_from_disk(&dirs.state_root);
    expire_everything(&mut middle);
    drop(middle);
    assert!(
        run_dir.join(RUN_RECORD_FILE_NAME).exists() && !run_dir.join("transcript.log").exists()
    );

    let mut later = project_session(&dirs, 1);
    later.load_transcripts_from_disk(&dirs.state_root);
    assert_eq!(
        later.restored_agent_runs().len(),
        1,
        "the run is still listed"
    );
    assert_eq!(later.purgeable_run_data().0, 1);
    later.purge_project_transcripts().unwrap();

    assert!(!run_dir.exists());
    assert!(later.restored_agent_runs().is_empty());
    assert_eq!(later.runtime_summary().agent_run_count, Some(0));
}

fn tree_contains(root: &Path, needle: &[u8]) -> bool {
    fs::read_dir(root).unwrap().flatten().any(|entry| {
        let path = entry.path();
        let name_matches = path
            .to_string_lossy()
            .as_bytes()
            .windows(needle.len())
            .any(|window| window == needle);
        let metadata = fs::symlink_metadata(&path).unwrap();
        name_matches
            || (metadata.is_dir() && tree_contains(&path, needle))
            || (metadata.is_file()
                && fs::read(&path)
                    .unwrap()
                    .windows(needle.len())
                    .any(|window| window == needle))
    })
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
