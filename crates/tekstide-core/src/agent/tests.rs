use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::approval::{ApprovalCoordinator, DecideOutcome, ReceiveOutcome, SimpleDecision};
use crate::content::{SaveDecision, TextDocumentState};
use crate::domain::{
    AgentCompatibilityLevel, AgentRunId, AgentRunStatus, OwnershipError, TerminalId, TerminalKind,
    TerminalSession, TerminalStatus, TruncationState,
};
use crate::project::{
    ProjectActiveFileLaunchBlockReason, ProjectAgentActiveFileLaunchError, ProjectAgentLaunchError,
    ProjectAgentRuntimeLaunchError, ProjectContentError, ProjectId, ProjectSession,
};
use crate::runtime::terminal::{
    BoundedRuntimeSummary, LinuxTerminalRuntime, TerminalEnvironmentPolicy, TerminalLaunchError,
    TerminalRuntimeEvent, TerminalRuntimeHandle, TerminationOutcome, TerminationSignal,
};
use crate::transcript::{
    TranscriptCaptureMode, TranscriptPathRequest, TranscriptPathResolver, TranscriptRetentionLimits,
};

use super::{
    AgentRunLaunchPlan, AgentRunLaunchRequest, AgentRunLaunchValidationError,
    AgentRunLaunchValidator, AgentRunTranscriptCaptureError, AiCliAdapterCapabilities,
    AiCliEnvironmentPolicy, AiCliExecutable, AiCliExecutableProvenance, AiCliProfile,
    AiCliProfileSource, AiCliPromptPolicy, AiCliWorkspaceDiscoveryPolicy, ExecutableLookupPath,
};

#[test]
fn built_in_profile_validates_in_restricted_mode_with_reviewed_system_executable() {
    let root = test_root("agent-valid-restricted");
    let bin = test_root("agent-valid-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&executable);
    let request = AgentRunLaunchRequest::new(
        project.id().clone(),
        profile.id.clone(),
        "summarize current project",
    );

    let validation = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect("reviewed built-in profile should validate");

    assert_eq!(validation.project_id(), project.id());
    assert_eq!(validation.profile_id(), "builtin-ai");
    assert_eq!(
        validation.executable_path(),
        executable.canonicalize().unwrap().as_path()
    );
    assert_eq!(
        validation.executable_provenance(),
        AiCliExecutableProvenance::SystemPathReviewed
    );
    assert_eq!(validation.cwd(), root.canonicalize().unwrap().as_path());
    assert_eq!(
        validation.compatibility_level(),
        AgentCompatibilityLevel::Supervised
    );
    assert_eq!(validation.prompt_summary(), "summarize current project");
    assert_eq!(
        validation.workspace_discovery_summary().as_str(),
        "CLI started with reviewed flag that disables project config discovery"
    );

    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn restricted_mode_rejects_workspace_local_profile_source() {
    let root = test_root("agent-workspace-profile");
    let executable = executable_file(&root, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.source = AiCliProfileSource::WorkspaceLocal;
    let request = request_for(&project, &profile);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("workspace-local profile source must be blocked");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::WorkspaceLocalProfileBlocked
    );
    cleanup_root(root);
}

#[test]
fn restricted_mode_rejects_workspace_local_executable_path() {
    let root = test_root("agent-workspace-executable");
    let executable = executable_file(&root, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.executable = AiCliExecutable::Absolute {
        path: executable.clone(),
        provenance: AiCliExecutableProvenance::WorkspaceLocal,
    };
    let request = request_for(&project, &profile);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("workspace-local executable must be blocked");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::WorkspaceLocalExecutableBlocked {
            path: executable.canonicalize().unwrap(),
        }
    );
    cleanup_root(root);
}

#[test]
fn restricted_mode_rejects_symlink_target_inside_project_root() {
    let root = test_root("agent-symlink-target");
    let outside = test_root("agent-symlink-outside");
    let executable = executable_file(&root, "ai-cli");
    let symlink_path = outside.join("ai-cli-link");
    symlink(&executable, &symlink_path).expect("test symlink should be created");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&symlink_path);
    let request = request_for(&project, &profile);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("symlink resolving into project root must be blocked");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::WorkspaceLocalExecutableBlocked {
            path: executable.canonicalize().unwrap(),
        }
    );
    cleanup_root(root);
    cleanup_root(outside);
}

#[test]
fn restricted_mode_rejects_project_local_path_lookup_before_resolution() {
    let root = test_root("agent-project-path");
    let project_bin = root.join("bin");
    std::fs::create_dir_all(&project_bin).expect("project bin should be created");
    executable_file(&project_bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&project_bin.join("ai-cli"));
    profile.executable = AiCliExecutable::PathLookup {
        command: "ai-cli".to_owned(),
        lookup_paths: vec![ExecutableLookupPath::project_local(&project_bin)],
        provenance: AiCliExecutableProvenance::SystemPathReviewed,
    };
    let request = request_for(&project, &profile);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("project-local PATH lookup must be blocked");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::ProjectLocalPathLookupBlocked { path: project_bin }
    );
    cleanup_root(root);
}

#[test]
fn restricted_mode_rejects_misclassified_project_local_path_lookup() {
    let root = test_root("agent-misclassified-project-path");
    let project_bin = root.join("bin");
    std::fs::create_dir_all(&project_bin).expect("project bin should be created");
    executable_file(&project_bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&project_bin.join("ai-cli"));
    profile.executable = AiCliExecutable::PathLookup {
        command: "ai-cli".to_owned(),
        lookup_paths: vec![ExecutableLookupPath::reviewed_system(&project_bin)],
        provenance: AiCliExecutableProvenance::SystemPathReviewed,
    };
    let request = request_for(&project, &profile);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("canonical project-local PATH lookup must be blocked");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::ProjectLocalPathLookupBlocked { path: project_bin }
    );
    cleanup_root(root);
}

#[test]
fn restricted_mode_rejects_misclassified_project_path_before_symlink_to_outside_resolution() {
    let root = test_root("agent-project-path-symlink");
    let outside = test_root("agent-project-path-outside");
    let project_bin = root.join("bin");
    std::fs::create_dir_all(&project_bin).expect("project bin should be created");
    let outside_executable = executable_file(&outside, "ai-cli-real");
    let symlink_path = project_bin.join("ai-cli");
    symlink(&outside_executable, &symlink_path).expect("PATH symlink should be created");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&symlink_path);
    profile.executable = AiCliExecutable::PathLookup {
        command: "ai-cli".to_owned(),
        lookup_paths: vec![ExecutableLookupPath::reviewed_system(&project_bin)],
        provenance: AiCliExecutableProvenance::SystemPathReviewed,
    };
    let request = request_for(&project, &profile);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("project-local PATH dir must be blocked before symlink target resolution");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::ProjectLocalPathLookupBlocked { path: project_bin }
    );
    cleanup_root(root);
    cleanup_root(outside);
}

