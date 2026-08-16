use crate::approval::should_promote_to_modal;
use crate::domain::RiskLevel;

/// **RFC-022 §"The arrival model"**: `High`/`Destructive` promote when
/// no modal is open and the proposal belongs to the active project --
/// every other combination of risk level, modal state, and project
/// membership must not. Enumerated exhaustively rather than spot-checked,
/// since a promotion policy this consequential should not have an
/// untested corner.
#[test]
fn promotion_requires_high_or_destructive_no_modal_and_the_active_project() {
    for risk_level in [
        RiskLevel::Low,
        RiskLevel::Medium,
        RiskLevel::High,
        RiskLevel::Destructive,
    ] {
        for modal_is_open in [false, true] {
            for belongs_to_active_project in [false, true] {
                let expected = matches!(risk_level, RiskLevel::High | RiskLevel::Destructive)
                    && !modal_is_open
                    && belongs_to_active_project;
                assert_eq!(
                    should_promote_to_modal(risk_level, modal_is_open, belongs_to_active_project),
                    expected,
                    "risk_level={risk_level:?} modal_is_open={modal_is_open} \
                     belongs_to_active_project={belongs_to_active_project}"
                );
            }
        }
    }
}

/// The cross-project guard, isolated to this pure function: a
/// `Destructive` proposal with no modal open -- the strongest case for
/// promotion otherwise possible -- must still not promote when it does
/// not belong to the active project. Named separately from the
/// exhaustive sweep above because this is the specific property response
/// 224 required be proven, not merely one row of a larger table.
#[test]
fn a_destructive_proposal_from_a_background_project_does_not_promote() {
    assert!(!should_promote_to_modal(
        RiskLevel::Destructive,
        false,
        false
    ));
}

/// The open-modal guard, isolated the same way: a `Destructive` proposal
/// belonging to the active project must not promote over an already-open
/// modal (whatever that modal is -- this function only knows "one is
/// open," not which).
#[test]
fn a_destructive_proposal_does_not_promote_over_an_open_modal() {
    assert!(!should_promote_to_modal(RiskLevel::Destructive, true, true));
}

/// The habituation guard: even the ideal case for promotion in every
/// other respect (no modal open, active project) must not promote for
/// `Low`/`Medium` -- rare interruption is what keeps interruption
/// meaningful.
#[test]
fn low_and_medium_never_promote_regardless_of_modal_or_project_state() {
    for risk_level in [RiskLevel::Low, RiskLevel::Medium] {
        assert!(!should_promote_to_modal(risk_level, false, true));
    }
}
