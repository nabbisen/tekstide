use super::AppState;
use crate::app::{AddProjectOutcome, RemoveProjectError};
use crate::close::{
    CloseAssessment, CloseReason, CloseReasonCode, CloseResourceProviderState, CloseResourceSummary,
};
use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunStatus, DomainTimestamp, TerminalKind,
    TerminalSession, TerminalStatus,
};
use crate::project::recent::{RecentProject, RecentProjectState, Timestamp};
use crate::project::root::{ProjectRootValidationError, SymlinkPolicy};
use crate::project::{ProjectId, ProjectProviderState, ProjectRuntimeSummary};
use crate::runtime::terminal::TerminationOutcome;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn new_state_has_no_projects() {
    let state = AppState::default();

    assert!(state.projects().is_empty());
    assert!(state.active_project_id().is_none());
}

#[test]
fn first_project_becomes_active() {
    let mut state = AppState::default();

    let project_id =
        state.add_project_session("Tekstide", "/workspace/tekstide", "/workspace/tekstide");

    assert_eq!(state.active_project_id(), Some(&project_id));
    assert_eq!(
        state
            .active_project()
            .expect("first project should be active")
            .display_name(),
        "Tekstide"
    );
}

#[test]
fn switching_active_project_requires_existing_project_id() {
    let mut state = AppState::default();
    let first_id = state.add_project_session("First", "/workspace/first", "/workspace/first");
    let second_id = state.add_project_session("Second", "/workspace/second", "/workspace/second");

    assert!(state.switch_active_project(&second_id));
    assert_eq!(state.active_project_id(), Some(&second_id));

    let missing_id = ProjectId::for_test(999);
    assert!(!state.switch_active_project(&missing_id));
    assert_eq!(state.active_project_id(), Some(&second_id));
    assert_ne!(first_id, second_id);
}

#[test]
fn add_project_from_path_validates_and_restricts_before_display() {
    let sandbox = TestSandbox::new("add-valid-project");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();

    let outcome = state
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    let project_id = match outcome {
        AddProjectOutcome::Added(project_id) => project_id,
        AddProjectOutcome::FocusedExisting(_) => panic!("new project should be added"),
    };
    let project = state
        .project(&project_id)
        .expect("added project should be available");
    assert_eq!(project.display_name(), "project");
    assert_eq!(project.root_path(), &project_dir);
    assert_eq!(
        project.canonical_root_path(),
        &fs::canonicalize(&project_dir).unwrap()
    );
    assert_eq!(project.trust_state().label(), "Restricted");
}

#[test]
fn duplicate_canonical_root_focuses_existing_project() {
    let sandbox = TestSandbox::new("duplicate-canonical");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();

    let first = state
        .add_project_from_path(&project_dir)
        .expect("first add should succeed");
    let second = state
        .add_project_from_path(&project_dir)
        .expect("duplicate add should focus existing project");

    assert!(matches!(first, AddProjectOutcome::Added(_)));
    assert_eq!(
        second,
        AddProjectOutcome::FocusedExisting(first.project_id().clone())
    );
    assert_eq!(state.projects().len(), 1);
    assert_eq!(state.recent_projects().len(), 1);
    assert_eq!(state.active_project_id(), Some(first.project_id()));
}

#[test]
fn add_project_from_path_rejects_files() {
    let sandbox = TestSandbox::new("reject-file");
    let file_path = sandbox.create_file("file.txt");
    let mut state = AppState::default();

    let error = state
        .add_project_from_path(&file_path)
        .expect_err("file path should be rejected");

    assert_eq!(
        error,
        ProjectRootValidationError::NotDirectory { path: file_path }
    );
    assert!(state.projects().is_empty());
}

#[cfg(unix)]
#[test]
fn add_project_from_path_fails_closed_on_symlink_root() {
    let sandbox = TestSandbox::new("reject-symlink");
    let target_dir = sandbox.create_dir("target");
    let link_path = sandbox.path("link");
    std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();
    let mut state = AppState::default();

    let error = state
        .add_project_from_path(&link_path)
        .expect_err("symlink should require explicit confirmation");

    assert_eq!(
        error,
        ProjectRootValidationError::SymlinkAmbiguous {
            selected_path: link_path,
            canonical_path: fs::canonicalize(target_dir).unwrap(),
        }
    );
    assert!(state.projects().is_empty());
}

