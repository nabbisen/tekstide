//! RFC-021: command approval model and adapter capability.
//!
//! This module defines the sideband approval protocol between an AI CLI
//! adapter and Tekstide. It is headless product code, not a spike --
//! everything here is a security boundary in the same sense as
//! `audit::record` and `runtime::terminal::security`: untrusted input in,
//! fail-closed decisions out.
//!
//! PR-021-B: protocol message types and bounded validation.
//! PR-021-C (this slice): structural risk classifier.
//! No channel, no coordinator yet -- those are `approval::channel` and
//! `approval::coordinator`, added in later slices.

mod protocol;
mod risk;

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
