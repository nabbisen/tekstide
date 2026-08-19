//! RFC-023 PR-023-E's own review gate: "write the bypass tests first."
//! Every test spawning a real executable and a real project root proves
//! `config::to_ai_cli_profile`'s output reaches `AgentRunLaunchValidator::validate`
//! -- RFC-010's real, unmodified provenance validation -- not a
//! reimplementation or a shortcut. `agent::tests` already proves these
//! same four guards for `BuiltIn`-sourced profiles constructed directly;
//! this module duplicates the same shape (this crate's own established
//! convention for small test-only infrastructure -- see
//! `agent::tests::reference_adapter_binary_path`'s own doc comment for
//! the identical reasoning) with one deliberate difference: every
//! profile here is built by the real translator, from a real
//! `ConfiguredAiCliProfile`, so what is proven is "config text, through
//! real code, reaches real validation" -- not merely "the validator
//! works," which is already covered elsewhere.

use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agent::{
    AgentRunLaunchRequest, AgentRunLaunchValidationError, AgentRunLaunchValidator, AiCliExecutable,
    AiCliExecutableProvenance, AiCliProfileSource, ExecutableLookupPath,
};
use crate::config::{ConfiguredAiCliProfile, to_ai_cli_profile};
use crate::domain::AgentCompatibilityLevel;
use crate::project::{ProjectId, ProjectSession};

fn configured_profile(command: impl Into<String>) -> ConfiguredAiCliProfile {
    ConfiguredAiCliProfile {
        display_name: "Test Profile".to_owned(),
        command: command.into(),
        args: Vec::new(),
        adapter: "terminal-native".to_owned(),
        environment_policy: "explicit".to_owned(),
    }
}

fn request_for(project: &ProjectSession, profile_id: &str) -> AgentRunLaunchRequest {
    AgentRunLaunchRequest::new(project.id().clone(), profile_id, "prompt")
}

fn restricted_project(root: &Path) -> ProjectSession {
    ProjectSession::new(ProjectId::for_test(1), "Project", root, root)
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
    let root = std::env::temp_dir().join(format!(
        "tekstide-config-{name}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

// --- Bypass case 1: project-root executable -------------------------

#[test]
fn config_profile_pointing_at_a_project_root_executable_is_rejected() {
    let root = test_root("bypass-project-root-exe");
    let executable = executable_file(&root, "ai-cli");
    let project = restricted_project(&root);
    let configured = configured_profile(executable.to_str().unwrap());
    let profile = to_ai_cli_profile("hostile", &configured);
    let request = request_for(&project, &profile.id);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("a config profile naming a project-root executable must be rejected");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::WorkspaceLocalExecutableBlocked {
            path: executable.canonicalize().unwrap(),
        }
    );
    cleanup_root(root);
}

// --- Bypass case 2: a wrapper script inside the project root --------

#[test]
fn config_profile_pointing_at_a_wrapper_script_inside_the_project_root_is_rejected() {
    let root = test_root("bypass-wrapper");
    // A "wrapper" in the sense the handoff names: a thin script that
    // would re-exec something else, living inside the project root --
    // the same `executable.starts_with(root)` guard as case 1 catches
    // it, proven as its own case because the review gate names it
    // separately, not because the mechanism differs.
    let wrapper = root.join("wrapper.sh");
    std::fs::write(&wrapper, "#!/bin/sh\nexec /usr/bin/real-ai-cli \"$@\"\n")
        .expect("wrapper script should be written");
    let mut permissions = std::fs::metadata(&wrapper).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&wrapper, permissions).unwrap();
    let project = restricted_project(&root);
    let configured = configured_profile(wrapper.to_str().unwrap());
    let profile = to_ai_cli_profile("hostile-wrapper", &configured);
    let request = request_for(&project, &profile.id);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err(
            "a config profile naming a wrapper script inside the project root must be rejected",
        );

    assert_eq!(
        error,
        AgentRunLaunchValidationError::WorkspaceLocalExecutableBlocked {
            path: wrapper.canonicalize().unwrap(),
        }
    );
    cleanup_root(root);
}

// --- Bypass case 3: symlink resolving into the project root ---------

#[test]
fn config_profile_pointing_at_a_symlink_resolving_into_the_project_root_is_rejected() {
    let root = test_root("bypass-symlink-target");
    let outside = test_root("bypass-symlink-outside");
    let target = executable_file(&root, "ai-cli");
    let symlink_path = outside.join("ai-cli-link");
    symlink(&target, &symlink_path).expect("test symlink should be created");
    let project = restricted_project(&root);
    let configured = configured_profile(symlink_path.to_str().unwrap());
    let profile = to_ai_cli_profile("hostile-symlink", &configured);
    let request = request_for(&project, &profile.id);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err(
            "a config profile naming a symlink resolving into the project root must be rejected",
        );

    assert_eq!(
        error,
        AgentRunLaunchValidationError::WorkspaceLocalExecutableBlocked {
            path: target.canonicalize().unwrap(),
        }
    );
    cleanup_root(root);
    cleanup_root(outside);
}

// --- Bypass case 4: project-local PATH entry -------------------------

/// `to_ai_cli_profile` cannot produce a `project_local` lookup path --
/// `ConfiguredAiCliProfile` has no field that could request one (see
/// `resolve_executable`'s own doc comment). This test constructs the
/// shape directly, exactly because the translator cannot: what must be
/// proven is that `AgentRunLaunchValidator::validate` -- unmodified,
/// the same validator every other case here also reaches -- refuses a
/// `UserGlobal`-sourced profile relying on a project-local `PATH` entry,
/// regardless of what constructed it, as defense-in-depth against a
/// translator that gains this capability later.
#[test]
fn config_profile_relying_on_a_project_local_path_entry_is_rejected() {
    let root = test_root("bypass-project-local-path");
    let project_bin = root.join("bin");
    std::fs::create_dir_all(&project_bin).expect("project bin should be created");
    executable_file(&project_bin, "ai-cli");
    let project = restricted_project(&root);
    let mut profile = to_ai_cli_profile("hostile-path", &configured_profile("ai-cli"));
    profile.executable = AiCliExecutable::PathLookup {
        command: "ai-cli".to_owned(),
        lookup_paths: vec![ExecutableLookupPath::project_local(&project_bin)],
        provenance: AiCliExecutableProvenance::UserGlobal,
    };
    let request = request_for(&project, &profile.id);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("a config profile relying on a project-local PATH entry must be rejected");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::ProjectLocalPathLookupBlocked { path: project_bin }
    );
    cleanup_root(root);
}

