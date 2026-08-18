use std::collections::BTreeMap;

/// RFC-023 PR-023-B: the typed configuration document. Every field has
/// a compiled default (see each section's `Default` impl below), so a
/// missing file, an empty file, and a file setting one key all produce
/// the same shape -- no caller downstream handles "unset."
///
/// **Scope, restated from the RFC's own Scoping section (2026-08-19):**
/// this type is storage for the eight `REQ-CONFIG-007` / external-design
/// sections. It is not, by itself, consumed anywhere yet. Keybindings,
/// theme/font values, and resource limits are named in the RFC's own
/// Scoping section as accumulated consumers with a design of their own
/// still to come; `security` and `agent`'s concurrency/profile fields
/// carry the same status here even though the Scoping section does not
/// name them individually -- none of the eight sections has a runtime
/// reader in this crate yet. Recorded in `qa-evidence.md`, not left to
/// be discovered by a caller expecting one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigurationDocument {
    pub core: CoreSettings,
    pub ui: UiSettings,
    pub keybindings: KeybindingSettings,
    pub terminal: TerminalSettings,
    pub projects: ProjectSettings,
    pub agent: AgentSettings,
    pub security: SecuritySettings,
    pub resources: ResourceSettings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreSettings {
    pub default_project_board: bool,
    pub recent_projects_limit: u32,
}

impl Default for CoreSettings {
    fn default() -> Self {
        Self {
            default_project_board: true,
            recent_projects_limit: 20,
        }
    }
}

/// External-design §11.5's illustrative shape. `font_size` matches
/// `Theme::default()`'s real compiled `font_size_body` (14.0, rounded
/// to the whole points this section models); the font-family strings
/// and `theme` name have no existing compiled equivalent to mirror --
/// `theme.rs` has no named-palette or font-family concept yet --
/// so these are the external design's suggested defaults, not a value
/// drawn from running code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiSettings {
    pub theme: String,
    pub editor_font_family: String,
    pub terminal_font_family: String,
    pub ui_font_family: String,
    pub font_size: u32,
    pub show_status_labels: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: "dark-default".to_owned(),
            editor_font_family: "JetBrains Mono".to_owned(),
            terminal_font_family: "JetBrains Mono".to_owned(),
            ui_font_family: "system".to_owned(),
            font_size: 14,
            show_status_labels: true,
        }
    }
}

/// `action name -> binding string`, unparsed at this layer -- validating
/// a binding string against the real grammar `input.rs` uses is
/// PR-023-C's "validate whole document" concern, not this typed
/// container's. Empty by default: an empty map overrides nothing, which
/// is the correct default under the RFC's own precedence rule
/// ("later sources override earlier ones per-key").
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KeybindingSettings {
    pub overrides: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSettings {
    pub shell_path: String,
    pub scrollback_lines: u32,
    pub multiline_paste_protection: bool,
    pub safe_escape_sequences: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            shell_path: "auto".to_owned(),
            scrollback_lines: 10_000,
            multiline_paste_protection: true,
            safe_escape_sequences: true,
        }
    }
}

/// `default_trust` and `open_duplicate_root` are plain strings, not
/// enums, on purpose: only one value of each is actually implemented
/// today (`"restricted"` -- every project starts untrusted, confirmed
/// against `AppState::add_project_session`; `"focus_existing"` --
/// `AddProjectOutcome::FocusedExisting` is the only real behavior an
/// already-open root produces), so an enum would assert a choice space
/// that does not exist yet. Validating the string against whatever
/// choice space PR-023-C settles on is that slice's job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSettings {
    pub default_trust: String,
    pub restore_recent_projects: bool,
    pub open_duplicate_root: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            default_trust: "restricted".to_owned(),
            restore_recent_projects: true,
            open_duplicate_root: "focus_existing".to_owned(),
        }
    }
}

/// A configuration-defined AI CLI profile, as the config document
/// itself represents it -- **not** [`crate::agent::profile::AiCliProfile`].
/// Translating one into the other, through RFC-010's unmodified
/// provenance validation, is PR-023-E's entire scope; this type only
/// stores what a `[agent.profile.*]` table says.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredAiCliProfile {
    pub display_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub adapter: String,
    pub environment_policy: String,
}

/// `transcript_retention_days` reuses the real compiled default
/// ([`crate::transcript::DEFAULT_TRANSCRIPT_MAX_AGE_DAYS`]) rather than
/// repeating the number, so the two cannot drift out of sync silently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSettings {
    pub max_concurrent_global: u32,
    pub max_concurrent_per_project: u32,
    pub default_environment_policy: String,
    pub transcript_retention_days: u32,
    pub capture_changed_files: bool,
    pub profiles: BTreeMap<String, ConfiguredAiCliProfile>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_concurrent_global: 2,
            max_concurrent_per_project: 1,
            default_environment_policy: "explicit".to_owned(),
            transcript_retention_days: crate::transcript::DEFAULT_TRANSCRIPT_MAX_AGE_DAYS,
            capture_changed_files: true,
            profiles: BTreeMap::new(),
        }
    }
}

/// External-design §11.5's illustrative `[security]` shape. All five
/// default to the tightest (`true`/blocking) setting, matching
/// "fail closed on every parse, validation, and IO error" -- but none
/// is read anywhere yet. `RestrictedModeFeature`'s real nine-variant
/// vocabulary (`security.rs`) is not reused as field names here: these
/// three `restricted_mode_blocks_*` flags are the external design's
/// illustrative subset, and deciding whether configuration can
/// selectively disable individual `RestrictedModeFeature` blocks at all
/// is a real design question PR-023-D's classification work, not this
/// typed container, has to settle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecuritySettings {
    pub restricted_mode_blocks_workspace_prompts: bool,
    pub restricted_mode_blocks_workspace_lsp: bool,
    pub restricted_mode_blocks_workspace_plugins: bool,
    pub redact_secret_like_environment_names: bool,
    pub require_approval_for_adapter_destructive_commands: bool,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            restricted_mode_blocks_workspace_prompts: true,
            restricted_mode_blocks_workspace_lsp: true,
            restricted_mode_blocks_workspace_plugins: true,
            redact_secret_like_environment_names: true,
            require_approval_for_adapter_destructive_commands: true,
        }
    }
}

/// No existing compiled bound to mirror for any of these three --
/// terminal output, transcript-per-run, and file-watch batch size are
/// all currently unbounded in production. Defaults are the external
/// design's own suggested numbers (§11.5), reserved surface until a
/// caller reads them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceSettings {
    pub max_terminal_output_mb_per_session: u32,
    pub max_agent_transcript_mb_per_run: u32,
    pub max_file_watch_events_per_batch: u32,
}

impl Default for ResourceSettings {
    fn default() -> Self {
        Self {
            max_terminal_output_mb_per_session: 64,
            max_agent_transcript_mb_per_run: 128,
            max_file_watch_events_per_batch: 1_000,
        }
    }
}
