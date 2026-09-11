use std::collections::BTreeMap;

/// RFC-023 PR-023-B, narrowed by **RFC-045 D3'**: the typed configuration
/// document is now exactly the set of settings that have a consumer.
///
/// RFC-023 modelled all eight `REQ-CONFIG-007` sections as typed storage
/// awaiting readers, and every one of them was still unread when RFC-045
/// measured it. D3' decided the parser must not accept what it cannot
/// deliver -- **a key the file accepts and the product ignores is a lie
/// the user reads** -- so the unreached fields are gone from this model
/// rather than parsed into fields nothing reads, and
/// [`super::parse_and_validate`] refuses each of them by name.
///
/// Each future RFC that wires a setting adds its key back **in the same
/// change as the consumer** (RFC-036 D2's named-consumer rule, applied to
/// configuration keys). Nothing here is a judgement that the withdrawn
/// settings are unwanted -- `ui`, `terminal`, `keybindings`, `core`,
/// `projects` and `security` were all real schema, and each returns with
/// the code that reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigurationDocument {
    pub agent: AgentSettings,
    pub resources: ResourceSettings,
}

/// A configuration-defined AI CLI profile, as the config document itself
/// represents it -- **not** [`crate::agent::profile::AiCliProfile`].
/// Translating one into the other is [`super::to_ai_cli_profile`].
///
/// **RFC-045 D3': `args`, `adapter` and `environment_policy` are gone.**
/// `AiCliProfile` has no field an argv template could go into, and
/// `to_ai_cli_profile` read none of the three -- so a user could write
/// `args = ["--model", "x"]`, watch the file validate, launch the
/// profile, and get no `--model`, with nothing saying so. That is §1 of
/// the risk document in its original form. An argv template is an
/// RFC-010 amendment to `AiCliProfile`, reserved and not written; until
/// then the parser refuses the key rather than storing it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredAiCliProfile {
    pub display_name: String,
    pub command: String,
}

/// `transcript_retention_days` reuses the real compiled default
/// ([`crate::transcript::DEFAULT_TRANSCRIPT_MAX_AGE_DAYS`]) rather than
/// repeating the number, so the two cannot drift out of sync silently.
///
/// **`default_profile` is RFC-045 D8**: `profiles` is a map, and RFC-023
/// never said which entry the launch button uses -- without this key,
/// a profile is definable and unlaunchable. `None` means
/// `AiCliProfile::claude_code_linux_default()`, unchanged from today; a
/// value naming an id the file does not define is a diagnostic. It is
/// **not** a [`super::SecuritySensitiveField`]: pointing the default at
/// a different profile does not bypass RFC-045 D4's first-use gate,
/// which is keyed by profile id, so switching to a profile this session
/// has not confirmed still requires the confirmation that names its
/// resolved executable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSettings {
    pub default_profile: Option<String>,
    pub transcript_retention_days: u32,
    pub profiles: BTreeMap<String, ConfiguredAiCliProfile>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            default_profile: None,
            transcript_retention_days: crate::transcript::DEFAULT_TRANSCRIPT_MAX_AGE_DAYS,
            profiles: BTreeMap::new(),
        }
    }
}

/// **RFC-045 D6.** RFC-023's three `[resources]` keys
/// (`max_terminal_output_mb_per_session`, `max_agent_transcript_mb_per_run`,
/// `max_file_watch_events_per_batch`) share no field with
/// `ProjectResourceLimits`, so `set_resource_limits` -- one of the four
/// rows RFC-036 kept against this RFC -- had nothing in the file to feed
/// it. `agent_run_limit` is the one limit the launch path already
/// enforces (`project.resource_limits().agent_run_limit`), so it is the
/// one whose consumer is not hypothetical.
///
/// `None` means **unlimited**, matching `ProjectResourceLimits`' own
/// `Option<u32>`; `0` is refused at parse, since a limit of zero refuses
/// every launch and is far more likely a typo than an intent. Not a
/// [`super::SecuritySensitiveField`]: RFC-023's classification does not
/// cover it, and widening that vocabulary inside a reachability slice is
/// what D7 refuses to do.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourceSettings {
    pub agent_run_limit: Option<u32>,
}