#[cfg(unix)]
#[test]
fn confirmed_symlink_root_creates_session_for_canonical_identity() {
    let sandbox = TestSandbox::new("confirm-symlink");
    let target_dir = sandbox.create_dir("target");
    let link_path = sandbox.path("link");
    std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();
    let mut state = AppState::default();

    let outcome = state
        .add_project_from_path_with_symlink_policy(&link_path, SymlinkPolicy::AllowCanonicalTarget)
        .expect("confirmed symlink should be added");

    let project = state.project(outcome.project_id()).unwrap();
    assert_eq!(project.root_path(), &link_path);
    assert_eq!(
        project.canonical_root_path(),
        &fs::canonicalize(target_dir).unwrap()
    );
}

#[test]
fn recent_project_state_uses_uuid_project_ids() {
    let sandbox = TestSandbox::new("uuid-recent");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();

    state
        .add_project_from_path(&project_dir)
        .expect("valid project should be added");

    let recent_state = state.recent_project_state();
    let project_id = recent_state.projects[0].project_id.as_str();
    assert_eq!(project_id.len(), 36);
    assert_eq!(project_id.as_bytes()[14], b'4');
}

#[test]
fn recent_project_state_exports_project_session_timestamps() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");
    let opened_at = DomainTimestamp::from_utc_string("2026-07-05T01:02:03Z").unwrap();
    let activity_at = DomainTimestamp::from_utc_string("2026-07-05T04:05:06Z").unwrap();
    let project = state.project_mut(&project_id).unwrap();
    project.mark_opened_at(opened_at);
    project.record_activity_at(activity_at);

    let recent_state = state.recent_project_state();
    let recent_project = &recent_state.projects[0];

    assert_eq!(
        recent_project.last_opened_at.as_str(),
        "2026-07-05T01:02:03Z"
    );
    assert_eq!(
        recent_project.last_activity.as_str(),
        "2026-07-05T04:05:06Z"
    );
}

#[test]
fn restored_recent_project_id_is_reused_when_project_is_added_again() {
    let sandbox = TestSandbox::new("reuse-restored-id");
    let project_dir = sandbox.create_dir("project");
    let canonical = fs::canonicalize(&project_dir).unwrap();
    let restored_id = ProjectId::for_test(42);
    let mut state = AppState::default();
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            restored_id.clone(),
            "project",
            project_dir.clone(),
            canonical,
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            crate::project::WorkspaceTrust::Trusted,
        )],
    });

    let outcome = state
        .add_project_from_path(&project_dir)
        .expect("restored project should validate");

    assert_eq!(outcome, AddProjectOutcome::Added(restored_id.clone()));
    assert_eq!(state.active_project_id(), Some(&restored_id));
    assert_eq!(state.projects().len(), 1);
    assert_eq!(state.recent_projects().len(), 1);
    assert_eq!(
        state.projects()[0].trust_state().label(),
        "Trusted",
        "RFC-032: reopening at the exact same canonical path a prior session trusted must \
         restore that trust, not silently drop back to Restricted -- persistence bound to the \
         canonical path is the whole point of this slice"
    );
}

// --- RFC-032 PR-032-B: persistence, bound to the canonical path -------
//
// The review gate's own words: proven against a **real symlink**,
// redirected for real between sessions -- not a synthesised path
// string. `restored_recent_project_id_is_reused_when_project_is_added_again`
// above already proves the mechanism for an unchanged, non-symlinked
// path; these four prove the actual binding decision RFC-032 makes.