#[test]
fn restricted_mode_rejects_implicit_workspace_discovery_when_not_disabled() {
    let root = test_root("agent-discovery-blocked");
    let bin = test_root("agent-discovery-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.workspace_discovery_policy = AiCliWorkspaceDiscoveryPolicy::MayDiscoverWorkspaceFiles {
        summary: "CLI auto-loads project agent instructions".to_owned(),
    };
    let request = request_for(&project, &profile);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("implicit workspace discovery must be blocked");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked {
            summary: super::AgentLaunchSummary::new("CLI auto-loads project agent instructions"),
        }
    );
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn restricted_mode_rejects_workspace_prompt_and_environment_sources() {
    let root = test_root("agent-workspace-policy");
    let bin = test_root("agent-workspace-policy-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.prompt_policy = AiCliPromptPolicy::WorkspaceLocalTemplate;
    let request = request_for(&project, &profile);

    assert_eq!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &request)
            .expect_err("workspace prompt template must be blocked"),
        AgentRunLaunchValidationError::WorkspaceLocalPromptBlocked
    );

    profile.prompt_policy = AiCliPromptPolicy::Interactive;
    profile.environment_policy = AiCliEnvironmentPolicy::WorkspaceLocalEnvFile(root.join(".env"));

    assert_eq!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &request)
            .expect_err("workspace environment file must be blocked"),
        AgentRunLaunchValidationError::WorkspaceLocalEnvironmentBlocked
    );
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn managed_profile_requires_structured_action_capability() {
    let root = test_root("agent-managed-capability");
    let bin = test_root("agent-managed-capability-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.compatibility_level = AgentCompatibilityLevel::Managed;
    let request = request_for(&project, &profile);

    assert_eq!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &request)
            .expect_err("managed profile requires structured action capability"),
        AgentRunLaunchValidationError::ManagedCapabilityMissing
    );

    profile.adapter_capabilities = AiCliAdapterCapabilities {
        structured_action_approval: true,
    };

    AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect("managed profile validates with reviewed capability evidence");
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn required_transcript_capture_rejects_missing_state_root() {
    let root = test_root("agent-transcript-required-root");
    let bin = test_root("agent-transcript-required-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&executable);
    let mut request = request_for(&project, &profile);
    request.transcript_capture_mode = TranscriptCaptureMode::RequiredLocalBounded;

    assert_eq!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &request)
            .expect_err("required transcript capture needs a state root"),
        AgentRunLaunchValidationError::RequiredTranscriptStateRootMissing
    );
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn required_transcript_capture_rejects_unbounded_policy() {
    let root = test_root("agent-transcript-unbounded-root");
    let state_root = test_root("agent-transcript-unbounded-state");
    let bin = test_root("agent-transcript-unbounded-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&executable);
    let request = request_for(&project, &profile)
        .with_required_local_bounded_transcript(&state_root)
        .with_transcript_retention_limits(TranscriptRetentionLimits::new(0, 0, 0, 0));

    assert_eq!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &request)
            .expect_err("required transcript capture needs bounded policy"),
        AgentRunLaunchValidationError::RequiredTranscriptPolicyDoesNotPermitBytes
    );
    cleanup_root(root);
    cleanup_root(state_root);
    cleanup_root(bin);
}

#[test]
fn launch_validation_rejects_cross_project_and_cwd_escape() {
    let root = test_root("agent-cwd-root");
    let outside = test_root("agent-cwd-outside");
    let bin = test_root("agent-cwd-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&executable);
    let cross_project_request =
        AgentRunLaunchRequest::new(ProjectId::for_test(2), &profile.id, "prompt");

    assert_eq!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &cross_project_request)
            .expect_err("cross-project request must be blocked"),
        AgentRunLaunchValidationError::CrossProject
    );

    let mut escaping_request = request_for(&project, &profile);
    escaping_request.cwd = Some(outside.clone());

    assert!(matches!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &escaping_request)
            .expect_err("cwd escape must be blocked"),
        AgentRunLaunchValidationError::CwdEscapesProjectRoot { .. }
    ));
    cleanup_root(root);
    cleanup_root(outside);
    cleanup_root(bin);
}

