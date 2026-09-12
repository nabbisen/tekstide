use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::audit::AuditReference;
use crate::config::ConfigurationDocument;
use crate::config::model::{AgentSettings, ConfiguredAiCliProfile, ResourceSettings};
use crate::config::sensitive::{self, SecuritySensitiveField};

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
///
/// **RFC-045 D3' narrowed what reaches this type, and the distinction is
/// the point.** A key this parser has never heard of -- a typo, or a key
/// from a newer Tekstide -- still warns. A key RFC-023 really did define
/// and this build has no consumer for is a [`ConfigDiagnostic`] instead:
/// the user wrote something the schema documents, and telling them
/// "unknown key" would be false while telling them nothing at all is the
/// failure §1 of the risk document names. See [`WITHDRAWN_KEYS`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigWarning {
    pub key: String,
}

/// RFC-045 D3': the withdrawn keys **of the two sections that still
/// exist**. Each is refused by name rather than warned (it is real
/// schema, not a typo) and rather than parsed into a field nothing reads
/// (§1: *a key that does nothing is a lie the user reads*).
///
/// `(section, field)`. Only `agent` and `resources` appear, and the
/// reason is worth stating: a section that survives has live keys, so
/// this table is what separates a withdrawn key (refuse) from one this
/// build has never heard of (warn). The six sections that lost *every*
/// key -- `core`, `ui`, `keybindings`, `terminal`, `projects`,
/// `security` -- need no rows, because nothing in them is accepted at
/// all: [`refuse_withdrawn_free_form_section`] refuses every key they
/// contain, known or not. Listing them here as well would be data no
/// behaviour depends on, which a test cannot hold to account.
///
/// **Adding a consumer removes its row from this table in the same
/// change** -- RFC-036 D2's named-consumer rule. Do not remove a row to
/// make a file load.
const WITHDRAWN_KEYS: &[(&str, &str)] = &[
    ("agent", "max_concurrent_global"),
    ("agent", "max_concurrent_per_project"),
    ("agent", "default_environment_policy"),
    ("agent", "capture_changed_files"),
    ("resources", "max_terminal_output_mb_per_session"),
    ("resources", "max_agent_transcript_mb_per_run"),
    ("resources", "max_file_watch_events_per_batch"),
];

/// Keys refused for a reason that is **not** "no consumer yet," and
/// whose message must therefore not say so. All three sit in sections
/// that are otherwise entirely withdrawn, so
/// [`refuse_withdrawn_free_form_section`] would already refuse them --
/// what this table changes is *what the diagnostic says*, which is the
/// whole of the difference between a refusal a user can wait out and one
/// they cannot.
///
/// RFC-023 made each of these unrepresentable in memory -- a field type
/// with exactly one value -- after response 266/270 found that each would
/// bypass a deliberate per-use act another RFC requires
/// (`default_trust` bypasses RFC-032's trust grant; the other two bypass
/// confirmations that are unconditional in the real pipelines today).
/// D3' withdraws the *fields*, which removes the type-level guarantee
/// along with the field it guarded, leaving this refusal as the whole of
/// the protection. **It must keep saying why.** Telling a user that
/// `default_trust = "trusted"` "has no effect yet" would promise a
/// future version in which configuration grants workspace trust, which is
/// the opposite of a settled decision -- §1's own failure shape, pointed
/// at the reader of a security setting.
const PERMANENTLY_REFUSED_KEYS: &[(&str, &str, &str)] = &[
    (
        "projects",
        "default_trust",
        "configuration cannot grant workspace trust, at any value -- a project is trusted by a \
         deliberate per-project act; see RFC-032",
    ),
    (
        "terminal",
        "multiline_paste_protection",
        "configuration cannot disable this protection -- every multiline paste is confirmed in \
         the terminal itself; see RFC-018",
    ),
    (
        "security",
        "require_approval_for_adapter_destructive_commands",
        "configuration cannot disable this protection -- a destructive adapter command is \
         approved per command, in the moment; see RFC-021",
    ),
];

/// The message every ordinary withdrawn key carries. Deliberately says
/// **"no effect yet"** and names the return condition: the key is not
/// wrong, it is early, and it comes back with the code that reads it.
const NO_CONSUMER_MESSAGE: &str = "this key has no consumer in this build, so it would have no effect yet -- it returns to the \
     file in the same change as the feature that reads it";

fn permanently_refused_key_message(section: &str, field: &str) -> Option<&'static str> {
    PERMANENTLY_REFUSED_KEYS
        .iter()
        .find(|(refused_section, refused_field, _)| {
            *refused_section == section && *refused_field == field
        })
        .map(|(_, _, message)| *message)
}