/// **Positive control, required before the negative case means
/// anything.** Without this, `a_redirected_symlink_is_not_trusted_on_reopen`
/// passing would be equally consistent with "nothing is ever trusted on
/// reopen" as with "redirection specifically breaks trust."
#[cfg(unix)]
#[test]
fn an_unredirected_symlinked_project_is_still_trusted_on_reopen() {
    let sandbox = TestSandbox::new("trust-unredirected");
    let target = sandbox.create_dir("target");
    let link = sandbox.path("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let mut first_session = AppState::default();
    let outcome = first_session
        .add_project_from_path_with_symlink_policy(&link, SymlinkPolicy::AllowCanonicalTarget)
        .expect("symlinked root should be added with explicit confirmation");
    first_session
        .project_mut(outcome.project_id())
        .unwrap()
        .grant_trust("test grant");
    let persisted = first_session.recent_project_state();

    let mut second_session = AppState::default();
    second_session.restore_recent_projects(persisted);
    let reopened = second_session
        .add_project_from_path_with_symlink_policy(&link, SymlinkPolicy::AllowCanonicalTarget)
        .expect("reopening the same, unredirected symlink should validate");

    assert_eq!(
        second_session
            .project(reopened.project_id())
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "a symlink still resolving to the same real folder must keep its trust across sessions"
    );
}

/// **The review gate's own falsifiable claim**: a real symlink,
/// redirected for real to a *different* real folder between the grant
/// and the reopen, must not carry the old grant over -- the fresh
/// canonical resolution no longer matches what the persisted entry was
/// keyed by, so the lookup finds nothing and the reopened session keeps
/// `ProjectSession::new`'s own `Restricted` default.
#[cfg(unix)]
#[test]
fn a_redirected_symlink_is_not_trusted_on_reopen() {
    let sandbox = TestSandbox::new("trust-redirected");
    let original_target = sandbox.create_dir("original-target");
    let redirected_target = sandbox.create_dir("redirected-target");
    let link = sandbox.path("link");
    std::os::unix::fs::symlink(&original_target, &link).unwrap();

    let mut first_session = AppState::default();
    let outcome = first_session
        .add_project_from_path_with_symlink_policy(&link, SymlinkPolicy::AllowCanonicalTarget)
        .expect("symlinked root should be added with explicit confirmation");
    first_session
        .project_mut(outcome.project_id())
        .unwrap()
        .grant_trust("test grant");
    let persisted = first_session.recent_project_state();
    assert_eq!(
        persisted.projects[0].trust_state,
        crate::project::WorkspaceTrust::Trusted,
        "test precondition: the grant must actually be in the persisted snapshot"
    );

    // Redirect the *same* link path to a genuinely different real folder
    // -- a real filesystem operation, not a second, synthesised
    // canonical-path string standing in for one.
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&redirected_target, &link).unwrap();
    assert_ne!(
        fs::canonicalize(&link).unwrap(),
        fs::canonicalize(&original_target).unwrap(),
        "test precondition: the redirect must actually change what the link resolves to"
    );

    let mut second_session = AppState::default();
    second_session.restore_recent_projects(persisted);
    let reopened = second_session
        .add_project_from_path_with_symlink_policy(&link, SymlinkPolicy::AllowCanonicalTarget)
        .expect("reopening the redirected symlink should still validate as a project root");

    assert_eq!(
        second_session
            .project(reopened.project_id())
            .unwrap()
            .trust_state()
            .label(),
        "Restricted",
        "a symlink redirected to a different real folder must not inherit the old grant -- an \
         entirely different folder's contents must not run with the trust granted to something \
         else"
    );
}

/// **Ablation, per the review gate**: if trust were bound to the
/// literal path (what `RecentProject::root_path` stores, "as opened")
/// rather than the canonical path, the exact redirected-symlink scenario
/// above *would* inherit the old grant, because the literal path string
/// (the symlink itself) never changes -- only what it resolves to does.
/// This does not exercise production code (which correctly uses
/// `canonical_root_path`, proven above) -- it exercises the same lookup
/// shape keyed on the wrong field, against the same real, redirected
/// fixture, to show the specific divergence the real implementation
/// avoids.
#[cfg(unix)]
#[test]
fn ablation_binding_trust_to_the_literal_path_would_inherit_a_redirected_symlinks_trust() {
    let sandbox = TestSandbox::new("trust-ablation");
    let original_target = sandbox.create_dir("original-target");
    let redirected_target = sandbox.create_dir("redirected-target");
    let link = sandbox.path("link");
    std::os::unix::fs::symlink(&original_target, &link).unwrap();

    let mut first_session = AppState::default();
    let outcome = first_session
        .add_project_from_path_with_symlink_policy(&link, SymlinkPolicy::AllowCanonicalTarget)
        .expect("symlinked root should be added with explicit confirmation");
    first_session
        .project_mut(outcome.project_id())
        .unwrap()
        .grant_trust("test grant");
    let persisted = first_session.recent_project_state();

    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&redirected_target, &link).unwrap();

    // The wrong lookup: keyed on `root_path` (the literal path as
    // opened -- here, the symlink itself, which is unchanged by the
    // redirect) instead of `canonical_root_path`.
    let literal_path_trust = persisted
        .projects
        .iter()
        .find(|project| project.root_path == link)
        .map(|project| project.trust_state);

    assert_eq!(
        literal_path_trust,
        Some(crate::project::WorkspaceTrust::Trusted),
        "binding by the literal path would find the old grant even after the redirect -- this \
         is the specific failure canonical-path binding exists to prevent, demonstrated here \
         rather than assumed"
    );
}