#[test]
fn launch_plan_builds_ready_agent_run_and_matching_terminal_spec() {
    let root = test_root("agent-plan-root");
    let bin = test_root("agent-plan-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.environment_policy = AiCliEnvironmentPolicy::Named("agent-minimal".to_owned());
    let request = request_for(&project, &profile);
    let validation = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect("profile should validate before plan construction");

    let plan = AgentRunLaunchPlan::from_validation(validation, "Built-in AI")
        .expect("validated launch should create a plan");

    assert_eq!(plan.spec().project_id(), project.id());
    assert_eq!(plan.agent_run().project_id, project.id().clone());
    assert_eq!(plan.agent_run().profile_id, "builtin-ai");
    assert_eq!(plan.agent_run().status, AgentRunStatus::Ready);
    assert_eq!(plan.agent_run().terminal_id, None);
    assert_eq!(plan.terminal_launch_spec().project_id, project.id().clone());
    assert_eq!(plan.terminal_launch_spec().kind, TerminalKind::Supervised);
    assert_eq!(
        plan.terminal_launch_spec().cwd,
        root.canonicalize().unwrap()
    );
    assert_eq!(
        plan.terminal_launch_spec().shell,
        executable.canonicalize().unwrap()
    );
    assert_eq!(
        plan.terminal_launch_spec().environment_policy,
        TerminalEnvironmentPolicy::Named("agent-minimal".to_owned())
    );

    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn launch_plan_preserves_distinct_explicit_environment_allowlists() {
    let root = test_root("agent-plan-allowlist-root");
    let bin = test_root("agent-plan-allowlist-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut path_only_profile = built_in_profile(&executable);
    path_only_profile.environment_policy =
        AiCliEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned()]);
    let mut path_home_profile = path_only_profile.clone();
    path_home_profile.environment_policy =
        AiCliEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned(), "HOME".to_owned()]);

    let path_only_plan = launch_plan_for(&project, &path_only_profile);
    let path_home_plan = launch_plan_for(&project, &path_home_profile);

    assert_eq!(
        path_only_plan.terminal_launch_spec().environment_policy,
        TerminalEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned()])
    );
    assert_eq!(
        path_home_plan.terminal_launch_spec().environment_policy,
        TerminalEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned(), "HOME".to_owned()])
    );
    assert_ne!(
        path_only_plan.terminal_launch_spec().environment_policy,
        path_home_plan.terminal_launch_spec().environment_policy
    );

    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_attaches_launch_plan_to_matching_terminal() {
    let root = test_root("agent-attach-root");
    let bin = test_root("agent-attach-bin");
    let executable = executable_file(&bin, "ai-cli");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.environment_policy = AiCliEnvironmentPolicy::Named("agent-minimal".to_owned());
    let plan = launch_plan_for(&project, &profile);
    let terminal = terminal_from_plan(&plan);
    let terminal_id = terminal.id.clone();
    let agent_run_id = plan.agent_run().id.clone();

    let attached_agent_run_id = project
        .attach_agent_launch_plan(plan, terminal)
        .expect("matching plan and terminal should attach");

    assert_eq!(attached_agent_run_id, agent_run_id);
    assert_eq!(project.terminal_sessions().len(), 1);
    assert_eq!(project.agent_runs().len(), 1);
    let run = &project.agent_runs()[0];
    assert_eq!(run.id, agent_run_id);
    assert_eq!(run.status, AgentRunStatus::Ready);
    assert_eq!(run.terminal_id, Some(terminal_id));
    assert_eq!(
        project.terminal_sessions()[0].environment_policy_ref,
        Some("agent-minimal".to_owned())
    );
    assert_eq!(project.runtime_summary().agent_run_count, Some(1));
    assert_eq!(project.runtime_summary().terminal_count, Some(1));

    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_records_explicit_environment_allowlist_metadata() {
    let root = test_root("agent-attach-allowlist-root");
    let bin = test_root("agent-attach-allowlist-bin");
    let executable = executable_file(&bin, "ai-cli");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.environment_policy =
        AiCliEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned(), "HOME".to_owned()]);
    let plan = launch_plan_for(&project, &profile);
    let terminal = terminal_from_plan(&plan);

    project
        .attach_agent_launch_plan(plan, terminal)
        .expect("explicit allowlist launch plan should attach");

    assert_eq!(
        project.terminal_sessions()[0].environment_policy_ref,
        Some("explicit allowlist: PATH, HOME".to_owned())
    );

    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_launches_agent_run_through_terminal_runtime_and_completes() {
    let root = test_root("agent-runtime-launch-root");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, events) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("validated minimal AgentRun should launch through terminal runtime");
    let run = project
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .expect("launched AgentRun should be in project collection");
    let terminal_id = run
        .terminal_id
        .clone()
        .expect("runtime-launched AgentRun should be attached to terminal");
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    assert_eq!(run.status, AgentRunStatus::Running);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().kind,
        TerminalKind::Supervised
    );
    assert_eq!(project.runtime_summary().running_processes, 1);
    assert_eq!(
        project.runtime_summary().close_resources.running_processes,
        1
    );
    assert!(matches!(
        events.as_slice(),
        [
            TerminalRuntimeEvent::LaunchAccepted { .. },
            TerminalRuntimeEvent::ProcessStarted { .. },
        ]
    ));

    runtime
        .write_input(&handle, b"exit 0\n")
        .expect("cleanup exit should write to AgentRun PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("AgentRun shell wait should not fail")
        .expect("AgentRun shell should exit");
    assert_eq!(outcome, TerminationOutcome::Exited { exit_status: 0 });

    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("clean terminal exit should complete AgentRun");

    let run = project
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .unwrap();
    assert_eq!(run.status, AgentRunStatus::Completed);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        crate::domain::TerminalStatus::Exited
    );
    assert_eq!(project.runtime_summary().running_processes, 0);

    cleanup_root(root);
}

#[test]
fn local_bounded_agent_run_transcript_capture_attaches_metadata_and_writes_output() {
    let root = test_root("agent-transcript-capture-root");
    let state_root = test_root("agent-transcript-capture-state");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let validation = AgentRunLaunchValidator
        .validate(
            &project,
            &profile,
            &request_for(&project, &profile).with_local_bounded_transcript(&state_root),
        )
        .expect("local bounded transcript launch should validate");
    let plan = AgentRunLaunchPlan::from_validation(validation, "Agent").unwrap();
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("transcript-enabled AgentRun should launch");
    let run = project
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .expect("AgentRun should be recorded");
    let transcript_id = run
        .transcript_ref
        .clone()
        .expect("AgentRun should have transcript metadata");
    let terminal_id = run
        .terminal_id
        .clone()
        .expect("AgentRun should attach terminal");
    let terminal = project.terminal_session(&terminal_id).unwrap();
    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == transcript_id)
        .expect("transcript metadata should be recorded");

    assert_eq!(terminal.transcript_ref, Some(transcript_id.clone()));
    assert_eq!(transcript.agent_run_id, Some(agent_run_id.clone()));
    assert_eq!(transcript.terminal_id, terminal_id);
    assert!(transcript.storage_path.starts_with(&state_root));
    assert!(!transcript.storage_path.starts_with(&root));

    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());
    runtime
        .write_input(&handle, b"printf 'tekstide-transcript-ok\\n'\nexit 0\n")
        .expect("transcript marker command should write to PTY");
    let output = read_until_contains(&mut runtime, &handle, b"tekstide-transcript-ok");
    assert!(contains_subsequence(&output, b"tekstide-transcript-ok"));
    let transcript_bytes = std::fs::read(&transcript.storage_path).unwrap();
    assert!(contains_subsequence(
        &transcript_bytes,
        b"tekstide-transcript-ok"
    ));
    let write_summary = runtime
        .transcript_write_summary(&handle)
        .unwrap()
        .expect("transcript-enabled runtime should expose writer summary");
    project
        .record_terminal_transcript_write_summary(&terminal_id, write_summary)
        .expect("ProjectSession should reconcile transcript metadata");
    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == transcript_id)
        .expect("transcript metadata should still be recorded");
    assert_eq!(transcript.byte_count, transcript_bytes.len() as u64);
    assert_eq!(transcript.truncation_state, TruncationState::Complete);
    assert!(transcript.last_write_at.is_some());

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("transcript AgentRun wait should not fail")
        .expect("transcript AgentRun should exit");
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("transcript AgentRun cleanup should apply");

    cleanup_root(root);
    cleanup_root(state_root);
}

