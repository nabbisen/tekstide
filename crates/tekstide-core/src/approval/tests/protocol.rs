use crate::approval::{
    CommandDecision, CommandProposal, DecisionOutcome, DecisionValidationErrorReason,
    MAX_ARGV_ENTRIES, MAX_ARGV_ENTRY_LEN, MAX_ARGV_TOTAL_LEN, MAX_CWD_LEN, MAX_EFFECTS_HINT_LEN,
    MAX_INTENT_LEN, MAX_PROPOSAL_ID_LEN, MAX_TOKEN_LEN, PROTOCOL_VERSION, ProposalValidationError,
};
use std::path::PathBuf;

const TOKEN: &str = "cap-token-0123456789abcdef";
const PROPOSAL_ID: &str = "proposal-0001";

fn valid_argv() -> Vec<String> {
    vec!["git".to_string(), "status".to_string()]
}

fn valid_cwd() -> PathBuf {
    PathBuf::from("/home/user/project")
}

fn decode_proposal(
    token: &str,
    proposal_id: &str,
    argv: Vec<String>,
    cwd: PathBuf,
) -> Result<CommandProposal, ProposalValidationError> {
    CommandProposal::decode(
        PROTOCOL_VERSION,
        token.to_string(),
        proposal_id.to_string(),
        argv,
        cwd,
        None,
        None,
    )
}

#[test]
fn valid_proposal_decodes_and_preserves_fields() {
    let proposal = decode_proposal(TOKEN, PROPOSAL_ID, valid_argv(), valid_cwd())
        .expect("valid proposal must decode");

    assert_eq!(proposal.run_token().as_str(), TOKEN);
    assert_eq!(proposal.proposal_id().as_str(), PROPOSAL_ID);
    assert_eq!(proposal.argv(), valid_argv().as_slice());
    assert_eq!(proposal.cwd(), valid_cwd());
    assert_eq!(proposal.declared_intent(), None);
    assert_eq!(proposal.declared_effects(), None);
}

#[test]
fn valid_proposal_with_intent_and_effects_decodes() {
    let proposal = CommandProposal::decode(
        PROTOCOL_VERSION,
        TOKEN.to_string(),
        PROPOSAL_ID.to_string(),
        valid_argv(),
        valid_cwd(),
        Some("run the test suite".to_string()),
        Some("reads only".to_string()),
    )
    .expect("valid proposal with display fields must decode");

    assert_eq!(proposal.declared_intent(), Some("run the test suite"));
    assert_eq!(
        proposal.declared_effects().map(|e| e.as_str()),
        Some("reads only")
    );
}

#[test]
fn unsupported_protocol_version_is_rejected() {
    let result = CommandProposal::decode(
        PROTOCOL_VERSION + 1,
        TOKEN.to_string(),
        PROPOSAL_ID.to_string(),
        valid_argv(),
        valid_cwd(),
        None,
        None,
    );
    assert_eq!(
        result,
        Err(ProposalValidationError::UnsupportedProtocolVersion)
    );
}

#[test]
fn empty_token_is_rejected() {
    let result = decode_proposal("", PROPOSAL_ID, valid_argv(), valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::TokenInvalid));
}

#[test]
fn oversized_token_is_rejected() {
    let oversized = "a".repeat(MAX_TOKEN_LEN + 1);
    let result = decode_proposal(&oversized, PROPOSAL_ID, valid_argv(), valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::TokenInvalid));
}

#[test]
fn token_at_the_bound_is_accepted() {
    let at_bound = "a".repeat(MAX_TOKEN_LEN);
    let result = decode_proposal(&at_bound, PROPOSAL_ID, valid_argv(), valid_cwd());
    assert!(
        result.is_ok(),
        "a token exactly at the bound must be accepted"
    );
}

#[test]
fn token_with_whitespace_is_rejected() {
    let result = decode_proposal("token with space", PROPOSAL_ID, valid_argv(), valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::TokenInvalid));
}

#[test]
fn token_with_control_character_is_rejected() {
    let with_control = "token\u{0007}rest".to_string();
    let result = decode_proposal(&with_control, PROPOSAL_ID, valid_argv(), valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::TokenInvalid));
}

#[test]
fn token_debug_output_never_contains_the_raw_value() {
    let proposal = decode_proposal(TOKEN, PROPOSAL_ID, valid_argv(), valid_cwd())
        .expect("valid proposal must decode");
    let rendered = format!("{:?}", proposal.run_token());
    assert!(
        !rendered.contains(TOKEN),
        "Debug output must never leak the capability token: {rendered}"
    );
}

#[test]
fn empty_proposal_id_is_rejected() {
    let result = decode_proposal(TOKEN, "", valid_argv(), valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::ProposalIdInvalid));
}

#[test]
fn oversized_proposal_id_is_rejected() {
    let oversized = "p".repeat(MAX_PROPOSAL_ID_LEN + 1);
    let result = decode_proposal(TOKEN, &oversized, valid_argv(), valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::ProposalIdInvalid));
}

