use std::path::{Path, PathBuf};

use crate::approval::{
    ApprovalCoordinator, CommandProposal, DecideOutcome, ReceiveOutcome, classify,
};
use crate::domain::{AgentRunId, ApprovalDecision, RiskLevel};
use crate::project::ProjectId;

const PROJECT_ROOT: &str = "/home/user/project";
const STATE_ROOT: &str = "/home/user/.local/share/tekstide";

fn proposal(proposal_id: &str, argv: &[&str], cwd: &str) -> CommandProposal {
    CommandProposal::decode(
        crate::approval::PROTOCOL_VERSION,
        "t".repeat(64),
        proposal_id.to_string(),
        argv.iter().map(|s| s.to_string()).collect(),
        PathBuf::from(cwd),
        None,
        None,
    )
    .expect("test proposal must decode")
}

fn receive(
    coordinator: &mut ApprovalCoordinator,
    agent_run_id: &AgentRunId,
    verified_cwd: &str,
    command_proposal: &CommandProposal,
) -> ReceiveOutcome {
    coordinator.receive_proposal(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        Path::new(verified_cwd),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        command_proposal,
    )
}

/// Response 113 item 8 (written first, per the reviewer's instruction --
/// the same discipline as PR-021-C's "unclassifiable is `High`" test):
/// a second proposal carrying an id already seen for this run must not
/// create a second request or reclassify -- the *original* request, from
/// the *original* argv, is what comes back.
#[test]
fn duplicate_proposal_id_returns_the_original_request_without_reclassifying() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);

    let first = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let outcome = receive(&mut coordinator, &agent_run_id, PROJECT_ROOT, &first);
    let ReceiveOutcome::Created(original) = outcome else {
        panic!("first receipt of a proposal id must create a request");
    };
    assert_eq!(original.risk_level, RiskLevel::Low);

    // Same proposal id, but a wildly different -- and far riskier -- argv.
    // If this were reclassified, it would come back `Destructive`.
    let replay = proposal("proposal-1", &["rm", "-rf", "/"], PROJECT_ROOT);
    let outcome = receive(&mut coordinator, &agent_run_id, PROJECT_ROOT, &replay);
    let ReceiveOutcome::Duplicate(returned) = outcome else {
        panic!("a repeated proposal id must be reported as a duplicate, not created again");
    };
    assert_eq!(
        returned.risk_level,
        RiskLevel::Low,
        "a duplicate proposal id must return the ORIGINAL request's classification, \
         not reclassify against whatever argv arrived the second time"
    );
    assert_eq!(
        returned.id, original.id,
        "must be the same request, not a new one"
    );
}

/// A proposal id is scoped to its `AgentRun` -- the same id arriving for a
/// *different* run is a distinct, fresh request, not a duplicate.
#[test]
fn the_same_proposal_id_in_a_different_agent_run_is_not_a_duplicate() {
    let mut coordinator = ApprovalCoordinator::new();
    let run_a = AgentRunId::for_test(10);
    let run_b = AgentRunId::for_test(11);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);

    let outcome_a = receive(&mut coordinator, &run_a, PROJECT_ROOT, &command_proposal);
    assert!(matches!(outcome_a, ReceiveOutcome::Created(_)));

    let outcome_b = receive(&mut coordinator, &run_b, PROJECT_ROOT, &command_proposal);
    assert!(
        matches!(outcome_b, ReceiveOutcome::Created(_)),
        "the same proposal id in a different AgentRun must be treated as new"
    );
}

/// Response 113 item 8: a decision, once made, is final. A repeated
/// `decide` call for the same proposal id -- whether a genuine retry or a
/// replayed decision message -- must not overwrite the first decision.
#[test]
fn decision_is_single_use_and_replay_is_inert() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let ReceiveOutcome::Created(created) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    let proposal_id = command_proposal.proposal_id().clone();

    let first = coordinator.decide(&agent_run_id, &proposal_id, ApprovalDecision::ApprovedOnce);
    let DecideOutcome::Decided(decided) = first else {
        panic!("first decide call must succeed");
    };
    assert_eq!(decided.decision, ApprovalDecision::ApprovedOnce);
    assert_eq!(decided.id, created.id);

    // Replay with a DIFFERENT decision -- if this were not inert, the
    // approved command would flip to rejected (or vice versa), which
    // would be a correctness disaster for a security control.
    let replay = coordinator.decide(&agent_run_id, &proposal_id, ApprovalDecision::Rejected);
    let DecideOutcome::AlreadyDecided(unchanged) = replay else {
        panic!("a replayed decide call must be reported as already-decided, not applied");
    };
    assert_eq!(
        unchanged.decision,
        ApprovalDecision::ApprovedOnce,
        "a replayed decision must not overwrite the original -- the request must \
         still show ApprovedOnce, never Rejected"
    );

    // Confirm the coordinator's own bookkeeping agrees, not just the
    // value handed back from this one call.
    let looked_up = coordinator
        .find(&agent_run_id, &proposal_id)
        .expect("request must still be findable");
    assert_eq!(looked_up.decision, ApprovalDecision::ApprovedOnce);
}