#[test]
fn transcript_capture_retains_pty_bytes_dropped_from_ui_buffer() {
    let root = test_root("agent-transcript-dropped-ui-root");
    let state_root = test_root("agent-transcript-dropped-ui-state");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let validation = AgentRunLaunchValidator
        .validate(
            &project,
            &profile,
            &request_for(&project, &profile).with_local_bounded_transcript(&state_root),
        )
        .expect("local bounded transcript launch should validate");
    let plan = AgentRunLaunchPlan::from_validation(validation, "Agent").unwrap();
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("transcript-enabled AgentRun should launch");
    let run = project
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .expect("AgentRun should be recorded");
    let transcript_id = run.transcript_ref.clone().unwrap();
    let terminal_id = run.terminal_id.clone().unwrap();
    let transcript_path = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.id == transcript_id)
        .unwrap()
        .storage_path
        .clone();
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    runtime
        .write_input(
            &handle,
            b"printf 'aaaaabbbbbccccc-transcript-tail\\n'\nexit 0\n",
        )
        .expect("large transcript marker command should write to PTY");
    let mut returned_output = Vec::new();
    let mut saw_dropped_bytes = false;
    for _ in 0..80 {
        let (chunk, event) = runtime
            .read_available_bounded_for(&handle, Duration::from_millis(50), 8)
            .expect("bounded PTY output read should succeed");
        returned_output.extend_from_slice(&chunk);
        if let TerminalRuntimeEvent::OutputBuffered { summary, .. } = event
            && summary.dropped_bytes > 0
        {
            saw_dropped_bytes = true;
            break;
        }
    }

    assert!(saw_dropped_bytes);
    assert!(!contains_subsequence(&returned_output, b"transcript-tail"));
    let transcript_bytes = std::fs::read(&transcript_path).unwrap();
    assert!(contains_subsequence(&transcript_bytes, b"transcript-tail"));

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("overflow AgentRun wait should not fail")
        .expect("overflow AgentRun should exit");
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("overflow AgentRun cleanup should apply");

    cleanup_root(root);
    cleanup_root(state_root);
}

