use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{
    ConfigStore, ConfigurationDocument, RequiredDestructiveCommandApproval,
    RequiredMultilinePasteConfirmation, RestrictedDefaultTrust, parse_and_validate,
};

struct TestDir {
    base: PathBuf,
}

impl TestDir {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-config-load-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&base).unwrap();
        Self { base }
    }

    fn config_file(&self) -> PathBuf {
        self.base.join("config.toml")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn an_empty_document_produces_every_default_with_no_warnings() {
    let outcome = parse_and_validate("").unwrap();
    assert_eq!(outcome.document, ConfigurationDocument::default());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn a_partially_specified_document_defaults_every_other_field() {
    let outcome = parse_and_validate("[core]\nrecent_projects_limit = 99\n").unwrap();
    assert_eq!(outcome.document.core.recent_projects_limit, 99);
    assert!(outcome.document.core.default_project_board); // untouched, default
    assert_eq!(outcome.document.ui, ConfigurationDocument::default().ui);
}

#[test]
fn a_fully_specified_document_round_trips_every_section() {
    let source = r#"
[core]
default_project_board = false
recent_projects_limit = 5

[ui]
theme = "custom"
editor_font_family = "Fira Code"
terminal_font_family = "Fira Code"
ui_font_family = "system"
font_size = 16
show_status_labels = false

[keybindings]
toggle_mode = "ctrl+escape"

[terminal]
shell_path = "/bin/zsh"
scrollback_lines = 5000
multiline_paste_protection = true
safe_escape_sequences = false

[projects]
default_trust = "restricted"
restore_recent_projects = false
open_duplicate_root = "focus_existing"

[agent]
max_concurrent_global = 4
max_concurrent_per_project = 2
default_environment_policy = "explicit"
transcript_retention_days = 7
capture_changed_files = false

[agent.profile.codex]
command = "codex"
args = ["--project", "."]

[security]
restricted_mode_blocks_workspace_prompts = false
restricted_mode_blocks_workspace_lsp = false
restricted_mode_blocks_workspace_plugins = false
redact_secret_like_environment_names = false
require_approval_for_adapter_destructive_commands = true

[resources]
max_terminal_output_mb_per_session = 32
max_agent_transcript_mb_per_run = 64
max_file_watch_events_per_batch = 500
"#;

    let outcome = parse_and_validate(source).unwrap();
    assert!(outcome.warnings.is_empty(), "{:?}", outcome.warnings);

    let document = outcome.document;
    assert!(!document.core.default_project_board);
    assert_eq!(document.core.recent_projects_limit, 5);
    assert_eq!(document.ui.theme, "custom");
    assert_eq!(document.ui.font_size, 16);
    assert_eq!(
        document.keybindings.overrides.get("toggle_mode").unwrap(),
        "ctrl+escape"
    );
    assert_eq!(document.terminal.shell_path, "/bin/zsh");
    assert_eq!(document.terminal.scrollback_lines, 5000);
    assert_eq!(
        document.terminal.multiline_paste_protection,
        RequiredMultilinePasteConfirmation
    );
    assert_eq!(document.projects.default_trust, RestrictedDefaultTrust);
    assert!(!document.projects.restore_recent_projects);
    assert_eq!(document.agent.max_concurrent_global, 4);
    assert_eq!(document.agent.transcript_retention_days, 7);
    let profile = document.agent.profiles.get("codex").unwrap();
    assert_eq!(profile.command, "codex");
    assert_eq!(profile.args, vec!["--project".to_owned(), ".".to_owned()]);
    assert_eq!(profile.adapter, "terminal-native"); // defaulted, not specified above
    assert!(!document.security.restricted_mode_blocks_workspace_prompts);
    assert_eq!(
        document
            .security
            .require_approval_for_adapter_destructive_commands,
        RequiredDestructiveCommandApproval
    );
    assert_eq!(document.resources.max_terminal_output_mb_per_session, 32);
}

#[test]
fn an_unrecognized_top_level_section_warns_and_does_not_fail() {
    let outcome = parse_and_validate("[nonsense]\nvalue = 1\n").unwrap();
    assert_eq!(outcome.document, ConfigurationDocument::default());
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "nonsense");
}

#[test]
fn an_unrecognized_key_inside_a_known_section_warns_and_does_not_fail() {
    let outcome = parse_and_validate("[core]\nnonsense = 1\n").unwrap();
    assert_eq!(outcome.document, ConfigurationDocument::default());
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "core.nonsense");
}

