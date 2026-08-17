use super::project_session;
use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunStatus, ChangeAssociationConfidence,
    ChangeDetectionFailureReason, ChangeDetectionSource, ChangeDetectionStatus,
};
use crate::project::{
    ChangeLifecycle, ChangePathKind, ChangedPathValidationErrorReason, DetectedChangedPath,
    DetectedChanges, GeneratedChangeDetectionPolicy, GeneratedChangeDetector,
    ProjectChangeSetError, ProjectId, ProjectSession,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn filesystem_detector_reports_created_modified_deleted_and_renamed_metadata_paths() {
    let sandbox = TestSandbox::new("change-detection-basic");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"fn main() {}\n");
    sandbox.create_file_with_contents("project/src/delete.rs", b"delete me\n");
    sandbox.create_file_with_contents("project/src/old.rs", b"rename me\n");

    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);

    fs::write(
        sandbox.path("project/src/lib.rs"),
        b"fn main() { println!(\"changed\"); }\n",
    )
    .unwrap();
    fs::remove_file(sandbox.path("project/src/delete.rs")).unwrap();
    fs::rename(
        sandbox.path("project/src/old.rs"),
        sandbox.path("project/src/new.rs"),
    )
    .unwrap();
    sandbox.create_file_with_contents("project/src/created.rs", b"new file\n");

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(baseline.source, ChangeDetectionSource::FilesystemSnapshot);
    assert_eq!(baseline.status, ChangeDetectionStatus::Complete);
    assert_eq!(detected.status, ChangeDetectionStatus::Complete);
    assert_eq!(
        detected.changed_files(),
        vec![
            PathBuf::from("src/created.rs"),
            PathBuf::from("src/delete.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/new.rs"),
            PathBuf::from("src/old.rs"),
        ]
    );
    // RFC-012 Amendment 1: lifecycle (what happened) is now orthogonal to
    // kind (what kind of thing) -- checked for every case this fixture
    // already produces, not only the deleted one, since a single test
    // this cheap to extend is the strongest evidence the two axes are
    // both real for a single detector run rather than only individually
    // constructible fixtures.
    let lifecycle_of = |relative_path: &str| {
        detected
            .changed_paths
            .iter()
            .find(|path| path.relative_path == Path::new(relative_path))
            .unwrap_or_else(|| panic!("{relative_path} must be a detected change"))
            .lifecycle
    };
    assert_eq!(lifecycle_of("src/created.rs"), ChangeLifecycle::Added);
    assert_eq!(lifecycle_of("src/lib.rs"), ChangeLifecycle::Modified);
    assert_eq!(lifecycle_of("src/delete.rs"), ChangeLifecycle::Deleted);
    // A rename is untracked by this detector -- it reports as a deletion
    // at the old path and an addition at the new one, not a rename.
    assert_eq!(lifecycle_of("src/old.rs"), ChangeLifecycle::Deleted);
    assert_eq!(lifecycle_of("src/new.rs"), ChangeLifecycle::Added);

    // A deleted path's `kind` reports what it *was* (from the baseline),
    // not `Deleted` -- that variant no longer exists on `ChangePathKind`
    // at all, since deletion is a lifecycle, not a kind.
    assert_eq!(
        detected
            .changed_paths
            .iter()
            .find(|path| path.relative_path == Path::new("src/delete.rs"))
            .unwrap()
            .kind,
        ChangePathKind::File
    );
}

#[test]
fn changed_path_validation_accepts_absolute_paths_only_after_root_containment() {
    let sandbox = TestSandbox::new("change-detection-absolute");
    let project = sandbox.project_session(1);
    let in_root_file = sandbox.create_file_with_contents("project/src/lib.rs", b"metadata\n");
    let outside_file = sandbox.create_file_with_contents("outside.rs", b"outside secret\n");

    let detector = GeneratedChangeDetector::default();
    let normalized = detector
        .validate_changed_path(&project, &in_root_file)
        .expect("absolute path under root should normalize");
    let error = detector
        .validate_changed_path(&project, &outside_file)
        .expect_err("absolute path outside root should be rejected");

    assert_eq!(normalized, PathBuf::from("src/lib.rs"));
    assert_eq!(error.project_id, *project.id());
    assert_eq!(error.reason, ChangedPathValidationErrorReason::RootEscape);
    assert!(
        !format!("{error:?}").contains("outside secret"),
        "diagnostics must not include file contents"
    );
}