#[test]
fn transcript_opt_out_launches_without_transcript_metadata() {
    let root = test_root("agent-transcript-opt-out-root");
    let state_root = test_root("agent-transcript-opt-out-state");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let validation = AgentRunLaunchValidator
        .validate(
            &project,
            &profile,
            &request_for(&project, &profile)
                .with_local_bounded_transcript(&state_root)
                .without_transcript_capture(),
        )
        .expect("opted-out transcript launch should validate");
    let plan = AgentRunLaunchPlan::from_validation(validation, "Agent").unwrap();
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("opted-out AgentRun should launch");
    let run = project
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .expect("AgentRun should be recorded");
    let terminal_id = run.terminal_id.clone().unwrap();
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    assert!(run.transcript_ref.is_none());
    assert!(
        project
            .terminal_session(&terminal_id)
            .unwrap()
            .transcript_ref
            .is_none()
    );
    assert!(project.transcripts().is_empty());

    runtime
        .write_input(&handle, b"exit 0\n")
        .expect("cleanup exit should write to opted-out AgentRun PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("opted-out AgentRun wait should not fail")
        .expect("opted-out AgentRun should exit");
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("opted-out cleanup should apply");

    cleanup_root(root);
    cleanup_root(state_root);
}

#[test]
fn required_transcript_capture_rejects_project_local_state_root_before_runtime_launch() {
    let root = test_root("agent-transcript-required-project-root");
    let state_root = root.join(".tekstide-state");
    std::fs::create_dir_all(&state_root).unwrap();
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let validation = AgentRunLaunchValidator
        .validate(
            &project,
            &profile,
            &request_for(&project, &profile).with_required_local_bounded_transcript(&state_root),
        )
        .expect("required transcript request should validate before path preflight");
    let plan = AgentRunLaunchPlan::from_validation(validation, "Agent").unwrap();
    let mut runtime = LinuxTerminalRuntime::new();

    let error = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect_err("required project-local transcript path must reject before runtime launch");

    assert!(matches!(
        error,
        ProjectAgentRuntimeLaunchError::TranscriptCapture(AgentRunTranscriptCaptureError::Path(_))
    ));
    assert!(project.agent_runs().is_empty());
    assert!(project.terminal_sessions().is_empty());
    assert!(project.transcripts().is_empty());
    assert!(!state_root.join("transcripts").exists());

    cleanup_root(root);
}

#[test]
fn local_bounded_transcript_capture_disables_when_path_preflight_fails() {
    let root = test_root("agent-transcript-local-disabled-root");
    let state_root = root.join(".tekstide-state");
    std::fs::create_dir_all(&state_root).unwrap();
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let validation = AgentRunLaunchValidator
        .validate(
            &project,
            &profile,
            &request_for(&project, &profile).with_local_bounded_transcript(&state_root),
        )
        .expect("local bounded transcript request should validate before path preflight");
    let plan = AgentRunLaunchPlan::from_validation(validation, "Agent").unwrap();
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("local bounded capture should disable when preflight fails");
    let run = project
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .unwrap();
    let terminal_id = run.terminal_id.clone().unwrap();
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    assert!(run.transcript_ref.is_none());
    assert!(project.transcripts().is_empty());
    assert!(!state_root.join("transcripts").exists());

    runtime
        .write_input(&handle, b"exit 0\n")
        .expect("cleanup exit should write to local bounded AgentRun PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("local bounded disabled AgentRun wait should not fail")
        .expect("local bounded disabled AgentRun should exit");
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("local bounded disabled cleanup should apply");

    cleanup_root(root);
}

#[test]
fn project_session_launches_validated_managed_agent_run_through_terminal_runtime() {
    let root = test_root("agent-runtime-managed-root");
    // Deliberately not `test_root`: this test binds a real
    // `ApprovalChannelEndpoint` socket under this root, and a Unix
    // `sun_path` is bounded to ~107 bytes. The real (UUID-based)
    // `agent_run.id` this launch generates already spends 46 of that
    // budget on its own (`agent-run-<uuid>`), and `test_root`'s own
    // `tekstide-{name}-{pid}-{nanos}` scheme is descriptive enough to
    // blow the rest even with a short `name` -- see
    // `approval::tests::reference_adapter`'s own `unique_temp_dir` doc
    // comment for the same finding, made first there.
    let state_root = std::env::temp_dir().join(format!("ta-mg-{}", std::process::id()));
    std::fs::create_dir_all(&state_root).expect("approval state root should be creatable");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(Path::new("/bin/sh"));
    profile.compatibility_level = AgentCompatibilityLevel::Managed;
    profile.adapter_capabilities = AiCliAdapterCapabilities {
        structured_action_approval: true,
    };
    // RFC-022 PR-022-C: a `Managed` launch now also binds a real
    // `ApprovalChannelEndpoint`, which needs a real state root the same
    // way transcript capture does -- `request_for`'s bare default (no
    // state root at all) stopped being a complete `Managed` request the
    // moment that became true, so this test supplies one explicitly
    // rather than relying on the shared helper's plain-shell-era default.
    let request = AgentRunLaunchRequest::new(project.id().clone(), &profile.id, "prompt")
        .with_local_bounded_transcript(state_root.clone());
    let validation = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect("Managed profile should validate before launch plan creation");
    let plan = AgentRunLaunchPlan::from_validation(validation, "Built-in AI")
        .expect("validated Managed launch should produce a launch plan");
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("validated Managed AgentRun should launch through terminal runtime");
    let terminal_id = project.agent_runs()[0]
        .terminal_id
        .clone()
        .expect("runtime-launched AgentRun should have terminal id");
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    assert_eq!(project.agent_runs()[0].id, agent_run_id);
    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Running);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().kind,
        TerminalKind::Managed
    );
    assert_eq!(project.runtime_summary().running_processes, 1);

    runtime
        .write_input(&handle, b"exit 0\n")
        .expect("cleanup exit should write to Managed AgentRun PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("Managed AgentRun shell wait should not fail")
        .expect("Managed AgentRun shell should exit");
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("Managed AgentRun cleanup outcome should apply");

    cleanup_root(root);
    cleanup_root(state_root);
}

/// RFC-022 PR-022-C: the adapter spawn path proven for real, end to end,
/// headless -- the reference adapter (PR-022-B) launched through the
/// *production* spawn path (`prepare_agent_run_launch` ->
/// `launch_prepared_agent_run_with_runtime` -> `launch_project_adapter` ->
/// `spawn_adapter`), not the bare `Command` PR-022-B's own tests use,
/// completing a real approval round trip against a real, production-bound
/// `ApprovalChannelEndpoint` and `ApprovalCoordinator`.
///
/// **What this proves.** `spawn_adapter`'s environment wiring
/// (`.env_clear()`, the token, the socket path) is correct against a
/// real client that only knows the two sanctioned env vars, not a
/// test-only shortcut. `inject_token_into_environment`'s first
/// production caller really delivers a token the real channel accepts.
/// Transcript capture, configured via `prepare_transcript_capture`
/// (already non-test code, now reached for the first time by a
/// `Managed` production `AgentRun`) really writes the adapter's own PTY
/// output to disk -- read back below and checked, not assumed.
///
/// **What this does not prove.** That a real AI CLI behaves this way --
/// nothing speaks this protocol except what this project wrote (RFC-022's
/// own scope). Nor a GUI-triggered launch or the dialog -- PR-022-D/E's
/// job, not this test's.
#[test]
fn a_real_adapter_completes_a_real_approval_round_trip_through_the_production_spawn_path() {
    let root = test_root("agent-adapter-roundtrip-root");
    // Deliberately not `test_root` -- see the previous test's own
    // comment on why: a real socket bind needs a short state root.
    let state_root = std::env::temp_dir().join(format!("ta-rt-{}", std::process::id()));
    std::fs::create_dir_all(&state_root)
        .expect("approval/transcript state root should be creatable");
    let mut project = restricted_project(ProjectId::for_test(1), &root);

    let mut profile = built_in_profile(&reference_adapter_binary_path());
    profile.compatibility_level = AgentCompatibilityLevel::Managed;
    profile.adapter_capabilities = AiCliAdapterCapabilities {
        structured_action_approval: true,
    };
    let request = AgentRunLaunchRequest::new(project.id().clone(), &profile.id, "prompt")
        .with_local_bounded_transcript(state_root.clone());
    let validation = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect("Managed reference-adapter profile should validate");
    let mut plan = AgentRunLaunchPlan::from_validation(validation, "Reference Adapter")
        .expect("validated Managed launch should produce a launch plan");

    let endpoint = project
        .prepare_agent_run_launch(&mut plan)
        .expect("prepare should succeed for a Managed profile with a real state root")
        .expect("a Managed launch must bind a real approval channel endpoint");
    let verified_cwd = plan.spec().verified_cwd().clone();
    let project_root = plan.spec().project_root().to_path_buf();

    let mut runtime = LinuxTerminalRuntime::new();
    let (agent_run_id, _events) = project
        .launch_prepared_agent_run_with_runtime(plan, &mut runtime)
        .expect("prepared Managed launch should spawn the real adapter binary");
    let terminal_id = project.agent_runs()[0]
        .terminal_id
        .clone()
        .expect("runtime-launched AgentRun should have a terminal id");
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    let accepted = endpoint
        .accept_proposal()
        .expect("the real, production-spawned adapter's proposal should authenticate and parse");
    let proposal_id = accepted.proposal.proposal_id().clone();

    let mut coordinator = ApprovalCoordinator::new();
    let mut audit = RoundTripAudit::new("adapter-roundtrip");
    let receive_outcome = coordinator.receive_proposal(
        project.id().clone(),
        agent_run_id.clone(),
        &verified_cwd,
        &project_root,
        &state_root,
        accepted,
        &mut audit.coordinator(),
    );
    assert!(
        matches!(receive_outcome, ReceiveOutcome::Created { .. }),
        "the real adapter's first proposal should be accepted as Created: {receive_outcome:?}"
    );

    let decide_outcome = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::ApprovedOnce,
        &mut audit.coordinator(),
    );
    let DecideOutcome::Decided { sent, .. } = decide_outcome else {
        panic!("deciding a freshly-created proposal should reach Decided: {decide_outcome:?}");
    };
    sent.expect(
        "sending the decision back over the real production-spawned connection should succeed",
    );

    // The decision travels back over the PTY, not a piped `Stdio` --
    // `spawn_adapter` goes through the same PTY machinery `spawn_shell`
    // does, unlike PR-022-B's own tests, which spawn the adapter as a
    // bare `Command` outside any terminal at all.
    let reader = runtime
        .spawn_output_reader(&handle)
        .expect("reader thread should spawn against the real adapter's PTY master");
    let mut drained = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !String::from_utf8_lossy(&drained).contains("approved_once")
        && std::time::Instant::now() < deadline
    {
        let drain = reader.drain_available();
        drained.extend_from_slice(drain.bytes());
        if drain.bytes().is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert!(
        String::from_utf8_lossy(&drained).contains("approved_once"),
        "the real adapter should have printed the decision it actually received over the PTY; \
         captured: {}",
        String::from_utf8_lossy(&drained)
    );

    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("waiting on the real adapter's exit should not fail")
        .expect("the real adapter should have exited");
    assert_eq!(
        outcome,
        TerminationOutcome::Exited { exit_status: 0 },
        "the reference adapter's own exit-code contract: 0 means approved_once"
    );
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("adapter run cleanup outcome should apply");

    // Transcript capture, exercised for the first time by a production
    // `Managed` `AgentRun` (RFC-011 Amendment 2's first real production
    // exercise) -- read back from disk, not assumed from configuration
    // alone. Proves the mechanism reached this specific, real spawn path;
    // it does not re-prove Amendment 2's own byte-identical/ordering
    // guarantees, which are that amendment's own evidence, not this
    // test's.
    let transcript_storage = TranscriptPathResolver
        .resolve_agent_run(TranscriptPathRequest::new(
            &state_root,
            &root,
            project.id().clone(),
            agent_run_id.clone(),
        ))
        .expect("the same transcript path production code resolved should resolve again");
    let transcript_bytes = std::fs::read(transcript_storage.transcript_file())
        .expect("the transcript file production code wrote to should be readable");
    assert!(
        String::from_utf8_lossy(&transcript_bytes).contains("approved_once"),
        "the transcript must contain the same real adapter output the channel carried; \
         transcript: {}",
        String::from_utf8_lossy(&transcript_bytes)
    );

    drop(reader);
    cleanup_root(root);
    cleanup_root(state_root);
}

#[test]
fn authorized_supervised_plan_spec_cannot_be_mutated_to_managed_runtime_launch() {
    let root = test_root("agent-runtime-authority-mutation-root");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut terminal_spec = plan.terminal_launch_spec_for_runtime();
    terminal_spec.kind = TerminalKind::Managed;

    let error = LinuxTerminalRuntime::new()
        .launch_project_shell(&project, terminal_spec)
        .expect_err("authorized Supervised plan spec must not be mutable into Managed launch");

    assert_eq!(error, TerminalLaunchError::UnsupportedTerminalKind);
    cleanup_root(root);
}

#[test]
fn active_clean_text_document_permits_agent_runtime_launch() {
    let root = test_root("agent-active-clean-root");
    std::fs::write(root.join("note.txt"), "original\n").expect("active file should be written");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    project
        .open_text_document("note.txt")
        .expect("active clean document should open");
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("clean active document should permit launch");
    let terminal_id = project.agent_runs()[0]
        .terminal_id
        .clone()
        .expect("launched AgentRun should attach terminal");
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    assert_eq!(project.agent_runs()[0].id, agent_run_id);
    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Running);
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Clean
    );

    runtime
        .write_input(&handle, b"exit 0\n")
        .expect("cleanup exit should write to AgentRun PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("AgentRun shell wait should not fail")
        .expect("AgentRun shell should exit");
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("cleanup outcome should apply");

    cleanup_root(root);
}