// --- Managed does not confer Managed ---------------------------------

/// `to_ai_cli_profile` never sets `compatibility_level: Managed` --
/// structural, not a policy this function enforces at runtime (see its
/// own doc comment). Proven directly, for a wide range of `adapter`
/// strings including ones that read as an attempt to request Managed.
#[test]
fn to_ai_cli_profile_never_sets_managed_compatibility_or_structured_action_approval() {
    for adapter in ["terminal-native", "managed", "reference", "", "MANAGED"] {
        let mut configured = configured_profile("claude");
        configured.adapter = adapter.to_owned();
        let profile = to_ai_cli_profile("probe", &configured);

        assert_eq!(
            profile.compatibility_level,
            AgentCompatibilityLevel::Supervised,
            "adapter = {adapter:?} must not produce a Managed profile"
        );
        assert!(
            !profile.adapter_capabilities.structured_action_approval,
            "adapter = {adapter:?} must not set structured_action_approval"
        );
    }
}

/// The second, independent guarantee `to_ai_cli_profile`'s own doc
/// comment names: even if a future translator bug set
/// `compatibility_level: Managed` directly, without also setting
/// `structured_action_approval: true`, `AgentRunLaunchValidator::validate`'s
/// own, unmodified `validate_compatibility` check still refuses it.
/// This is not testing `to_ai_cli_profile` (which never does this) --
/// it is testing that RFC-010's validator would catch the mistake even
/// if it did, the same "prove the path reaches it" standard the other
/// bypass tests hold to.
#[test]
fn managed_compatibility_level_without_structured_action_approval_is_still_rejected() {
    let root = test_root("bypass-managed-without-capability");
    let bin = test_root("bypass-managed-without-capability-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(&root);
    let mut profile = to_ai_cli_profile(
        "hostile-managed",
        &configured_profile(executable.to_str().unwrap()),
    );
    profile.compatibility_level = AgentCompatibilityLevel::Managed;
    let request = request_for(&project, &profile.id);

    let error = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect_err("Managed without structured_action_approval must be rejected");

    assert_eq!(
        error,
        AgentRunLaunchValidationError::ManagedCapabilityMissing
    );
    cleanup_root(root);
    cleanup_root(bin);
}

// --- Positive control: the translator does not also break the happy path

/// The bypass tests above prove hostile config is rejected; this proves
/// the translator does not *also* reject a legitimate configuration --
/// a real executable outside every project root, real config-defined
/// profile, real validation, real success. Without this, a translator
/// that rejected everything would trivially pass every bypass test.
#[test]
fn a_legitimate_config_profile_outside_the_project_root_validates_successfully() {
    let root = test_root("legit-config-profile-root");
    let bin = test_root("legit-config-profile-bin");
    let executable = executable_file(&bin, "ai-cli");
    let project = restricted_project(&root);
    let profile = to_ai_cli_profile(
        "legit-config-profile",
        &configured_profile(executable.to_str().unwrap()),
    );
    let request = request_for(&project, &profile.id);

    let validation = AgentRunLaunchValidator
        .validate(&project, &profile, &request)
        .expect("a legitimate config-defined profile outside the project root should validate");

    assert_eq!(
        validation.executable_path(),
        executable.canonicalize().unwrap().as_path()
    );
    assert_eq!(
        validation.compatibility_level(),
        AgentCompatibilityLevel::Supervised
    );
    cleanup_root(root);
    cleanup_root(bin);
}

// --- Translator shape ------------------------------------------------

#[test]
fn to_ai_cli_profile_sets_user_global_source() {
    let profile = to_ai_cli_profile("id", &configured_profile("claude"));
    assert_eq!(profile.source, AiCliProfileSource::UserGlobal);
}

#[test]
fn to_ai_cli_profile_treats_a_path_like_command_as_absolute() {
    let profile = to_ai_cli_profile("id", &configured_profile("/usr/bin/claude"));
    match profile.executable {
        AiCliExecutable::Absolute { path, .. } => {
            assert_eq!(path, PathBuf::from("/usr/bin/claude"));
        }
        other => panic!("expected AiCliExecutable::Absolute, got {other:?}"),
    }
}

#[test]
fn to_ai_cli_profile_treats_a_bare_command_as_a_reviewed_system_path_lookup() {
    let profile = to_ai_cli_profile("id", &configured_profile("claude"));
    match profile.executable {
        AiCliExecutable::PathLookup {
            command,
            lookup_paths,
            ..
        } => {
            assert_eq!(command, "claude");
            assert!(!lookup_paths.is_empty());
            assert!(
                lookup_paths.iter().all(|entry| !entry.project_local),
                "a config-defined profile must never get a project-local lookup path: \
                 {lookup_paths:?}"
            );
        }
        other => panic!("expected AiCliExecutable::PathLookup, got {other:?}"),
    }
}
