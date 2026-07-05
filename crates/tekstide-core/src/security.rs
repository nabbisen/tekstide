use crate::domain::{AgentCompatibilityLevel, TerminalKind};
use crate::project::WorkspaceTrust;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestrictedModeFeature {
    AutomaticLspStartup,
    WorkspaceAiProfileLoading,
    WorkspaceAiPromptLoading,
    WorkspaceEnvironmentLoading,
    WorkspacePluginLoading,
    AutomaticTaskExecution,
    TekstideInitiatedGitHook,
    WorkspaceCommandPaletteEntry,
    BackgroundProjectAutomation,
}

impl RestrictedModeFeature {
    pub const ALL: [Self; 9] = [
        Self::AutomaticLspStartup,
        Self::WorkspaceAiProfileLoading,
        Self::WorkspaceAiPromptLoading,
        Self::WorkspaceEnvironmentLoading,
        Self::WorkspacePluginLoading,
        Self::AutomaticTaskExecution,
        Self::TekstideInitiatedGitHook,
        Self::WorkspaceCommandPaletteEntry,
        Self::BackgroundProjectAutomation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AutomaticLspStartup => "automatic LSP startup",
            Self::WorkspaceAiProfileLoading => "workspace AI profile loading",
            Self::WorkspaceAiPromptLoading => "workspace AI prompt loading",
            Self::WorkspaceEnvironmentLoading => "workspace environment loading",
            Self::WorkspacePluginLoading => "workspace plugin loading",
            Self::AutomaticTaskExecution => "automatic task execution",
            Self::TekstideInitiatedGitHook => "Tekstide-initiated Git hook",
            Self::WorkspaceCommandPaletteEntry => "workspace command palette entry",
            Self::BackgroundProjectAutomation => "background project automation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityPolicyDecision {
    /// This Restricted Mode policy does not block the feature. Callers must still apply
    /// feature-specific configuration, user approval, adapter capability, and OS-level checks.
    Allowed,
    Blocked {
        feature: RestrictedModeFeature,
        reason: &'static str,
    },
}

pub fn effective_workspace_trust(trust: WorkspaceTrust) -> WorkspaceTrust {
    match trust {
        WorkspaceTrust::Trusted => WorkspaceTrust::Trusted,
        WorkspaceTrust::Unknown | WorkspaceTrust::Restricted | WorkspaceTrust::Revoked => {
            WorkspaceTrust::Restricted
        }
    }
}

pub fn is_restricted_mode(trust: WorkspaceTrust) -> bool {
    effective_workspace_trust(trust) == WorkspaceTrust::Restricted
}

pub fn assess_restricted_mode_feature(
    trust: WorkspaceTrust,
    feature: RestrictedModeFeature,
) -> SecurityPolicyDecision {
    if is_restricted_mode(trust) {
        SecurityPolicyDecision::Blocked {
            feature,
            reason: "workspace-local automation is blocked in Restricted Mode",
        }
    } else {
        SecurityPolicyDecision::Allowed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiSessionSecurityLevel {
    Plain,
    Supervised,
    Managed,
}

impl AiSessionSecurityLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Plain => "Plain",
            Self::Supervised => "Supervised",
            Self::Managed => "Managed",
        }
    }

    pub fn surface_label(self) -> &'static str {
        match self {
            Self::Plain => "Plain Terminal",
            Self::Supervised => "Supervised AgentRun",
            Self::Managed => "Managed AgentRun",
        }
    }

    pub fn can_claim_managed_command_approval(self) -> bool {
        self == Self::Managed
    }
}

impl From<AgentCompatibilityLevel> for AiSessionSecurityLevel {
    fn from(level: AgentCompatibilityLevel) -> Self {
        match level {
            AgentCompatibilityLevel::Plain => Self::Plain,
            AgentCompatibilityLevel::Supervised => Self::Supervised,
            AgentCompatibilityLevel::Managed => Self::Managed,
        }
    }
}

impl From<TerminalKind> for AiSessionSecurityLevel {
    fn from(kind: TerminalKind) -> Self {
        match kind {
            TerminalKind::Plain => Self::Plain,
            TerminalKind::Supervised => Self::Supervised,
            TerminalKind::Managed => Self::Managed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptStoragePolicy {
    MetadataOnlyNoBytes,
    LocalOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedTranscriptRetention {
    pub max_bytes: Option<u64>,
    pub max_age_days: Option<u32>,
}

impl BoundedTranscriptRetention {
    pub fn by_size(max_bytes: u64) -> Self {
        Self {
            max_bytes: Some(max_bytes),
            max_age_days: None,
        }
    }

    pub fn by_age_days(max_age_days: u32) -> Self {
        Self {
            max_bytes: None,
            max_age_days: Some(max_age_days),
        }
    }

    pub fn by_size_and_age(max_bytes: u64, max_age_days: u32) -> Self {
        Self {
            max_bytes: Some(max_bytes),
            max_age_days: Some(max_age_days),
        }
    }

    pub fn is_bounded(self) -> bool {
        self.max_bytes.is_some_and(|max_bytes| max_bytes > 0)
            || self
                .max_age_days
                .is_some_and(|max_age_days| max_age_days > 0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptCaptureDefault {
    DisabledUntilRetentionPolicyExists,
    EnabledForTekstideAgentRuns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptSearchIndexing {
    Disabled,
    ExplicitlyEnabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedactionClaimScope {
    StructuredMetadataOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptPrivacyPolicy {
    pub storage: TranscriptStoragePolicy,
    pub retention: Option<BoundedTranscriptRetention>,
    pub capture_default: TranscriptCaptureDefault,
    pub per_run_opt_out_available: bool,
    pub purge_supported: bool,
    pub search_indexing: TranscriptSearchIndexing,
    pub redaction_scope: RedactionClaimScope,
}

impl TranscriptPrivacyPolicy {
    pub fn metadata_only_until_retention_ready() -> Self {
        Self {
            storage: TranscriptStoragePolicy::MetadataOnlyNoBytes,
            retention: None,
            capture_default: TranscriptCaptureDefault::DisabledUntilRetentionPolicyExists,
            per_run_opt_out_available: true,
            purge_supported: false,
            search_indexing: TranscriptSearchIndexing::Disabled,
            redaction_scope: RedactionClaimScope::StructuredMetadataOnly,
        }
    }

    pub fn local_bounded_agent_run_default(retention: BoundedTranscriptRetention) -> Self {
        Self {
            storage: TranscriptStoragePolicy::LocalOnly,
            retention: Some(retention),
            capture_default: TranscriptCaptureDefault::EnabledForTekstideAgentRuns,
            per_run_opt_out_available: true,
            purge_supported: true,
            search_indexing: TranscriptSearchIndexing::Disabled,
            redaction_scope: RedactionClaimScope::StructuredMetadataOnly,
        }
    }

    pub fn permits_transcript_byte_persistence(self) -> bool {
        self.storage == TranscriptStoragePolicy::LocalOnly
            && self
                .retention
                .map(BoundedTranscriptRetention::is_bounded)
                .unwrap_or(false)
            && self.capture_default == TranscriptCaptureDefault::EnabledForTekstideAgentRuns
            && self.per_run_opt_out_available
            && self.purge_supported
    }
}

#[cfg(test)]
mod tests;
