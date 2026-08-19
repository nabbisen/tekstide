use crate::config::sensitive::{
    SecuritySensitiveDirection, SecuritySensitiveField, apply_safe_fields, direction,
    security_sensitive_diff,
};
use crate::config::{ConfigurationDocument, ConfiguredAiCliProfile};

#[test]
fn two_identical_documents_have_no_security_sensitive_diff() {
    let document = ConfigurationDocument::default();
    assert!(security_sensitive_diff(&document, &document).is_empty());
}

#[test]
fn a_change_to_every_security_sensitive_field_is_named_in_the_diff() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.security.restricted_mode_blocks_workspace_prompts = false;
    candidate.security.restricted_mode_blocks_workspace_lsp = false;
    candidate.security.restricted_mode_blocks_workspace_plugins = false;
    candidate.security.redact_secret_like_environment_names = false;
    candidate.agent.default_environment_policy = "different".to_owned();
    candidate.agent.transcript_retention_days = 90;
    candidate
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());
    candidate.resources.max_agent_transcript_mb_per_run = 4096;

    let diff = security_sensitive_diff(&current, &candidate);
    for field in [
        SecuritySensitiveField::RestrictedModeBlocksWorkspacePrompts,
        SecuritySensitiveField::RestrictedModeBlocksWorkspaceLsp,
        SecuritySensitiveField::RestrictedModeBlocksWorkspacePlugins,
        SecuritySensitiveField::RedactSecretLikeEnvironmentNames,
        SecuritySensitiveField::AgentDefaultEnvironmentPolicy,
        SecuritySensitiveField::AgentTranscriptRetentionDays,
        SecuritySensitiveField::AgentProfiles,
        SecuritySensitiveField::ResourcesMaxAgentTranscriptMbPerRun,
    ] {
        assert!(diff.contains(&field), "{field:?} missing from {diff:?}");
    }
    assert_eq!(diff.len(), 8, "no extra fields should appear: {diff:?}");
}

/// A change to a *non*-security-sensitive field (safe: hot-reloadable)
/// must never appear in the diff -- this is the boundary the whole
/// mechanism exists to draw correctly.
#[test]
fn a_change_to_a_safe_field_does_not_appear_in_the_diff() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.ui.theme = "different".to_owned();
    candidate.core.recent_projects_limit = 99;
    candidate.terminal.scrollback_lines = 1;
    // The other two `[resources]` fields, deliberately excluded per
    // response 272: `max_terminal_output_mb_per_session` bounds live
    // terminal output, not persisted except through the transcript path
    // already covered by `ResourcesMaxAgentTranscriptMbPerRun`;
    // `max_file_watch_events_per_batch` is a throughput bound for the
    // M13 watcher, which does not exist yet. Neither is retention
    // policy.
    candidate.resources.max_terminal_output_mb_per_session = 1;
    candidate.resources.max_file_watch_events_per_batch = 1;

    assert!(security_sensitive_diff(&current, &candidate).is_empty());
}

/// The inert settings can never differ at all -- confirms they are
/// correctly absent from `SecuritySensitiveField` rather than merely
/// untested. There is no way to construct a `candidate` that differs
/// from `current` in these two fields (both are zero-field unit
/// structs), so this test demonstrates the absence is structural, not
/// an oversight.
#[test]
fn the_inert_settings_have_no_corresponding_diff_variant() {
    let current = ConfigurationDocument::default();
    let candidate = current.clone();
    assert_eq!(
        current.projects.default_trust,
        candidate.projects.default_trust
    );
    assert_eq!(
        current.terminal.multiline_paste_protection,
        candidate.terminal.multiline_paste_protection
    );
    assert_eq!(
        current
            .security
            .require_approval_for_adapter_destructive_commands,
        candidate
            .security
            .require_approval_for_adapter_destructive_commands
    );
}

#[test]
fn disabling_a_restricted_mode_block_is_an_increase() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.security.restricted_mode_blocks_workspace_prompts = false;
    assert_eq!(
        direction(
            SecuritySensitiveField::RestrictedModeBlocksWorkspacePrompts,
            &current,
            &candidate
        ),
        Some(SecuritySensitiveDirection::Increase)
    );
}

