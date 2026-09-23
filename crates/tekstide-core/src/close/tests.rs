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
                    message: "1 unsaved file".to_owned(),
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

#[test]
fn known_resources_need_confirmation_even_when_provider_is_unavailable() {
    let assessment = assess_close(&CloseResourceSummary {
        provider_state: CloseResourceProviderState::Unavailable,
        running_processes: 1,
        dirty_files: 0,
        pending_approvals: 0,
        review_ready_changes: 0,
    });

    assert_eq!(
        assessment,
        CloseAssessment::NeedsConfirmation {
            reasons: vec![
                CloseReason {
                    code: CloseReasonCode::RunningProcess,
                    message: "1 running process".to_owned(),
                },
                CloseReason {
                    code: CloseReasonCode::ProviderUnavailable,
                    message: "active-resource state is unavailable".to_owned(),
                },
            ],
        }
    );
}

/// RFC-053 review 418, C1: the close confirmation names the same fact the
/// board's count names, in the same word. Open editor buffers with unsaved
/// edits are "unsaved", never "dirty" (D6: one fact, one word). Asserted
/// on `assess_close`'s own output -- the fixtures elsewhere in this crate
/// pass their own message strings through and would not notice the
/// production wording changing back.
#[test]
fn the_close_confirmation_calls_unsaved_buffers_unsaved_not_dirty() {
    for (count, expected) in [(1, "1 unsaved file"), (3, "3 unsaved files")] {
        let assessment = assess_close(&CloseResourceSummary {
            provider_state: CloseResourceProviderState::Complete,
            running_processes: 0,
            dirty_files: count,
            pending_approvals: 0,
            review_ready_changes: 0,
        });

        let CloseAssessment::NeedsConfirmation { reasons } = assessment else {
            panic!("unsaved buffers must need confirmation");
        };
        assert_eq!(reasons.len(), 1);
        assert_eq!(reasons[0].code, CloseReasonCode::DirtyFile);
        assert_eq!(reasons[0].message, expected);
        assert!(!reasons[0].message.contains("dirty"));
    }
}
