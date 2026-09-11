use crate::config::{
    ConfigurationDocument, ConfiguredAiCliProfile, SecuritySensitiveDirection,
    SecuritySensitiveField, apply_safe_fields, direction, security_sensitive_diff,
};

#[test]
fn two_identical_documents_have_no_security_sensitive_diff() {
    let document = ConfigurationDocument::default();
    assert!(security_sensitive_diff(&document, &document).is_empty());
}

/// **RFC-045 D3' narrowed this list from eight fields to two**, and the
/// narrowing is the point: the six that went classified settings no code
/// read, so their confirmation gate governed nothing. This test now
/// covers the whole surviving list, in declaration order, which is the
/// order `security_sensitive_diff` promises.
#[test]
fn a_change_to_every_security_sensitive_field_is_named_in_the_diff() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.agent.transcript_retention_days = 90;
    candidate
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());

    assert_eq!(
        security_sensitive_diff(&current, &candidate),
        vec![
            SecuritySensitiveField::AgentTranscriptRetentionDays,
            SecuritySensitiveField::AgentProfiles,
        ]
    );
}

/// The two fields a reload may apply without asking. `default_profile`
/// is the interesting one: repointing it changes which executable the
/// launch button runs, and it is still safe here because RFC-045 D4's
/// first-use gate is keyed by profile *id* -- a default moved to a
/// profile this session has not confirmed still stops for the
/// confirmation that names its resolved executable.
#[test]
fn a_change_to_a_safe_field_does_not_appear_in_the_diff() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.agent.default_profile = Some("codex".to_owned());
    candidate.resources.agent_run_limit = Some(4);

    assert!(security_sensitive_diff(&current, &candidate).is_empty());
}

/// RFC-011/RFC-033's retention posture: a **larger** bound keeps more
/// data around for longer, so it weakens privacy and requires
/// authorization.
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
        SecuritySensitiveDirection::Increase
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
        SecuritySensitiveDirection::Reduce
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
        SecuritySensitiveDirection::Increase
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
        SecuritySensitiveDirection::Reduce
    );
}

/// Not one of RFC-023's own worked examples -- this module's own
/// worst-case-wins rule (`agent_profiles_direction`'s doc comment):
/// changing an existing profile's own fields is an `Increase`, the same
/// as adding one, since there is no principled way to say a new
/// `command` value is "less" than the old one.
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
        SecuritySensitiveDirection::Increase
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
        },
    );

    assert_eq!(
        direction(SecuritySensitiveField::AgentProfiles, &current, &candidate),
        SecuritySensitiveDirection::Increase
    );
}

#[test]
fn applying_safe_fields_takes_every_safe_field_from_the_candidate() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.agent.default_profile = Some("codex".to_owned());
    candidate.resources.agent_run_limit = Some(9);

    let applied = apply_safe_fields(&current, &candidate);
    assert_eq!(applied.agent.default_profile.as_deref(), Some("codex"));
    assert_eq!(applied.resources.agent_run_limit, Some(9));
}

/// The property that matters most: applying safe fields must **never**
/// pull a security-sensitive field's new value from the candidate, even
/// though the candidate is the only argument that changed.
#[test]
fn applying_safe_fields_never_takes_a_security_sensitive_field_from_the_candidate() {
    let current = ConfigurationDocument::default();
    let mut candidate = current.clone();
    candidate.agent.transcript_retention_days = 999;
    candidate
        .agent
        .profiles
        .insert("codex".to_owned(), test_profile());

    let applied = apply_safe_fields(&current, &candidate);
    assert_eq!(
        applied.agent.transcript_retention_days,
        current.agent.transcript_retention_days
    );
    assert_eq!(applied.agent.profiles, current.agent.profiles);
}

fn test_profile() -> ConfiguredAiCliProfile {
    ConfiguredAiCliProfile {
        display_name: "Codex CLI".to_owned(),
        command: "codex".to_owned(),
    }
}
