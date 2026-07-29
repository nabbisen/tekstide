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
//! PR-021-D (this slice): the sideband channel.
//! No coordinator yet -- that is `approval::coordinator`, added in the
//! next slice.

mod channel;
mod protocol;
mod risk;

pub use channel::{
    AcceptedProposal, ApprovalChannelDirectory, ApprovalChannelEndpoint, ApprovalChannelError,
    ApprovalChannelErrorReason, ApprovalChannelPathError, ApprovalChannelPathErrorReason,
    ApprovalChannelPathRequest, ApprovalChannelPathResolver,
};
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
