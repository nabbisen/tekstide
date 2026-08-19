use crate::config::model::{
    AgentSettings, ConfigurationDocument, ResourceSettings, SecuritySettings,
};

/// RFC-023 §Security-Sensitive Settings, as corrected by this pack's
/// own review responses (266-272): the fields that "may never be
/// applied silently, may never be hot-reloaded, and may never come
/// from workspace configuration." `default_trust`,
/// `multiline_paste_protection`, and
/// `require_approval_for_adapter_destructive_commands` are **not**
/// listed here -- they are inert by construction
/// (`RestrictedDefaultTrust`/`RequiredMultilinePasteConfirmation`/
/// `RequiredDestructiveCommandApproval`), so they cannot differ between
/// two [`ConfigurationDocument`]s at all, and gating a value that can
/// never change would be a no-op dressed up as a control.
///
/// `AgentProfiles` is one coarse entry covering the whole
/// `[agent.profile.*]` table -- per-profile add/remove/modify direction
/// (RFC-023's own examples: adding a profile increases permitted
/// capability, removing one reduces it) is PR-023-E's to classify,
/// since that slice is where profile identity and validation actually
/// live. `AgentDefaultEnvironmentPolicy` has no defined value ordering
/// yet (today's only real value is `"explicit"`); reload-gated here
/// like every other field in this list, but its audit-producer
/// direction is deferred for the same reason.
///
/// Response 272: RFC-023 §Security-Sensitive Settings names "transcript
/// retention **and purge** policy" as one policy -- RFC-011 implements
/// it as four bounds (per-transcript bytes, per-project bytes, app-wide
/// bytes, max age), and this classification originally covered only
/// `AgentTranscriptRetentionDays` (max age), because the model happens
/// to put the other three under `[resources]` rather than `[agent]`.
/// `ResourcesMaxAgentTranscriptMbPerRun` (`max_bytes_per_transcript`)
/// closes that gap -- classified by the *policy* the RFC names, not by
/// which section of the typed model the field happens to sit in.
/// `max_terminal_output_mb_per_session` (live output, not persisted
/// except through the already-covered transcript path) and
/// `max_file_watch_events_per_batch` (a throughput bound for the M13
/// watcher, which does not exist yet) are deliberately **not**
/// classified -- neither is retention policy, so the boundary excludes
/// them on purpose, not because nobody looked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecuritySensitiveField {
    RestrictedModeBlocksWorkspacePrompts,
    RestrictedModeBlocksWorkspaceLsp,
    RestrictedModeBlocksWorkspacePlugins,
    RedactSecretLikeEnvironmentNames,
    AgentDefaultEnvironmentPolicy,
    AgentTranscriptRetentionDays,
    AgentProfiles,
    ResourcesMaxAgentTranscriptMbPerRun,
}

/// A change's direction under RFC-013's frozen `config_policy_increase`/
/// `config_policy_reduce` vocabulary, pinned (response context: RFC-023
/// §Audit) against the authorization asymmetry rather than the
/// misleading names: `Increase` **weakens** the security posture
/// (requires authorization); `Reduce` **tightens** it (applies
/// directly).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecuritySensitiveDirection {
    Increase,
    Reduce,
}

/// Compares two documents and returns every security-sensitive field
/// that differs -- the set a reload must hold back from applying
/// silently. Order is fixed (declaration order of
/// [`SecuritySensitiveField`]), not diff-discovery order, so the same
/// two documents always produce the same list.
pub fn security_sensitive_diff(
    current: &ConfigurationDocument,
    candidate: &ConfigurationDocument,
) -> Vec<SecuritySensitiveField> {
    let mut diff = Vec::new();
    if current.security.restricted_mode_blocks_workspace_prompts
        != candidate.security.restricted_mode_blocks_workspace_prompts
    {
        diff.push(SecuritySensitiveField::RestrictedModeBlocksWorkspacePrompts);
    }
    if current.security.restricted_mode_blocks_workspace_lsp
        != candidate.security.restricted_mode_blocks_workspace_lsp
    {
        diff.push(SecuritySensitiveField::RestrictedModeBlocksWorkspaceLsp);
    }
    if current.security.restricted_mode_blocks_workspace_plugins
        != candidate.security.restricted_mode_blocks_workspace_plugins
    {
        diff.push(SecuritySensitiveField::RestrictedModeBlocksWorkspacePlugins);
    }
    if current.security.redact_secret_like_environment_names
        != candidate.security.redact_secret_like_environment_names
    {
        diff.push(SecuritySensitiveField::RedactSecretLikeEnvironmentNames);
    }
    if current.agent.default_environment_policy != candidate.agent.default_environment_policy {
        diff.push(SecuritySensitiveField::AgentDefaultEnvironmentPolicy);
    }
    if current.agent.transcript_retention_days != candidate.agent.transcript_retention_days {
        diff.push(SecuritySensitiveField::AgentTranscriptRetentionDays);
    }
    if current.agent.profiles != candidate.agent.profiles {
        diff.push(SecuritySensitiveField::AgentProfiles);
    }
    if current.resources.max_agent_transcript_mb_per_run
        != candidate.resources.max_agent_transcript_mb_per_run
    {
        diff.push(SecuritySensitiveField::ResourcesMaxAgentTranscriptMbPerRun);
    }
    diff
}

