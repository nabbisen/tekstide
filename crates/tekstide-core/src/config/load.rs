use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::ConfigurationDocument;
use crate::config::model::{
    AgentSettings, ConfiguredAiCliProfile, CoreSettings, KeybindingSettings, ProjectSettings,
    ResourceSettings, RestrictedDefaultTrust, SecuritySettings, TerminalSettings, UiSettings,
};

/// RFC-023 PR-023-C: a bounded, content-free diagnostic. `message` is
/// `&'static str` -- a compile-time-fixed string, never `String` --
/// specifically so there is no code path by which a runtime value from
/// the file (a raw setting, a rejected value, a secret-shaped string)
/// could ever flow into it. The same "inert by construction" shape
/// response 266 used for `RestrictedDefaultTrust`, applied to a
/// different property: not "cannot express a dangerous value" but
/// "cannot leak file content," because the type simply has no
/// constructor that accepts one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDiagnostic {
    /// The resolved configuration file's own path -- not file
    /// *contents*, just the well-known location RFC-023 §Format and
    /// Location already resolves. `None` from [`parse_and_validate`]
    /// itself, which validates source text with no path of its own;
    /// [`ConfigStore`] fills this in (via [`ConfigDiagnostic::with_path`])
    /// on every diagnostic it returns, since it is the one thing in
    /// this module that actually knows a path.
    pub path: Option<PathBuf>,
    /// `"section.field"`, or a fixed sentinel (`"<toml>"`, `"<file>"`)
    /// for a failure that precedes knowing which key was responsible.
    pub key: String,
    /// A byte-offset span into the source, when the underlying parser
    /// provides one (TOML syntax errors only) -- never source text.
    pub location: Option<String>,
    pub message: &'static str,
}

impl ConfigDiagnostic {
    fn with_path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }
}

/// Response 268: a *known* `key` (`"core.recent_projects_limit"`, and
/// so on) is a fixed literal this module wrote, already bounded by
/// being source code. An **unknown** key -- the TOML table key a warn
/// or error path names -- is whatever the file said, unfiltered. RFC-023
/// requires "a bounded diagnostic," and workspace configuration is
/// untrusted by this RFC's own design: a cloned repository's
/// `.tekstide/config.toml` can carry a key of arbitrary length or one
/// containing a bidi override, control characters, or other text shaped
/// to mislead. `AuditReference::new()` bounds its own untrusted segment
/// the same way -- capped length -- and this reuses that number rather
/// than inventing a second one.
///
/// Response 269: length is bounded here; **character shape is bounded
/// by [`crate::text_safety::escape_untrusted_chars`]**, not a second,
/// ad-hoc character filter. An earlier draft replaced every non-ASCII
/// character with `?`, which is a second escaping primitive next to the
/// one this project already reviewed, and a lossy one: it destroyed
/// legitimate non-Latin text along with the hostile characters, so a
/// Polish `ł`/`ą` or a profile named in Japanese or Cyrillic became an
/// unreadable row of `?`, defeating the diagnostic for exactly the
/// users the i18n work exists to serve. `escape_untrusted_chars` turns
/// only control and bidi-override characters into a visible `<U+XXXX>`
/// marker and passes every other character through unchanged. Truncate
/// first, escape second -- escaping expands (a marker is several
/// characters), so truncating the *raw* input to the cap keeps the
/// escaped result bounded without ever risking cutting a `<U+XXXX>`
/// marker in half.
const MAX_UNTRUSTED_KEY_SEGMENT_CHARS: usize = 128;

fn bound_key_segment(raw: &str) -> String {
    let truncated_raw: String = raw.chars().take(MAX_UNTRUSTED_KEY_SEGMENT_CHARS).collect();
    let was_truncated = raw.chars().count() > MAX_UNTRUSTED_KEY_SEGMENT_CHARS;
    let mut bounded = crate::text_safety::escape_untrusted_chars(&truncated_raw);
    if was_truncated {
        bounded.push('\u{2026}');
    }
    bounded
}