#[test]
fn an_unrecognized_key_inside_a_profile_table_warns_and_does_not_fail() {
    let outcome =
        parse_and_validate("[agent.profile.codex]\ncommand = \"codex\"\nnonsense = 1\n").unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "agent.profile.codex.nonsense");
}

/// Response 268: an unknown key is text the *file* supplied, and
/// workspace configuration is untrusted by this RFC's own design.
/// A key longer than the bound must be truncated, not carried through
/// verbatim -- RFC-023 requires "a bounded diagnostic."
#[test]
fn an_overlong_unknown_key_is_truncated_in_the_warning() {
    let long_key = "a".repeat(500);
    let source = format!("[core]\n{long_key} = 1\n");
    let outcome = parse_and_validate(&source).unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    let warning_key = &outcome.warnings[0].key;
    assert!(
        warning_key.chars().count() < 500,
        "the raw 500-character key must not survive into the warning verbatim: {warning_key}"
    );
    assert!(warning_key.ends_with('\u{2026}'));
}

/// The regression response 269 exists to prevent: an earlier draft of
/// `bound_key_segment` replaced every non-ASCII character with `?`,
/// which would turn this key -- containing real Polish and Japanese
/// text, not a hostile character -- into an unreadable row of `?` and
/// defeat the diagnostic for exactly the users the i18n work exists to
/// serve. Legitimate non-Latin text must pass through unchanged.
#[test]
fn legitimate_non_latin_text_in_an_unknown_key_survives_unescaped() {
    let source = "[core]\n\"ustawienia_łąka_設定\" = 1\n";
    let outcome = parse_and_validate(source).unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "core.ustawienia_łąka_設定");
}

/// Response 269's ordering requirement, proven directly rather than by
/// inspection: truncate the *raw* segment to the 128-character cap
/// first, escape second. A bidi override placed right at the boundary
/// (127 safe characters, then the hostile one) must produce either the
/// **whole** `<U+202E>` marker or none of it at all -- never a mangled
/// fragment, which is what escaping first and truncating the expanded
/// result afterward would risk.
#[test]
fn a_hostile_character_at_the_truncation_boundary_is_never_split() {
    let safe_prefix = "a".repeat(127);
    let source = format!("[core]\n\"{safe_prefix}\u{202E}\" = 1\n");
    let outcome = parse_and_validate(&source).unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    let warning_key = &outcome.warnings[0].key;
    let after_prefix = warning_key
        .strip_prefix(&format!("core.{safe_prefix}"))
        .unwrap();
    assert!(
        after_prefix.is_empty() || after_prefix.starts_with("<U+202E>"),
        "the marker must appear whole or not at all, never a fragment: {warning_key:?}"
    );
}

/// The concrete threat response 268 names: a cloned repository's
/// `.tekstide/config.toml` can carry a key containing a bidi override
/// or control characters, shaped to mislead whatever eventually renders
/// this text. Neither may survive into the warning unescaped. Written
/// as a TOML quoted key with `\uXXXX` escapes -- a bare key cannot
/// contain either character, so a quoted key is the real shape this
/// attack would have to take.
///
/// Response 269: asserts the real `escape_untrusted_chars` marker
/// (`<U+202E>`/`<U+0007>`), not mere absence of the raw character -- the
/// earlier `?`-replacement draft would also have passed an
/// absence-only assertion, which is why this is stronger than the
/// version response 268 originally landed.
#[test]
fn a_bidi_override_or_control_character_in_an_unknown_key_is_neutralized() {
    let source = "[core]\n\"safe\\u202Eevil\\u0007bell\" = 1\n";
    let outcome = parse_and_validate(source).unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    let warning_key = &outcome.warnings[0].key;
    assert!(!warning_key.contains('\u{202E}'));
    assert!(!warning_key.contains('\u{0007}'));
    assert!(warning_key.contains("<U+202E>"));
    assert!(warning_key.contains("<U+0007>"));
    assert!(warning_key.contains("safe"));
    assert!(warning_key.contains("evil"));
    assert!(warning_key.contains("bell"));
}