#[test]
fn empty_argv_is_rejected() {
    let result = decode_proposal(TOKEN, PROPOSAL_ID, Vec::new(), valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::ArgvEmpty));
}

#[test]
fn argv_with_too_many_entries_is_rejected() {
    let argv = vec!["x".to_string(); MAX_ARGV_ENTRIES + 1];
    let result = decode_proposal(TOKEN, PROPOSAL_ID, argv, valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::ArgvTooManyEntries));
}

#[test]
fn argv_at_the_entry_count_bound_is_accepted() {
    let argv = vec!["x".to_string(); MAX_ARGV_ENTRIES];
    let result = decode_proposal(TOKEN, PROPOSAL_ID, argv, valid_cwd());
    assert!(result.is_ok());
}

#[test]
fn oversized_argv_entry_is_rejected() {
    let argv = vec!["x".repeat(MAX_ARGV_ENTRY_LEN + 1)];
    let result = decode_proposal(TOKEN, PROPOSAL_ID, argv, valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::ArgvEntryInvalid));
}

#[test]
fn empty_argv_entry_is_accepted() {
    // Response 109 Q2: real commands legitimately pass an empty string
    // (`printf '%s' ""`, `grep "" file`), and rejecting it buys no
    // security -- the property that matters is that argv is a vector,
    // not whether any one entry happens to be empty.
    let argv = vec!["printf".to_string(), "%s".to_string(), String::new()];
    let result = decode_proposal(TOKEN, PROPOSAL_ID, argv, valid_cwd());
    assert!(result.is_ok(), "an empty argv entry must be accepted");
}

#[test]
fn argv_total_size_over_the_bound_is_rejected_even_with_entries_under_the_per_entry_bound() {
    let entry_len = MAX_ARGV_ENTRY_LEN / 2;
    let entries_needed = MAX_ARGV_TOTAL_LEN / entry_len + 2;
    let argv = vec!["x".repeat(entry_len); entries_needed.min(MAX_ARGV_ENTRIES)];
    // Guard the test's own premise: enough entries to exceed the total
    // bound while each individual entry stays under MAX_ARGV_ENTRY_LEN,
    // and without themselves exceeding MAX_ARGV_ENTRIES.
    assert!(argv.len() <= MAX_ARGV_ENTRIES);
    assert!(argv.iter().map(String::len).sum::<usize>() > MAX_ARGV_TOTAL_LEN);

    let result = decode_proposal(TOKEN, PROPOSAL_ID, argv, valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::ArgvTotalTooLarge));
}

#[test]
fn argv_entry_containing_nul_is_rejected() {
    let argv = vec!["git".to_string(), "sta\0tus".to_string()];
    let result = decode_proposal(TOKEN, PROPOSAL_ID, argv, valid_cwd());
    assert_eq!(result, Err(ProposalValidationError::ArgvEntryInvalid));
}

#[test]
fn argv_with_normal_spaces_and_quotes_in_one_entry_is_accepted() {
    // argv is a vector, never a shell string -- a single entry may
    // legitimately contain spaces or quote characters, since there is no
    // shell grammar here to be confused by them.
    let argv = vec![
        "printf".to_string(),
        "hello \"world\" with spaces".to_string(),
    ];
    let result = decode_proposal(TOKEN, PROPOSAL_ID, argv, valid_cwd());
    assert!(result.is_ok());
}

#[test]
fn relative_cwd_is_rejected() {
    let result = decode_proposal(
        TOKEN,
        PROPOSAL_ID,
        valid_argv(),
        PathBuf::from("relative/path"),
    );
    assert_eq!(result, Err(ProposalValidationError::CwdInvalid));
}

#[test]
fn empty_cwd_is_rejected() {
    let result = decode_proposal(TOKEN, PROPOSAL_ID, valid_argv(), PathBuf::new());
    assert_eq!(result, Err(ProposalValidationError::CwdInvalid));
}

#[test]
fn oversized_cwd_is_rejected() {
    let oversized = PathBuf::from(format!("/{}", "a".repeat(MAX_CWD_LEN)));
    let result = decode_proposal(TOKEN, PROPOSAL_ID, valid_argv(), oversized);
    assert_eq!(result, Err(ProposalValidationError::CwdInvalid));
}

#[test]
fn cwd_containing_nul_is_rejected() {
    let with_nul = PathBuf::from("/home/user/pro\0ject");
    let result = decode_proposal(TOKEN, PROPOSAL_ID, valid_argv(), with_nul);
    assert_eq!(result, Err(ProposalValidationError::CwdInvalid));
}

#[test]
fn oversized_intent_is_rejected() {
    let oversized = "i".repeat(MAX_INTENT_LEN + 1);
    let result = CommandProposal::decode(
        PROTOCOL_VERSION,
        TOKEN.to_string(),
        PROPOSAL_ID.to_string(),
        valid_argv(),
        valid_cwd(),
        Some(oversized),
        None,
    );
    assert_eq!(result, Err(ProposalValidationError::IntentInvalid));
}