fn is_withdrawn_key(section: &str, field: &str) -> bool {
    WITHDRAWN_KEYS
        .iter()
        .any(|(withdrawn_section, withdrawn_field)| {
            *withdrawn_section == section && *withdrawn_field == field
        })
}

/// Refuses any withdrawn key present in `table`, before the section's
/// own surviving keys are read. Called first in every section extractor
/// so a file mixing a live key with a withdrawn one is refused whole --
/// there is no order of keys in which half a section applies.
///
/// Only one diagnostic can be returned, so when a section carries both
/// kinds, **the permanent refusal is reported in preference to the
/// "no effect yet" one** -- deliberately, rather than by whichever key
/// happens to sort first. A user who asked for something this product
/// will never grant should read that, not a sentence about a different
/// key that is merely early.
fn refuse_withdrawn_keys(table: &toml::Table, section: &str) -> Result<(), ConfigDiagnostic> {
    let refuse = |field: &str, message: &'static str| ConfigDiagnostic {
        path: None,
        key: format!("{section}.{field}"),
        location: None,
        message,
    };
    for field in table.keys() {
        if let Some(message) = permanently_refused_key_message(section, field) {
            return Err(refuse(field, message));
        }
    }
    for field in table.keys() {
        if is_withdrawn_key(section, field) {
            return Err(refuse(field, NO_CONSUMER_MESSAGE));
        }
    }
    Ok(())
}

/// A section whose every key is withdrawn, including the free-form
/// `[keybindings]` map and the sections `ConfigurationDocument` no longer
/// models at all. An **empty** section header configures nothing and
/// says nothing false, so it passes: §1 is about a key that does
/// nothing, and there is no key here.
fn refuse_withdrawn_free_form_section(
    root: &mut toml::Table,
    section: &str,
) -> Result<(), ConfigDiagnostic> {
    let Some(table) = section_table(root, section)? else {
        return Ok(());
    };
    refuse_withdrawn_keys(&table, section)?;
    match table.keys().next() {
        None => Ok(()),
        Some(field) => Err(ConfigDiagnostic {
            path: None,
            key: format!("{section}.{}", bound_key_segment(field)),
            location: None,
            message: NO_CONSUMER_MESSAGE,
        }),
    }
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
///
/// **RFC-045 D3': this function accepts exactly the keys with a
/// consumer.** Four, plus `[agent.profile.<id>]`'s `display_name` and
/// `command`. Everything else RFC-023's schema defined is refused by
/// name -- see [`WITHDRAWN_KEYS`] for why that is a refusal and not a
/// warning, and [`PERMANENTLY_REFUSED_KEYS`] for the three whose refusal
/// is not waiting on anything.
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

    // RFC-045 D3': the six sections with nothing left in them are
    // refused before the two that survive are read. Only one diagnostic
    // is ever returned, so which problem a user is shown first is this
    // order, not the order they wrote their file in -- a fixed order is
    // the point (the same two documents always produce the same
    // diagnostic), but it is not "the first mistake in the file," and a
    // caller must not describe it as one.
    refuse_withdrawn_free_form_section(&mut root, "core")?;
    refuse_withdrawn_free_form_section(&mut root, "ui")?;
    refuse_withdrawn_free_form_section(&mut root, "keybindings")?;
    refuse_withdrawn_free_form_section(&mut root, "terminal")?;
    refuse_withdrawn_free_form_section(&mut root, "projects")?;
    refuse_withdrawn_free_form_section(&mut root, "security")?;

    let document = ConfigurationDocument {
        agent: extract_agent(&mut root, &mut warnings)?,
        resources: extract_resources(&mut root, &mut warnings)?,
    };

    validate_default_profile(&document)?;

    for key in root.keys() {
        warnings.push(ConfigWarning {
            key: bound_key_segment(key),
        });
    }

    Ok(ConfigLoadOutcome { document, warnings })
}

