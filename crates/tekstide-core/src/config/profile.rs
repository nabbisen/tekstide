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
/// **RFC-045 D3' removed the three fields this comment used to explain
/// away.** RFC-023 parsed `args`, `adapter` and `environment_policy`
/// into [`ConfiguredAiCliProfile`] and this function read none of them,
/// recording that fact in a doc comment and in `qa-evidence.md` --
/// which is not where a user of the configuration file looks. A profile
/// with `args = ["--model", "x"]` validated, appeared, launched, and
/// dropped `--model` in silence. The fields are gone from the model and
/// [`super::parse_and_validate`] now refuses the keys by name, so the
/// silence is gone with them; each returns when `AiCliProfile` grows the
/// field that would carry it (an argv template is an RFC-010 amendment,
/// reserved and not written).
///
/// Every configuration-defined profile therefore gets
/// [`AiCliEnvironmentPolicy::Minimal`] -- the least environment exposure
/// `AiCliProfile::new` itself already defaults to, not a weaker one
/// invented for this translation, and now not a value a file can ask to
/// change without the code to honour the request.
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