#[test]
fn active_dirty_text_document_blocks_agent_runtime_launch_before_process_start() {
    let root = test_root("agent-active-dirty-root");
    std::fs::write(root.join("note.txt"), "original\n").expect("active file should be written");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    project
        .open_text_document("note.txt")
        .expect("active document should open");
    project
        .replace_active_text("edited\n")
        .expect("active document should become dirty");
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let error = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect_err("dirty active document should block launch");

    assert_active_file_blocked(error, ProjectActiveFileLaunchBlockReason::Dirty);
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());
    assert_eq!(project.runtime_summary().dirty_files, 1);
    assert_eq!(
        project.content_workspace().status().label(),
        "edited",
        "dirty refresh before launch must keep dirty state visible"
    );
    cleanup_root(root);
}

#[test]
fn active_external_changed_text_document_blocks_agent_runtime_launch_before_process_start() {
    let root = test_root("agent-active-external-root");
    std::fs::write(root.join("note.txt"), "original\n").expect("active file should be written");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    project
        .open_text_document("note.txt")
        .expect("active document should open");
    std::fs::write(root.join("note.txt"), "external\n")
        .expect("active file should be externally changed");
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let error = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect_err("externally changed active document should block launch");

    assert_active_file_blocked(error, ProjectActiveFileLaunchBlockReason::ExternalChanged);
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::ExternalChanged
    );
    cleanup_root(root);
}

#[test]
fn active_conflict_text_document_blocks_agent_runtime_launch_before_process_start() {
    let root = test_root("agent-active-conflict-root");
    std::fs::write(root.join("note.txt"), "original\n").expect("active file should be written");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    project
        .open_text_document("note.txt")
        .expect("active document should open");
    project
        .replace_active_text("edited\n")
        .expect("active document should become dirty");
    std::fs::write(root.join("note.txt"), "external\n")
        .expect("active file should be externally changed");
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let error = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect_err("conflicted active document should block launch");

    assert_active_file_blocked(error, ProjectActiveFileLaunchBlockReason::Conflict);
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Conflict
    );
    cleanup_root(root);
}

#[test]
fn active_save_error_text_document_blocks_agent_runtime_launch_before_process_start() {
    let root = test_root("agent-active-save-error-root");
    std::fs::write(root.join("target.txt"), "original\n").expect("target file should be written");
    symlink(root.join("target.txt"), root.join("link.txt")).expect("in-root symlink should exist");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    project
        .open_text_document("link.txt")
        .expect("in-root symlink document should open");
    project
        .replace_active_text("edited\n")
        .expect("active document should become dirty");
    project
        .save_active_text_document()
        .expect_err("unsafe symlink save should set SaveError state");
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let error = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect_err("save-error active document should block launch");

    assert_active_file_blocked(error, ProjectActiveFileLaunchBlockReason::SaveError);
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::SaveError
    );
    assert_eq!(project.content_workspace().status().label(), "save error");
    cleanup_root(root);
}

#[test]
fn safe_save_blocks_external_change_while_agent_run_is_active() {
    let root = test_root("agent-active-safe-save-root");
    std::fs::write(root.join("note.txt"), "original\n").expect("active file should be written");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    project
        .open_text_document("note.txt")
        .expect("active clean document should open");
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("clean active document should permit launch");
    let terminal_id = project.agent_runs()[0]
        .terminal_id
        .clone()
        .expect("launched AgentRun should attach terminal");
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    project
        .replace_active_text("edited\n")
        .expect("active document edit should be accepted while AgentRun is active");
    std::fs::write(root.join("note.txt"), "external\n")
        .expect("external change should be written while AgentRun is active");

    let error = project
        .save_active_text_document()
        .expect_err("safe-save should block external overwrite while AgentRun is active");

    assert!(matches!(
        error,
        ProjectContentError::Save(ref save_error)
            if save_error.decision() == SaveDecision::BlockedExternalChange
    ));
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Conflict
    );
    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Running);

    runtime
        .write_input(&handle, b"exit 0\n")
        .expect("cleanup exit should write to AgentRun PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("AgentRun shell wait should not fail")
        .expect("AgentRun shell should exit");
    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("cleanup outcome should apply");

    cleanup_root(root);
}

#[test]
fn runtime_launch_rejects_non_minimal_environment_policy_without_project_mutation() {
    let root = test_root("agent-runtime-env-reject-root");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(Path::new("/bin/sh"));
    profile.environment_policy = AiCliEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned()]);
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let error = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect_err("runtime must reject unsupported env policy before process launch");

    assert!(matches!(
        error,
        ProjectAgentRuntimeLaunchError::TerminalLaunch(
            TerminalLaunchError::UnsupportedEnvironmentPolicy { .. }
        )
    ));
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());

    cleanup_root(root);
}