/// RFC-045 D8: `default_profile` naming an id the file does not define
/// is a diagnostic, not a silent fallback to the built-in profile. A
/// user who misspells their own profile's id would otherwise get
/// `claude_code_linux_default()` and no indication that the profile they
/// configured is not the one that launched.
fn validate_default_profile(document: &ConfigurationDocument) -> Result<(), ConfigDiagnostic> {
    let Some(default_profile) = &document.agent.default_profile else {
        return Ok(());
    };
    if document.agent.profiles.contains_key(default_profile) {
        return Ok(());
    }
    Err(ConfigDiagnostic {
        path: None,
        key: "agent.default_profile".to_owned(),
        location: None,
        message: "names a profile this file does not define",
    })
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

fn extract_agent(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<AgentSettings, ConfigDiagnostic> {
    let defaults = AgentSettings::default();
    let Some(mut table) = section_table(root, "agent")? else {
        return Ok(defaults);
    };
    refuse_withdrawn_keys(&table, "agent")?;
    let profiles = extract_agent_profiles(&mut table, warnings)?;
    let settings = AgentSettings {
        default_profile: match table.remove("default_profile") {
            None => None,
            Some(toml::Value::String(value)) => Some(value),
            Some(_) => {
                return Err(ConfigDiagnostic {
                    path: None,
                    key: "agent.default_profile".to_owned(),
                    location: None,
                    message: "expected a string",
                });
            }
        },
        transcript_retention_days: take_u32(
            &mut table,
            "agent",
            "transcript_retention_days",
            defaults.transcript_retention_days,
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

        // RFC-045 D2: the id is validated **by calling
        // `AuditReference::new`**, never by a character class copied
        // from it. RFC-046's `plan_is_auditable` routes a launch whose
        // profile id this function rejects to the *unaudited* fallback
        // rather than crashing -- correct for a launch path, and the
        // wrong outcome for a definition: a user who writes
        // `[agent.profile."my tool"]` would get an executable every
        // launch of which is silently unrecorded. If the two checks ever
        // disagreed, this one would be wrong in the direction of
        // accepting something the trail will not hold, so there is only
        // one check.
        if AuditReference::new(name.as_str()).is_none() {
            return Err(ConfigDiagnostic {
                path: None,
                key: section,
                location: None,
                message: "profile ids must be recordable in the audit trail: 1 to 128 characters \
                          from A-Z, a-z, 0-9, and -_.: only",
            });
        }

        refuse_withdrawn_profile_keys(&profile_table, &section)?;
        let display_name = take_string(&mut profile_table, &section, "display_name", &name)?;
        let command = require_string(&mut profile_table, &section, "command")?;

        warn_unconsumed(profile_table, &section, warnings);

        profiles.insert(
            name,
            ConfiguredAiCliProfile {
                display_name,
                command,
            },
        );
    }
    Ok(profiles)
}

/// RFC-045 D3' inside a `[agent.profile.<id>]` table. `args` is §1's own
/// opening example -- the key RFC-023 parsed, `to_ai_cli_profile`
/// dropped, and nothing reported. `adapter` and `environment_policy` are
/// the same shape. Each returns with the field on `AiCliProfile` that
/// would carry it (an argv template is an RFC-010 amendment, reserved).
///
/// Not in [`WITHDRAWN_KEYS`] because a profile's section name contains
/// the user's own profile id, so these three cannot be matched by a
/// fixed `(section, field)` pair.
const WITHDRAWN_PROFILE_KEYS: &[&str] = &["args", "adapter", "environment_policy"];

fn refuse_withdrawn_profile_keys(
    profile_table: &toml::Table,
    section: &str,
) -> Result<(), ConfigDiagnostic> {
    for field in profile_table.keys() {
        if WITHDRAWN_PROFILE_KEYS.contains(&field.as_str()) {
            return Err(ConfigDiagnostic {
                path: None,
                key: format!("{section}.{field}"),
                location: None,
                message: NO_CONSUMER_MESSAGE,
            });
        }
    }
    Ok(())
}

/// RFC-045 D6. **Absent and zero are different answers, so this cannot
/// be `take_u32` with a default**: absent means *unlimited*, matching
/// `ProjectResourceLimits`' own `Option<u32>`, while `0` is refused --
/// a limit of zero refuses every launch, and a file that quietly turned
/// the launch button off would be indistinguishable from one that never
/// set a limit at all. Far more likely a typo than an intent.
fn take_agent_run_limit(table: &mut toml::Table) -> Result<Option<u32>, ConfigDiagnostic> {
    let limit = match table.remove("agent_run_limit") {
        None => return Ok(None),
        Some(toml::Value::Integer(value)) => {
            u32::try_from(value).map_err(|_| ConfigDiagnostic {
                path: None,
                key: "resources.agent_run_limit".to_owned(),
                location: None,
                message: "expected an integer between 1 and 4294967295",
            })?
        }
        Some(_) => {
            return Err(ConfigDiagnostic {
                path: None,
                key: "resources.agent_run_limit".to_owned(),
                location: None,
                message: "expected an integer",
            });
        }
    };
    if limit == 0 {
        return Err(ConfigDiagnostic {
            path: None,
            key: "resources.agent_run_limit".to_owned(),
            location: None,
            message: "a limit of zero would refuse every agent run -- omit the key entirely for \
                      no limit",
        });
    }
    Ok(Some(limit))
}

fn extract_resources(
    root: &mut toml::Table,
    warnings: &mut Vec<ConfigWarning>,
) -> Result<ResourceSettings, ConfigDiagnostic> {
    let defaults = ResourceSettings::default();
    let Some(mut table) = section_table(root, "resources")? else {
        return Ok(defaults);
    };
    refuse_withdrawn_keys(&table, "resources")?;
    let settings = ResourceSettings {
        agent_run_limit: take_agent_run_limit(&mut table)?,
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

/// PR-023-D: what a successful `reload` produced. `pending_security_sensitive_changes`
/// names every security-sensitive field the new file asked to change --
/// none of them took effect; `ConfigStore::current()` still reflects
/// their old values. Actually confirming and applying one is not built
/// yet (no confirmation surface exists to drive it -- M12 UI work, per
/// this RFC's own Non-Goals); this type exists so the *fact* that a
/// change is pending is observable and testable now, rather than
/// silently dropped on the floor with nothing to show it was ever
/// requested.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigReloadOutcome {
    pub warnings: Vec<ConfigWarning>,
    pub pending_security_sensitive_changes: Vec<SecuritySensitiveField>,
    /// RFC-045 PR-045-C: the freshly parsed document, **including the
    /// sensitive values that were held back**. `ConfigStore::current()`
    /// does not have them; this is the only place they exist, and
    /// [`ConfigStore::apply_security_sensitive_field`] is what releases
    /// one once its own deliberate act has happened.
    ///
    /// Carried on the outcome rather than retained inside the store so
    /// that "which parse is this value from" stays a visible property of
    /// the caller's own code — a retained candidate would go stale on
    /// the next reload with nothing in the type system to say so.
    pub candidate: ConfigurationDocument,
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
    pub fn reload(&mut self) -> Result<ConfigReloadOutcome, ConfigDiagnostic> {
        let source = match fs::read_to_string(&self.config_file) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A missing file is not "the file changed to say
                // something different" -- it is "no user configuration
                // exists," the same unambiguous case the initial `load`
                // handles by resetting to compiled defaults outright,
                // not by holding anything pending.
                self.current = ConfigurationDocument::default();
                return Ok(ConfigReloadOutcome {
                    warnings: Vec::new(),
                    pending_security_sensitive_changes: Vec::new(),
                    // Nothing is held back, so the candidate *is* what
                    // took effect -- there is no second document here
                    // the way there is on a real parse.
                    candidate: ConfigurationDocument::default(),
                });
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
        let pending = sensitive::security_sensitive_diff(&self.current, &outcome.document);
        self.current = sensitive::apply_safe_fields(&self.current, &outcome.document);
        Ok(ConfigReloadOutcome {
            warnings: outcome.warnings,
            pending_security_sensitive_changes: pending,
            candidate: outcome.document,
        })
    }

    /// RFC-045 PR-045-C: apply **one** held-back security-sensitive
    /// field, once its own deliberate act has happened.
    ///
    /// `reload` holds every sensitive change back because RFC-023 had no
    /// confirmation surface to release one from; D4 builds that surface,
    /// and this is the only way it can say *this field, now*. One field
    /// per call, never "apply the candidate": a reload can hold two
    /// changes whose directions differ, and RFC-023's asymmetry means
    /// they are released by different acts (a reduce applies directly, an
    /// increase waits for a confirmation) — a whole-document swap would
    /// silently carry the unconfirmed one along with the confirmed one.
    ///
    /// **The caller passes back the `candidate` its own reload
    /// returned**, rather than this type retaining it. A retained
    /// candidate is a second source of truth that goes stale the moment
    /// anything else reloads, and nothing in the type system would say
    /// so; passing it back makes "which parse is this from" the caller's
    /// visible responsibility.
    pub fn apply_security_sensitive_field(
        &mut self,
        field: SecuritySensitiveField,
        candidate: &ConfigurationDocument,
    ) {
        match field {
            SecuritySensitiveField::AgentTranscriptRetentionDays => {
                self.current.agent.transcript_retention_days =
                    candidate.agent.transcript_retention_days;
            }
            SecuritySensitiveField::AgentProfiles => {
                self.current.agent.profiles = candidate.agent.profiles.clone();
            }
        }
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
