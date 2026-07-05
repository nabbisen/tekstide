use super::{
    AiSessionSecurityLevel, BoundedTranscriptRetention, RedactionClaimScope, RestrictedModeFeature,
    SecurityPolicyDecision, TranscriptCaptureDefault, TranscriptPrivacyPolicy,
    TranscriptSearchIndexing, TranscriptStoragePolicy, assess_restricted_mode_feature,
    effective_workspace_trust, is_restricted_mode,
};
use crate::domain::AgentCompatibilityLevel;
use crate::domain::TerminalKind;
use crate::project::WorkspaceTrust;

#[test]
fn unknown_and_revoked_trust_are_effectively_restricted() {
    assert_eq!(
        effective_workspace_trust(WorkspaceTrust::Unknown),
        WorkspaceTrust::Restricted
    );
    assert_eq!(
        effective_workspace_trust(WorkspaceTrust::Revoked),
        WorkspaceTrust::Restricted
    );
    assert!(is_restricted_mode(WorkspaceTrust::Unknown));
    assert!(is_restricted_mode(WorkspaceTrust::Restricted));
    assert!(is_restricted_mode(WorkspaceTrust::Revoked));
    assert!(!is_restricted_mode(WorkspaceTrust::Trusted));
}

#[test]
fn restricted_mode_blocks_workspace_local_automation_paths() {
    for feature in RestrictedModeFeature::ALL {
        let decision = assess_restricted_mode_feature(WorkspaceTrust::Restricted, feature);
        assert_eq!(
            decision,
            SecurityPolicyDecision::Blocked {
                feature,
                reason: "workspace-local automation is blocked in Restricted Mode",
            }
        );
        assert!(!feature.label().is_empty());
    }
}

#[test]
fn trusted_workspace_allows_policy_checked_automation_paths() {
    for feature in RestrictedModeFeature::ALL {
        assert_eq!(
            assess_restricted_mode_feature(WorkspaceTrust::Trusted, feature),
            SecurityPolicyDecision::Allowed
        );
    }
}

#[test]
fn compatibility_labels_do_not_overclaim_command_approval() {
    let plain_agent = AiSessionSecurityLevel::from(AgentCompatibilityLevel::Plain);
    let supervised_agent = AiSessionSecurityLevel::from(AgentCompatibilityLevel::Supervised);
    let managed_agent = AiSessionSecurityLevel::from(AgentCompatibilityLevel::Managed);
    let plain_terminal = AiSessionSecurityLevel::from(TerminalKind::Plain);
    let supervised_terminal = AiSessionSecurityLevel::from(TerminalKind::Supervised);

    assert_eq!(plain_agent.label(), "Plain");
    assert_eq!(plain_agent.surface_label(), "Plain Terminal");
    assert_eq!(supervised_agent.surface_label(), "Supervised AgentRun");
    assert_eq!(managed_agent.surface_label(), "Managed AgentRun");
    assert!(!plain_agent.can_claim_managed_command_approval());
    assert!(!supervised_agent.can_claim_managed_command_approval());
    assert!(!plain_terminal.can_claim_managed_command_approval());
    assert!(!supervised_terminal.can_claim_managed_command_approval());
    assert!(managed_agent.can_claim_managed_command_approval());
}

#[test]
fn transcript_bytes_are_blocked_until_local_bounded_purgeable_policy_exists() {
    let metadata_only = TranscriptPrivacyPolicy::metadata_only_until_retention_ready();

    assert_eq!(
        metadata_only.storage,
        TranscriptStoragePolicy::MetadataOnlyNoBytes
    );
    assert_eq!(
        metadata_only.capture_default,
        TranscriptCaptureDefault::DisabledUntilRetentionPolicyExists
    );
    assert_eq!(
        metadata_only.redaction_scope,
        RedactionClaimScope::StructuredMetadataOnly
    );
    assert!(!metadata_only.permits_transcript_byte_persistence());

    let bounded = TranscriptPrivacyPolicy::local_bounded_agent_run_default(
        BoundedTranscriptRetention::by_size_and_age(32 * 1024 * 1024, 30),
    );

    assert_eq!(bounded.storage, TranscriptStoragePolicy::LocalOnly);
    assert_eq!(bounded.search_indexing, TranscriptSearchIndexing::Disabled);
    assert!(bounded.per_run_opt_out_available);
    assert!(bounded.purge_supported);
    assert!(bounded.permits_transcript_byte_persistence());

    let unbounded = TranscriptPrivacyPolicy::local_bounded_agent_run_default(
        BoundedTranscriptRetention::by_size_and_age(0, 0),
    );

    assert!(!unbounded.permits_transcript_byte_persistence());
}