/// An unrecognized key: not fatal, per RFC-023's own rule ("unknown
/// keys warn; they do not fail") -- forward compatibility for a file
/// users hand-edit matters more than strictness here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigWarning {
    pub key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigLoadOutcome {
    pub document: ConfigurationDocument,
    pub warnings: Vec<ConfigWarning>,
}

/// The atomic pipeline's middle three stages -- parse, validate whole
/// document, construct -- as one pure function. No stage mutates
/// anything outside its own locals, so "no partial application" is
/// structural: either every section validates and a complete
/// [`ConfigurationDocument`] comes back, or the first problem found
/// aborts the whole call and nothing is returned at all. There is no
/// value this function can hand back that represents "half-applied."
pub fn parse_and_validate(source: &str) -> Result<ConfigLoadOutcome, ConfigDiagnostic> {
    let mut root: toml::Table =
        source
            .parse()
            .map_err(|error: toml::de::Error| ConfigDiagnostic {
                path: None,
                key: "<toml>".to_owned(),
                location: error
                    .span()
                    .map(|span| format!("byte {}..{}", span.start, span.end)),
                message: "malformed TOML syntax",
            })?;

    let mut warnings = Vec::new();

    let document = ConfigurationDocument {
        core: extract_core(&mut root, &mut warnings)?,
        ui: extract_ui(&mut root, &mut warnings)?,
        keybindings: extract_keybindings(&mut root)?,
        terminal: extract_terminal(&mut root, &mut warnings)?,
        projects: extract_projects(&mut root, &mut warnings)?,
        agent: extract_agent(&mut root, &mut warnings)?,
        security: extract_security(&mut root, &mut warnings)?,
        resources: extract_resources(&mut root, &mut warnings)?,
    };

    for key in root.keys() {
        warnings.push(ConfigWarning {
            key: bound_key_segment(key),
        });
    }

    Ok(ConfigLoadOutcome { document, warnings })
}

fn section_table(
    root: &mut toml::Table,
    section: &str,
) -> Result<Option<toml::Table>, ConfigDiagnostic> {
    match root.remove(section) {
        None => Ok(None),
        Some(toml::Value::Table(table)) => Ok(Some(table)),
        Some(_) => Err(ConfigDiagnostic {
            path: None,
            key: section.to_owned(),
            location: None,
            message: "expected a table",
        }),
    }
}

fn warn_unconsumed(table: toml::Table, section: &str, warnings: &mut Vec<ConfigWarning>) {
    for key in table.keys() {
        warnings.push(ConfigWarning {
            key: format!("{section}.{}", bound_key_segment(key)),
        });
    }
}

fn take_bool(
    table: &mut toml::Table,
    section: &str,
    field: &str,
    default: bool,
) -> Result<bool, ConfigDiagnostic> {
    match table.remove(field) {
        None => Ok(default),
        Some(toml::Value::Boolean(value)) => Ok(value),
        Some(_) => Err(ConfigDiagnostic {
            path: None,
            key: format!("{section}.{field}"),
            location: None,
            message: "expected a boolean",
        }),
    }
}

fn take_u32(
    table: &mut toml::Table,
    section: &str,
    field: &str,
    default: u32,
) -> Result<u32, ConfigDiagnostic> {
    match table.remove(field) {
        None => Ok(default),
        Some(toml::Value::Integer(value)) => u32::try_from(value).map_err(|_| ConfigDiagnostic {
            path: None,
            key: format!("{section}.{field}"),
            location: None,
            message: "expected an integer between 0 and 4294967295",
        }),
        Some(_) => Err(ConfigDiagnostic {
            path: None,
            key: format!("{section}.{field}"),
            location: None,
            message: "expected an integer",
        }),
    }
}