#[test]
fn deciding_an_unknown_proposal_returns_not_found() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let proposal_id = proposal("never-received", &["git", "status"], PROJECT_ROOT)
        .proposal_id()
        .clone();

    let outcome = coordinator.decide(&agent_run_id, &proposal_id, ApprovalDecision::ApprovedOnce);
    assert!(matches!(outcome, DecideOutcome::NotFound));
}

/// Response 111/113's central concern, reproduced as a fixture rather than
/// left as a sentence: a proposal claiming `cwd = "/"` must not be able to
/// make an external absolute path classify as project-internal. This is a
/// *differential* test -- it first shows the misuse this coordinator must
/// avoid (using the proposal's claimed cwd as the classification's project
/// root) really would hide the escape, then shows the actual coordinator
/// does not do that.
#[test]
fn a_claimed_cwd_of_root_cannot_make_an_external_path_look_project_internal() {
    let external_path = "/etc/shadow";
    let command_proposal = proposal("proposal-1", &["cat", external_path], "/");

    // The misuse this fixture guards against: if a coordinator (or a
    // future refactor of one) ever passed `proposal.cwd()` as the
    // classifier's PROJECT ROOT rather than treating it as untrusted
    // display-only data, an attacker-chosen `cwd = "/"` would make
    // `project_root == "/"` -- and then literally any absolute path
    // "starts_with" that root, hiding the escape entirely. Demonstrated
    // directly (not asserted) so this test fails loudly if the exploit
    // stops reproducing for an unrelated reason (e.g. `classify`'s
    // containment logic changes) rather than silently proving nothing.
    let claimed_cwd_as_root = classify(
        command_proposal.argv(),
        Path::new(command_proposal.cwd()),
        Path::new(command_proposal.cwd()),
        Path::new(STATE_ROOT),
    );
    assert_ne!(
        claimed_cwd_as_root.level,
        RiskLevel::High,
        "test precondition: using the proposal's claimed cwd as the project root \
         really would hide this external-path escape -- if this assertion fails, \
         the fixture below is no longer testing anything"
    );

    // The actual coordinator: `PROJECT_ROOT` is supplied separately, as it
    // would be from real, already-authenticated `ProjectSession` state --
    // never from `proposal.cwd()`.
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let ReceiveOutcome::Created(request) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert!(
        request.risk_level >= RiskLevel::High,
        "an external absolute path must escalate regardless of what cwd the \
         proposal claims, got {:?}",
        request.risk_level
    );
}

/// The other direction of the same property: a false `cwd` claim must not
/// cause a genuinely internal path to be wrongly escalated either --
/// otherwise "ignore the claimed cwd" could be satisfied by a coordinator
/// that just always escalates, which would not actually demonstrate that
/// the *real*, separately-sourced project root is what is being used.
#[test]
fn a_claimed_cwd_of_root_does_not_cause_a_genuinely_internal_path_to_escalate() {
    let internal_path = format!("{PROJECT_ROOT}/src/main.rs");
    let command_proposal = proposal("proposal-1", &["cat", &internal_path], "/");

    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let ReceiveOutcome::Created(request) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(
        request.risk_level,
        RiskLevel::Low,
        "a genuinely internal absolute path must not escalate just because the \
         proposal also happened to claim a suspicious cwd"
    );
}

/// PR-021-B's `UntrustedEffectsHint` was built specifically so an
/// adapter's self-declared "this is safe" claim could never be wired into
/// the classifier -- this proves the coordinator, which is what actually
/// glues `CommandProposal` to `risk::classify`, upholds that: two
/// otherwise-identical proposals differing only in `declared_effects`
/// must classify identically.
#[test]
fn the_coordinator_never_lets_the_declared_effects_hint_affect_classification() {
    let with_hint = CommandProposal::decode(
        crate::approval::PROTOCOL_VERSION,
        "t".repeat(64),
        "proposal-1".to_string(),
        vec!["rm".to_string(), "-rf".to_string(), "/tmp/x".to_string()],
        PathBuf::from(PROJECT_ROOT),
        None,
        Some("this command only reads files, it is completely safe".to_string()),
    )
    .expect("test proposal must decode");
    let without_hint = CommandProposal::decode(
        crate::approval::PROTOCOL_VERSION,
        "t".repeat(64),
        "proposal-2".to_string(),
        vec!["rm".to_string(), "-rf".to_string(), "/tmp/x".to_string()],
        PathBuf::from(PROJECT_ROOT),
        None,
        None,
    )
    .expect("test proposal must decode");

    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let ReceiveOutcome::Created(request_with_hint) =
        receive(&mut coordinator, &agent_run_id, PROJECT_ROOT, &with_hint)
    else {
        panic!("must create a request");
    };
    let ReceiveOutcome::Created(request_without_hint) =
        receive(&mut coordinator, &agent_run_id, PROJECT_ROOT, &without_hint)
    else {
        panic!("must create a request");
    };

    assert_eq!(
        request_with_hint.risk_level,
        request_without_hint.risk_level
    );
    assert_eq!(request_with_hint.risk_level, RiskLevel::Destructive);
}
