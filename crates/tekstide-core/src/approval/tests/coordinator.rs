use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::agent::VerifiedCwd;
use crate::approval::{
    AcceptedProposal, ApprovalCoordinator, CommandProposal, DecideOutcome, ReceiveOutcome,
    SimpleDecision, classify,
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

/// Builds a real, connected `AcceptedProposal` (via `UnixStream::pair()`,
/// not a mock) and calls `receive_proposal` with it -- response 114
/// Required 1's fix means `receive_proposal` now takes ownership of the
/// connection a proposal arrived on, so it can send a decision back over
/// it later. Returns the peer half of the pair so a test can read back
/// whatever `decide`/`decide_with_edited_argv` actually sends.
fn receive(
    coordinator: &mut ApprovalCoordinator,
    agent_run_id: &AgentRunId,
    verified_cwd: &str,
    command_proposal: &CommandProposal,
) -> (ReceiveOutcome, UnixStream) {
    let (accepted, peer) = AcceptedProposal::for_test(command_proposal.clone());
    let outcome = coordinator.receive_proposal(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        &VerifiedCwd::for_test(verified_cwd),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        accepted,
    );
    (outcome, peer)
}

/// Reads one length-prefixed JSON frame from `peer` and returns it parsed
/// -- the same wire shape `approval::channel` writes decisions in. A
/// bounded read timeout is set first so a genuine regression (nothing
/// ever sent) fails this test promptly rather than hanging the whole
/// suite -- discovered the hard way while ablating `decide`'s send call
/// during response-115-era testing, which hung indefinitely without one.
fn read_decision_frame(peer: &mut UnixStream) -> serde_json::Value {
    peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("set a bounded read timeout");
    let mut len_bytes = [0_u8; 4];
    peer.read_exact(&mut len_bytes)
        .expect("read decision length prefix");
    let len = u32::from_be_bytes(len_bytes) as usize;
    let mut buffer = vec![0_u8; len];
    peer.read_exact(&mut buffer).expect("read decision body");
    serde_json::from_slice(&buffer).expect("decision must be valid JSON")
}

/// Response 114 Required 1 -- the exact laundering sequence the reviewer
/// probed: an adapter submits `git status` under id `1`, the user
/// approves it, and the adapter resubmits a wildly different, far riskier
/// argv under the SAME id. The second receive must reject outright: no
/// `ApprovalRequest` is returned at all, so there is nothing a caller
/// could misread as an `ApprovedOnce` authorization for the new argv.
#[test]
fn a_repeated_proposal_id_is_rejected_outright_even_after_approval_and_never_authorizes_new_argv() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);

    let first = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (ReceiveOutcome::Created { request }, _peer) =
        receive(&mut coordinator, &agent_run_id, PROJECT_ROOT, &first)
    else {
        panic!("first receipt of a proposal id must create a request");
    };
    let proposal_id = first.proposal_id().clone();
    let decided = coordinator.decide(&agent_run_id, &proposal_id, SimpleDecision::ApprovedOnce);
    assert!(matches!(decided, DecideOutcome::Decided { .. }));
    assert_eq!(request.risk_level, RiskLevel::Low);

    // Same proposal id, resubmitted with a far riskier argv -- the attack
    // this fixture reproduces: laundering the earlier approval onto a
    // command that was never classified or decided on its own merits.
    let laundering_attempt = proposal("proposal-1", &["rm", "-rf", "/etc"], PROJECT_ROOT);
    let (outcome, _peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &laundering_attempt,
    );
    assert!(
        outcome.is_duplicate_rejected(),
        "a repeated proposal id must be rejected outright"
    );
    assert!(
        outcome.request().is_none(),
        "a rejected duplicate must carry NO ApprovalRequest -- there must be nothing \
         a caller could misread as authorization for the new argv, approved or not"
    );

    // The ORIGINAL request, looked up directly, must still reflect only
    // what it was actually decided on -- the second receive must not have
    // mutated it either.
    let original_still_intact = coordinator
        .find(&agent_run_id, &proposal_id)
        .expect("the original request must still exist, unmutated");
    assert_eq!(
        original_still_intact.decision,
        ApprovalDecision::ApprovedOnce
    );
    assert_eq!(original_still_intact.risk_level, RiskLevel::Low);
}