/// RFC-022 PR-022-C: the same rejection, proven again for the *adapter*
/// spawn path specifically -- the review gate's own wording ("nothing
/// inherited -- `ExplicitAllowlist` stays rejected, and a test pins that
/// it is still rejected") is about this path, not the shell one the
/// test above already covers. `launch_project_adapter` calls the same,
/// unmodified `validate_launch_spec` `launch_project_shell` does, so
/// this is a real re-proof that the shared check still reaches the new
/// path, not an assumption that it must.
#[test]
fn runtime_launch_rejects_non_minimal_environment_policy_for_the_adapter_path_too() {
    let root = test_root("agent-runtime-env-reject-adapter-root");
    let state_root = std::env::temp_dir().join(format!("ta-envrej-{}", std::process::id()));
    std::fs::create_dir_all(&state_root)
        .expect("approval/transcript state root should be creatable");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&reference_adapter_binary_path());
    profile.compatibility_level = AgentCompatibilityLevel::Managed;
    profile.adapter_capabilities = AiCliAdapterCapabilities {
        structured_action_approval: true,
    };
    profile.environment_policy = AiCliEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned()]);
    let request = AgentRunLaunchRequest::new(project.id().clone(), &profile.id, "prompt")
        .with_local_bounded_transcript(state_root.clone());
    let validation = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect("Managed profile should validate before launch plan creation");
    let plan = AgentRunLaunchPlan::from_validation(validation, "Reference Adapter")
        .expect("validated Managed launch should produce a launch plan");
    let mut runtime = LinuxTerminalRuntime::new();

    let error = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect_err("the adapter path must reject unsupported env policy before process launch");

    assert!(matches!(
        error,
        ProjectAgentRuntimeLaunchError::TerminalLaunch(
            TerminalLaunchError::UnsupportedEnvironmentPolicy { .. }
        )
    ));
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());

    cleanup_root(root);
    cleanup_root(state_root);
}

#[test]
fn project_session_maps_nonzero_agent_run_exit_to_failed() {
    let root = test_root("agent-runtime-nonzero-root");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(Path::new("/bin/sh"));
    let plan = launch_plan_for(&project, &profile);
    let mut runtime = LinuxTerminalRuntime::new();

    let (agent_run_id, _) = project
        .launch_agent_run_with_runtime(plan, &mut runtime)
        .expect("validated minimal AgentRun should launch through terminal runtime");
    let terminal_id = project.agent_runs()[0]
        .terminal_id
        .clone()
        .expect("runtime-launched AgentRun should have terminal id");
    let handle = TerminalRuntimeHandle::new(terminal_id.clone(), project.id().clone());

    runtime
        .write_input(&handle, b"exit 7\n")
        .expect("nonzero exit should write to AgentRun PTY");
    let outcome = runtime
        .wait_for_exit(&handle, Duration::from_secs(5))
        .expect("AgentRun shell wait should not fail")
        .expect("AgentRun shell should exit");
    assert_eq!(outcome, TerminationOutcome::Exited { exit_status: 7 });

    project
        .apply_agent_terminal_outcome(&agent_run_id, &terminal_id, &outcome)
        .expect("nonzero terminal exit should update AgentRun");

    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Failed);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        crate::domain::TerminalStatus::Exited
    );

    cleanup_root(root);
}

#[test]
fn project_session_maps_signal_agent_run_outcome_to_cancelled() {
    let (mut project, agent_run_id, terminal_id, root, bin) =
        project_with_running_agent_for_outcome("agent-outcome-signal");

    project
        .apply_agent_terminal_outcome(
            &agent_run_id,
            &terminal_id,
            &TerminationOutcome::TerminatedBySignal {
                signal: TerminationSignal::Sigterm,
            },
        )
        .expect("signal terminal outcome should update AgentRun");

    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Cancelled);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        TerminalStatus::Exited
    );
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_maps_timeout_agent_run_outcome_to_cancelled() {
    let (mut project, agent_run_id, terminal_id, root, bin) =
        project_with_running_agent_for_outcome("agent-outcome-timeout");

    project
        .apply_agent_terminal_outcome(
            &agent_run_id,
            &terminal_id,
            &TerminationOutcome::KilledAfterTimeout {
                initial_signal: TerminationSignal::Sigterm,
                fallback_signal: TerminationSignal::Sigkill,
            },
        )
        .expect("timeout terminal outcome should update AgentRun");

    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Cancelled);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        TerminalStatus::Exited
    );
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_maps_orphaned_agent_run_outcome_to_detached() {
    let (mut project, agent_run_id, terminal_id, root, bin) =
        project_with_running_agent_for_outcome("agent-outcome-orphaned");

    project
        .apply_agent_terminal_outcome(
            &agent_run_id,
            &terminal_id,
            &TerminationOutcome::OrphanedUnknown {
                summary: BoundedRuntimeSummary::new("process group state unknown"),
            },
        )
        .expect("orphaned terminal outcome should update AgentRun");

    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Detached);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        TerminalStatus::OrphanedUnknown
    );
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_maps_failed_agent_run_outcome_to_failed() {
    let (mut project, agent_run_id, terminal_id, root, bin) =
        project_with_running_agent_for_outcome("agent-outcome-failed");

    project
        .apply_agent_terminal_outcome(
            &agent_run_id,
            &terminal_id,
            &TerminationOutcome::Failed {
                summary: BoundedRuntimeSummary::new("runtime wait failed"),
            },
        )
        .expect("failed terminal outcome should update AgentRun");

    assert_eq!(project.agent_runs()[0].status, AgentRunStatus::Failed);
    assert_eq!(
        project.terminal_session(&terminal_id).unwrap().status(),
        TerminalStatus::Failed
    );
    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_rejects_mismatched_terminal_without_partial_attachment() {
    let root = test_root("agent-attach-mismatch-root");
    let bin = test_root("agent-attach-mismatch-bin");
    let executable = executable_file(&bin, "ai-cli");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&executable);
    let plan = launch_plan_for(&project, &profile);
    let mut terminal = terminal_from_plan(&plan);
    terminal.command_line_summary = "different command".to_owned();

    let error = project
        .attach_agent_launch_plan(plan, terminal)
        .expect_err("terminal metadata must match launch spec");

    assert_eq!(
        error,
        ProjectAgentLaunchError::TerminalDoesNotMatchLaunchSpec
    );
    assert!(project.terminal_sessions().is_empty());
    assert!(project.agent_runs().is_empty());

    cleanup_root(root);
    cleanup_root(bin);
}

#[test]
fn project_session_rejects_duplicate_launch_plan_without_extra_attachment() {
    let root = test_root("agent-attach-duplicate-root");
    let bin = test_root("agent-attach-duplicate-bin");
    let executable = executable_file(&bin, "ai-cli");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&executable);
    let plan = launch_plan_for(&project, &profile);
    let terminal = terminal_from_plan(&plan);

    project
        .attach_agent_launch_plan(plan.clone(), terminal.clone())
        .expect("first launch plan attachment should succeed");
    let error = project
        .attach_agent_launch_plan(plan, terminal)
        .expect_err("duplicate plan attachment must be rejected");

    assert_eq!(
        error,
        ProjectAgentLaunchError::Ownership(OwnershipError::DuplicateAttachment)
    );
    assert_eq!(project.terminal_sessions().len(), 1);
    assert_eq!(project.agent_runs().len(), 1);

    cleanup_root(root);
    cleanup_root(bin);
}

