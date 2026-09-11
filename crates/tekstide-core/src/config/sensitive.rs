use std::collections::BTreeMap;

use crate::config::model::{AgentSettings, ConfigurationDocument, ConfiguredAiCliProfile};

/// RFC-023 §Security-Sensitive Settings: the fields that "may never be
/// applied silently, may never be hot-reloaded, and may never come from
/// workspace configuration."
///
/// **RFC-045 D3' removed six of the eight variants, and removed them for
/// a reason worth stating rather than inferring**: each classified a
/// field that no code read. `RestrictedModeBlocksWorkspace*` and
/// `RedactSecretLikeEnvironmentNames` classified the `[security]`
/// settings the sensitive machinery was built around -- which never
/// reached `RestrictedModeFeature` at all; `AgentDefaultEnvironmentPolicy`
/// and `ResourcesMaxAgentTranscriptMbPerRun` the same. A confirmation
/// gate on a value nothing consumes is a control over nothing. **Each
/// returns with its own consumer, in the same change** -- and Restricted
/// Mode policy from configuration specifically is RFC-004 territory,
/// reserved there, not here.
///
/// What is left is what a file can actually change today:
/// `AgentTranscriptRetentionDays` (RFC-045 D9 -- reaches
/// `TranscriptPrivacyPolicy::max_age_days`) and `AgentProfiles` (D8 --
/// reaches the launch path through `default_profile`). Both are real
/// policy, both have a direction, and `AgentProfiles` additionally
/// re-arms D4's first-use confirmation when a reload changes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecuritySensitiveField {
    AgentTranscriptRetentionDays,
    AgentProfiles,
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
    if current.agent.transcript_retention_days != candidate.agent.transcript_retention_days {
        diff.push(SecuritySensitiveField::AgentTranscriptRetentionDays);
    }
    if current.agent.profiles != candidate.agent.profiles {
        diff.push(SecuritySensitiveField::AgentProfiles);
    }
    diff
}

/// The direction a *specific, already-known-to-differ* field's change
/// takes. Both surviving fields have a defined ordering, so unlike
/// RFC-023's version this returns a direction rather than an `Option`:
/// the one variant that had none (`AgentDefaultEnvironmentPolicy`, whose
/// only real value was `"explicit"`) is gone with the field it
/// classified, and a `None` arm no caller can reach would be a branch
/// inviting a reader to handle a case that cannot occur.
pub fn direction(
    field: SecuritySensitiveField,
    current: &ConfigurationDocument,
    candidate: &ConfigurationDocument,
) -> SecuritySensitiveDirection {
    match field {
        SecuritySensitiveField::AgentTranscriptRetentionDays => retention_direction(
            current.agent.transcript_retention_days,
            candidate.agent.transcript_retention_days,
        ),
        SecuritySensitiveField::AgentProfiles => {
            agent_profiles_direction(&current.agent.profiles, &candidate.agent.profiles)
        }
    }
}

/// RFC-023 PR-023-E: `AgentProfiles`'s own direction, completing the
/// classification `sensitive.rs`'s own doc comment deferred to this
/// slice. RFC-023 gives two clean examples -- adding a profile
/// increases permitted capability, removing one reduces it -- but does
/// not say what *modifying* an existing profile is. Since RFC-045 D3'
/// that means its `display_name` or its `command`, and a `command` in
/// particular could tighten or loosen depending on specifics this module
/// has no principled way to judge (a new executable path is not
/// comparably "more" or "less" than the old one the way a retention
/// day-count is). So the rule here is
/// worst-case-wins, the same reasoning `retention_direction`'s
/// less-is-safer logic generalises to a set-valued field: `candidate`
/// is `Reduce` only if it is a **pure subset** of `current` -- every
/// entry `candidate` still has exists in `current` under the same key
/// with an *identical* value. Anything else -- a new key, a changed
/// value for an existing key, or both -- is `Increase`, because at
/// least one entry in `candidate` was not already authorized under
/// `current`'s own values.
fn agent_profiles_direction(
    current: &BTreeMap<String, ConfiguredAiCliProfile>,
    candidate: &BTreeMap<String, ConfiguredAiCliProfile>,
) -> SecuritySensitiveDirection {
    debug_assert_ne!(
        current, candidate,
        "agent_profiles_direction called on unchanged profiles"
    );
    let candidate_is_a_pure_subset_of_current = candidate
        .iter()
        .all(|(name, value)| current.get(name) == Some(value));
    if candidate_is_a_pure_subset_of_current {
        SecuritySensitiveDirection::Reduce
    } else {
        SecuritySensitiveDirection::Increase
    }
}

/// A **larger** retention bound keeps more data around for longer --
/// weakens the privacy posture RFC-011/RFC-033 exist to bound.
///
/// This used to serve both halves of the retention policy response 272
/// classified (max age, and max bytes per transcript). RFC-045 D3'
/// withdrew `resources.max_agent_transcript_mb_per_run`, so `max_age_days`
/// is the only half a file can still ask about; the unit-agnostic
/// wording is kept because the other half returns with its consumer.
fn retention_direction(was: u32, now: u32) -> SecuritySensitiveDirection {
    debug_assert_ne!(was, now, "retention_direction called on an unchanged value");
    if now > was {
        SecuritySensitiveDirection::Increase
    } else {
        SecuritySensitiveDirection::Reduce
    }
}

/// Builds the document that actually takes effect on reload: every safe
/// field from `candidate` (the freshly parsed file), every
/// security-sensitive field held at `current`'s existing value. Because
/// this constructs one complete, new `ConfigurationDocument` value
/// before the caller ever assigns it anywhere, there is no intermediate
/// state where some but not all of a section's fields have been decided
/// -- the same "compute first, assign once" discipline
/// `ConfigStore::reload` already uses for the atomic
/// parse/validate/construct/swap pipeline, extended one level.
///
/// **RFC-045: `default_profile` and `agent_run_limit` are safe fields**,
/// and neither is an oversight. Repointing `default_profile` does not
/// bypass D4's first-use gate -- that gate is keyed by profile id, so a
/// default moved to a profile this session has not confirmed still
/// stops for the confirmation that names its resolved executable.
/// `agent_run_limit` is not in RFC-023's classification, and D7 declines
/// to widen that vocabulary from inside a reachability slice.
pub fn apply_safe_fields(
    current: &ConfigurationDocument,
    candidate: &ConfigurationDocument,
) -> ConfigurationDocument {
    ConfigurationDocument {
        agent: AgentSettings {
            default_profile: candidate.agent.default_profile.clone(),
            transcript_retention_days: current.agent.transcript_retention_days,
            profiles: current.agent.profiles.clone(),
        },
        resources: candidate.resources.clone(),
    }
}
