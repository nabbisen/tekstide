//! RFC-021: command approval model and adapter capability.
//!
//! This module defines the sideband approval protocol between an AI CLI
//! adapter and Tekstide. It is headless product code, not a spike --
//! everything here is a security boundary in the same sense as
//! `audit::record` and `runtime::terminal::security`: untrusted input in,
//! fail-closed decisions out.
//!
//! PR-021-B: protocol message types and bounded validation.
//! PR-021-C: structural risk classifier.
//! PR-021-D: the sideband channel.
//! PR-021-E1 (this slice): the trusted-context coordinator -- verified
//! cwd, proposal-id uniqueness, single-use decisions. Audit wiring and the
//! decision round-trip over the channel are PR-021-E2, not yet built.

mod channel;
mod coordinator;
mod protocol;
mod risk;

pub use channel::{
    APPROVAL_TOKEN_ENV_VAR, AcceptedProposal, ApprovalChannelDirectory, ApprovalChannelEndpoint,
    ApprovalChannelError, ApprovalChannelErrorReason, ApprovalChannelPathError,
    ApprovalChannelPathErrorReason, ApprovalChannelPathRequest, ApprovalChannelPathResolver,
    inject_token_into_environment,
};
pub use coordinator::{ApprovalCoordinator, DecideOutcome, ReceiveOutcome};
pub use protocol::{
    CommandDecision, CommandProposal, DecisionOutcome, DecisionValidationError,
    DecisionValidationErrorReason, MAX_ARGV_ENTRIES, MAX_ARGV_ENTRY_LEN, MAX_ARGV_TOTAL_LEN,
    MAX_CWD_LEN, MAX_EFFECTS_HINT_LEN, MAX_INTENT_LEN, MAX_PROPOSAL_ID_LEN, MAX_TOKEN_LEN,
    PROTOCOL_VERSION, ProposalId, ProposalValidationError, RunCapabilityToken,
    UntrustedEffectsHint,
};
pub use risk::{RiskAssessment, RiskReason, classify};

#[cfg(test)]
mod tests;