/// The profile-table case, response 268's own `section` construction
/// fix: a hostile profile *name* -- the one thing in `[agent.profile.*]`
/// that is itself an untrusted TOML key, not a value -- must not survive
/// into any diagnostic or warning `key` built from it, even though the
/// same name is used unbounded for the profile's own real identity.
/// Written as a quoted dotted-table segment (`[agent.profile."..."]`),
/// the real TOML shape a name containing a bidi override would need.
#[test]
fn a_hostile_profile_name_is_bounded_in_diagnostics_but_not_in_the_stored_profile() {
    let hostile_name = format!("evil\u{202E}{}", "x".repeat(200));
    let source = format!("[agent.profile.\"{hostile_name}\"]\ncommand = \"x\"\nnonsense = 1\n");
    let outcome = parse_and_validate(&source).unwrap();

    assert_eq!(outcome.warnings.len(), 1);
    assert!(!outcome.warnings[0].key.contains('\u{202E}'));
    assert!(outcome.warnings[0].key.chars().count() < hostile_name.chars().count());

    // The real profile name is untouched -- PR-023-E validates it on
    // its own terms; bounding it here would corrupt data, not protect it.
    assert!(outcome.document.agent.profiles.contains_key(&hostile_name));
}

#[test]
fn an_unknown_value_for_a_known_key_is_an_error_naming_the_key() {
    let error =
        parse_and_validate("[core]\nrecent_projects_limit = \"not a number\"\n").unwrap_err();
    assert_eq!(error.key, "core.recent_projects_limit");
}

#[test]
fn a_missing_required_key_inside_a_profile_is_an_error() {
    let error = parse_and_validate("[agent.profile.codex]\nargs = []\n").unwrap_err();
    assert_eq!(error.key, "agent.profile.codex.command");
    assert_eq!(error.message, "this key is required");
}

#[test]
fn malformed_toml_syntax_is_a_parse_error_with_a_location_but_no_content() {
    let error = parse_and_validate("this is not valid = = toml [[[").unwrap_err();
    assert_eq!(error.key, "<toml>");
    assert!(error.location.is_some());
}

/// Response 266/267's carried-forward requirement, response 267's own
/// words: *"the outcome to avoid is silent acceptance ... a false
/// belief [that blanket trust was configured] in that direction is
/// exactly what this whole finding was about."* A file that says
/// `default_trust = "trusted"` must be an explicit, named error -- not
/// silently coerced to the safe default, which would leave a user
/// believing they configured something that was quietly ignored.
#[test]
fn default_trust_set_to_trusted_in_the_file_is_an_explicit_named_error() {
    let error = parse_and_validate("[projects]\ndefault_trust = \"trusted\"\n").unwrap_err();
    assert_eq!(error.key, "projects.default_trust");
    assert!(
        !format!("{error:?}").is_empty(),
        "sanity: the diagnostic itself must exist and be inspectable"
    );
}

#[test]
fn default_trust_set_to_restricted_in_the_file_is_accepted() {
    let outcome = parse_and_validate("[projects]\ndefault_trust = \"restricted\"\n").unwrap();
    assert_eq!(
        outcome.document.projects.default_trust,
        RestrictedDefaultTrust
    );
}