/// A proposal id is scoped to its `AgentRun` -- the same id arriving for a
/// *different* run is a distinct, fresh request, not a duplicate.
#[test]
fn the_same_proposal_id_in_a_different_agent_run_is_not_a_duplicate() {
    let mut coordinator = ApprovalCoordinator::new();
    let run_a = AgentRunId::for_test(10);
    let run_b = AgentRunId::for_test(11);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);

    let (outcome_a, _peer_a) = receive(&mut coordinator, &run_a, PROJECT_ROOT, &command_proposal);
    assert!(matches!(outcome_a, ReceiveOutcome::Created { .. }));

    let (outcome_b, _peer_b) = receive(&mut coordinator, &run_b, PROJECT_ROOT, &command_proposal);
    assert!(
        matches!(outcome_b, ReceiveOutcome::Created { .. }),
        "the same proposal id in a different AgentRun must be treated as new"
    );
}

/// A decision, once made, is final. A repeated `decide` call for the same
/// proposal id -- whether a genuine retry or a replayed decision message
/// -- must not overwrite the first decision.
#[test]
fn decision_is_single_use_and_replay_is_inert() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (ReceiveOutcome::Created { request: created }, _peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    let proposal_id = command_proposal.proposal_id().clone();

    let first = coordinator.decide(&agent_run_id, &proposal_id, SimpleDecision::ApprovedOnce);
    let DecideOutcome::Decided {
        request: decided, ..
    } = first
    else {
        panic!("first decide call must succeed");
    };
    assert_eq!(decided.decision, ApprovalDecision::ApprovedOnce);
    assert_eq!(decided.id, created.id);

    // Replay with a DIFFERENT decision -- if this were not inert, the
    // approved command would flip to rejected (or vice versa), which
    // would be a correctness disaster for a security control.
    let replay = coordinator.decide(&agent_run_id, &proposal_id, SimpleDecision::Rejected);
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

    let outcome = coordinator.decide(&agent_run_id, &proposal_id, SimpleDecision::ApprovedOnce);
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
    let (ReceiveOutcome::Created { request }, _peer) = receive(
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
    let (ReceiveOutcome::Created { request }, _peer) = receive(
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
    let (
        ReceiveOutcome::Created {
            request: request_with_hint,
        },
        _peer1,
    ) = receive(&mut coordinator, &agent_run_id, PROJECT_ROOT, &with_hint)
    else {
        panic!("must create a request");
    };
    let (
        ReceiveOutcome::Created {
            request: request_without_hint,
        },
        _peer2,
    ) = receive(&mut coordinator, &agent_run_id, PROJECT_ROOT, &without_hint)
    else {
        panic!("must create a request");
    };

    assert_eq!(
        request_with_hint.risk_level,
        request_without_hint.risk_level
    );
    assert_eq!(request_with_hint.risk_level, RiskLevel::Destructive);
}

// --- Response 114 Required 2: display_command must not be argv.join(" ") ---

/// The reviewer's five probe cases, each a different way `argv.join(" ")`
/// misrepresented the vector it was built from. Asserted through
/// `ApprovalRequest.display_command`, since `display_argv` itself is a
/// private implementation detail of this module.
#[test]
fn display_command_quotes_an_entry_that_would_otherwise_read_as_a_second_shell_command() {
    let command_proposal = proposal(
        "proposal-1",
        &["git", "commit", "-m", "fix; rm -rf /etc"],
        PROJECT_ROOT,
    );
    let mut coordinator = ApprovalCoordinator::new();
    let (ReceiveOutcome::Created { request }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(request.display_command, "git commit -m 'fix; rm -rf /etc'");
}

#[test]
fn display_command_quotes_an_entry_containing_a_space_so_it_does_not_read_as_two_arguments() {
    let command_proposal = proposal("proposal-1", &["rm", "-rf", "my documents"], PROJECT_ROOT);
    let mut coordinator = ApprovalCoordinator::new();
    let (ReceiveOutcome::Created { request }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(request.display_command, "rm -rf 'my documents'");
}

#[test]
fn display_command_escapes_an_embedded_newline_to_a_visible_marker() {
    let command_proposal = proposal("proposal-1", &["echo", "safe\nrm -rf /etc"], PROJECT_ROOT);
    let mut coordinator = ApprovalCoordinator::new();
    let (ReceiveOutcome::Created { request }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(
        request.display_command, "echo 'safe<U+000A>rm -rf /etc'",
        "the embedded newline must render as a visible marker, not an actual line break \
         that could hide the rest of the argument below a fold"
    );
}

#[test]
fn display_command_escapes_a_bidi_override_per_rfc_016_rather_than_letting_it_reverse_text() {
    let command_proposal = proposal("proposal-1", &["rm", "\u{202e}gpj.exe"], PROJECT_ROOT);
    let mut coordinator = ApprovalCoordinator::new();
    let (ReceiveOutcome::Created { request }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(
        request.display_command, "rm '<U+202E>gpj.exe'",
        "a RIGHT-TO-LEFT OVERRIDE must render as a visible <U+202E> marker per RFC-016's \
         escape-and-isolate policy, never passed through where a renderer could obey it \
         as a directionality instruction (the Trojan Source pattern)"
    );
}

#[test]
fn display_command_renders_an_empty_argument_visibly_rather_than_letting_it_vanish() {
    let command_proposal = proposal("proposal-1", &["printf", "%s", ""], PROJECT_ROOT);
    let mut coordinator = ApprovalCoordinator::new();
    let (ReceiveOutcome::Created { request }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(request.display_command, "printf %s ''");
}

/// Response 115 Required A: the reviewer's ten probe cases showing the
/// bidi-range-only escape (response 114's fix) missed three genuine bidi
/// controls and six other invisible/format characters RFC-016 §Security
/// point 3 also requires be made visible. Each must render as a visible
/// `<U+XXXX>` marker, not pass through raw -- the `important.txt` case
/// (index 5, ZWSP) is the reviewer's exact concrete example of the
/// display-ambiguity defect surviving in a form the original fix did not
/// cover: `impor<ZWSP>tant.txt` renders as `important.txt` with no visual
/// cue that a command targets a different file than the one displayed.
#[test]
fn display_command_escapes_every_bidi_and_format_probe_from_response_115() {
    let probes: &[(char, &str)] = &[
        ('\u{202E}', "202E"), // RLO -- already covered by response 114
        ('\u{2066}', "2066"), // LRI -- already covered by response 114
        ('\u{200E}', "200E"), // LRM -- a bidi mark, missed by the bidi-range-only rule
        ('\u{200F}', "200F"), // RLM -- a bidi mark, missed
        ('\u{061C}', "061C"), // ALM -- a bidi mark, missed
        ('\u{200B}', "200B"), // ZWSP -- the "important.txt" case
        ('\u{200C}', "200C"), // ZWNJ
        ('\u{200D}', "200D"), // ZWJ
        ('\u{00AD}', "00AD"), // soft hyphen
        ('\u{FEFF}', "FEFF"), // ZWNBSP / BOM
    ];
    for (index, (codepoint, hex)) in probes.iter().enumerate() {
        let entry = format!("impor{codepoint}tant.txt");
        let command_proposal =
            proposal(&format!("proposal-{index}"), &["cat", &entry], PROJECT_ROOT);
        let mut coordinator = ApprovalCoordinator::new();
        let (ReceiveOutcome::Created { request }, _peer) = receive(
            &mut coordinator,
            &AgentRunId::for_test(index as u64),
            PROJECT_ROOT,
            &command_proposal,
        ) else {
            panic!("must create a request");
        };
        assert!(
            request.display_command.contains(&format!("<U+{hex}>")),
            "U+{hex} must be escaped to a visible marker, got {:?}",
            request.display_command
        );
    }
}

/// The classifier's `RiskReason`s must actually reach the caller, and must
/// still be readable from a stored `ApprovalRequest` after `ReceiveOutcome`
/// itself is gone (response 114 Recommended 2, relocated onto
/// `ApprovalRequest.risk_reasons` per response 115 Q2 -- carrying reasons
/// only on the transient `ReceiveOutcome` displaced the original
/// write-only failure by one hop rather than resolving it).
#[test]
fn approval_request_carries_the_classifiers_reasons() {
    let command_proposal = proposal("proposal-1", &["sudo", "ls"], PROJECT_ROOT);
    let mut coordinator = ApprovalCoordinator::new();
    let (ReceiveOutcome::Created { request }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(
        request.risk_reasons,
        vec![crate::approval::RiskReason::PrivilegeElevation]
    );
}

// --- PR-021-E2: the CommandDecision round trip ---------------------------

/// The core new property for this slice: a decision must actually be sent
/// back over the connection the proposal arrived on, in the frame shape
/// `approval::channel` writes -- read via a real, connected peer socket,
/// not asserted only against the in-memory `ApprovalRequest`.
#[test]
fn decide_sends_a_real_command_decision_back_over_the_proposals_own_connection() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (ReceiveOutcome::Created { .. }, mut peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };

    let outcome = coordinator.decide(
        &agent_run_id,
        command_proposal.proposal_id(),
        SimpleDecision::ApprovedOnce,
    );
    let DecideOutcome::Decided { sent, .. } = outcome else {
        panic!("must decide");
    };
    sent.expect("sending the decision over a live connection must succeed");

    let frame = read_decision_frame(&mut peer);
    assert_eq!(frame["outcome"], "approved_once");
    assert_eq!(frame["proposal_id"], "proposal-1");
}

/// A replayed `decide` call must not send anything a second time -- a
/// replay is fully inert, not merely non-overwriting. Verified by reading
/// exactly one frame off the real connection and then confirming a second
/// read blocks (times out) rather than returning a second frame.
#[test]
fn a_replayed_decision_does_not_send_a_second_frame() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (ReceiveOutcome::Created { .. }, mut peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };

    let proposal_id = command_proposal.proposal_id().clone();
    let first = coordinator.decide(&agent_run_id, &proposal_id, SimpleDecision::ApprovedOnce);
    assert!(matches!(first, DecideOutcome::Decided { .. }));
    let _ = read_decision_frame(&mut peer);

    let replay = coordinator.decide(&agent_run_id, &proposal_id, SimpleDecision::Rejected);
    assert!(matches!(replay, DecideOutcome::AlreadyDecided(_)));

    peer.set_read_timeout(Some(std::time::Duration::from_millis(100)))
        .expect("set a short read timeout for the check");
    let mut buffer = [0_u8; 1];
    let result = peer.read(&mut buffer);
    assert!(
        result.is_err(),
        "a replayed decision must not send a second frame -- the connection must have \
         nothing further to read"
    );
}

/// `implementation-handoff.md` §7: edit-and-approve must re-run the risk
/// classifier on the *edited* argv, and the audit-facing fields
/// (`display_command`, `risk_level`, `risk_reasons`) must describe what
/// was actually approved, not what was originally proposed. Also confirms
/// the wire decision sent back carries the edited argv, per
/// `CommandDecision`'s own shape requirement for `EditedAndApproved`.
#[test]
fn decide_with_edited_argv_reclassifies_and_sends_the_edited_argv() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    // Proposed as an ordinary, Low-risk command...
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (ReceiveOutcome::Created { request: original }, mut peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
    ) else {
        panic!("must create a request");
    };
    assert_eq!(original.risk_level, RiskLevel::Low);

    // ...but the user edits it, before approving, into something far
    // riskier. The stored request must reflect the EDIT, not the original
    // proposal.
    let edited_argv = vec!["rm".to_string(), "-rf".to_string(), "/etc".to_string()];
    let outcome = coordinator.decide_with_edited_argv(
        &agent_run_id,
        command_proposal.proposal_id(),
        edited_argv.clone(),
        &VerifiedCwd::for_test(PROJECT_ROOT),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
    );
    let DecideOutcome::Decided { request, sent } = outcome else {
        panic!("must decide");
    };
    sent.expect("sending the decision must succeed");
    assert_eq!(request.decision, ApprovalDecision::EditedAndApproved);
    assert_eq!(
        request.risk_level,
        RiskLevel::Destructive,
        "the stored request must be reclassified against the EDITED argv, not the \
         original Low-risk proposal"
    );
    assert_eq!(request.display_command, "rm -rf /etc");

    let frame = read_decision_frame(&mut peer);
    assert_eq!(frame["outcome"], "edited_and_approved");
    assert_eq!(
        frame["edited_argv"],
        serde_json::json!(["rm", "-rf", "/etc"])
    );
}