#[test]
fn changed_path_validation_rejects_parent_traversal_before_resolution() {
    let sandbox = TestSandbox::new("change-detection-traversal");
    let project = sandbox.project_session(1);
    let _outside_file = sandbox.create_file_with_contents("outside.rs", b"outside secret\n");

    let error = GeneratedChangeDetector::default()
        .validate_changed_path(&project, "../outside.rs")
        .expect_err("relative traversal should not be normalized into a project path");

    assert_eq!(
        error.reason,
        ChangedPathValidationErrorReason::InvalidRelativePath
    );
}

#[cfg(unix)]
#[test]
fn changed_path_validation_allows_valid_paths_when_root_has_symlinked_ancestor() {
    let sandbox = TestSandbox::new("change-detection-root-symlink-ancestor");
    let real_root = sandbox.create_dir("real");
    let real_project = sandbox.create_dir("real/project");
    let link_root = sandbox.path("link");
    std::os::unix::fs::symlink(&real_root, &link_root).unwrap();
    sandbox.create_file_with_contents("real/project/src/lib.rs", b"metadata\n");
    let project = ProjectSession::new(
        ProjectId::for_test(1),
        "Project 1",
        link_root.join("project"),
        fs::canonicalize(real_project).unwrap(),
    );

    let normalized = GeneratedChangeDetector::default()
        .validate_changed_path(&project, "src/lib.rs")
        .expect("symlinked ancestors above the project root are not project escapes");

    assert_eq!(normalized, PathBuf::from("src/lib.rs"));
}

#[cfg(unix)]
#[test]
fn filesystem_detector_labels_symlinks_and_does_not_follow_escape_targets() {
    let sandbox = TestSandbox::new("change-detection-symlink");
    let project = sandbox.project_session(1);
    let outside_dir = sandbox.create_dir("outside");
    sandbox.create_file_with_contents("outside/secret.txt", b"outside secret\n");
    std::os::unix::fs::symlink(&outside_dir, sandbox.path("project/outside-link")).unwrap();

    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);
    let error = detector
        .validate_changed_path(&project, "outside-link/secret.txt")
        .expect_err("paths through escaping symlinks should be rejected");

    assert_eq!(baseline.status, ChangeDetectionStatus::Complete);
    assert_eq!(
        baseline
            .entries
            .iter()
            .find(|entry| entry.relative_path == Path::new("outside-link"))
            .unwrap()
            .kind,
        ChangePathKind::Symlink
    );
    assert!(
        baseline
            .entries
            .iter()
            .all(|entry| entry.relative_path != Path::new("outside-link/secret.txt")),
        "scanner must not follow symlink targets outside the root"
    );
    assert_eq!(
        error.reason,
        ChangedPathValidationErrorReason::SymlinkEscape
    );
}

#[test]
fn detector_reports_partial_status_when_entry_limit_is_hit() {
    let sandbox = TestSandbox::new("change-detection-partial");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/a.txt", b"a\n");
    sandbox.create_file_with_contents("project/b.txt", b"b\n");

    let detector = GeneratedChangeDetector::new(GeneratedChangeDetectionPolicy {
        max_entries: 1,
        max_changed_paths: 8,
        ignored_directory_names: crate::project::IGNORED_DIRECTORY_NAMES,
    });
    let baseline = detector.capture_filesystem_baseline(&project);

    assert_eq!(baseline.entries.len(), 1);
    assert_eq!(baseline.status, ChangeDetectionStatus::Partial { limit: 1 });
}