/// Response 270's same carried-forward requirement, applied to the two
/// settings the review flagged: `multiline_paste_protection = false`
/// and `require_approval_for_adapter_destructive_commands = false` must
/// both be explicit, named errors -- not silently coerced to the safe
/// default, which would leave a user believing they disabled a
/// protection that was quietly ignored.
#[test]
fn multiline_paste_protection_set_to_false_in_the_file_is_an_explicit_named_error() {
    let error = parse_and_validate("[terminal]\nmultiline_paste_protection = false\n").unwrap_err();
    assert_eq!(error.key, "terminal.multiline_paste_protection");
    assert_eq!(
        error.message,
        "must be true -- configuration cannot disable this protection"
    );
}

#[test]
fn multiline_paste_protection_set_to_true_in_the_file_is_accepted() {
    let outcome = parse_and_validate("[terminal]\nmultiline_paste_protection = true\n").unwrap();
    assert_eq!(
        outcome.document.terminal.multiline_paste_protection,
        RequiredMultilinePasteConfirmation
    );
}

#[test]
fn destructive_command_approval_set_to_false_in_the_file_is_an_explicit_named_error() {
    let error = parse_and_validate(
        "[security]\nrequire_approval_for_adapter_destructive_commands = false\n",
    )
    .unwrap_err();
    assert_eq!(
        error.key,
        "security.require_approval_for_adapter_destructive_commands"
    );
    assert_eq!(
        error.message,
        "must be true -- configuration cannot disable this protection"
    );
}

#[test]
fn destructive_command_approval_set_to_true_in_the_file_is_accepted() {
    let outcome = parse_and_validate(
        "[security]\nrequire_approval_for_adapter_destructive_commands = true\n",
    )
    .unwrap();
    assert_eq!(
        outcome
            .document
            .security
            .require_approval_for_adapter_destructive_commands,
        RequiredDestructiveCommandApproval
    );
}

/// The sentinel-privacy shape this pack's own gate requires elsewhere
/// in the project (modeled on RFC-012's sentinel test, per
/// `implementation-handoff.md` §8): a distinctive, secret-shaped string
/// placed in a rejected value must never reach the diagnostic. Because
/// `ConfigDiagnostic::message` is `&'static str`, this cannot fail by
/// construction -- the test still exists because the property is worth
/// naming and re-verifying rather than left to be true only by accident
/// of today's implementation.
#[test]
fn a_secret_shaped_rejected_value_never_reaches_the_diagnostic() {
    let sentinel = "sk-live-51ABCDEF0123456789zzYYYYxxxx";
    let source = format!("[core]\nrecent_projects_limit = \"{sentinel}\"\n");
    let error = parse_and_validate(&source).unwrap_err();
    assert!(!format!("{error:?}").contains(sentinel));
    assert!(!error.message.contains(sentinel));
    assert!(
        error
            .location
            .is_none_or(|location| !location.contains(sentinel))
    );
}

#[test]
fn store_load_with_no_file_present_yields_defaults_and_no_diagnostic() {
    let temp = TestDir::new("no-file");
    let (store, report) = ConfigStore::load(temp.config_file());
    assert_eq!(store.current(), &ConfigurationDocument::default());
    assert!(report.diagnostic.is_none());
    assert!(report.warnings.is_empty());
}

#[test]
fn store_load_with_an_invalid_file_at_first_start_yields_defaults_with_a_diagnostic() {
    let temp = TestDir::new("invalid-first-start");
    fs::write(
        temp.config_file(),
        "[core]\nrecent_projects_limit = \"bad\"\n",
    )
    .unwrap();

    let (store, report) = ConfigStore::load(temp.config_file());
    assert_eq!(
        store.current(),
        &ConfigurationDocument::default(),
        "an invalid file at first start must not prevent startup with working defaults"
    );
    let diagnostic = report.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic.key, "core.recent_projects_limit");
    assert_eq!(
        diagnostic.path.as_deref(),
        Some(temp.config_file().as_path())
    );
}

/// `parse_and_validate` alone has no path to give -- only [`ConfigStore`],
/// which actually resolved one, fills it in. Pinning this distinguishes
/// "this diagnostic came from the pure function" from "this diagnostic
/// was never given a path by its caller," which would otherwise look
/// identical.
#[test]
fn parse_and_validate_alone_leaves_the_diagnostics_path_unset() {
    let error = parse_and_validate("[core]\nrecent_projects_limit = \"bad\"\n").unwrap_err();
    assert_eq!(error.path, None);
}

