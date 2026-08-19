use std::path::{Path, PathBuf};

use crate::agent::{
    AiCliEnvironmentPolicy, AiCliExecutable, AiCliExecutableProvenance, AiCliProfile,
    AiCliProfileSource, AiCliWorkspaceDiscoveryPolicy, ExecutableLookupPath,
};
use crate::domain::AgentCompatibilityLevel;

use super::model::ConfiguredAiCliProfile;

/// RFC-023 PR-023-E: `ConfiguredAiCliProfile` -> `AiCliProfile` --
/// `ConfiguredAiCliProfile`'s own doc comment names this translation as
/// "PR-023-E's entire scope." `id` is the `[agent.profile.*]` table key
/// (the real, unbounded profile identity -- see `extract_agent_profiles`'s
/// own doc comment for why it is never bounded/escaped the way a
/// diagnostic's `key` is).
///
/// **RFC-010's validation is not touched by this function**, and cannot
/// be: it only *constructs* an `AiCliProfile` value. Whether a launch
/// actually succeeds is still entirely `AgentRunLaunchValidator::validate`'s
/// call, unmodified -- this function's whole job is producing a value
/// that validator can correctly accept or reject on its own, existing
/// terms. `source` is always `UserGlobal`, per RFC-023 §AI CLI Profiles
/// From Configuration ("a configuration-defined profile is an
/// `AiCliProfileSource::UserGlobal` profile") -- never `WorkspaceLocal`,
/// because RFC-023 v1 loads only defaults and user-global configuration;
/// there is no workspace-config loading code path that could produce a
/// profile from a file inside a project root at all (see this crate's
/// own handoff pack, Workspace Configuration Checklist).
///
/// `compatibility_level` is always [`AgentCompatibilityLevel::Supervised`],
/// and `adapter_capabilities` is always the default
/// (`structured_action_approval: false`) -- there is no field on
/// `ConfiguredAiCliProfile` a document could use to request `Managed`,
/// so this is not merely a policy choice this function enforces, it is
/// the only value this function is capable of producing. **Declaring
/// `Managed` in configuration does not confer it** (RFC-023's own
/// requirement) holds twice over: structurally, because there is
/// nowhere in the schema to declare it, and independently, because even
/// a future translator bug that set `compatibility_level: Managed`
/// without also setting `structured_action_approval: true` would still
/// be rejected by `AgentRunLaunchValidator::validate`'s own, unmodified
/// `validate_compatibility` check
/// (`managed_compatibility_level_without_structured_action_approval_is_still_rejected`,
/// `config/tests/profile.rs`, proves the second guarantee directly,
/// independent of this function).
///
/// `configured.args` has no corresponding field on `AiCliProfile` at
/// all -- the launch pipeline has no argv-template concept for a
/// profile to configure yet, so this function does not attempt to wire
/// it anywhere. Not silently dropped without record: this is the same
/// "typed storage, no consumer yet" status this pack's own Scoping
/// section already gives keybindings, theme values, and several other
/// fields -- stated here, and in `qa-evidence.md`, rather than left for
/// a reader to discover that a configured value does nothing.
/// `environment_policy`'s string value is the same story: nothing here
/// parses it into a specific [`AiCliEnvironmentPolicy`] yet, so every
/// configuration-defined profile gets [`AiCliEnvironmentPolicy::Minimal`]
/// -- the least environment exposure `AiCliProfile::new` itself already
/// defaults to, not a weaker one invented for this translation.
pub fn to_ai_cli_profile(id: &str, configured: &ConfiguredAiCliProfile) -> AiCliProfile {
    let mut profile = AiCliProfile::new(
        id,
        configured.display_name.clone(),
        AiCliProfileSource::UserGlobal,
        resolve_executable(&configured.command),
        AgentCompatibilityLevel::Supervised,
    );
    profile.environment_policy = AiCliEnvironmentPolicy::Minimal;
    profile.workspace_discovery_policy = AiCliWorkspaceDiscoveryPolicy::NoKnownWorkspaceDiscovery {
        evidence: "configuration-defined profile: no workspace discovery declared".to_owned(),
    };
    profile
}

/// A `command` containing a path separator (an absolute path, or a
/// relative one like `./bin/tool`) is resolved as-is; a bare name
/// (`"claude"`, `"codex"`) is resolved by reviewed-system-path lookup
/// only. **Never `ExecutableLookupPath::project_local`** -- there is no
/// field on `ConfiguredAiCliProfile` that could request one, the same
/// "structurally incapable, not merely policy-refused" property
/// `to_ai_cli_profile`'s own doc comment gives `Managed`. The
/// project-local-`PATH` bypass case
/// (`config_profile_relying_on_a_project_local_path_entry_is_rejected`,
/// `config/tests/profile.rs`) is proven by constructing an
/// `AiCliProfile` directly with a `project_local` lookup path, exactly
/// because this function cannot produce one -- the test exists to prove
/// `AgentRunLaunchValidator::validate` itself still refuses that shape,
/// not to prove this function avoids a mistake it structurally cannot
/// make.
fn resolve_executable(command: &str) -> AiCliExecutable {
    if command.contains(std::path::MAIN_SEPARATOR) || Path::new(command).is_absolute() {
        AiCliExecutable::Absolute {
            path: PathBuf::from(command),
            provenance: AiCliExecutableProvenance::UserGlobal,
        }
    } else {
        AiCliExecutable::PathLookup {
            command: command.to_owned(),
            lookup_paths: vec![
                ExecutableLookupPath::reviewed_system("/usr/local/bin"),
                ExecutableLookupPath::reviewed_system("/usr/bin"),
            ],
            provenance: AiCliExecutableProvenance::UserGlobal,
        }
    }
}
