use std::path::{Path, PathBuf};

use crate::agent::{
    AiCliEnvironmentPolicy, AiCliExecutable, AiCliExecutableProvenance, AiCliProfile,
    AiCliProfileSource, AiCliWorkspaceDiscoveryPolicy, ExecutableLookupPath,
};
use crate::domain::AgentCompatibilityLevel;

use super::model::{ConfiguredAiCliAdapter, ConfiguredAiCliProfile};

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
/// **RFC-060 D1/D2: `compatibility_level` and `adapter_capabilities.
/// structured_action_approval` both follow from `configured.adapter`,
/// the one match below -- never independently.** Before this RFC
/// neither could be requested at all (the match did not exist, only
/// `Supervised`); the match is now the *entire* surface for requesting
/// `Managed`, and there is no second field on `ConfiguredAiCliProfile`
/// either output could disagree with. **Declaring `Managed` in
/// configuration does not confer it** (RFC-023's own requirement) still
/// holds for the reason it always has, independent of this function:
/// even a translator bug that set `compatibility_level: Managed`
/// without also setting `structured_action_approval: true` -- which this
/// match cannot produce, since both arms set both together -- would
/// still be rejected by `AgentRunLaunchValidator::validate`'s own,
/// unmodified `validate_compatibility` check
/// (`managed_compatibility_level_without_structured_action_approval_is_still_rejected`,
/// `config/tests/profile.rs`, proves that guarantee directly, independent
/// of this function). What is new here is that `Managed` is *reachable*
/// at all, and reachable only this one way.
///
/// **RFC-045 D3' removed two fields this comment used to explain away,
/// and RFC-060 reintroduces a third under a different shape.** RFC-023
/// parsed `args`, `adapter` and `environment_policy` into
/// [`ConfiguredAiCliProfile`] and this function read none of them,
/// recording that fact in a doc comment and in `qa-evidence.md` -- which
/// is not where a user of the configuration file looks. A profile with
/// `args = ["--model", "x"]` validated, appeared, launched, and dropped
/// `--model` in silence. `args` and `environment_policy` stay gone; an
/// argv template is an RFC-010 amendment, reserved and not written.
/// `adapter` is back, but not as the withdrawn freestanding string --
/// [`ConfiguredAiCliAdapter`] is a two-variant enum this function reads
/// exhaustively, not a value it could silently ignore the way the old
/// field was ignored.
///
/// Every configuration-defined profile therefore gets
/// [`AiCliEnvironmentPolicy::Minimal`] -- the least environment exposure
/// `AiCliProfile::new` itself already defaults to, not a weaker one
/// invented for this translation, and now not a value a file can ask to
/// change without the code to honour the request.
pub fn to_ai_cli_profile(id: &str, configured: &ConfiguredAiCliProfile) -> AiCliProfile {
    // Review 535 Required 2: one match producing both answers at once,
    // not two independent reads of `configured.adapter` -- the first
    // draft had a no-wildcard `match` for `compatibility_level` and a
    // separate `if matches!(..., Managed)` for
    // `structured_action_approval`, which a third `ConfiguredAiCliAdapter`
    // variant could pass through one and fall through the other (the
    // `if` has no "and nothing else" arm to force a decision). This
    // tuple match cannot half-decide: the compiler demands both answers
    // for every variant in the same place they are produced.
    let (compatibility_level, structured_action_approval) = match configured.adapter {
        ConfiguredAiCliAdapter::None => (AgentCompatibilityLevel::Supervised, false),
        ConfiguredAiCliAdapter::Managed => (AgentCompatibilityLevel::Managed, true),
    };
    let mut profile = AiCliProfile::new(
        id,
        configured.display_name.clone(),
        AiCliProfileSource::UserGlobal,
        resolve_executable(&configured.command),
        compatibility_level,
    );
    profile.environment_policy = AiCliEnvironmentPolicy::Minimal;
    profile.workspace_discovery_policy = AiCliWorkspaceDiscoveryPolicy::NoKnownWorkspaceDiscovery {
        evidence: "configuration-defined profile: no workspace discovery declared".to_owned(),
    };
    profile.adapter_capabilities.structured_action_approval = structured_action_approval;
    profile
}

/// A `command` containing a path separator (an absolute path, or a
/// relative one like `./bin/tool`) is resolved as-is; a bare name
/// (`"claude"`, `"codex"`) is resolved by reviewed-system-path lookup
/// only. **Never `ExecutableLookupPath::project_local`** -- there is no
/// field on `ConfiguredAiCliProfile` that could request one, the same
/// "structurally incapable, not merely policy-refused" shape
/// `to_ai_cli_profile`'s own doc comment now gives *the combination* of
/// `Managed` without an adapter declaration (RFC-060) -- a sibling
/// property, not the identical one: `Managed` itself is reachable since
/// RFC-060, `project_local` is not reachable at all. The
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
                // RFC-028 D8: a GUI-launched macOS app does not inherit the
                // user's interactive shell `PATH` at all, so a bare command
                // name must still find an Apple Silicon Homebrew install.
                // Harmless on every other platform: lookup only checks
                // whether the path actually exists.
                ExecutableLookupPath::reviewed_system("/opt/homebrew/bin"),
                ExecutableLookupPath::reviewed_system("/usr/local/bin"),
                ExecutableLookupPath::reviewed_system("/usr/bin"),
            ],
            provenance: AiCliExecutableProvenance::UserGlobal,
        }
    }
}