#[test]
fn re_enabling_a_restricted_mode_block_is_a_reduce() {
    let mut current = ConfigurationDocument::default();
    current.security.restricted_mode_blocks_workspace_prompts = false;
    let candidate = ConfigurationDocument::default();
    assert_eq!(
        direction(
            SecuritySensitiveField::RestrictedModeBlocksWorkspacePrompts,
            &current,
            &candidate
        ),
        Some(SecuritySensitiveDirection::Reduce)
    );
}

#[test]
fn disabling_environment_name_redaction_is_an_increase() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.security.redact_secret_like_environment_names = false;
    assert_eq!(
        direction(
            SecuritySensitiveField::RedactSecretLikeEnvironmentNames,
            &current,
            &candidate
        ),
        Some(SecuritySensitiveDirection::Increase)
    );
}

#[test]
fn a_longer_transcript_retention_is_an_increase() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.agent.transcript_retention_days = current.agent.transcript_retention_days + 1;
    assert_eq!(
        direction(
            SecuritySensitiveField::AgentTranscriptRetentionDays,
            &current,
            &candidate
        ),
        Some(SecuritySensitiveDirection::Increase)
    );
}

#[test]
fn a_shorter_transcript_retention_is_a_reduce() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.agent.transcript_retention_days = current.agent.transcript_retention_days - 1;
    assert_eq!(
        direction(
            SecuritySensitiveField::AgentTranscriptRetentionDays,
            &current,
            &candidate
        ),
        Some(SecuritySensitiveDirection::Reduce)
    );
}

/// Response 272: the other half of RFC-011's retention policy this
/// module classifies, split across a different section
/// (`[resources]`), same direction rule as retention days -- more
/// bytes kept per run is the weaker privacy posture.
#[test]
fn a_larger_per_run_transcript_byte_limit_is_an_increase() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.resources.max_agent_transcript_mb_per_run =
        current.resources.max_agent_transcript_mb_per_run + 1;
    assert_eq!(
        direction(
            SecuritySensitiveField::ResourcesMaxAgentTranscriptMbPerRun,
            &current,
            &candidate
        ),
        Some(SecuritySensitiveDirection::Increase)
    );
}

#[test]
fn a_smaller_per_run_transcript_byte_limit_is_a_reduce() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.resources.max_agent_transcript_mb_per_run =
        current.resources.max_agent_transcript_mb_per_run - 1;
    assert_eq!(
        direction(
            SecuritySensitiveField::ResourcesMaxAgentTranscriptMbPerRun,
            &current,
            &candidate
        ),
        Some(SecuritySensitiveDirection::Reduce)
    );
}

/// Direction is deliberately undefined for this one -- documented as a
/// real limitation, not a bug, in `sensitive.rs`'s own doc comment.
/// `AgentProfiles` is no longer in this group -- see the
/// `agent_profiles_direction_*` tests below, PR-023-E's own addition.
#[test]
fn agent_default_environment_policy_has_no_defined_direction_yet() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.agent.default_environment_policy = "different".to_owned();

    assert_eq!(
        direction(
            SecuritySensitiveField::AgentDefaultEnvironmentPolicy,
            &current,
            &candidate
        ),
        None
    );
}

/// RFC-023 PR-023-E, RFC-023's own example verbatim: adding a profile
/// increases permitted capability.
#[test]
fn agent_profiles_direction_adding_a_profile_is_an_increase() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());

    assert_eq!(
        direction(SecuritySensitiveField::AgentProfiles, &current, &candidate),
        Some(SecuritySensitiveDirection::Increase)
    );
}

/// RFC-023's other worked example: removing a profile reduces permitted
/// capability.
#[test]
fn agent_profiles_direction_removing_a_profile_is_a_reduce() {
    let mut current = ConfigurationDocument::default();
    current
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());
    let candidate = ConfigurationDocument::default();

    assert_eq!(
        direction(SecuritySensitiveField::AgentProfiles, &current, &candidate),
        Some(SecuritySensitiveDirection::Reduce)
    );
}