#[test]
fn store_load_with_a_valid_file_uses_it() {
    let temp = TestDir::new("valid-first-start");
    fs::write(temp.config_file(), "[core]\nrecent_projects_limit = 42\n").unwrap();

    let (store, report) = ConfigStore::load(temp.config_file());
    assert_eq!(store.current().core.recent_projects_limit, 42);
    assert!(report.diagnostic.is_none());
}

/// The review's own planned test, verbatim: "a file that is valid in
/// its first half and invalid in its second, then assert nothing from
/// the first half took effect." This is the atomicity proof --
/// `ConfigStore::reload` must leave `current()` completely unchanged
/// when the new file fails validation, even though the failing key
/// comes *after* several keys that would otherwise have applied
/// cleanly.
#[test]
fn reload_with_a_file_valid_in_its_first_half_and_invalid_in_its_second_changes_nothing() {
    let temp = TestDir::new("atomicity");
    fs::write(temp.config_file(), "[core]\nrecent_projects_limit = 99\n").unwrap();
    let (mut store, report) = ConfigStore::load(temp.config_file());
    assert!(report.diagnostic.is_none());
    assert_eq!(store.current().core.recent_projects_limit, 99);

    fs::write(
        temp.config_file(),
        "[core]\nrecent_projects_limit = 5\n\n[ui]\nfont_size = \"not a number\"\n",
    )
    .unwrap();
    let error = store.reload().unwrap_err();
    assert_eq!(error.key, "ui.font_size");

    let mut expected = ConfigurationDocument::default();
    expected.core.recent_projects_limit = 99;
    assert_eq!(
        store.current(),
        &expected,
        "the failed reload must not have changed anything at all -- not just the one field \
         that would have come from its own section, but every other section too, since the \
         whole document is one atomic swap"
    );
}

#[test]
fn reload_when_the_file_disappears_falls_back_to_defaults() {
    let temp = TestDir::new("reload-disappears");
    fs::write(temp.config_file(), "[core]\nrecent_projects_limit = 7\n").unwrap();
    let (mut store, _) = ConfigStore::load(temp.config_file());
    assert_eq!(store.current().core.recent_projects_limit, 7);

    fs::remove_file(temp.config_file()).unwrap();
    let warnings = store.reload().unwrap();
    assert!(warnings.is_empty());
    assert_eq!(store.current(), &ConfigurationDocument::default());
}

/// `path` is the one field the diagnostic is *supposed* to carry --
/// RFC-023's own "diagnostics carry file path, error location where
/// available, and the offending key." What must never leak is content:
/// the message stays the fixed `&'static str` regardless of what the
/// file held, and there is no field anywhere the file's bytes could
/// reach.
#[cfg(unix)]
#[test]
fn reload_with_an_unreadable_file_is_an_error_naming_the_path_but_not_its_content() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TestDir::new("unreadable");
    fs::write(temp.config_file(), "[core]\nrecent_projects_limit = 3\n").unwrap();
    let (mut store, _) = ConfigStore::load(temp.config_file());
    assert_eq!(store.current().core.recent_projects_limit, 3);

    fs::set_permissions(temp.config_file(), fs::Permissions::from_mode(0o000)).unwrap();
    let result = store.reload();
    fs::set_permissions(temp.config_file(), fs::Permissions::from_mode(0o644)).unwrap();

    if let Err(error) = result {
        assert_eq!(error.key, "<file>");
        assert_eq!(error.path.as_deref(), Some(temp.config_file().as_path()));
        assert_eq!(error.message, "failed to read the configuration file");
        assert_eq!(
            store.current().core.recent_projects_limit,
            3,
            "an unreadable file on reload must not change the active configuration"
        );
    }
    // Running as root (some CI/dev containers) makes permissions unenforceable;
    // in that case the read succeeds and this test has nothing to assert.
}