/// **Revocation must clear persisted state, not only the in-memory
/// one — proven across a reopen**, not merely by reading the in-memory
/// `trust_state()` right after `revoke_trust` is called.
#[test]
fn revoking_trust_persists_and_survives_a_reopen() {
    let sandbox = TestSandbox::new("trust-revoke");
    let project_dir = sandbox.create_dir("project");

    let mut first_session = AppState::default();
    let outcome = first_session
        .add_project_from_path(&project_dir)
        .expect("plain project root should be added");
    first_session
        .project_mut(outcome.project_id())
        .unwrap()
        .grant_trust("test grant");
    let trusted_snapshot = first_session.recent_project_state();
    assert_eq!(
        trusted_snapshot.projects[0].trust_state,
        crate::project::WorkspaceTrust::Trusted,
        "test precondition: the grant must actually be in the persisted snapshot"
    );

    first_session
        .project_mut(outcome.project_id())
        .unwrap()
        .revoke_trust("test revocation");
    let revoked_snapshot = first_session.recent_project_state();
    assert_ne!(
        revoked_snapshot.projects[0].trust_state,
        crate::project::WorkspaceTrust::Trusted,
        "the persisted snapshot must reflect the revocation, not the stale grant -- this is the \
         in-memory half; the reopen below is the half that actually crosses a session boundary"
    );

    let mut second_session = AppState::default();
    second_session.restore_recent_projects(revoked_snapshot);
    let reopened = second_session
        .add_project_from_path(&project_dir)
        .expect("reopening the same project should validate");

    assert_ne!(
        second_session
            .project(reopened.project_id())
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "a revoked project reopened in a fresh session must not come back trusted -- revocation \
         that only lasts until the next restart is not revocation"
    );
}

/// RFC-033 PR-033-B: mirrors `revoking_trust_persists_and_survives_a_reopen`
/// exactly, for the transcript-capture opt-out -- a setting that only lasts
/// until the next restart is not durable, and the acceptance checklist
/// requires the opt-out to survive one.
#[test]
fn declining_transcript_capture_persists_and_survives_a_reopen() {
    let sandbox = TestSandbox::new("capture-decline-reopen");
    let project_dir = sandbox.create_dir("project");

    let mut first_session = AppState::default();
    let outcome = first_session
        .add_project_from_path(&project_dir)
        .expect("plain project root should be added");
    first_session
        .project_mut(outcome.project_id())
        .unwrap()
        .set_transcript_capture_declined(true);
    let declined_snapshot = first_session.recent_project_state();
    assert!(
        declined_snapshot.projects[0].transcript_capture_declined,
        "test precondition: the decline must actually be in the persisted snapshot"
    );

    let mut second_session = AppState::default();
    second_session.restore_recent_projects(declined_snapshot);
    let reopened = second_session
        .add_project_from_path(&project_dir)
        .expect("reopening the same project should validate");

    assert!(
        second_session
            .project(reopened.project_id())
            .unwrap()
            .transcript_capture_declined(),
        "a declined-capture project reopened in a fresh session must still be declined -- an \
         opt-out that only lasts until the next restart is not an opt-out"
    );
}

#[test]
fn active_project_with_missing_close_provider_is_not_closed() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");

    let assessment = state
        .close_project(&project_id)
        .expect("assessment should be returned");

    assert_eq!(
        assessment,
        CloseAssessment::UnsupportedOrUnknown {
            reason: "active-resource state is unavailable".to_owned()
        }
    );
    assert!(state.project(&project_id).is_some());
}