/// Change-detection-wiring handoff, D1: the exact failure mode the
/// handoff's own "positive control" section warns about, made concrete
/// -- `.git/`, `target/`, and `node_modules/` each hold more entries
/// than `max_entries`, but the ignore list means none of them are ever
/// walked at all, so the real project tree is scanned to completion
/// regardless.
#[test]
fn filesystem_scan_skips_ignored_directories_entirely() {
    let sandbox = TestSandbox::new("change-detection-ignored-directories");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"fn main() {}\n");
    for index in 0..20 {
        sandbox.create_file_with_contents(
            &format!("project/.git/objects/{index:02}"),
            b"git internals\n",
        );
        sandbox.create_file_with_contents(
            &format!("project/target/debug/build-{index:02}"),
            b"build output\n",
        );
        sandbox.create_file_with_contents(
            &format!("project/node_modules/pkg/file-{index:02}.js"),
            b"node stuff\n",
        );
    }

    let detector = GeneratedChangeDetector::new(GeneratedChangeDetectionPolicy {
        max_entries: 10,
        max_changed_paths: 100,
        ignored_directory_names: crate::project::IGNORED_DIRECTORY_NAMES,
    });
    let baseline = detector.capture_filesystem_baseline(&project);

    assert_eq!(
        baseline.status,
        ChangeDetectionStatus::Complete,
        "with the ignored directories skipped entirely, a max_entries of 10 must still be \
         enough to scan the one real file this project has -- if this is Partial, an ignored \
         directory is still being walked"
    );
    assert_eq!(
        baseline
            .entries
            .iter()
            .map(|entry| entry.relative_path.clone())
            .collect::<Vec<_>>(),
        vec![PathBuf::from("src"), PathBuf::from("src/lib.rs")],
        "the baseline must contain only the real project tree, none of .git/target/node_modules"
    );
}

/// Review response 251, finding 1: the ignore rule must match
/// **directories** named e.g. `target`, not any entry of that name. A
/// file named `target` is something an agent could plausibly create or
/// edit, and before this fix it would vanish from change detection
/// exactly like the real `target/` build directory does.
#[test]
fn a_file_named_like_an_ignored_directory_is_not_skipped() {
    let sandbox = TestSandbox::new("change-detection-ignore-name-vs-kind");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/target", b"not a directory\n");
    sandbox.create_file_with_contents("project/.git", b"also not a directory\n");

    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);

    assert_eq!(baseline.status, ChangeDetectionStatus::Complete);
    assert_eq!(
        baseline
            .entries
            .iter()
            .map(|entry| entry.relative_path.clone())
            .collect::<Vec<_>>(),
        vec![PathBuf::from(".git"), PathBuf::from("target")],
        "files named like ignored directories must still be scanned -- only a real directory by \
         that name is skipped: {:?}",
        baseline.entries
    );
}

/// The handoff's own required ablation shape: remove one entry from the
/// ignore list and watch the specific, named directory it used to
/// exclude reappear -- proving the mechanism above is real, not that it
/// merely looks unreachable because nothing tries to defeat it.
#[test]
fn ablation_without_target_in_the_ignore_list_it_reappears_and_truncates_the_scan() {
    let sandbox = TestSandbox::new("change-detection-ignore-ablation");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"fn main() {}\n");
    for index in 0..20 {
        sandbox.create_file_with_contents(
            &format!("project/target/debug/build-{index:02}"),
            b"build output\n",
        );
    }

    // Deliberately omits "target" -- the ablation.
    let detector = GeneratedChangeDetector::new(GeneratedChangeDetectionPolicy {
        max_entries: 10,
        max_changed_paths: 100,
        ignored_directory_names: &[".git", "node_modules"],
    });
    let baseline = detector.capture_filesystem_baseline(&project);

    assert_eq!(
        baseline.status,
        ChangeDetectionStatus::Partial { limit: 10 },
        "without 'target' in the ignore list, its own contents must reappear and truncate the \
         scan before the real project tree is even reached -- proving the ignore list is what \
         keeps the sibling test's scan Complete, not an accident of the fixture"
    );
    assert!(
        baseline
            .entries
            .iter()
            .any(|entry| entry.relative_path.starts_with("target")),
        "the reappeared entries must specifically be inside target/, naming the directory that \
         came back: {:?}",
        baseline.entries
    );
}