fn take_string(
    table: &mut toml::Table,
    section: &str,
    field: &str,
    default: &str,
) -> Result<String, ConfigDiagnostic> {
    match table.remove(field) {
        None => Ok(default.to_owned()),
        Some(toml::Value::String(value)) => Ok(value),
        Some(_) => Err(ConfigDiagnostic {
            path: None,
            key: format!("{section}.{field}"),
            location: None,
            message: "expected a string",
        }),
    }
}

fn require_string(
    table: &mut toml::Table,
    section: &str,
    field: &str,
) -> Result<String, ConfigDiagnostic> {
    match table.remove(field) {
        Some(toml::Value::String(value)) => Ok(value),
        Some(_) => Err(ConfigDiagnostic {
            path: None,
            key: format!("{section}.{field}"),
            location: None,
            message: "expected a string",
        }),
        None => Err(ConfigDiagnostic {
            path: None,
            key: format!("{section}.{field}"),
            location: None,
            message: "this key is required",
        }),
    }
}

fn take_string_array(
    table: &mut toml::Table,
    section: &str,
    field: &str,
) -> Result<Vec<String>, ConfigDiagnostic> {
    match table.remove(field) {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(items)) => items
            .into_iter()
            .map(|item| match item {
                toml::Value::String(value) => Ok(value),
                _ => Err(ConfigDiagnostic {
                    path: None,
                    key: format!("{section}.{field}"),
                    location: None,
                    message: "expected an array of strings",
                }),
            })
            .collect(),
        Some(_) => Err(ConfigDiagnostic {
            path: None,
            key: format!("{section}.{field}"),
            location: None,
            message: "expected an array of strings",
        }),
    }
}

