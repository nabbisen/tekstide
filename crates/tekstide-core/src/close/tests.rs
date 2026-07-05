use super::{
    CloseAssessment, CloseReason, CloseReasonCode, CloseResourceProviderState,
    CloseResourceSummary, assess_close,
};

#[test]
fn provider_missing_is_not_safe_to_close() {
    let assessment = assess_close(&CloseResourceSummary::provider_missing());

    assert_eq!(
        assessment,
        CloseAssessment::UnsupportedOrUnknown {
            reason: "active-resource state is unavailable".to_owned()
        }
    );
}

#[test]
fn not_implemented_provider_is_not_safe_to_close() {
    let assessment = assess_close(&CloseResourceSummary {
        provider_state: CloseResourceProviderState::NotImplemented,
        running_processes: 0,
        dirty_files: 0,
        pending_approvals: 0,
        review_ready_changes: 0,
    });

    assert!(matches!(
        assessment,
        CloseAssessment::UnsupportedOrUnknown { .. }
    ));
}

#[test]
fn complete_empty_summary_is_safe_to_close() {
    let assessment = assess_close(&CloseResourceSummary {
        provider_state: CloseResourceProviderState::Complete,
        running_processes: 0,
        dirty_files: 0,
        pending_approvals: 0,
        review_ready_changes: 0,
    });

    assert_eq!(assessment, CloseAssessment::SafeToClose);
}

#[test]
fn active_resources_need_confirmation() {
    let assessment = assess_close(&CloseResourceSummary {
        provider_state: CloseResourceProviderState::Complete,
        running_processes: 2,
        dirty_files: 1,
        pending_approvals: 1,
        review_ready_changes: 3,
    });

    assert_eq!(
        assessment,
        CloseAssessment::NeedsConfirmation {
            reasons: vec![
                CloseReason {
                    code: CloseReasonCode::RunningProcess,
                    message: "2 running processes".to_owned(),
                },
                CloseReason {
                    code: CloseReasonCode::DirtyFile,
                    message: "1 dirty file".to_owned(),
                },
                CloseReason {
                    code: CloseReasonCode::PendingApproval,
                    message: "1 pending approval".to_owned(),
                },
                CloseReason {
                    code: CloseReasonCode::ReviewReadyChange,
                    message: "3 review-ready changes".to_owned(),
                },
            ]
        }
    );
}