/// Not one of RFC-023's own worked examples -- this module's own
/// worst-case-wins rule (`agent_profiles_direction`'s doc comment):
/// changing an existing profile's own fields is an `Increase`, the same
/// as adding one, since there is no principled way to say a new
/// `command`/`args`/`adapter`/`environment_policy` value is "less"
/// than the old one.
#[test]
fn agent_profiles_direction_modifying_an_existing_profile_is_an_increase() {
    let mut current = ConfigurationDocument::default();
    current
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());
    let mut candidate = current.clone();
    candidate.agent.profiles.get_mut("codex").unwrap().command = "different-command".to_owned();

    assert_eq!(
        direction(SecuritySensitiveField::AgentProfiles, &current, &candidate),
        Some(SecuritySensitiveDirection::Increase)
    );
}

/// A mixed change -- one profile added, a different one removed in the
/// same reload -- must still be `Increase`: the removal alone would be
/// safe, but the addition means `candidate` is not a pure subset of
/// `current`, and worst-case-wins.
#[test]
fn agent_profiles_direction_a_mixed_add_and_remove_is_an_increase() {
    let mut current = ConfigurationDocument::default();
    current
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());
    let mut candidate = ConfigurationDocument::default();
    candidate.agent.profiles.insert(
        "claude".to_owned(),
        ConfiguredAiCliProfile {
            display_name: "Claude".to_owned(),
            command: "claude".to_owned(),
            args: Vec::new(),
            adapter: "terminal-native".to_owned(),
            environment_policy: "explicit".to_owned(),
        },
    );

    assert_eq!(
        direction(SecuritySensitiveField::AgentProfiles, &current, &candidate),
        Some(SecuritySensitiveDirection::Increase)
    );
}

#[test]
fn applying_safe_fields_takes_every_safe_field_from_the_candidate() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.ui.theme = "candidate-theme".to_owned();
    candidate.core.recent_projects_limit = 42;
    candidate.terminal.scrollback_lines = 1;
    candidate.resources.max_terminal_output_mb_per_session = 1;
    candidate.agent.max_concurrent_global = 9;

    let applied = apply_safe_fields(&current, &candidate);
    assert_eq!(applied.ui.theme, "candidate-theme");
    assert_eq!(applied.core.recent_projects_limit, 42);
    assert_eq!(applied.terminal.scrollback_lines, 1);
    assert_eq!(applied.resources.max_terminal_output_mb_per_session, 1);
    assert_eq!(applied.agent.max_concurrent_global, 9);
}

/// The property that matters most: applying safe fields must **never**
/// pull a security-sensitive field's new value from the candidate, even
/// though the candidate is the only argument that changed.
#[test]
fn applying_safe_fields_never_takes_a_security_sensitive_field_from_the_candidate() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.security.restricted_mode_blocks_workspace_prompts = false;
    candidate.security.redact_secret_like_environment_names = false;
    candidate.agent.transcript_retention_days = 999;
    candidate.agent.default_environment_policy = "different".to_owned();
    candidate
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());
    candidate.resources.max_agent_transcript_mb_per_run = 4096;

    let applied = apply_safe_fields(&current, &candidate);
    assert_eq!(
        applied.security.restricted_mode_blocks_workspace_prompts,
        current.security.restricted_mode_blocks_workspace_prompts
    );
    assert_eq!(
        applied.security.redact_secret_like_environment_names,
        current.security.redact_secret_like_environment_names
    );
    assert_eq!(
        applied.agent.transcript_retention_days,
        current.agent.transcript_retention_days
    );
    assert_eq!(
        applied.agent.default_environment_policy,
        current.agent.default_environment_policy
    );
    assert_eq!(applied.agent.profiles, current.agent.profiles);
    assert_eq!(
        applied.resources.max_agent_transcript_mb_per_run,
        current.resources.max_agent_transcript_mb_per_run
    );
}

fn test_profile() -> crate::config::ConfiguredAiCliProfile {
    crate::config::ConfiguredAiCliProfile {
        display_name: "Codex CLI".to_owned(),
        command: "codex".to_owned(),
        args: Vec::new(),
        adapter: "terminal-native".to_owned(),
        environment_policy: "explicit".to_owned(),
    }
}