/// D1's own "one shared definition, not a second literal list"
/// requirement, checked directly rather than trusted by construction --
/// the explorer's `collapsed_directory_names` and change detection's
/// `ignored_directory_names` must name the exact same directories, in
/// the same order, because both are built from the one
/// `IGNORED_DIRECTORY_NAMES` array. A future edit to one without the
/// other -- the defect this slice exists to prevent -- fails here by
/// name, not just by behaviour drifting apart silently.
#[test]
fn explorer_and_change_detection_share_the_exact_same_ignored_directory_list() {
    let explorer_list =
        crate::project::root::FileExplorerScanPolicy::linux_mvp().collapsed_directory_names;
    let detector_list: Vec<String> = GeneratedChangeDetectionPolicy::default()
        .ignored_directory_names
        .iter()
        .map(|name| (*name).to_owned())
        .collect();

    assert_eq!(explorer_list, detector_list);
    assert_eq!(
        explorer_list,
        crate::project::IGNORED_DIRECTORY_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn detector_suppresses_changed_paths_when_changed_path_limit_is_hit() {
    let sandbox = TestSandbox::new("change-detection-path-limit");
    let project = sandbox.project_session(1);
    let detector = GeneratedChangeDetector::new(GeneratedChangeDetectionPolicy {
        max_entries: 8,
        max_changed_paths: 1,
        ignored_directory_names: crate::project::IGNORED_DIRECTORY_NAMES,
    });
    let baseline = detector.capture_filesystem_baseline(&project);
    sandbox.create_file_with_contents("project/a.txt", b"a\n");
    sandbox.create_file_with_contents("project/b.txt", b"b\n");

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(detected.status, ChangeDetectionStatus::Partial { limit: 1 });
    assert!(detected.changed_paths.is_empty());
}

#[test]
fn detector_suppresses_changed_paths_when_current_scan_fails() {
    let sandbox = TestSandbox::new("change-detection-failed-scan");
    let project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"metadata\n");
    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);
    fs::remove_dir_all(sandbox.path("project")).unwrap();

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(
        detected.status,
        ChangeDetectionStatus::Failed {
            reason: ChangeDetectionFailureReason::RootUnavailable,
        }
    );
    assert!(detected.changed_paths.is_empty());
}

#[test]
fn detector_rejects_cross_project_baselines_without_reporting_paths() {
    let project = project_session(1);
    let other_project = project_session(2);
    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&other_project);

    let detected = detector.detect_filesystem_changes(&project, &baseline);

    assert_eq!(
        detected.status,
        ChangeDetectionStatus::Failed {
            reason: ChangeDetectionFailureReason::CrossProjectBaseline,
        }
    );
    assert!(detected.changed_paths.is_empty());
}

#[test]
fn git_status_detection_reports_unavailable_or_unsupported_without_running_git() {
    let project = project_session(1);
    let detector = GeneratedChangeDetector::default();

    let unavailable = detector.detect_git_status_unavailable(&project);
    let unsupported = detector.detect_git_status_unsupported(&project);

    assert_eq!(unavailable.source, ChangeDetectionSource::GitStatus);
    assert_eq!(unavailable.status, ChangeDetectionStatus::Unavailable);
    assert!(unavailable.changed_paths.is_empty());
    assert_eq!(unsupported.source, ChangeDetectionSource::GitStatus);
    assert_eq!(unsupported.status, ChangeDetectionStatus::Unsupported);
    assert!(unsupported.changed_paths.is_empty());
}

