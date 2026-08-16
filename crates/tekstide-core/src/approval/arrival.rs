//! RFC-022 PR-022-E: "the arrival model" (open question 3, answered by
//! response 220, written into RFC-022's own proposed document under
//! that heading). The policy decided there -- when a queued proposal
//! promotes itself to a modal -- lives here as a single pure function,
//! the same way `approval::risk::classify` is the policy for how a
//! proposal's severity is decided: small, direct, and testable without
//! any of `ApprovalCoordinator`'s own state.
//!
//! Everything else the arrival model specifies (the bounded queue, live
//! counting, expiry) is already built -- `ApprovalCoordinator`'s
//! `ApprovalQueueLimits`/`is_still_answerable`/`ReceiveOutcome::QueueLimitExceeded`,
//! and `ProjectSession`'s `expired_approval_ids`/`mark_approval_expired`/
//! `approval_history_limit`. This module is only the one remaining piece
//! of policy: given a live, queued proposal, should it become a modal
//! right now.

use crate::domain::RiskLevel;

/// **RFC-022 §"The arrival model"**: "`High` and `Destructive` promote
/// to a modal automatically, if no modal is open and the proposal
/// belongs to the active project."
///
/// `belongs_to_active_project` exists specifically because
/// `ApprovalCoordinator` is one flat, `AgentRunId`-keyed structure
/// holding proposals from every open project at once (response 224's
/// own required guard) -- a proposal from a project that is not on
/// screen must never promote, the same confusion the escaped `cwd`
/// field exists to prevent, arriving through the front door instead.
///
/// `Low`/`Medium` never promote, regardless of the other two
/// conditions -- **habituation is a security property**: an adapter
/// making many requests in one task, each seizing the screen, teaches
/// the user the keystroke that dismisses it, manufacturing a record of
/// consent nobody meaningfully gave. Rare interruption is what keeps
/// interruption meaningful.
pub fn should_promote_to_modal(
    risk_level: RiskLevel,
    modal_is_open: bool,
    belongs_to_active_project: bool,
) -> bool {
    matches!(risk_level, RiskLevel::High | RiskLevel::Destructive)
        && !modal_is_open
        && belongs_to_active_project
}