#[test]
fn intent_with_embedded_newline_is_rejected() {
    // Control characters (including newline) in text a GUI dialog renders
    // verbatim are a display-spoofing vector independent of the
    // terminal-escape concern RFC-009 already covers.
    let result = CommandProposal::decode(
        PROTOCOL_VERSION,
        TOKEN.to_string(),
        PROPOSAL_ID.to_string(),
        valid_argv(),
        valid_cwd(),
        Some("line one\nline two".to_string()),
        None,
    );
    assert_eq!(result, Err(ProposalValidationError::IntentInvalid));
}

#[test]
fn oversized_effects_hint_is_rejected() {
    let oversized = "e".repeat(MAX_EFFECTS_HINT_LEN + 1);
    let result = CommandProposal::decode(
        PROTOCOL_VERSION,
        TOKEN.to_string(),
        PROPOSAL_ID.to_string(),
        valid_argv(),
        valid_cwd(),
        None,
        Some(oversized),
    );
    assert_eq!(result, Err(ProposalValidationError::EffectsHintInvalid));
}

// --- CommandDecision -------------------------------------------------

#[test]
fn approved_once_decision_decodes_without_edited_argv() {
    let decision = CommandDecision::decode(
        PROTOCOL_VERSION,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::ApprovedOnce,
        None,
    )
    .expect("valid ApprovedOnce decision must decode");
    assert_eq!(decision.outcome(), DecisionOutcome::ApprovedOnce);
    assert_eq!(decision.edited_argv(), None);
}

#[test]
fn rejected_decision_decodes_without_edited_argv() {
    let decision = CommandDecision::decode(
        PROTOCOL_VERSION,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::Rejected,
        None,
    )
    .expect("valid Rejected decision must decode");
    assert_eq!(decision.outcome(), DecisionOutcome::Rejected);
    assert_eq!(decision.edited_argv(), None);
}

#[test]
fn edited_and_approved_decision_requires_edited_argv() {
    let result = CommandDecision::decode(
        PROTOCOL_VERSION,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::EditedAndApproved,
        None,
    );
    assert_eq!(
        result.unwrap_err().reason,
        DecisionValidationErrorReason::EditedArgvMissingForEditedAndApproved
    );
}

#[test]
fn edited_and_approved_decision_with_edited_argv_decodes() {
    let decision = CommandDecision::decode(
        PROTOCOL_VERSION,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::EditedAndApproved,
        Some(vec!["git".to_string(), "status".to_string()]),
    )
    .expect("valid EditedAndApproved decision must decode");
    assert_eq!(
        decision.edited_argv(),
        Some(["git".to_string(), "status".to_string()].as_slice())
    );
}

#[test]
fn approved_once_decision_with_edited_argv_is_rejected() {
    let result = CommandDecision::decode(
        PROTOCOL_VERSION,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::ApprovedOnce,
        Some(vec!["git".to_string()]),
    );
    assert_eq!(
        result.unwrap_err().reason,
        DecisionValidationErrorReason::EditedArgvPresentForOtherDecision
    );
}

#[test]
fn rejected_decision_with_edited_argv_is_rejected() {
    let result = CommandDecision::decode(
        PROTOCOL_VERSION,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::Rejected,
        Some(vec!["git".to_string()]),
    );
    assert_eq!(
        result.unwrap_err().reason,
        DecisionValidationErrorReason::EditedArgvPresentForOtherDecision
    );
}

#[test]
fn edited_and_approved_decision_re_validates_the_edited_argv_bounds() {
    // The edited vector goes through the exact same argv validation as a
    // fresh proposal -- an edit is not exempt from the same bounds.
    let result = CommandDecision::decode(
        PROTOCOL_VERSION,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::EditedAndApproved,
        Some(Vec::new()),
    );
    assert_eq!(
        result.unwrap_err().reason,
        DecisionValidationErrorReason::EditedArgvInvalid
    );
}

#[test]
fn decision_unsupported_protocol_version_is_rejected() {
    let result = CommandDecision::decode(
        PROTOCOL_VERSION + 1,
        PROPOSAL_ID.to_string(),
        DecisionOutcome::ApprovedOnce,
        None,
    );
    assert_eq!(
        result.unwrap_err().reason,
        DecisionValidationErrorReason::UnsupportedProtocolVersion
    );
}

#[test]
fn decision_with_invalid_proposal_id_is_rejected() {
    let result = CommandDecision::decode(
        PROTOCOL_VERSION,
        String::new(),
        DecisionOutcome::ApprovedOnce,
        None,
    );
    assert_eq!(
        result.unwrap_err().reason,
        DecisionValidationErrorReason::ProposalIdInvalid
    );
}