#[test]
fn active_idle_project_closes_when_provider_proves_safe() {
    let sandbox = TestSandbox::new("close-idle");
    let project_dir = sandbox.create_dir("project");
    let mut state = AppState::default();
    let project_id = state
        .add_project_from_path(&project_dir)
        .expect("project should add")
        .project_id()
        .clone();
    state
        .project_mut(&project_id)
        .unwrap()
        .set_runtime_summary(ProjectRuntimeSummary {
            close_resources: CloseResourceSummary {
                provider_state: CloseResourceProviderState::Complete,
                running_processes: 0,
                dirty_files: 0,
                pending_approvals: 0,
                review_ready_changes: 0,
            },
            ..ProjectRuntimeSummary::default()
        });

    let assessment = state
        .close_project(&project_id)
        .expect("safe close should remove session");

    assert_eq!(assessment, CloseAssessment::SafeToClose);
    assert!(state.project(&project_id).is_none());
    assert!(
        project_dir.exists(),
        "close must not delete workspace contents"
    );
    assert!(
        state
            .recent_projects()
            .iter()
            .any(|restored| restored.recent_project.project_id == project_id),
        "closing active session should preserve recent entry"
    );
}

#[test]
fn active_project_with_resources_needs_confirmation_and_stays_open() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");
    state
        .project_mut(&project_id)
        .unwrap()
        .set_runtime_summary(ProjectRuntimeSummary {
            close_resources: CloseResourceSummary {
                provider_state: CloseResourceProviderState::Complete,
                running_processes: 1,
                dirty_files: 2,
                pending_approvals: 0,
                review_ready_changes: 0,
            },
            ..ProjectRuntimeSummary::default()
        });

    let assessment = state
        .close_project(&project_id)
        .expect("assessment should be returned");

    assert_eq!(
        assessment,
        CloseAssessment::NeedsConfirmation {
            reasons: vec![
                CloseReason {
                    code: CloseReasonCode::RunningProcess,
                    message: "1 running process".to_owned(),
                },
                CloseReason {
                    code: CloseReasonCode::DirtyFile,
                    message: "2 dirty files".to_owned(),
                },
            ]
        }
    );
    assert!(state.project(&project_id).is_some());
}

#[test]
fn active_project_with_real_running_terminal_needs_confirmation_and_stays_open() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");
    let mut terminal = TerminalSession::new(
        project_id.clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project",
        "bash",
    );
    terminal.transition_to(TerminalStatus::Running).unwrap();
    state
        .project_mut(&project_id)
        .unwrap()
        .add_terminal_session(terminal)
        .unwrap();

    let assessment = state
        .close_project(&project_id)
        .expect("assessment should be returned");

    assert_eq!(
        assessment,
        CloseAssessment::NeedsConfirmation {
            reasons: vec![
                CloseReason {
                    code: CloseReasonCode::RunningProcess,
                    message: "1 running process".to_owned(),
                },
                CloseReason {
                    code: CloseReasonCode::ProviderUnavailable,
                    message: "active-resource state is unavailable".to_owned(),
                },
            ],
        }
    );
    assert!(state.project(&project_id).is_some());
}

/// RFC-039 PR-039-C: the one property [`active_project_with_real_running_terminal_needs_confirmation_and_stays_open`]
/// above cannot show -- `apply_agent_terminal_outcome` (the pre-existing
/// wrapper) is hardcoded to `active_project_mut()`, so it can only ever
/// be exercised against whichever project happens to be active.
/// `apply_agent_terminal_outcome_for_project` exists specifically so
/// closing a *different* tab's project can retire its own live agent
/// run -- this proves that project-scoped path completes the run and
/// clears `running_processes` on a project that is not, and never
/// becomes, the active one.
#[test]
fn apply_agent_terminal_outcome_for_project_completes_a_run_on_a_project_that_is_not_active() {
    let mut state = AppState::default();
    let active_id = state.add_project_session("Active", "/workspace/active", "/workspace/active");
    let background_id = state.add_project_session(
        "Background",
        "/workspace/background",
        "/workspace/background",
    );
    assert_eq!(
        state.active_project_id(),
        Some(&active_id),
        "test precondition: the first project added is active, not the second"
    );

    let mut terminal = TerminalSession::new(
        background_id.clone(),
        TerminalKind::Plain,
        "Agent",
        "/workspace/background",
        "bash",
    );
    terminal.transition_to(TerminalStatus::Running).unwrap();
    let terminal_id = terminal.id.clone();

    let agent_run = AgentRun {
        terminal_id: Some(terminal_id.clone()),
        status: AgentRunStatus::Running,
        ..AgentRun::draft(
            background_id.clone(),
            "profile",
            "prompt",
            AgentCompatibilityLevel::Supervised,
        )
    };
    let agent_run_id = agent_run.id.clone();

    let background = state.project_mut(&background_id).unwrap();
    background.add_terminal_session(terminal).unwrap();
    background.add_agent_run(agent_run).unwrap();
    assert_eq!(
        background
            .runtime_summary()
            .close_resources
            .running_processes,
        1,
        "test precondition: the background project's own terminal reads as a live process"
    );

    state
        .apply_agent_terminal_outcome_for_project(
            &background_id,
            &agent_run_id,
            &terminal_id,
            &TerminationOutcome::Exited { exit_status: 0 },
        )
        .expect("a clean exit should complete the run on the non-active project");

    assert_eq!(
        state.active_project_id(),
        Some(&active_id),
        "completing a background run must not change which project is active"
    );
    let background = state.project(&background_id).unwrap();
    let run = background
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .unwrap();
    assert_eq!(run.status, AgentRunStatus::Completed);
    assert_eq!(
        background
            .runtime_summary()
            .close_resources
            .running_processes,
        0,
        "the run's own terminal exiting must clear the background project's running-process count"
    );
}