#[test]
fn projectsession_creates_strong_agent_run_changeset_only_from_complete_detection() {
    let sandbox = TestSandbox::new("change-detection-strong-association");
    let mut project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"before\n");
    let detector = GeneratedChangeDetector::default();
    let run = completed_agent_run(project.id().clone());
    let run_id = run.id.clone();
    let baseline = detector.capture_agent_run_filesystem_baseline(&project, run_id.clone());
    project.add_agent_run(run).unwrap();
    fs::write(sandbox.path("project/src/lib.rs"), b"after\n").unwrap();
    let detected = detector.detect_filesystem_changes(&project, &baseline);

    let change_set_id = project
        .add_detected_generated_change_set(
            &baseline,
            &detected,
            Some(&run_id),
            "detected generated changes",
        )
        .expect("complete strongly associated detection should create a ChangeSet")
        .expect("changed paths should create a ChangeSet");

    let change_set = project
        .change_sets()
        .iter()
        .find(|change_set| change_set.id == change_set_id)
        .unwrap();
    assert_eq!(change_set.agent_run_id, Some(run_id.clone()));
    assert_eq!(
        change_set.association_confidence,
        ChangeAssociationConfidence::Strong
    );
    assert_eq!(
        change_set.baseline_snapshot_ref,
        Some(baseline.baseline_snapshot_ref)
    );
    assert_eq!(change_set.changed_files, vec![PathBuf::from("src/lib.rs")]);
    assert_eq!(project.runtime_summary().review_ready_changes, 1);
    assert_eq!(project.close_resource_summary().review_ready_changes, 1);
    assert_eq!(
        project.agent_runs()[0].change_set_ids,
        vec![change_set_id],
        "strong association is the only path that attaches the ChangeSet to the AgentRun"
    );
}

#[test]
fn projectsession_refuses_changeset_creation_from_non_complete_detection() {
    let sandbox = TestSandbox::new("change-detection-non-complete");
    let mut project = sandbox.project_session(1);
    let detector = GeneratedChangeDetector::new(GeneratedChangeDetectionPolicy {
        max_entries: 8,
        max_changed_paths: 1,
        ignored_directory_names: crate::project::IGNORED_DIRECTORY_NAMES,
    });
    let baseline = detector.capture_filesystem_baseline(&project);
    sandbox.create_file_with_contents("project/a.txt", b"a\n");
    sandbox.create_file_with_contents("project/b.txt", b"b\n");
    let detected = detector.detect_filesystem_changes(&project, &baseline);

    let error = project
        .add_detected_generated_change_set(&baseline, &detected, None, "partial changes")
        .expect_err("partial detection should not create review ChangeSets");

    assert_eq!(
        error,
        ProjectChangeSetError::DetectionNotComplete(ChangeDetectionStatus::Partial { limit: 1 })
    );
    assert!(project.change_sets().is_empty());
}

#[test]
fn projectsession_keeps_detached_agentrun_detection_unlinked_and_ambiguous() {
    let sandbox = TestSandbox::new("change-detection-detached-ambiguous");
    let mut project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"before\n");
    let detector = GeneratedChangeDetector::default();
    let run = detached_agent_run(project.id().clone());
    let run_id = run.id.clone();
    let baseline = detector.capture_agent_run_filesystem_baseline(&project, run_id.clone());
    project.add_agent_run(run).unwrap();
    fs::write(sandbox.path("project/src/lib.rs"), b"after\n").unwrap();
    let detected = detector.detect_filesystem_changes(&project, &baseline);

    let change_set_id = project
        .add_detected_generated_change_set(&baseline, &detected, Some(&run_id), "ambiguous changes")
        .expect("ambiguous complete detection should still create an unlinked ChangeSet")
        .unwrap();

    let change_set = project
        .change_sets()
        .iter()
        .find(|change_set| change_set.id == change_set_id)
        .unwrap();
    assert_eq!(change_set.agent_run_id, None);
    assert_eq!(
        change_set.association_confidence,
        ChangeAssociationConfidence::Ambiguous
    );
    assert!(project.agent_runs()[0].change_set_ids.is_empty());
}