fn built_in_profile(executable: &Path) -> AiCliProfile {
    let mut profile = AiCliProfile::new(
        "builtin-ai",
        "Built-in AI",
        AiCliProfileSource::BuiltIn,
        AiCliExecutable::Absolute {
            path: executable.to_path_buf(),
            provenance: AiCliExecutableProvenance::SystemPathReviewed,
        },
        AgentCompatibilityLevel::Supervised,
    );
    profile.workspace_discovery_policy = AiCliWorkspaceDiscoveryPolicy::DisabledByLaunch {
        evidence: "CLI started with reviewed flag that disables project config discovery"
            .to_owned(),
    };
    profile
}

fn launch_plan_for(project: &ProjectSession, profile: &AiCliProfile) -> AgentRunLaunchPlan {
    let request = request_for(project, profile);
    let validation = AgentRunLaunchValidator
        .validate(project, profile, &request)
        .expect("profile should validate before launch plan creation");
    AgentRunLaunchPlan::from_validation(validation, "Built-in AI")
        .expect("validated launch should produce a launch plan")
}

fn terminal_from_plan(plan: &AgentRunLaunchPlan) -> TerminalSession {
    TerminalSession::new(
        plan.terminal_launch_spec().project_id.clone(),
        plan.terminal_launch_spec().kind,
        plan.terminal_launch_spec().title.clone(),
        plan.terminal_launch_spec().cwd.clone(),
        plan.terminal_launch_spec().command_line_summary.clone(),
    )
}

fn assert_active_file_blocked(
    error: ProjectAgentRuntimeLaunchError,
    expected_reason: ProjectActiveFileLaunchBlockReason,
) {
    match error {
        ProjectAgentRuntimeLaunchError::ActiveFile(ProjectAgentActiveFileLaunchError::Blocked(
            assessment,
        )) => {
            assert_eq!(
                assessment.decision,
                crate::project::ProjectActiveFileLaunchDecision::Blocked(expected_reason)
            );
            assert!(assessment.active_path_hint.is_some());
            assert!(assessment.state.is_some());
        }
        other => panic!("expected active-file launch block, got {other:?}"),
    }
}

fn project_with_running_agent_for_outcome(
    name: &str,
) -> (ProjectSession, AgentRunId, TerminalId, PathBuf, PathBuf) {
    let root = test_root(&format!("{name}-root"));
    let bin = test_root(&format!("{name}-bin"));
    let executable = executable_file(&bin, "ai-cli");
    let mut project = restricted_project(ProjectId::for_test(1), &root);
    let profile = built_in_profile(&executable);
    let mut plan = launch_plan_for(&project, &profile);
    plan.transition_agent_run_to(AgentRunStatus::Preparing)
        .unwrap();
    plan.transition_agent_run_to(AgentRunStatus::Running)
        .unwrap();
    let agent_run_id = plan.agent_run().id.clone();
    let mut terminal = terminal_from_plan(&plan);
    terminal.transition_to(TerminalStatus::Running).unwrap();
    let terminal_id = terminal.id.clone();

    project
        .attach_agent_launch_plan(plan, terminal)
        .expect("running model AgentRun should attach");

    (project, agent_run_id, terminal_id, root, bin)
}

fn request_for(project: &ProjectSession, profile: &AiCliProfile) -> AgentRunLaunchRequest {
    AgentRunLaunchRequest::new(project.id().clone(), &profile.id, "prompt")
}

fn restricted_project(project_id: ProjectId, root: &Path) -> ProjectSession {
    ProjectSession::new(project_id, "Project", root, root)
}

fn executable_file(root: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(root).expect("test executable directory should exist");
    let executable = root.join(name);
    std::fs::write(&executable, "#!/bin/sh\n").expect("test executable should be written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("test executable metadata should be readable")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&executable, permissions)
        .expect("test executable permissions should be set");
    executable
}

fn test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

/// `CARGO_BIN_EXE_<name>` is only guaranteed for genuine integration test
/// targets (`tests/*.rs`), not a lib's own `#[cfg(test)]` unit tests --
/// duplicated from `approval::tests::reference_adapter`'s own identically-
/// named, identically-documented helper rather than shared across test
/// modules, matching this crate's established convention for small test-
/// only infrastructure.
fn reference_adapter_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_reference_adapter") {
        return PathBuf::from(path);
    }
    let test_exe = std::env::current_exe().expect("current_exe should resolve for a running test");
    let profile_dir = test_exe
        .parent()
        .and_then(Path::parent)
        .expect("test binary should live under target/<profile>/deps");
    let candidate = profile_dir.join("reference_adapter");
    assert!(
        candidate.is_file(),
        "expected the reference_adapter binary at {}; the [[bin]] target may not have built",
        candidate.display()
    );
    candidate
}

/// A real, sqlite-backed `AuditStore` -- duplicated from
/// `approval::tests::coordinator`'s own `TestAudit` (private to a sibling
/// module tree), same reasoning as `reference_adapter_binary_path` above.
struct RoundTripAudit {
    store: crate::audit::AuditStore,
    health: crate::audit::AuditHealth,
}

impl RoundTripAudit {
    fn new(name: &str) -> Self {
        let state_root =
            std::env::temp_dir().join(format!("ta-audit-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&state_root).expect("create temp audit state root");
        let state_root = state_root
            .canonicalize()
            .expect("canonicalize temp audit state root");
        let storage_path = crate::audit::AuditPathResolver
            .resolve(crate::audit::AuditPathRequest::new(state_root, Vec::new()))
            .expect("resolve audit storage path");
        let store = crate::audit::AuditStore::open(storage_path).expect("open a real audit store");
        Self {
            store,
            health: crate::audit::AuditHealth::default(),
        }
    }

    fn coordinator(&mut self) -> crate::audit::AuditCoordinator<'_> {
        crate::audit::AuditCoordinator::new(&mut self.store, &mut self.health)
    }
}

fn read_until_contains(
    runtime: &mut LinuxTerminalRuntime,
    handle: &TerminalRuntimeHandle,
    marker: &[u8],
) -> Vec<u8> {
    let mut output = Vec::new();
    for _ in 0..80 {
        let (chunk, _) = runtime
            .read_available_bounded_for(handle, Duration::from_millis(50), 16 * 1024)
            .expect("PTY output read should succeed");
        output.extend_from_slice(&chunk);
        if contains_subsequence(&output, marker) {
            break;
        }
    }
    output
}

fn contains_subsequence(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