#[test]
fn active_text_document_dirty_state_updates_project_runtime_summary() {
    let sandbox = TestSandbox::new("content-dirty-runtime");
    let project_dir = sandbox.create_dir("project");
    fs::write(project_dir.join("file.txt"), b"original\n").unwrap();
    let mut state = AppState::default();
    let project_id = state
        .add_project_from_path(&project_dir)
        .expect("project should add")
        .project_id()
        .clone();

    state
        .open_active_project_text_document("file.txt")
        .expect("text file should open");
    state
        .replace_active_project_text("changed\n")
        .expect("text edit should succeed");
    let project = state.project(&project_id).unwrap();

    assert_eq!(
        project.file_state().provider_state,
        ProjectProviderState::Complete
    );
    assert_eq!(project.file_state().open_buffer_count, 1);
    assert_eq!(project.file_state().dirty_file_count, 1);
    assert_eq!(project.runtime_summary().dirty_files, 1);
    assert_eq!(project.close_resource_summary().dirty_files, 1);

    state
        .save_active_project_text_document()
        .expect("save should clear dirty state");
    let project = state.project(&project_id).unwrap();
    assert_eq!(project.file_state().dirty_file_count, 0);
    assert_eq!(project.runtime_summary().dirty_files, 0);
}

#[test]
fn stale_recent_project_removal_only_removes_recent_metadata() {
    let sandbox = TestSandbox::new("remove-stale-recent");
    let missing_path = sandbox.path("missing");
    let recent_id = ProjectId::for_test(77);
    let mut state = AppState::default();
    state.restore_recent_projects(RecentProjectState {
        state_version: 1,
        projects: vec![RecentProject::new(
            recent_id.clone(),
            "Missing",
            missing_path.clone(),
            missing_path,
            Timestamp::from_persisted("2026-07-04T00:00:00Z"),
            crate::project::WorkspaceTrust::Restricted,
        )],
    });

    state
        .remove_recent_project(&recent_id)
        .expect("stale recent entry should be removable");

    assert!(state.recent_projects().is_empty());
    assert!(state.projects().is_empty());
}

#[test]
fn active_project_cannot_be_removed_as_recent_metadata() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");

    let error = state
        .remove_recent_project(&project_id)
        .expect_err("active project requires close flow");

    assert_eq!(error, RemoveProjectError::ProjectIsActive);
    assert!(state.project(&project_id).is_some());
}

// --- RFC-017 PR-017-E: attach_terminal_session / assign_terminal_visible_slot ---

#[test]
fn attach_terminal_session_registers_it_on_the_active_project() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");
    let terminal = TerminalSession::new(
        project_id.clone(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project",
        "/bin/sh",
    );
    let terminal_id = terminal.id.clone();

    state
        .attach_terminal_session(terminal)
        .expect("attaching a terminal owned by the active project must succeed");

    assert!(
        state
            .project(&project_id)
            .unwrap()
            .terminal_sessions()
            .iter()
            .any(|session| session.id == terminal_id)
    );
}

#[test]
fn attach_terminal_session_fails_closed_with_no_active_project() {
    let mut state = AppState::default();
    let terminal = TerminalSession::new(
        ProjectId::new_uuid(),
        TerminalKind::Plain,
        "Shell",
        "/workspace/project",
        "/bin/sh",
    );

    assert_eq!(
        state.attach_terminal_session(terminal),
        Err(crate::project::ProjectTerminalError::Ownership(
            crate::domain::OwnershipError::MissingProject
        )),
        "no active project must be a real, distinguishable error, not a silent no-op"
    );
}