#[test]
fn projectsession_blocks_strong_association_when_another_run_is_active() {
    let sandbox = TestSandbox::new("change-detection-overlapping-run");
    let mut project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"before\n");
    let detector = GeneratedChangeDetector::default();
    let target_run = completed_agent_run(project.id().clone());
    let target_run_id = target_run.id.clone();
    let active_run = running_agent_run(project.id().clone());
    let baseline = detector.capture_agent_run_filesystem_baseline(&project, target_run_id.clone());
    project.add_agent_run(target_run).unwrap();
    project.add_agent_run(active_run).unwrap();
    fs::write(sandbox.path("project/src/lib.rs"), b"after\n").unwrap();
    let detected = detector.detect_filesystem_changes(&project, &baseline);

    let change_set_id = project
        .add_detected_generated_change_set(
            &baseline,
            &detected,
            Some(&target_run_id),
            "overlapping changes",
        )
        .expect("complete ambiguous detection should create an unlinked ChangeSet")
        .unwrap();

    let change_set = project
        .change_sets()
        .iter()
        .find(|change_set| change_set.id == change_set_id)
        .unwrap();
    assert_eq!(change_set.agent_run_id, None);
    assert_eq!(
        change_set.association_confidence,
        ChangeAssociationConfidence::Ambiguous
    );
    assert!(
        project
            .agent_runs()
            .iter()
            .find(|run| run.id == target_run_id)
            .unwrap()
            .change_set_ids
            .is_empty()
    );
}

#[test]
fn projectsession_blocks_strong_association_when_another_run_closed_after_baseline() {
    let sandbox = TestSandbox::new("change-detection-since-closed-overlap");
    let mut project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"before\n");
    let detector = GeneratedChangeDetector::default();
    let target_run = completed_agent_run(project.id().clone());
    let target_run_id = target_run.id.clone();
    let baseline = detector.capture_agent_run_filesystem_baseline(&project, target_run_id.clone());
    let mut overlapping_run = completed_agent_run(project.id().clone());
    overlapping_run.ended_at = Some(baseline.captured_at.clone());
    project.add_agent_run(target_run).unwrap();
    project.add_agent_run(overlapping_run).unwrap();
    fs::write(sandbox.path("project/src/lib.rs"), b"after\n").unwrap();
    let detected = detector.detect_filesystem_changes(&project, &baseline);

    let change_set_id = project
        .add_detected_generated_change_set(
            &baseline,
            &detected,
            Some(&target_run_id),
            "temporally overlapping changes",
        )
        .expect("complete temporally ambiguous detection should create an unlinked ChangeSet")
        .unwrap();

    let change_set = project
        .change_sets()
        .iter()
        .find(|change_set| change_set.id == change_set_id)
        .unwrap();
    assert_eq!(change_set.agent_run_id, None);
    assert_eq!(
        change_set.association_confidence,
        ChangeAssociationConfidence::Ambiguous
    );
    assert!(
        project
            .agent_runs()
            .iter()
            .find(|run| run.id == target_run_id)
            .unwrap()
            .change_set_ids
            .is_empty()
    );
}

#[test]
fn projectsession_blocks_strong_association_when_closed_bystander_has_unknown_end_time() {
    let sandbox = TestSandbox::new("change-detection-closed-unknown-end");
    let mut project = sandbox.project_session(1);
    sandbox.create_file_with_contents("project/src/lib.rs", b"before\n");
    let detector = GeneratedChangeDetector::default();
    let target_run = completed_agent_run(project.id().clone());
    let target_run_id = target_run.id.clone();
    let baseline = detector.capture_agent_run_filesystem_baseline(&project, target_run_id.clone());
    let completed_bystander = completed_agent_run(project.id().clone());
    assert!(
        completed_bystander.ended_at.is_none(),
        "normal AgentRun lifecycle currently leaves ended_at unset"
    );
    project.add_agent_run(target_run).unwrap();
    project.add_agent_run(completed_bystander).unwrap();
    fs::write(sandbox.path("project/src/lib.rs"), b"after\n").unwrap();
    let detected = detector.detect_filesystem_changes(&project, &baseline);

    let change_set_id = project
        .add_detected_generated_change_set(
            &baseline,
            &detected,
            Some(&target_run_id),
            "unknown end-time overlap",
        )
        .expect("unknown closed-run ordering should create an unlinked ambiguous ChangeSet")
        .unwrap();

    let change_set = project
        .change_sets()
        .iter()
        .find(|change_set| change_set.id == change_set_id)
        .unwrap();
    assert_eq!(change_set.agent_run_id, None);
    assert_eq!(
        change_set.association_confidence,
        ChangeAssociationConfidence::Ambiguous
    );
    assert!(
        project
            .agent_runs()
            .iter()
            .find(|run| run.id == target_run_id)
            .unwrap()
            .change_set_ids
            .is_empty()
    );
}

