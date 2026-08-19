use crate::config::{
    ConfigurationDocument, RequiredDestructiveCommandApproval, RequiredMultilinePasteConfirmation,
    RestrictedDefaultTrust,
};
use crate::transcript::DEFAULT_TRANSCRIPT_MAX_AGE_DAYS;

/// Model Checklist: "Compiled defaults are total; no `Option` handling
/// downstream." There is no `Option` anywhere in `ConfigurationDocument`
/// to begin with -- the type system already proves totality. What this
/// test pins down instead is that every section's default is the
/// *specific, intended* value, not merely "some value the type allows,"
/// so a future edit that silently changes a default (e.g. flips a
/// security-relevant bool) fails here by name.
#[test]
fn every_section_default_is_the_documented_value() {
    let config = ConfigurationDocument::default();

    assert!(config.core.default_project_board);
    assert_eq!(config.core.recent_projects_limit, 20);

    assert_eq!(config.ui.theme, "dark-default");
    assert_eq!(config.ui.editor_font_family, "JetBrains Mono");
    assert_eq!(config.ui.terminal_font_family, "JetBrains Mono");
    assert_eq!(config.ui.ui_font_family, "system");
    assert_eq!(config.ui.font_size, 14);
    assert!(config.ui.show_status_labels);

    assert!(config.keybindings.overrides.is_empty());

    assert_eq!(config.terminal.shell_path, "auto");
    assert_eq!(config.terminal.scrollback_lines, 10_000);
    assert_eq!(
        config.terminal.multiline_paste_protection,
        RequiredMultilinePasteConfirmation
    );
    assert!(config.terminal.safe_escape_sequences);

    assert_eq!(config.projects.default_trust, RestrictedDefaultTrust);
    assert!(config.projects.restore_recent_projects);
    assert_eq!(config.projects.open_duplicate_root, "focus_existing");

    assert_eq!(config.agent.max_concurrent_global, 2);
    assert_eq!(config.agent.max_concurrent_per_project, 1);
    assert_eq!(config.agent.default_environment_policy, "explicit");
    assert!(config.agent.capture_changed_files);
    assert!(config.agent.profiles.is_empty());

    assert!(config.security.restricted_mode_blocks_workspace_prompts);
    assert!(config.security.restricted_mode_blocks_workspace_lsp);
    assert!(config.security.restricted_mode_blocks_workspace_plugins);
    assert!(config.security.redact_secret_like_environment_names);
    assert_eq!(
        config
            .security
            .require_approval_for_adapter_destructive_commands,
        RequiredDestructiveCommandApproval
    );

    assert_eq!(config.resources.max_terminal_output_mb_per_session, 64);
    assert_eq!(config.resources.max_agent_transcript_mb_per_run, 128);
    assert_eq!(config.resources.max_file_watch_events_per_batch, 1_000);
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

/// Response 266: `default_trust` must never be able to say "trusted" --
/// a two-valued setting here would be a trust-granting mechanism that
/// bypasses RFC-032's per-project, two-deliberate-act design. This is
/// not a runtime check because none is possible or needed:
/// `RestrictedDefaultTrust` is a zero-field unit struct, so there is no
/// value, constructor, or field assignment anywhere in this crate that
/// can produce anything other than the one value asserted here -- the
/// Rust compiler is the enforcement, not this test. What this test
/// documents is *that* the type is a unit struct (a future edit that
/// widened it back into a `String` or added a second variant would
/// still compile against this assertion, so the real guard is
/// `RestrictedDefaultTrust`'s own definition in `model.rs`, not this
/// file -- read together, not this test alone).
#[test]
fn default_trust_has_exactly_one_possible_value() {
    assert_eq!(
        ConfigurationDocument::default().projects.default_trust,
        RestrictedDefaultTrust
    );
}

/// Response 270: multiline paste confirmation is unconditional in the
/// real terminal input path today, and a config value able to disable
/// it would be a new bypass, the same shape `default_trust` failed the
/// RFC's own test on. Same enforcement, same disclaimer as
/// `default_trust_has_exactly_one_possible_value`: this documents the
/// property, the zero-field unit struct in `model.rs` is what enforces
/// it.
#[test]
fn multiline_paste_confirmation_has_exactly_one_possible_value() {
    assert_eq!(
        ConfigurationDocument::default()
            .terminal
            .multiline_paste_protection,
        RequiredMultilinePasteConfirmation
    );
}

/// Response 270: destructive-command approval is unconditional in the
/// real approval pipeline today, and a config value able to disable it
/// would let every future destructive command execute with no human in
/// the loop, ever, in any project -- the most severe of the three
/// settings this response corrected.
#[test]
fn destructive_command_approval_has_exactly_one_possible_value() {
    assert_eq!(
        ConfigurationDocument::default()
            .security
            .require_approval_for_adapter_destructive_commands,
        RequiredDestructiveCommandApproval
    );
}