#[test]
fn assign_terminal_visible_slot_enforces_at_most_one_terminal_per_slot() {
    let mut state = AppState::default();
    let project_id =
        state.add_project_session("Project", "/workspace/project", "/workspace/project");
    let first = TerminalSession::new(
        project_id.clone(),
        TerminalKind::Plain,
        "First",
        "/workspace/project",
        "/bin/sh",
    );
    let second = TerminalSession::new(
        project_id.clone(),
        TerminalKind::Plain,
        "Second",
        "/workspace/project",
        "/bin/sh",
    );
    let first_id = first.id.clone();
    let second_id = second.id.clone();
    state.attach_terminal_session(first).unwrap();
    state.attach_terminal_session(second).unwrap();

    state
        .assign_terminal_visible_slot(&first_id, crate::domain::VisibleSlot::Primary)
        .unwrap();
    state
        .assign_terminal_visible_slot(&second_id, crate::domain::VisibleSlot::Primary)
        .unwrap();

    let project = state.project(&project_id).unwrap();
    let find = |id: &crate::domain::TerminalId| {
        project
            .terminal_sessions()
            .iter()
            .find(|session| &session.id == id)
            .unwrap()
    };
    assert_eq!(
        find(&second_id).visible_slot(),
        crate::domain::VisibleSlot::Primary
    );
    assert_eq!(
        find(&first_id).visible_slot(),
        crate::domain::VisibleSlot::Hidden,
        "assigning Primary to the second terminal must bump the first back to Hidden -- at \
         most one terminal per non-Hidden slot"
    );
}

#[test]
fn assign_terminal_visible_slot_fails_closed_with_no_active_project() {
    let mut state = AppState::default();
    let terminal_id = crate::domain::TerminalId::new_uuid();

    assert_eq!(
        state.assign_terminal_visible_slot(&terminal_id, crate::domain::VisibleSlot::Primary),
        Err(crate::project::ProjectTerminalError::Ownership(
            crate::domain::OwnershipError::MissingProject
        ))
    );
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

    fn create_file(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, b"not a directory").unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// **Response 246**: verification (`crates/tekstide`'s
/// `verify_restored_trust`) and restoration (`restore_trust_state`) are
/// separated -- safe today only because `add_project_session` is
/// `restore_trust_state`'s one and only caller, so every restoration is
/// followed by verification at the next `State::new`. A second
/// production call site would restore trust nothing has verified, with
/// no compile error and no obviously-failing test to notice. Pinning
/// the count here means a second call site fails *this* test, by name,
/// forcing whoever adds it to decide about verification rather than
/// silently inherit the gap -- the same shape
/// `only_two_named_production_call_sites_ever_append_to_a_transcript_writer`
/// (`runtime::terminal::reader::tests`) already uses for exactly this
/// reason.
#[test]
fn only_one_production_call_site_ever_restores_a_projects_trust_state() {
    // Leading `.`, matching call syntax (`project.restore_trust_state(...)`)
    // specifically -- without it, this would also match `session.rs`'s own
    // `fn restore_trust_state(&mut self, ...)` definition line, which is
    // not a call.
    let occurrences = count_occurrences_in_crate(".restore_trust_state(");
    assert_eq!(
        occurrences,
        vec![("app.rs".to_string(), 1)],
        "exactly this one file may ever call ProjectSession::restore_trust_state -- a second \
         call site restores a cache-sourced trust decision with no corresponding \
         verify_restored_trust pass unless that caller is deliberately made to run through one: \
         {occurrences:?}"
    );
}

fn count_occurrences_in_crate(needle: &str) -> Vec<(String, usize)> {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    let mut counts: Vec<(String, usize)> = files
        .into_iter()
        .filter_map(|path| {
            let relative = relative_to_src(&path);
            if relative.contains("/tests/") || relative.ends_with("tests.rs") {
                return None;
            }
            let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
            let count = source.matches(needle).count();
            (count > 0).then_some((relative, count))
        })
        .collect();
    counts.sort();
    counts
}

fn crate_src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn relative_to_src(path: &std::path::Path) -> String {
    path.strip_prefix(crate_src_dir())
        .expect("file must be under src/")
        .to_str()
        .expect("path must be valid UTF-8")
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}
