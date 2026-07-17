use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{
    AgentCompatibilityLevel, AgentRunStatus, OwnershipError, TerminalKind, TerminalSession,
};
use crate::project::{ProjectAgentLaunchError, ProjectId, ProjectSession};
use crate::runtime::terminal::TerminalEnvironmentPolicy;
use crate::security::{BoundedTranscriptRetention, TranscriptPrivacyPolicy};

use super::{
    AgentRunLaunchPlan, AgentRunLaunchRequest, AgentRunLaunchValidationError,
    AgentRunLaunchValidator, AiCliAdapterCapabilities, AiCliEnvironmentPolicy, AiCliExecutable,
    AiCliExecutableProvenance, AiCliProfile, AiCliProfileSource, AiCliPromptPolicy,
    AiCliWorkspaceDiscoveryPolicy, ExecutableLookupPath,
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

    assert_eq!(validation.project_id, project.id().clone());
    assert_eq!(validation.profile_id, "builtin-ai");
    assert_eq!(
        validation.executable_path,
        executable.canonicalize().unwrap()
    );
    assert_eq!(
        validation.executable_provenance,
        AiCliExecutableProvenance::SystemPathReviewed
    );
    assert_eq!(validation.cwd, root.canonicalize().unwrap());
    assert_eq!(
        validation.compatibility_level,
        AgentCompatibilityLevel::Supervised
    );
    assert_eq!(validation.prompt_summary, "summarize current project");
    assert_eq!(
        validation.workspace_discovery_summary.as_str(),
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
fn transcript_byte_persistence_is_blocked_until_rfc011() {
    let root = test_root("agent-transcript-blocked");
    let bin = test_root("agent-transcript-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(ProjectId::for_test(1), &root);
    let mut profile = built_in_profile(&executable);
    profile.transcript_policy = TranscriptPrivacyPolicy::local_bounded_agent_run_default(
        BoundedTranscriptRetention::by_size_and_age(1024, 1),
    );
    let request = request_for(&project, &profile);

    assert_eq!(
        AgentRunLaunchValidator
            .validate(&project, &profile, &request)
            .expect_err("transcript bytes remain blocked until RFC-011"),
        AgentRunLaunchValidationError::TranscriptBytesBlockedUntilRetentionPolicy
    );
    cleanup_root(root);
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

    assert_eq!(plan.spec.project_id, project.id().clone());
    assert_eq!(plan.agent_run.project_id, project.id().clone());
    assert_eq!(plan.agent_run.profile_id, "builtin-ai");
    assert_eq!(plan.agent_run.status, AgentRunStatus::Ready);
    assert_eq!(plan.agent_run.terminal_id, None);
    assert_eq!(plan.terminal_launch_spec.project_id, project.id().clone());
    assert_eq!(plan.terminal_launch_spec.kind, TerminalKind::Supervised);
    assert_eq!(plan.terminal_launch_spec.cwd, root.canonicalize().unwrap());
    assert_eq!(
        plan.terminal_launch_spec.shell,
        executable.canonicalize().unwrap()
    );
    assert_eq!(
        plan.terminal_launch_spec.environment_policy,
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
        path_only_plan.terminal_launch_spec.environment_policy,
        TerminalEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned()])
    );
    assert_eq!(
        path_home_plan.terminal_launch_spec.environment_policy,
        TerminalEnvironmentPolicy::ExplicitAllowlist(vec!["PATH".to_owned(), "HOME".to_owned()])
    );
    assert_ne!(
        path_only_plan.terminal_launch_spec.environment_policy,
        path_home_plan.terminal_launch_spec.environment_policy
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
    let agent_run_id = plan.agent_run.id.clone();

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
        plan.terminal_launch_spec.project_id.clone(),
        plan.terminal_launch_spec.kind,
        plan.terminal_launch_spec.title.clone(),
        plan.terminal_launch_spec.cwd.clone(),
        plan.terminal_launch_spec.command_line_summary.clone(),
    )
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
