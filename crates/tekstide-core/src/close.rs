#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseAssessment {
    SafeToClose,
    NeedsConfirmation { reasons: Vec<CloseReason> },
    UnsupportedOrUnknown { reason: String },
}

impl CloseAssessment {
    pub fn is_safe_to_close(&self) -> bool {
        matches!(self, Self::SafeToClose)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseReason {
    pub code: CloseReasonCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReasonCode {
    RunningProcess,
    DirtyFile,
    PendingApproval,
    ReviewReadyChange,
    ProviderUnavailable,
    ProviderNotImplemented,
    ProviderUnknown,
    ProviderMissing,
    OtherKnownRisk,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseResourceSummary {
    pub provider_state: CloseResourceProviderState,
    pub running_processes: u32,
    pub dirty_files: u32,
    pub pending_approvals: u32,
    pub review_ready_changes: u32,
}

impl CloseResourceSummary {
    /// RFC-039 PR-039-C, response 311: the exceptional case -- a
    /// project whose resource state genuinely cannot be read (recorded
    /// as `Unavailable` even though every count here is 0, since 0 is
    /// not the same claim as "known to be 0"). **Not** the default state
    /// of a freshly constructed project; see [`Default`] below for that.
    pub fn provider_missing() -> Self {
        Self {
            provider_state: CloseResourceProviderState::Unavailable,
            running_processes: 0,
            dirty_files: 0,
            pending_approvals: 0,
            review_ready_changes: 0,
        }
    }
}

/// RFC-039 PR-039-C, response 311's confirmed fix: every count here
/// except `dirty_files` is tracked in-memory and incrementally
/// (`refresh_runtime_summary_from_collections`, called from every
/// mutation site that could change a terminal, agent run, approval, or
/// change set) and is genuinely known -- zero, not unknown -- from the
/// moment a project is constructed. `dirty_files` is the one count that
/// depends on an external scan (`content_workspace`), and a fresh
/// project has no open buffers, so 0 is honest there too.
/// `provider_state: Complete` is therefore the correct *default*;
/// [`Self::provider_missing`] above is for the genuinely exceptional
/// case, not the routine one. Before this fix, every `ProjectSession`
/// defaulted through `provider_missing()` instead, which made
/// `close_project` return `SafeToClose` for no project, ever --
/// `close_project` had zero production callers before this slice, so
/// the defect was never exercised end to end until PR-039-C's own tests
/// were the first to try.
impl Default for CloseResourceSummary {
    fn default() -> Self {
        Self {
            provider_state: CloseResourceProviderState::Complete,
            running_processes: 0,
            dirty_files: 0,
            pending_approvals: 0,
            review_ready_changes: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseResourceProviderState {
    Complete,
    Unavailable,
    NotImplemented,
    Unknown,
}

pub fn assess_close(summary: &CloseResourceSummary) -> CloseAssessment {
    let mut reasons = Vec::new();
    push_reason(
        &mut reasons,
        summary.running_processes,
        CloseReasonCode::RunningProcess,
        "running process",
    );
    push_reason(
        &mut reasons,
        summary.dirty_files,
        CloseReasonCode::DirtyFile,
        "dirty file",
    );
    push_reason(
        &mut reasons,
        summary.pending_approvals,
        CloseReasonCode::PendingApproval,
        "pending approval",
    );
    push_reason(
        &mut reasons,
        summary.review_ready_changes,
        CloseReasonCode::ReviewReadyChange,
        "review-ready change",
    );

    match summary.provider_state {
        CloseResourceProviderState::Complete => {}
        CloseResourceProviderState::Unavailable => {
            if reasons.is_empty() {
                return CloseAssessment::UnsupportedOrUnknown {
                    reason: "active-resource state is unavailable".to_owned(),
                };
            }
            reasons.push(provider_reason(
                CloseReasonCode::ProviderUnavailable,
                "active-resource state is unavailable",
            ));
        }
        CloseResourceProviderState::NotImplemented => {
            if reasons.is_empty() {
                return CloseAssessment::UnsupportedOrUnknown {
                    reason: "active-resource provider is not implemented".to_owned(),
                };
            }
            reasons.push(provider_reason(
                CloseReasonCode::ProviderNotImplemented,
                "active-resource provider is not implemented",
            ));
        }
        CloseResourceProviderState::Unknown => {
            if reasons.is_empty() {
                return CloseAssessment::UnsupportedOrUnknown {
                    reason: "active-resource state is unknown".to_owned(),
                };
            }
            reasons.push(provider_reason(
                CloseReasonCode::ProviderUnknown,
                "active-resource state is unknown",
            ));
        }
    }

    if reasons.is_empty() {
        CloseAssessment::SafeToClose
    } else {
        CloseAssessment::NeedsConfirmation { reasons }
    }
}

fn provider_reason(code: CloseReasonCode, message: &'static str) -> CloseReason {
    CloseReason {
        code,
        message: message.to_owned(),
    }
}

fn push_reason(
    reasons: &mut Vec<CloseReason>,
    count: u32,
    code: CloseReasonCode,
    singular: &'static str,
) {
    if count == 0 {
        return;
    }

    let label = match (count, singular) {
        (1, singular) => singular.to_owned(),
        (_, "running process") => "running processes".to_owned(),
        (_, singular) => format!("{singular}s"),
    };
    reasons.push(CloseReason {
        code,
        message: format!("{count} {label}"),
    });
}

#[cfg(test)]
mod tests;
