use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{
    ConfigStore, ConfigurationDocument, SecuritySensitiveField, parse_and_validate,
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

/// RFC-045 D3': the whole accepted key set, in one file, parsed into the
/// values it names. This is the positive half of the parser's promise --
/// every key below has a consumer, which is the entire admission
/// criterion, so this test is also the list a future RFC adds its own
/// key to when it adds the code that reads it.
#[test]
fn the_whole_accepted_key_set_parses() {
    let source = r#"
[agent]
default_profile = "codex"
transcript_retention_days = 7

[agent.profile.codex]
display_name = "Codex"
command = "/usr/local/bin/codex"

[resources]
agent_run_limit = 3
"#;
    let outcome = parse_and_validate(source).unwrap();
    assert!(outcome.warnings.is_empty());

    assert_eq!(
        outcome.document.agent.default_profile.as_deref(),
        Some("codex")
    );
    assert_eq!(outcome.document.agent.transcript_retention_days, 7);
    assert_eq!(outcome.document.resources.agent_run_limit, Some(3));

    let profile = &outcome.document.agent.profiles["codex"];
    assert_eq!(profile.display_name, "Codex");
    assert_eq!(profile.command, "/usr/local/bin/codex");
}

/// A profile's `display_name` still defaults to its own id, so the
/// minimum a definable profile needs is a `command`.
#[test]
fn a_profile_display_name_defaults_to_its_id() {
    let outcome = parse_and_validate("[agent.profile.codex]\ncommand = \"codex\"\n").unwrap();
    assert_eq!(
        outcome.document.agent.profiles["codex"].display_name,
        "codex"
    );
}

// --- RFC-045 D3': one refusal test per withdrawn section ---------------
//
// Each of the six sections below lost every one of its keys, and each
// test names one key from its own section, so removing one row from
// `WITHDRAWN_KEYS` fails exactly one of these -- not a shared table test
// where any regression fails the same assertion under another section's
// name.

#[test]
fn a_core_key_is_refused_because_nothing_reads_it() {
    let error = parse_and_validate("[core]\nrecent_projects_limit = 99\n").unwrap_err();
    assert_eq!(error.key, "core.recent_projects_limit");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_ui_key_is_refused_because_nothing_reads_it() {
    let error = parse_and_validate("[ui]\nfont_size = 18\n").unwrap_err();
    assert_eq!(error.key, "ui.font_size");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_keybinding_override_is_refused_because_nothing_reads_it() {
    let error = parse_and_validate("[keybindings]\nopen_help = \"ctrl+alt+h\"\n").unwrap_err();
    assert_eq!(error.key, "keybindings.open_help");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_terminal_key_is_refused_because_nothing_reads_it() {
    let error = parse_and_validate("[terminal]\nscrollback_lines = 50000\n").unwrap_err();
    assert_eq!(error.key, "terminal.scrollback_lines");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_projects_key_is_refused_because_nothing_reads_it() {
    let error = parse_and_validate("[projects]\nrestore_recent_projects = false\n").unwrap_err();
    assert_eq!(error.key, "projects.restore_recent_projects");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_security_key_is_refused_because_nothing_reads_it() {
    let error = parse_and_validate("[security]\nrestricted_mode_blocks_workspace_lsp = false\n")
        .unwrap_err();
    assert_eq!(error.key, "security.restricted_mode_blocks_workspace_lsp");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_withdrawn_agent_key_is_refused_because_nothing_reads_it() {
    let error = parse_and_validate("[agent]\ncapture_changed_files = false\n").unwrap_err();
    assert_eq!(error.key, "agent.capture_changed_files");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_withdrawn_resources_key_is_refused_because_nothing_reads_it() {
    let error =
        parse_and_validate("[resources]\nmax_file_watch_events_per_batch = 10\n").unwrap_err();
    assert_eq!(error.key, "resources.max_file_watch_events_per_batch");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

/// §1's own opening example, now a test: RFC-023 parsed `args`,
/// `to_ai_cli_profile` dropped it, and a user who wrote
/// `args = ["--model", "x"]` got no `--model` and no word about it.
#[test]
fn a_profile_args_key_is_refused_rather_than_parsed_and_dropped() {
    let error = parse_and_validate(
        "[agent.profile.codex]\ncommand = \"codex\"\nargs = [\"--model\", \"x\"]\n",
    )
    .unwrap_err();
    assert_eq!(error.key, "agent.profile.codex.args");
    assert!(error.message.contains("no effect yet"), "{}", error.message);
}

#[test]
fn a_profile_adapter_key_is_refused_rather_than_parsed_and_dropped() {
    let error = parse_and_validate("[agent.profile.codex]\ncommand = \"codex\"\nadapter = \"x\"\n")
        .unwrap_err();
    assert_eq!(error.key, "agent.profile.codex.adapter");
}

#[test]
fn a_profile_environment_policy_key_is_refused_rather_than_parsed_and_dropped() {
    let error = parse_and_validate(
        "[agent.profile.codex]\ncommand = \"codex\"\nenvironment_policy = \"inherit\"\n",
    )
    .unwrap_err();
    assert_eq!(error.key, "agent.profile.codex.environment_policy");
}

/// A live key and a withdrawn one in the same section: the file is
/// refused whole. There is no ordering of keys in which half a section
/// applies -- the same "no partial application" property
/// `parse_and_validate` already holds for whole sections, checked at
/// key granularity because D3' made per-key refusal possible for the
/// first time.
#[test]
fn a_withdrawn_key_refuses_the_file_even_beside_an_accepted_one() {
    let error =
        parse_and_validate("[agent]\ntranscript_retention_days = 7\nmax_concurrent_global = 4\n")
            .unwrap_err();
    assert_eq!(error.key, "agent.max_concurrent_global");
}

/// An empty section header asks for nothing, so it is not a lie: §1 is
/// about *a key* that does nothing, and there is no key here. Refusing
/// this would fail a file that is merely untidy.
#[test]
fn an_empty_withdrawn_section_header_is_harmless() {
    let outcome = parse_and_validate("[ui]\n[core]\n").unwrap();
    assert_eq!(outcome.document, ConfigurationDocument::default());
}

// --- RFC-045 D3', the other half: what still warns ----------------------

/// Forward compatibility survives D3' for keys this build has genuinely
/// never heard of. The distinction is load-bearing for PR-045-B's board
/// line, which has a *loaded with warnings* state to render, and for a
/// user of a newer Tekstide's file: an unknown key is a guess about the
/// future, a withdrawn key is a statement about this build.
#[test]
fn an_unrecognized_top_level_section_warns_and_does_not_fail() {
    let outcome = parse_and_validate("[nonsense]\nvalue = 1\n").unwrap();
    assert_eq!(outcome.document, ConfigurationDocument::default());
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "nonsense");
}

#[test]
fn an_unrecognized_key_inside_a_surviving_section_warns_and_does_not_fail() {
    let outcome = parse_and_validate("[agent]\nnonsense = 1\n").unwrap();
    assert_eq!(outcome.document, ConfigurationDocument::default());
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "agent.nonsense");
}

/// The other side of the same distinction: inside a section whose every
/// key was withdrawn, there is nothing to be forward-compatible *with* --
/// the section itself has no consumer -- so an unrecognized key there is
/// refused rather than warned, and the user learns `[core]` does nothing
/// instead of being told only that one key is unknown.
#[test]
fn an_unrecognized_key_inside_a_withdrawn_section_is_refused_not_warned() {
    let error = parse_and_validate("[core]\nnonsense = 1\n").unwrap_err();
    assert_eq!(error.key, "core.nonsense");
}

#[test]
fn an_unrecognized_key_inside_a_profile_table_warns_and_does_not_fail() {
    let outcome =
        parse_and_validate("[agent.profile.codex]\ncommand = \"codex\"\nnonsense = 1\n").unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "agent.profile.codex.nonsense");
}

// --- RFC-045 D2: the id must be recordable -----------------------------

/// D2, and the reason it is a *parse* error rather than RFC-046's
/// unaudited fallback: a launch must never panic on valid data, but a
/// definition the audit trail cannot hold is an executable whose every
/// launch would be silently unrecorded.
#[test]
fn a_profile_id_the_audit_trail_cannot_record_is_refused() {
    let error =
        parse_and_validate("[agent.profile.\"my tool\"]\ncommand = \"tool\"\n").unwrap_err();
    assert_eq!(error.key, "agent.profile.my tool");
    assert!(
        error.message.contains("audit trail"),
        "the diagnostic must say why the id is refused, not merely that it is: {}",
        error.message
    );
}

/// The bound is `AuditReference::new`'s own, so this checks the two
/// edges that a hand-written character class would be most likely to get
/// wrong: an empty id, and one past the 128-byte cap.
#[test]
fn a_profile_id_that_is_empty_or_overlong_is_refused_on_the_same_grounds() {
    let empty = parse_and_validate("[agent.profile.\"\"]\ncommand = \"tool\"\n").unwrap_err();
    assert!(empty.message.contains("audit trail"), "{}", empty.message);

    let overlong_id = "a".repeat(129);
    let overlong = parse_and_validate(&format!("[agent.profile.{overlong_id}]\ncommand = \"t\"\n"))
        .unwrap_err();
    assert!(
        overlong.message.contains("audit trail"),
        "{}",
        overlong.message
    );
}

/// The id charset is exactly what the audit trail accepts -- proven
/// against the real function rather than against this test's idea of it,
/// so the two cannot drift.
#[test]
fn every_character_the_audit_trail_accepts_is_accepted_in_a_profile_id() {
    let id = "Tool-9_x.y:z";
    assert!(
        crate::audit::AuditReference::new(id).is_some(),
        "precondition: the audit trail accepts this id"
    );
    let outcome =
        parse_and_validate(&format!("[agent.profile.{id:?}]\ncommand = \"t\"\n")).unwrap();
    assert!(outcome.document.agent.profiles.contains_key(id));
}

// --- RFC-045 D8 and D6 -------------------------------------------------

#[test]
fn a_default_profile_naming_an_undefined_profile_is_refused() {
    let error = parse_and_validate(
        "[agent]\ndefault_profile = \"nope\"\n\n[agent.profile.codex]\ncommand = \"codex\"\n",
    )
    .unwrap_err();
    assert_eq!(error.key, "agent.default_profile");
}

#[test]
fn a_default_profile_with_no_profiles_defined_at_all_is_refused() {
    let error = parse_and_validate("[agent]\ndefault_profile = \"codex\"\n").unwrap_err();
    assert_eq!(error.key, "agent.default_profile");
}

/// D6: `0` and absent are different answers, and only one of them is
/// allowed. A limit of zero refuses every launch, which is far more
/// likely a typo than an intent -- and silently applying it would turn
/// the launch button off with nothing to explain why.
#[test]
fn an_agent_run_limit_of_zero_is_refused() {
    let error = parse_and_validate("[resources]\nagent_run_limit = 0\n").unwrap_err();
    assert_eq!(error.key, "resources.agent_run_limit");
}

#[test]
fn an_absent_agent_run_limit_means_unlimited_not_zero() {
    let outcome = parse_and_validate("[resources]\n").unwrap();
    assert_eq!(outcome.document.resources.agent_run_limit, None);
}

// --- Bounding and escaping, unchanged in substance ---------------------

/// Response 268: an unknown key is text the *file* supplied, and
/// workspace configuration is untrusted by this RFC's own design.
/// A key longer than the bound must be truncated, not carried through
/// verbatim -- RFC-023 requires "a bounded diagnostic."
#[test]
fn an_overlong_unknown_key_is_truncated_in_the_warning() {
    let long_key = "a".repeat(500);
    let source = format!("[agent]\n{long_key} = 1\n");
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
    let source = "[agent]\n\"ustawienia_łąka_設定\" = 1\n";
    let outcome = parse_and_validate(source).unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.warnings[0].key, "agent.ustawienia_łąka_設定");
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
    let source = format!("[agent]\n\"{safe_prefix}\u{202E}\" = 1\n");
    let outcome = parse_and_validate(&source).unwrap();
    assert_eq!(outcome.warnings.len(), 1);
    let warning_key = &outcome.warnings[0].key;
    let after_prefix = warning_key
        .strip_prefix(&format!("agent.{safe_prefix}"))
        .unwrap();
    assert!(
        after_prefix.is_empty() || after_prefix.starts_with("<U+202E>"),
        "the marker must appear whole or not at all, never a fragment: {warning_key:?}"
    );
}

/// The concrete threat response 268 names: a cloned repository's
/// `.tekstide/config.toml` can carry a key containing a bidi override
/// or control characters, shaped to mislead whatever eventually renders
/// this text. Neither may survive into the warning unescaped.
///
/// Response 269: asserts the real `escape_untrusted_chars` marker
/// (`<U+202E>`/`<U+0007>`), not mere absence of the raw character -- the
/// earlier `?`-replacement draft would also have passed an
/// absence-only assertion.
#[test]
fn a_bidi_override_or_control_character_in_an_unknown_key_is_neutralized() {
    let source = "[agent]\n\"safe\\u202Eevil\\u0007bell\" = 1\n";
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
/// fix. **RFC-045 D2 changed what this test can show**: a name carrying
/// a bidi override is no longer storable at all, since
/// `AuditReference::new` rejects it -- so the property now is that the
/// *diagnostic refusing it* is itself bounded and escaped, which is the
/// half that still renders on a user's screen.
#[test]
fn a_hostile_profile_name_is_bounded_and_escaped_in_the_diagnostic_that_refuses_it() {
    let hostile_name = format!("evil\u{202E}{}", "x".repeat(200));
    let source = format!("[agent.profile.\"{hostile_name}\"]\ncommand = \"x\"\n");
    let error = parse_and_validate(&source).unwrap_err();

    assert!(!error.key.contains('\u{202E}'));
    assert!(error.key.contains("<U+202E>"));
    assert!(error.key.chars().count() < hostile_name.chars().count());
    assert!(error.message.contains("audit trail"));
}

#[test]
fn an_unknown_value_type_for_an_accepted_key_is_an_error_naming_the_key() {
    let error =
        parse_and_validate("[agent]\ntranscript_retention_days = \"not a number\"\n").unwrap_err();
    assert_eq!(error.key, "agent.transcript_retention_days");
}

#[test]
fn a_missing_required_key_inside_a_profile_is_an_error() {
    let error =
        parse_and_validate("[agent.profile.codex]\ndisplay_name = \"Codex\"\n").unwrap_err();
    assert_eq!(error.key, "agent.profile.codex.command");
    assert_eq!(error.message, "this key is required");
}

#[test]
fn malformed_toml_syntax_is_a_parse_error_with_a_location_but_no_content() {
    let error = parse_and_validate("this is not valid = = toml [[[").unwrap_err();
    assert_eq!(error.key, "<toml>");
    assert!(error.location.is_some());
}

/// The sentinel-privacy shape this pack's own gate requires elsewhere
/// in the project (modeled on RFC-012's sentinel test): a distinctive,
/// secret-shaped string placed in a rejected value must never reach the
/// diagnostic. Because `ConfigDiagnostic::message` is `&'static str`,
/// this cannot fail by construction -- the test still exists because the
/// property is worth naming and re-verifying rather than left to be true
/// only by accident of today's implementation.
#[test]
fn a_secret_shaped_rejected_value_never_reaches_the_diagnostic() {
    let sentinel = "sk-live-51ABCDEF0123456789zzYYYYxxxx";
    let source = format!("[agent]\ntranscript_retention_days = \"{sentinel}\"\n");
    let error = parse_and_validate(&source).unwrap_err();
    assert!(!format!("{error:?}").contains(sentinel));
    assert!(!error.message.contains(sentinel));
    assert!(
        error
            .location
            .is_none_or(|location| !location.contains(sentinel))
    );
}

// --- ConfigStore, unchanged in substance -------------------------------

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
        "[agent]\ntranscript_retention_days = \"bad\"\n",
    )
    .unwrap();

    let (store, report) = ConfigStore::load(temp.config_file());
    assert_eq!(
        store.current(),
        &ConfigurationDocument::default(),
        "an invalid file at first start must not prevent startup with working defaults"
    );
    let diagnostic = report.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic.key, "agent.transcript_retention_days");
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
    let error = parse_and_validate("[agent]\ntranscript_retention_days = \"bad\"\n").unwrap_err();
    assert_eq!(error.path, None);
}

#[test]
fn store_load_with_a_valid_file_uses_it() {
    let temp = TestDir::new("valid-first-start");
    fs::write(
        temp.config_file(),
        "[agent]\ntranscript_retention_days = 42\n",
    )
    .unwrap();

    let (store, report) = ConfigStore::load(temp.config_file());
    assert_eq!(store.current().agent.transcript_retention_days, 42);
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
    fs::write(temp.config_file(), "[resources]\nagent_run_limit = 99\n").unwrap();
    let (mut store, report) = ConfigStore::load(temp.config_file());
    assert!(report.diagnostic.is_none());
    assert_eq!(store.current().resources.agent_run_limit, Some(99));

    fs::write(
        temp.config_file(),
        "[resources]\nagent_run_limit = 5\n\n[agent]\ntranscript_retention_days = \"not a number\"\n",
    )
    .unwrap();
    let error = store.reload().unwrap_err();
    assert_eq!(error.key, "agent.transcript_retention_days");

    let mut expected = ConfigurationDocument::default();
    expected.resources.agent_run_limit = Some(99);
    assert_eq!(
        store.current(),
        &expected,
        "the failed reload must not have changed anything at all -- not just the one field \
         that would have come from its own section, but every other section too, since the \
         whole document is one atomic swap"
    );
}

/// PR-023-D's own required property, end to end through the real
/// `ConfigStore::reload` a caller actually uses. One reload changes a
/// safe field and a security-sensitive one at once; the safe field must
/// take effect, the security-sensitive one must not, and the outcome
/// must name it as pending.
///
/// **RFC-045 narrowed which fields are which**, so the fixture moved:
/// `agent_run_limit` is the safe field, `transcript_retention_days` the
/// security-sensitive one. The property under test is unchanged.
#[test]
fn reload_applies_a_safe_change_but_holds_a_security_sensitive_one_pending() {
    let temp = TestDir::new("reload-gating");
    fs::write(
        temp.config_file(),
        "[resources]\nagent_run_limit = 10\n\n[agent]\ntranscript_retention_days = 30\n",
    )
    .unwrap();
    let (mut store, report) = ConfigStore::load(temp.config_file());
    assert!(report.diagnostic.is_none());
    assert_eq!(store.current().resources.agent_run_limit, Some(10));
    assert_eq!(store.current().agent.transcript_retention_days, 30);

    fs::write(
        temp.config_file(),
        "[resources]\nagent_run_limit = 20\n\n[agent]\ntranscript_retention_days = 365\n",
    )
    .unwrap();
    let outcome = store.reload().unwrap();

    assert_eq!(
        store.current().resources.agent_run_limit,
        Some(20),
        "the safe field must apply"
    );
    assert_eq!(
        store.current().agent.transcript_retention_days,
        30,
        "the security-sensitive field must NOT apply -- it must still read the old value"
    );
    assert_eq!(
        outcome.pending_security_sensitive_changes,
        vec![SecuritySensitiveField::AgentTranscriptRetentionDays]
    );
}

#[test]
fn reload_when_the_file_disappears_falls_back_to_defaults() {
    let temp = TestDir::new("reload-disappears");
    fs::write(temp.config_file(), "[resources]\nagent_run_limit = 7\n").unwrap();
    let (mut store, _) = ConfigStore::load(temp.config_file());
    assert_eq!(store.current().resources.agent_run_limit, Some(7));

    fs::remove_file(temp.config_file()).unwrap();
    let outcome = store.reload().unwrap();
    assert!(outcome.warnings.is_empty());
    assert!(outcome.pending_security_sensitive_changes.is_empty());
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
    fs::write(temp.config_file(), "[resources]\nagent_run_limit = 3\n").unwrap();
    let (mut store, _) = ConfigStore::load(temp.config_file());
    assert_eq!(store.current().resources.agent_run_limit, Some(3));

    fs::set_permissions(temp.config_file(), fs::Permissions::from_mode(0o000)).unwrap();
    let result = store.reload();
    fs::set_permissions(temp.config_file(), fs::Permissions::from_mode(0o644)).unwrap();

    if let Err(error) = result {
        assert_eq!(error.key, "<file>");
        assert_eq!(error.path.as_deref(), Some(temp.config_file().as_path()));
        assert_eq!(error.message, "failed to read the configuration file");
        assert_eq!(
            store.current().resources.agent_run_limit,
            Some(3),
            "an unreadable file on reload must not change the active configuration"
        );
    }
    // Running as root (some CI/dev containers) makes permissions unenforceable;
    // in that case the read succeeds and this test has nothing to assert.
}

/// RFC-045 PR-045-C: `reload` holds every sensitive change back, and
/// this is the only way one is released. Proves the two halves that
/// matter: the candidate really does carry the held-back value (it is
/// nowhere else), and applying one field leaves the other alone — the
/// property that makes RFC-023's own asymmetry expressible, since a
/// reload can hold an increase and a reduce at the same time and they
/// are released by different acts.
#[test]
fn a_confirmed_security_sensitive_field_can_be_applied_one_at_a_time() {
    let temp = TestDir::new("apply-pending");
    fs::write(
        temp.config_file(),
        "[agent]\ntranscript_retention_days = 30\n",
    )
    .unwrap();
    let (mut store, _) = ConfigStore::load(temp.config_file());

    fs::write(
        temp.config_file(),
        "[agent]\ntranscript_retention_days = 365\n\n\
         [agent.profile.codex]\ncommand = \"codex\"\n",
    )
    .unwrap();
    let outcome = store.reload().unwrap();
    assert_eq!(
        outcome.pending_security_sensitive_changes,
        vec![
            SecuritySensitiveField::AgentTranscriptRetentionDays,
            SecuritySensitiveField::AgentProfiles,
        ]
    );
    assert_eq!(
        store.current().agent.transcript_retention_days,
        30,
        "nothing is applied by the reload itself"
    );
    assert_eq!(
        outcome.candidate.agent.transcript_retention_days, 365,
        "the held-back value exists only on the candidate"
    );

    store.apply_security_sensitive_field(
        SecuritySensitiveField::AgentTranscriptRetentionDays,
        &outcome.candidate,
    );
    assert_eq!(store.current().agent.transcript_retention_days, 365);
    assert!(
        store.current().agent.profiles.is_empty(),
        "applying one field must not carry the other, unconfirmed one along with it"
    );

    store.apply_security_sensitive_field(SecuritySensitiveField::AgentProfiles, &outcome.candidate);
    assert!(store.current().agent.profiles.contains_key("codex"));
}