/// The direction a *specific, already-known-to-differ* field's change
/// takes, for the six fields with a defined ordering. Returns `None`
/// for `AgentProfiles`/`AgentDefaultEnvironmentPolicy` -- not because
/// they have no direction, but because this module does not yet define
/// one for them (see the type's own doc comment). A caller must not
/// treat `None` as "safe to apply" -- direction is an audit-producer
/// question; reload-gating (`security_sensitive_diff`) already held the
/// field back regardless of whether a direction is classifiable yet.
pub fn direction(
    field: SecuritySensitiveField,
    current: &ConfigurationDocument,
    candidate: &ConfigurationDocument,
) -> Option<SecuritySensitiveDirection> {
    match field {
        SecuritySensitiveField::RestrictedModeBlocksWorkspacePrompts => Some(bool_direction(
            current.security.restricted_mode_blocks_workspace_prompts,
            candidate.security.restricted_mode_blocks_workspace_prompts,
        )),
        SecuritySensitiveField::RestrictedModeBlocksWorkspaceLsp => Some(bool_direction(
            current.security.restricted_mode_blocks_workspace_lsp,
            candidate.security.restricted_mode_blocks_workspace_lsp,
        )),
        SecuritySensitiveField::RestrictedModeBlocksWorkspacePlugins => Some(bool_direction(
            current.security.restricted_mode_blocks_workspace_plugins,
            candidate.security.restricted_mode_blocks_workspace_plugins,
        )),
        SecuritySensitiveField::RedactSecretLikeEnvironmentNames => Some(bool_direction(
            current.security.redact_secret_like_environment_names,
            candidate.security.redact_secret_like_environment_names,
        )),
        SecuritySensitiveField::AgentTranscriptRetentionDays => Some(retention_direction(
            current.agent.transcript_retention_days,
            candidate.agent.transcript_retention_days,
        )),
        SecuritySensitiveField::ResourcesMaxAgentTranscriptMbPerRun => Some(retention_direction(
            current.resources.max_agent_transcript_mb_per_run,
            candidate.resources.max_agent_transcript_mb_per_run,
        )),
        SecuritySensitiveField::AgentDefaultEnvironmentPolicy
        | SecuritySensitiveField::AgentProfiles => None,
    }
}

/// Shared by both halves of RFC-011's retention policy this module
/// classifies (max age, max bytes per transcript): a **larger** bound
/// keeps more data around for longer -- weakens the privacy posture
/// RFC-011/RFC-033 exist to bound, regardless of which unit (days,
/// megabytes) the bound is measured in.
fn retention_direction(was: u32, now: u32) -> SecuritySensitiveDirection {
    debug_assert_ne!(was, now, "retention_direction called on an unchanged value");
    if now > was {
        SecuritySensitiveDirection::Increase
    } else {
        SecuritySensitiveDirection::Reduce
    }
}

/// `true` is the blocking/protecting/redacting state for every boolean
/// this module classifies -- turning it `false` always weakens the
/// posture, matching every field's own compiled default (`true`) under
/// "fail closed."
fn bool_direction(was: bool, now: bool) -> SecuritySensitiveDirection {
    debug_assert_ne!(was, now, "bool_direction called on an unchanged value");
    if now {
        SecuritySensitiveDirection::Reduce
    } else {
        SecuritySensitiveDirection::Increase
    }
}

/// Builds the document that actually takes effect on reload: every
/// safe field from `candidate` (the freshly parsed file), every
/// security-sensitive field held at `current`'s existing value. Because
/// this constructs one complete, new `ConfigurationDocument` value
/// before the caller ever assigns it anywhere, there is no
/// intermediate state where some but not all of a section's fields
/// have been decided -- the same "compute first, assign once"
/// discipline `ConfigStore::reload` already uses for the atomic
/// parse/validate/construct/swap pipeline, extended one level.
pub fn apply_safe_fields(
    current: &ConfigurationDocument,
    candidate: &ConfigurationDocument,
) -> ConfigurationDocument {
    ConfigurationDocument {
        core: candidate.core.clone(),
        ui: candidate.ui.clone(),
        keybindings: candidate.keybindings.clone(),
        terminal: candidate.terminal.clone(),
        projects: candidate.projects.clone(),
        agent: AgentSettings {
            max_concurrent_global: candidate.agent.max_concurrent_global,
            max_concurrent_per_project: candidate.agent.max_concurrent_per_project,
            default_environment_policy: current.agent.default_environment_policy.clone(),
            transcript_retention_days: current.agent.transcript_retention_days,
            capture_changed_files: candidate.agent.capture_changed_files,
            profiles: current.agent.profiles.clone(),
        },
        security: SecuritySettings {
            restricted_mode_blocks_workspace_prompts: current
                .security
                .restricted_mode_blocks_workspace_prompts,
            restricted_mode_blocks_workspace_lsp: current
                .security
                .restricted_mode_blocks_workspace_lsp,
            restricted_mode_blocks_workspace_plugins: current
                .security
                .restricted_mode_blocks_workspace_plugins,
            redact_secret_like_environment_names: current
                .security
                .redact_secret_like_environment_names,
            // Inert -- always equal between `current` and `candidate`
            // (there is only one possible value), so which side this
            // reads from cannot matter. Reads from `candidate` only to
            // avoid a redundant `current`/`candidate` split for a field
            // that can never actually differ.
            require_approval_for_adapter_destructive_commands: candidate
                .security
                .require_approval_for_adapter_destructive_commands,
        },
        resources: ResourceSettings {
            max_terminal_output_mb_per_session: candidate
                .resources
                .max_terminal_output_mb_per_session,
            max_agent_transcript_mb_per_run: current.resources.max_agent_transcript_mb_per_run,
            max_file_watch_events_per_batch: candidate.resources.max_file_watch_events_per_batch,
        },
    }
}