fn extract_core(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<CoreSettings, ConfigDiagnostic> {
    let defaults = CoreSettings::default();
    let Some(mut table) = section_table(root, "core")? else {
        return Ok(defaults);
    };
    let settings = CoreSettings {
        default_project_board: take_bool(
            &mut table,
            "core",
            "default_project_board",
            defaults.default_project_board,
        )?,
        recent_projects_limit: take_u32(
            &mut table,
            "core",
            "recent_projects_limit",
            defaults.recent_projects_limit,
        )?,
    };
    warn_unconsumed(table, "core", warnings);
    Ok(settings)
}

fn extract_ui(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<UiSettings, ConfigDiagnostic> {
    let defaults = UiSettings::default();
    let Some(mut table) = section_table(root, "ui")? else {
        return Ok(defaults);
    };
    let settings = UiSettings {
        theme: take_string(&mut table, "ui", "theme", &defaults.theme)?,
        editor_font_family: take_string(
            &mut table,
            "ui",
            "editor_font_family",
            &defaults.editor_font_family,
        )?,
        terminal_font_family: take_string(
            &mut table,
            "ui",
            "terminal_font_family",
            &defaults.terminal_font_family,
        )?,
        ui_font_family: take_string(&mut table, "ui", "ui_font_family", &defaults.ui_font_family)?,
        font_size: take_u32(&mut table, "ui", "font_size", defaults.font_size)?,
        show_status_labels: take_bool(
            &mut table,
            "ui",
            "show_status_labels",
            defaults.show_status_labels,
        )?,
    };
    warn_unconsumed(table, "ui", warnings);
    Ok(settings)
}

/// Free-form: every key is a binding name, so there is no "unknown
/// key" for this section -- unlike the fixed-field sections, nothing
/// here is ever unconsumed.
fn extract_keybindings(root: &mut toml::Table) -> Result<KeybindingSettings, ConfigDiagnostic> {
    let Some(table) = section_table(root, "keybindings")? else {
        return Ok(KeybindingSettings::default());
    };
    let mut overrides = BTreeMap::new();
    for (key, value) in table {
        match value {
            toml::Value::String(binding) => {
                overrides.insert(key, binding);
            }
            _ => {
                return Err(ConfigDiagnostic {
                    path: None,
                    key: format!("keybindings.{}", bound_key_segment(&key)),
                    location: None,
                    message: "expected a binding string",
                });
            }
        }
    }
    Ok(KeybindingSettings { overrides })
}

fn extract_terminal(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<TerminalSettings, ConfigDiagnostic> {
    let defaults = TerminalSettings::default();
    let Some(mut table) = section_table(root, "terminal")? else {
        return Ok(defaults);
    };
    let settings = TerminalSettings {
        shell_path: take_string(&mut table, "terminal", "shell_path", &defaults.shell_path)?,
        scrollback_lines: take_u32(
            &mut table,
            "terminal",
            "scrollback_lines",
            defaults.scrollback_lines,
        )?,
        multiline_paste_protection: take_bool(
            &mut table,
            "terminal",
            "multiline_paste_protection",
            defaults.multiline_paste_protection,
        )?,
        safe_escape_sequences: take_bool(
            &mut table,
            "terminal",
            "safe_escape_sequences",
            defaults.safe_escape_sequences,
        )?,
    };
    warn_unconsumed(table, "terminal", warnings);
    Ok(settings)
}

/// `default_trust` is deliberately **not** a `take_string`/`take_bool`
/// call: response 266 requires the dangerous value to be an explicit,
/// named error rather than silently coerced or defaulted away, so a
/// user who writes `default_trust = "trusted"` learns their file was
/// refused, not that Tekstide quietly ignored what they asked for.
fn extract_projects(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<ProjectSettings, ConfigDiagnostic> {
    let defaults = ProjectSettings::default();
    let Some(mut table) = section_table(root, "projects")? else {
        return Ok(defaults);
    };
    let default_trust = match table.remove("default_trust") {
        None => RestrictedDefaultTrust,
        Some(toml::Value::String(value)) if value == "restricted" => RestrictedDefaultTrust,
        Some(_) => {
            return Err(ConfigDiagnostic {
                path: None,
                key: "projects.default_trust".to_owned(),
                location: None,
                message: "must be exactly \"restricted\" -- configuration cannot grant \
                          workspace trust; see RFC-032",
            });
        }
    };
    let settings = ProjectSettings {
        default_trust,
        restore_recent_projects: take_bool(
            &mut table,
            "projects",
            "restore_recent_projects",
            defaults.restore_recent_projects,
        )?,
        open_duplicate_root: take_string(
            &mut table,
            "projects",
            "open_duplicate_root",
            &defaults.open_duplicate_root,
        )?,
    };
    warn_unconsumed(table, "projects", warnings);
    Ok(settings)
}

fn extract_agent(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<AgentSettings, ConfigDiagnostic> {
    let defaults = AgentSettings::default();
    let Some(mut table) = section_table(root, "agent")? else {
        return Ok(defaults);
    };
    let profiles = extract_agent_profiles(&mut table, warnings)?;
    let settings = AgentSettings {
        max_concurrent_global: take_u32(
            &mut table,
            "agent",
            "max_concurrent_global",
            defaults.max_concurrent_global,
        )?,
        max_concurrent_per_project: take_u32(
            &mut table,
            "agent",
            "max_concurrent_per_project",
            defaults.max_concurrent_per_project,
        )?,
        default_environment_policy: take_string(
            &mut table,
            "agent",
            "default_environment_policy",
            &defaults.default_environment_policy,
        )?,
        transcript_retention_days: take_u32(
            &mut table,
            "agent",
            "transcript_retention_days",
            defaults.transcript_retention_days,
        )?,
        capture_changed_files: take_bool(
            &mut table,
            "agent",
            "capture_changed_files",
            defaults.capture_changed_files,
        )?,
        profiles,
    };
    warn_unconsumed(table, "agent", warnings);
    Ok(settings)
}

fn extract_agent_profiles(
    table: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<BTreeMap<String, ConfiguredAiCliProfile>, ConfigDiagnostic> {
    let Some(value) = table.remove("profile") else {
        return Ok(BTreeMap::new());
    };
    let toml::Value::Table(profiles_table) = value else {
        return Err(ConfigDiagnostic {
            path: None,
            key: "agent.profile".to_owned(),
            location: None,
            message: "expected a table",
        });
    };

    let mut profiles = BTreeMap::new();
    for (name, value) in profiles_table {
        // `section` is used only to build diagnostic/warning `key` text
        // below, so it is built from the bounded segment; `name` itself
        // stays unbounded for the real profile identity (the map key,
        // and `display_name`'s default) -- bounding it too would corrupt
        // data PR-023-E still has to validate on its own terms.
        let bounded_name = bound_key_segment(&name);
        let toml::Value::Table(mut profile_table) = value else {
            return Err(ConfigDiagnostic {
                path: None,
                key: format!("agent.profile.{bounded_name}"),
                location: None,
                message: "expected a table",
            });
        };
        let section = format!("agent.profile.{bounded_name}");
        let display_name = take_string(&mut profile_table, &section, "display_name", &name)?;
        let command = require_string(&mut profile_table, &section, "command")?;
        let args = take_string_array(&mut profile_table, &section, "args")?;
        let adapter = take_string(&mut profile_table, &section, "adapter", "terminal-native")?;
        let environment_policy = take_string(
            &mut profile_table,
            &section,
            "environment_policy",
            "explicit",
        )?;

        warn_unconsumed(profile_table, &section, warnings);

        profiles.insert(
            name,
            ConfiguredAiCliProfile {
                display_name,
                command,
                args,
                adapter,
                environment_policy,
            },
        );
    }
    Ok(profiles)
}

fn extract_security(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<SecuritySettings, ConfigDiagnostic> {
    let defaults = SecuritySettings::default();
    let Some(mut table) = section_table(root, "security")? else {
        return Ok(defaults);
    };
    let settings = SecuritySettings {
        restricted_mode_blocks_workspace_prompts: take_bool(
            &mut table,
            "security",
            "restricted_mode_blocks_workspace_prompts",
            defaults.restricted_mode_blocks_workspace_prompts,
        )?,
        restricted_mode_blocks_workspace_lsp: take_bool(
            &mut table,
            "security",
            "restricted_mode_blocks_workspace_lsp",
            defaults.restricted_mode_blocks_workspace_lsp,
        )?,
        restricted_mode_blocks_workspace_plugins: take_bool(
            &mut table,
            "security",
            "restricted_mode_blocks_workspace_plugins",
            defaults.restricted_mode_blocks_workspace_plugins,
        )?,
        redact_secret_like_environment_names: take_bool(
            &mut table,
            "security",
            "redact_secret_like_environment_names",
            defaults.redact_secret_like_environment_names,
        )?,
        require_approval_for_adapter_destructive_commands: take_bool(
            &mut table,
            "security",
            "require_approval_for_adapter_destructive_commands",
            defaults.require_approval_for_adapter_destructive_commands,
        )?,
    };
    warn_unconsumed(table, "security", warnings);
    Ok(settings)
}

fn extract_resources(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<ResourceSettings, ConfigDiagnostic> {
    let defaults = ResourceSettings::default();
    let Some(mut table) = section_table(root, "resources")? else {
        return Ok(defaults);
    };
    let settings = ResourceSettings {
        max_terminal_output_mb_per_session: take_u32(
            &mut table,
            "resources",
            "max_terminal_output_mb_per_session",
            defaults.max_terminal_output_mb_per_session,
        )?,
        max_agent_transcript_mb_per_run: take_u32(
            &mut table,
            "resources",
            "max_agent_transcript_mb_per_run",
            defaults.max_agent_transcript_mb_per_run,
        )?,
        max_file_watch_events_per_batch: take_u32(
            &mut table,
            "resources",
            "max_file_watch_events_per_batch",
            defaults.max_file_watch_events_per_batch,
        )?,
    };
    warn_unconsumed(table, "resources", warnings);
    Ok(settings)
}

/// What a load produced, beyond the document itself: warnings (always
/// non-fatal) and, when the file existed but did not parse or validate,
/// the diagnostic that explains why compiled defaults were used
/// instead.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigLoadReport {
    pub warnings: Vec<ConfigWarning>,
    pub diagnostic: Option<ConfigDiagnostic>,
}

/// RFC-023 PR-023-C: the stateful holder `reload` (this slice) and the
/// M13 file watcher (deferred) both call through. `current` is mutated
/// in exactly one place across this whole type (`reload`'s last line),
/// only after `parse_and_validate` has already returned a complete,
/// valid [`ConfigurationDocument`] -- so "no partial application" is
/// not merely tested, it is the only code path that can run.
#[derive(Clone, Debug)]
pub struct ConfigStore {
    config_file: PathBuf,
    current: ConfigurationDocument,
}

impl ConfigStore {
    /// Initial load. A missing file and an invalid file both start
    /// Tekstide normally with compiled defaults -- RFC-023 §Format and
    /// Location's "a missing configuration file is not an error"
    /// applies identically to an unreadable or invalid one: refusing to
    /// start would turn a typo into a denial of service. The two cases
    /// are distinguished only by whether `report.diagnostic` is `Some`.
    pub fn load(config_file: PathBuf) -> (Self, ConfigLoadReport) {
        let (current, report) = load_or_default(&config_file);
        (
            Self {
                config_file,
                current,
            },
            report,
        )
    }

    pub fn current(&self) -> &ConfigurationDocument {
        &self.current
    }

    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    /// Explicit reload (RFC-023 §Hot Reload: "a command or API call
    /// re-reads and re-validates"; the M13 watcher calls this same
    /// path with no policy change once it exists). On `Err`,
    /// `self.current` is left completely untouched -- the caller still
    /// has whatever was active before this call, which is the
    /// atomicity guarantee restated as "there is nothing else it could
    /// be," not merely "nothing else was observed."
    pub fn reload(&mut self) -> Result<Vec<ConfigWarning>, ConfigDiagnostic> {
        let source = match fs::read_to_string(&self.config_file) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.current = ConfigurationDocument::default();
                return Ok(Vec::new());
            }
            Err(_) => {
                return Err(ConfigDiagnostic {
                    path: None,
                    key: "<file>".to_owned(),
                    location: None,
                    message: "failed to read the configuration file",
                }
                .with_path(&self.config_file));
            }
        };
        let outcome =
            parse_and_validate(&source).map_err(|error| error.with_path(&self.config_file))?;
        self.current = outcome.document;
        Ok(outcome.warnings)
    }
}

fn load_or_default(config_file: &Path) -> (ConfigurationDocument, ConfigLoadReport) {
    let source = match fs::read_to_string(config_file) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                ConfigurationDocument::default(),
                ConfigLoadReport {
                    warnings: Vec::new(),
                    diagnostic: None,
                },
            );
        }
        Err(_) => {
            return (
                ConfigurationDocument::default(),
                ConfigLoadReport {
                    warnings: Vec::new(),
                    diagnostic: Some(
                        ConfigDiagnostic {
                            path: None,
                            key: "<file>".to_owned(),
                            location: None,
                            message: "failed to read the configuration file",
                        }
                        .with_path(config_file),
                    ),
                },
            );
        }
    };

    match parse_and_validate(&source) {
        Ok(outcome) => (
            outcome.document,
            ConfigLoadReport {
                warnings: outcome.warnings,
                diagnostic: None,
            },
        ),
        Err(diagnostic) => (
            ConfigurationDocument::default(),
            ConfigLoadReport {
                warnings: Vec::new(),
                diagnostic: Some(diagnostic.with_path(config_file)),
            },
        ),
    }
}