#[test]
fn projectsession_revalidates_detector_paths_before_changeset_creation() {
    let sandbox = TestSandbox::new("change-detection-revalidate");
    let mut project = sandbox.project_session(1);
    let detector = GeneratedChangeDetector::default();
    let baseline = detector.capture_filesystem_baseline(&project);
    let detected = DetectedChanges {
        project_id: project.id().clone(),
        source: ChangeDetectionSource::FilesystemSnapshot,
        baseline_snapshot_ref: Some(baseline.baseline_snapshot_ref.clone()),
        changed_paths: vec![DetectedChangedPath {
            relative_path: PathBuf::from("../outside.rs"),
            kind: ChangePathKind::File,
            lifecycle: ChangeLifecycle::Added,
        }],
        status: ChangeDetectionStatus::Complete,
        scanned_entry_count: 1,
    };

    let error = project
        .add_detected_generated_change_set(&baseline, &detected, None, "invalid path")
        .expect_err("ProjectSession must not trust detector payloads without validation");

    assert!(matches!(
        error,
        ProjectChangeSetError::InvalidChangedPath(error)
            if error.reason == ChangedPathValidationErrorReason::InvalidRelativePath
    ));
    assert!(project.change_sets().is_empty());
}

struct TestSandbox {
    root: PathBuf,
}

impl TestSandbox {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("project")).unwrap();
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn create_dir(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir(&path).unwrap();
        path
    }

    fn create_file_with_contents(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
        path
    }

    fn project_session(&self, sequence: u64) -> ProjectSession {
        let root_path = self.path("project");
        ProjectSession::new(
            ProjectId::for_test(sequence),
            format!("Project {sequence}"),
            root_path.clone(),
            fs::canonicalize(root_path).unwrap(),
        )
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn completed_agent_run(project_id: ProjectId) -> AgentRun {
    let mut run = AgentRun::draft(
        project_id,
        "plain",
        "generate changes",
        AgentCompatibilityLevel::Plain,
    );
    run.transition_to(AgentRunStatus::Ready).unwrap();
    run.transition_to(AgentRunStatus::Preparing).unwrap();
    run.transition_to(AgentRunStatus::Running).unwrap();
    run.transition_to(AgentRunStatus::Completed).unwrap();
    run
}

fn detached_agent_run(project_id: ProjectId) -> AgentRun {
    let mut run = AgentRun::draft(
        project_id,
        "plain",
        "generate changes",
        AgentCompatibilityLevel::Plain,
    );
    run.transition_to(AgentRunStatus::Ready).unwrap();
    run.transition_to(AgentRunStatus::Preparing).unwrap();
    run.transition_to(AgentRunStatus::Running).unwrap();
    run.transition_to(AgentRunStatus::Detached).unwrap();
    run
}

fn running_agent_run(project_id: ProjectId) -> AgentRun {
    let mut run = AgentRun::draft(
        project_id,
        "plain",
        "generate changes",
        AgentCompatibilityLevel::Plain,
    );
    run.transition_to(AgentRunStatus::Ready).unwrap();
    run.transition_to(AgentRunStatus::Preparing).unwrap();
    run.transition_to(AgentRunStatus::Running).unwrap();
    run
}
