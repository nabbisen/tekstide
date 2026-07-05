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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseResourceProviderState {
    Complete,
    Unavailable,
    NotImplemented,
    Unknown,
}

pub fn assess_close(summary: &CloseResourceSummary) -> CloseAssessment {
    match summary.provider_state {
        CloseResourceProviderState::Complete => {}
        CloseResourceProviderState::Unavailable => {
            return CloseAssessment::UnsupportedOrUnknown {
                reason: "active-resource state is unavailable".to_owned(),
            };
        }
        CloseResourceProviderState::NotImplemented => {
            return CloseAssessment::UnsupportedOrUnknown {
                reason: "active-resource provider is not implemented".to_owned(),
            };
        }
        CloseResourceProviderState::Unknown => {
            return CloseAssessment::UnsupportedOrUnknown {
                reason: "active-resource state is unknown".to_owned(),
            };
        }
    }

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

    if reasons.is_empty() {
        CloseAssessment::SafeToClose
    } else {
        CloseAssessment::NeedsConfirmation { reasons }
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
