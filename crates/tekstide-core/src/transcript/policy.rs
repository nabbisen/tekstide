use crate::security::{
    BoundedTranscriptRetention, RedactionClaimScope, TranscriptCaptureDefault,
    TranscriptPrivacyPolicy, TranscriptSearchIndexing, TranscriptStoragePolicy,
};

pub const DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES: u64 = 32 * 1024 * 1024;
pub const DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES: u64 = 256 * 1024 * 1024;
pub const DEFAULT_TRANSCRIPT_MAX_APP_BYTES: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_TRANSCRIPT_MAX_AGE_DAYS: u32 = 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptCaptureMode {
    Disabled,
    LocalBounded,
    RequiredLocalBounded,
}

impl TranscriptCaptureMode {
    pub fn captures_bytes(self) -> bool {
        matches!(self, Self::LocalBounded | Self::RequiredLocalBounded)
    }

    pub fn rejects_launch_when_unavailable(self) -> bool {
        self == Self::RequiredLocalBounded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptRetentionLimits {
    pub max_bytes_per_transcript: u64,
    pub max_bytes_per_project: u64,
    pub max_bytes_app_wide: u64,
    pub max_age_days: u32,
}

impl TranscriptRetentionLimits {
    pub fn agent_run_default() -> Self {
        Self {
            max_bytes_per_transcript: DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
            max_bytes_per_project: DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES,
            max_bytes_app_wide: DEFAULT_TRANSCRIPT_MAX_APP_BYTES,
            max_age_days: DEFAULT_TRANSCRIPT_MAX_AGE_DAYS,
        }
    }

    pub fn new(
        max_bytes_per_transcript: u64,
        max_bytes_per_project: u64,
        max_bytes_app_wide: u64,
        max_age_days: u32,
    ) -> Self {
        Self {
            max_bytes_per_transcript,
            max_bytes_per_project,
            max_bytes_app_wide,
            max_age_days,
        }
    }

    pub fn is_bounded(self) -> bool {
        self.max_bytes_per_transcript > 0
            && self.max_bytes_per_project > 0
            && self.max_bytes_app_wide > 0
            && self.max_age_days > 0
            && self.max_bytes_per_transcript <= self.max_bytes_per_project
            && self.max_bytes_per_project <= self.max_bytes_app_wide
    }

    pub fn per_transcript_retention(self) -> BoundedTranscriptRetention {
        BoundedTranscriptRetention::by_size_and_age(
            self.max_bytes_per_transcript,
            self.max_age_days,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptBudgetScope {
    Transcript,
    Project,
    App,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptRetentionState {
    Active,
    Truncated { scope: TranscriptBudgetScope },
    Expired,
    DisabledByOptOut,
    CaptureFailed,
    Purged,
}

impl TranscriptRetentionState {
    pub fn has_retained_bytes(self) -> bool {
        matches!(self, Self::Active | Self::Truncated { .. } | Self::Expired)
    }

    pub fn is_tombstone(self) -> bool {
        matches!(self, Self::Purged)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptLocalDataSummary {
    pub project_retained_bytes: u64,
    pub app_retained_bytes: u64,
    pub project_transcript_count: u64,
    pub budget_pressure: Option<TranscriptBudgetScope>,
}

impl TranscriptLocalDataSummary {
    pub fn new(
        project_retained_bytes: u64,
        app_retained_bytes: u64,
        project_transcript_count: u64,
        limits: TranscriptRetentionLimits,
    ) -> Self {
        let budget_pressure = if project_retained_bytes > limits.max_bytes_per_project {
            Some(TranscriptBudgetScope::Project)
        } else if app_retained_bytes > limits.max_bytes_app_wide {
            Some(TranscriptBudgetScope::App)
        } else {
            None
        };

        Self {
            project_retained_bytes,
            app_retained_bytes,
            project_transcript_count,
            budget_pressure,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptCapturePolicy {
    pub mode: TranscriptCaptureMode,
    pub privacy: TranscriptPrivacyPolicy,
    pub retention_limits: TranscriptRetentionLimits,
}

impl TranscriptCapturePolicy {
    pub fn metadata_only() -> Self {
        Self {
            mode: TranscriptCaptureMode::Disabled,
            privacy: TranscriptPrivacyPolicy::metadata_only_until_retention_ready(),
            retention_limits: TranscriptRetentionLimits::agent_run_default(),
        }
    }

    pub fn local_bounded_agent_run_default() -> Self {
        let retention_limits = TranscriptRetentionLimits::agent_run_default();
        Self {
            mode: TranscriptCaptureMode::LocalBounded,
            privacy: TranscriptPrivacyPolicy::local_bounded_agent_run_default(
                retention_limits.per_transcript_retention(),
            ),
            retention_limits,
        }
    }

    pub fn required_local_bounded(retention_limits: TranscriptRetentionLimits) -> Self {
        Self {
            mode: TranscriptCaptureMode::RequiredLocalBounded,
            privacy: TranscriptPrivacyPolicy::local_bounded_agent_run_default(
                retention_limits.per_transcript_retention(),
            ),
            retention_limits,
        }
    }

    pub fn with_limits(mut self, retention_limits: TranscriptRetentionLimits) -> Self {
        self.retention_limits = retention_limits;
        self.privacy = match self.mode {
            TranscriptCaptureMode::Disabled => {
                TranscriptPrivacyPolicy::metadata_only_until_retention_ready()
            }
            TranscriptCaptureMode::LocalBounded | TranscriptCaptureMode::RequiredLocalBounded => {
                TranscriptPrivacyPolicy::local_bounded_agent_run_default(
                    retention_limits.per_transcript_retention(),
                )
            }
        };
        self
    }

    pub fn permits_transcript_byte_persistence(self) -> bool {
        self.mode.captures_bytes()
            && self.retention_limits.is_bounded()
            && self.privacy.permits_transcript_byte_persistence()
            && self.privacy.storage == TranscriptStoragePolicy::LocalOnly
            && self.privacy.capture_default == TranscriptCaptureDefault::EnabledForTekstideAgentRuns
            && self.privacy.per_run_opt_out_available
            && self.privacy.purge_supported
            && self.privacy.search_indexing == TranscriptSearchIndexing::Disabled
            && self.privacy.redaction_scope == RedactionClaimScope::StructuredMetadataOnly
    }
}
