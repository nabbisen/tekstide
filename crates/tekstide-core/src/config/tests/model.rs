use crate::config::ConfigurationDocument;
use crate::transcript::DEFAULT_TRANSCRIPT_MAX_AGE_DAYS;

/// Model Checklist: "Compiled defaults are total; no `Option` handling
/// downstream." **RFC-045 introduced the first two `Option`s in this
/// type, and both mean something a total value could not say**:
/// `default_profile: None` is "launch the built-in profile" (D8) and
/// `agent_run_limit: None` is "unlimited" (D6), matching
/// `ProjectResourceLimits`' own `Option<u32>` rather than picking a
/// sentinel number that a user could also write by hand.
///
/// What this test pins is that each default is the *specific, intended*
/// value, so an edit that silently changes one fails here by name.
#[test]
fn every_section_default_is_the_documented_value() {
    let config = ConfigurationDocument::default();

    assert_eq!(config.agent.default_profile, None);
    assert!(config.agent.profiles.is_empty());
    assert_eq!(config.resources.agent_run_limit, None);
}

/// The one default explicitly sourced from a real existing constant
/// rather than the external design's illustrative numbers -- pinned
/// against the constant itself, not its current numeric value, so the
/// two cannot silently drift apart.
#[test]
fn transcript_retention_default_reuses_the_real_compiled_constant() {
    let config = ConfigurationDocument::default();
    assert_eq!(
        config.agent.transcript_retention_days,
        DEFAULT_TRANSCRIPT_MAX_AGE_DAYS
    );
}

#[test]
fn default_is_deterministic_and_total() {
    assert_eq!(
        ConfigurationDocument::default(),
        ConfigurationDocument::default()
    );
}

/// **RFC-045 D6: absent and zero are different answers.** The default
/// must be `None` (unlimited), never `Some(0)` -- the value the parser
/// refuses -- because a default of zero would mean an empty `[resources]`
/// section silently turned the launch button off.
#[test]
fn an_absent_agent_run_limit_is_unlimited_not_zero() {
    assert_eq!(
        ConfigurationDocument::default().resources.agent_run_limit,
        None
    );
}

// Response 266/270's three "inert by construction" types
// (`RestrictedDefaultTrust`, `RequiredMultilinePasteConfirmation`,
// `RequiredDestructiveCommandApproval`) are gone with the fields they
// guarded -- D3' withdrew `[projects]`, `[terminal]` and the `[security]`
// keys entirely, so there is no field left for a one-valued type to make
// safe.
//
// **The protection did not go with them**, and the three tests below are
// where that is checked rather than assumed: the refusal moved into the
// parser, which rejects each key outright and says why. They live in
// this file, next to where those types used to be defined, so a reader
// looking for what response 266 added and finding it absent is pointed
// at where the guarantee actually lives now.

/// Each of the three is its own security decision by its own RFC, so
/// each gets its own test: a regression in one must not be reported
/// under another's name.
fn assert_refused_permanently_not_pending_a_consumer(source: &str, expected_key: &str) {
    let diagnostic = crate::config::parse_and_validate(source)
        .expect_err("configuration must not be able to ask for this");
    assert_eq!(diagnostic.key, expected_key);
    assert!(
        !diagnostic.message.contains("no effect yet"),
        "this key is refused permanently, not pending a consumer -- \"no effect yet\" would \
         promise a future version that grants what {expected_key} asks for: {}",
        diagnostic.message
    );
}

/// Response 266: configuration must never grant workspace trust --
/// RFC-032 makes it a per-project, two-deliberate-act decision, and a
/// user-global file an already-trusted agent run can write would
/// otherwise trust every future project silently.
#[test]
fn configuration_can_never_grant_workspace_trust() {
    assert_refused_permanently_not_pending_a_consumer(
        "[projects]\ndefault_trust = \"trusted\"\n",
        "projects.default_trust",
    );
}

/// Response 270: every multiline paste is confirmed in the real terminal
/// input path today, with no existing code path that skips it. A file
/// able to turn that off would be a new bypass for every terminal in
/// every project.
#[test]
fn configuration_can_never_disable_multiline_paste_confirmation() {
    assert_refused_permanently_not_pending_a_consumer(
        "[terminal]\nmultiline_paste_protection = false\n",
        "terminal.multiline_paste_protection",
    );
}

/// Response 270, the most severe of the three: not "this one command,
/// this one time" but "never ask again, for any destructive command, in
/// any project, ever."
#[test]
fn configuration_can_never_disable_destructive_command_approval() {
    assert_refused_permanently_not_pending_a_consumer(
        "[security]\nrequire_approval_for_adapter_destructive_commands = false\n",
        "security.require_approval_for_adapter_destructive_commands",
    );
}

/// Only one diagnostic comes back per file, so when a section carries a
/// permanently-refused key **and** an ordinary withdrawn one, which the
/// user reads is a decision, not an accident of key ordering. Pinned
/// here because the ordinary key sorts first in both fixtures below --
/// so without the deliberate preference the user would be told about a
/// key that is merely early, while the one they will never be granted
/// went unmentioned.
#[test]
fn a_permanent_refusal_is_reported_in_preference_to_a_merely_early_one() {
    let diagnostic = crate::config::parse_and_validate(
        "[projects]\nopen_duplicate_root = \"focus_existing\"\ndefault_trust = \"trusted\"\n",
    )
    .expect_err("both keys are refused; the question is which is reported");
    assert_eq!(diagnostic.key, "projects.default_trust");

    let diagnostic = crate::config::parse_and_validate(
        "[security]\nredact_secret_like_environment_names = false\n\
         require_approval_for_adapter_destructive_commands = false\n",
    )
    .expect_err("both keys are refused; the question is which is reported");
    assert_eq!(
        diagnostic.key,
        "security.require_approval_for_adapter_destructive_commands"
    );
}
