use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::agent::VerifiedCwd;
use crate::approval::{
    AcceptedProposal, ApprovalCoordinator, ApprovalQueueLimitScope, ApprovalQueueLimits,
    CommandProposal, DecideOutcome, ReceiveOutcome, SimpleDecision, classify,
};
use crate::audit::{
    AuditCoordinator, AuditHealth, AuditPathRequest, AuditPathResolver, AuditStore,
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

/// A real, sqlite-backed `AuditStore` (via the same public
/// `AuditPathResolver`/`AuditStore::open` path production code uses, not a
/// fake writer -- this module cannot reach `audit`'s private test-only
/// fake-writer machinery, since it lives in a sibling module tree; that
/// machinery is what `audit::tests::integration` uses instead to prove the
/// required-vs-best-effort distinction this module's tests take as given).
struct TestAudit {
    store: AuditStore,
    health: AuditHealth,
}

impl TestAudit {
    fn new(name: &str) -> Self {
        let state_root =
            std::env::temp_dir().join(format!("approval-audit-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&state_root).expect("create temp audit state root");
        let state_root = state_root
            .canonicalize()
            .expect("canonicalize temp audit state root");
        let storage_path = AuditPathResolver
            .resolve(AuditPathRequest::new(state_root, Vec::new()))
            .expect("resolve audit storage path");
        let store = AuditStore::open(storage_path).expect("open a real audit store");
        Self {
            store,
            health: AuditHealth::default(),
        }
    }

    fn coordinator(&mut self) -> AuditCoordinator<'_> {
        AuditCoordinator::new(&mut self.store, &mut self.health)
    }
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
    audit: &mut AuditCoordinator,
) -> (ReceiveOutcome, UnixStream) {
    receive_with_limits(
        coordinator,
        agent_run_id,
        verified_cwd,
        command_proposal,
        ApprovalQueueLimits::default(),
        audit,
    )
}

/// RFC-022 PR-022-E: [`receive`] with an explicit
/// [`ApprovalQueueLimits`] -- `receive` itself passes `::default()`
/// (both bounds `None`, i.e. unlimited), which is what every
/// pre-existing test in this file wants; only the queue-limit tests
/// need this fuller form.
fn receive_with_limits(
    coordinator: &mut ApprovalCoordinator,
    agent_run_id: &AgentRunId,
    verified_cwd: &str,
    command_proposal: &CommandProposal,
    limits: ApprovalQueueLimits,
    audit: &mut AuditCoordinator,
) -> (ReceiveOutcome, UnixStream) {
    let (accepted, peer) = AcceptedProposal::for_test(command_proposal.clone());
    let outcome = coordinator.receive_proposal(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        &VerifiedCwd::for_test(verified_cwd),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        accepted,
        limits,
        audit,
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
    let mut test_audit = TestAudit::new("laundering");

    let first = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &first,
        &mut test_audit.coordinator(),
    ) else {
        panic!("first receipt of a proposal id must create a request");
    };
    let proposal_id = first.proposal_id().clone();
    let decided = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
    );
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
        &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("cross-run");

    let (outcome_a, _peer_a) = receive(
        &mut coordinator,
        &run_a,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome_a, ReceiveOutcome::Created { .. }));

    let (outcome_b, _peer_b) = receive(
        &mut coordinator,
        &run_b,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    );
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
    let mut test_audit = TestAudit::new("single-use");
    let (
        ReceiveOutcome::Created {
            request: created, ..
        },
        _peer,
    ) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    )
    else {
        panic!("must create a request");
    };
    let proposal_id = command_proposal.proposal_id().clone();

    let first = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
    );
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
    let replay = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::Rejected,
        &mut test_audit.coordinator(),
    );
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
    let mut test_audit = TestAudit::new("not-found");

    let outcome = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, DecideOutcome::NotFound));
}

/// RFC-022 PR-022-E ("the arrival model"): `is_still_answerable`'s own
/// two states, isolated from the full real-adapter-process integration
/// test (`approval::tests::reference_adapter`) -- a synthetic socket
/// pair is enough to prove the coordinator asks the right question of
/// the right connection, without paying for a spawned process on every
/// run. `_peer` dropped (not merely left open) closes its end for real:
/// `UnixStream`'s `Drop` calls `close(2)`, which is what an exited
/// adapter process's own kernel-level teardown does too -- see
/// `approval::tests::reference_adapter::deciding_a_proposal_whose_real_adapter_process_has_already_exited_is_undeliverable`
/// for the real-process version of this same property.
#[test]
fn is_still_answerable_reflects_the_real_connection_state() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let proposal_id = command_proposal.proposal_id().clone();
    let mut test_audit = TestAudit::new("still-answerable");

    let (outcome, peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, ReceiveOutcome::Created { .. }));
    assert!(
        coordinator.is_still_answerable(&agent_run_id, &proposal_id),
        "a freshly received request with its peer still connected must be answerable"
    );

    drop(peer);
    assert!(
        !coordinator.is_still_answerable(&agent_run_id, &proposal_id),
        "once the peer closes its end, the same request must no longer be answerable"
    );
}

/// The other two ways a request can stop being "still answerable" that
/// have nothing to do with the connection at all: it was never received,
/// or it already has a decision. `is_still_answerable` folds both into
/// `false` rather than a bool that only means one narrow thing (see its
/// own doc comment).
#[test]
fn is_still_answerable_is_false_for_unknown_and_already_decided_requests() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let proposal_id = command_proposal.proposal_id().clone();
    let mut test_audit = TestAudit::new("still-answerable-negative");

    assert!(
        !coordinator.is_still_answerable(&agent_run_id, &proposal_id),
        "a request that was never received cannot be answerable"
    );

    let (outcome, _peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, ReceiveOutcome::Created { .. }));

    let decide_outcome = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::Rejected,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(decide_outcome, DecideOutcome::Decided { .. }));
    assert!(
        !coordinator.is_still_answerable(&agent_run_id, &proposal_id),
        "an already-decided request is not still answerable, even though its connection \
         is still open"
    );
}

/// RFC-022 PR-022-E ("the arrival model"), response 224: "a looping
/// adapter must exhaust its own budget, not starve another agent's
/// proposals." Two live proposals admitted under a limit of two; a third
/// refused with the real limit named. **Live only**: expiring one of the
/// two (dropping its peer) frees a slot for a new one, proving the bound
/// counts open connections, not history -- the fd-exhaustion rationale
/// response 224 gave for why this bound exists at all.
#[test]
fn agent_run_queue_limit_is_enforced_and_only_counts_live_entries() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let mut test_audit = TestAudit::new("agent-run-queue-limit");
    let limits = ApprovalQueueLimits {
        per_agent_run: Some(2),
        per_project: None,
    };

    let first = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (outcome, first_peer) = receive_with_limits(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &first,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, ReceiveOutcome::Created { .. }));

    let second = proposal("proposal-2", &["git", "diff"], PROJECT_ROOT);
    let (outcome, _second_peer) = receive_with_limits(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &second,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, ReceiveOutcome::Created { .. }));

    let third = proposal("proposal-3", &["git", "log"], PROJECT_ROOT);
    let (outcome, _third_peer) = receive_with_limits(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &third,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(
        matches!(
            outcome,
            ReceiveOutcome::QueueLimitExceeded {
                scope: ApprovalQueueLimitScope::PerAgentRun,
                limit: 2
            }
        ),
        "a third live proposal under a limit of two must be refused, naming the real limit: \
         {outcome:?}"
    );

    // Expire the first proposal -- its slot must now be free.
    drop(first_peer);
    let fourth = proposal("proposal-4", &["git", "log", "-1"], PROJECT_ROOT);
    let (outcome, _fourth_peer) = receive_with_limits(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &fourth,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(
        matches!(outcome, ReceiveOutcome::Created { .. }),
        "an expired entry must not continue occupying the live budget: {outcome:?}"
    );
}

/// The project-wide half of the same requirement -- one project, two
/// different `AgentRun`s, a per-project ceiling lower than what either
/// run's own per-run budget alone would allow.
#[test]
fn project_wide_queue_limit_is_enforced_across_agent_runs() {
    let mut coordinator = ApprovalCoordinator::new();
    let run_a = AgentRunId::for_test(1);
    let run_b = AgentRunId::for_test(2);
    let mut test_audit = TestAudit::new("project-queue-limit");
    let limits = ApprovalQueueLimits {
        per_agent_run: None,
        per_project: Some(2),
    };

    let first = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (outcome, _peer) = receive_with_limits(
        &mut coordinator,
        &run_a,
        PROJECT_ROOT,
        &first,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, ReceiveOutcome::Created { .. }));

    let second = proposal("proposal-2", &["git", "diff"], PROJECT_ROOT);
    let (outcome, _peer) = receive_with_limits(
        &mut coordinator,
        &run_b,
        PROJECT_ROOT,
        &second,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(
        matches!(outcome, ReceiveOutcome::Created { .. }),
        "the project-wide budget is shared across agent runs, not per-run: {outcome:?}"
    );

    let third = proposal("proposal-3", &["git", "log"], PROJECT_ROOT);
    let (outcome, _peer) = receive_with_limits(
        &mut coordinator,
        &run_a,
        PROJECT_ROOT,
        &third,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(
        matches!(
            outcome,
            ReceiveOutcome::QueueLimitExceeded {
                scope: ApprovalQueueLimitScope::PerProject,
                limit: 2
            }
        ),
        "a third live proposal for the project must be refused regardless of which run sent \
         it: {outcome:?}"
    );
}

/// Response 224's required guard: a flat, `AgentRunId`-keyed coordinator
/// means cross-project data lives in one structure, so nothing about
/// this data model may let one project's queue pressure affect another's.
/// Project A held at its own per-project ceiling; project B's proposal
/// (a different `ProjectId`, arriving on a run of its own) must still be
/// admitted normally.
#[test]
fn queue_limits_do_not_cross_project_boundaries() {
    let mut coordinator = ApprovalCoordinator::new();
    let run_in_project_a = AgentRunId::for_test(1);
    let run_in_project_b = AgentRunId::for_test(2);
    let mut test_audit = TestAudit::new("cross-project-queue-limit");
    let limits = ApprovalQueueLimits {
        per_agent_run: None,
        per_project: Some(1),
    };

    let (accepted_a, _peer_a) =
        AcceptedProposal::for_test(proposal("proposal-a-1", &["git", "status"], PROJECT_ROOT));
    let outcome_a = coordinator.receive_proposal(
        ProjectId::for_test(1),
        run_in_project_a,
        &VerifiedCwd::for_test(PROJECT_ROOT),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        accepted_a,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome_a, ReceiveOutcome::Created { .. }));

    // Project A is now at its own limit of 1. Project B, a different
    // `ProjectId`, must be unaffected.
    let (accepted_b, _peer_b) =
        AcceptedProposal::for_test(proposal("proposal-b-1", &["git", "status"], PROJECT_ROOT));
    let outcome_b = coordinator.receive_proposal(
        ProjectId::for_test(2),
        run_in_project_b,
        &VerifiedCwd::for_test(PROJECT_ROOT),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        accepted_b,
        limits,
        &mut test_audit.coordinator(),
    );
    assert!(
        matches!(outcome_b, ReceiveOutcome::Created { .. }),
        "project B's proposal must not be refused by project A's own queue pressure: \
         {outcome_b:?}"
    );
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
    let mut test_audit = TestAudit::new("cwd-external");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("cwd-internal");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("effects-hint");
    let (
        ReceiveOutcome::Created {
            request: request_with_hint,
            ..
        },
        _peer1,
    ) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &with_hint,
        &mut test_audit.coordinator(),
    )
    else {
        panic!("must create a request");
    };
    let (
        ReceiveOutcome::Created {
            request: request_without_hint,
            ..
        },
        _peer2,
    ) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &without_hint,
        &mut test_audit.coordinator(),
    )
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
    let mut test_audit = TestAudit::new("display-1");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };
    assert_eq!(request.display_command, "git commit -m 'fix; rm -rf /etc'");
}

#[test]
fn display_command_quotes_an_entry_containing_a_space_so_it_does_not_read_as_two_arguments() {
    let command_proposal = proposal("proposal-1", &["rm", "-rf", "my documents"], PROJECT_ROOT);
    let mut coordinator = ApprovalCoordinator::new();
    let mut test_audit = TestAudit::new("display-2");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };
    assert_eq!(request.display_command, "rm -rf 'my documents'");
}

#[test]
fn display_command_escapes_an_embedded_newline_to_a_visible_marker() {
    let command_proposal = proposal("proposal-1", &["echo", "safe\nrm -rf /etc"], PROJECT_ROOT);
    let mut coordinator = ApprovalCoordinator::new();
    let mut test_audit = TestAudit::new("display-3");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("display-4");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("display-5");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
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
        let mut test_audit = TestAudit::new(&format!("display-probe-{index}"));
        let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
            &mut coordinator,
            &AgentRunId::for_test(index as u64),
            PROJECT_ROOT,
            &command_proposal,
            &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("reasons");
    let (ReceiveOutcome::Created { request, .. }, _peer) = receive(
        &mut coordinator,
        &AgentRunId::for_test(1),
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("round-trip");
    let (ReceiveOutcome::Created { .. }, mut peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };

    let outcome = coordinator.decide(
        &agent_run_id,
        command_proposal.proposal_id(),
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
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
    let mut test_audit = TestAudit::new("no-replay-send");
    let (ReceiveOutcome::Created { .. }, mut peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };

    let proposal_id = command_proposal.proposal_id().clone();
    let first = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(first, DecideOutcome::Decided { .. }));
    let _ = read_decision_frame(&mut peer);

    let replay = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::Rejected,
        &mut test_audit.coordinator(),
    );
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
    let mut test_audit = TestAudit::new("edit-and-approve");
    // Proposed as an ordinary, Low-risk command...
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let (
        ReceiveOutcome::Created {
            request: original, ..
        },
        mut peer,
    ) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    )
    else {
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
        &mut test_audit.coordinator(),
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

// --- PR-021-E2: audit-family wiring ---------------------------------------

/// The `command_request`/`command_approve` audit records must actually
/// land in a real durable store, in the shape the schema requires --
/// queried back after the fact, not just asserted from the in-memory
/// `AuditObservationStatus`.
#[test]
fn receive_and_approve_persist_the_expected_audit_records() {
    let mut coordinator = ApprovalCoordinator::new();
    // A real UUID-shaped id, not `AgentRunId::for_test` -- this test
    // queries a real store afterward, and `from_persisted` (the decode
    // path every query row goes through) requires the genuine
    // `<prefix>-<uuid>` shape `for_test`'s short, sequence-based ids do
    // not have.
    let agent_run_id = AgentRunId::new_uuid();
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let mut test_audit = TestAudit::new("audit-shape");

    let (
        ReceiveOutcome::Created {
            command_request_audit,
            ..
        },
        _peer,
    ) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    )
    else {
        panic!("must create a request");
    };
    assert_eq!(
        command_request_audit,
        crate::audit::AuditObservationStatus::Persisted
    );

    let outcome = coordinator.decide(
        &agent_run_id,
        command_proposal.proposal_id(),
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, DecideOutcome::Decided { .. }));

    let records = test_audit
        .store
        .query(&crate::audit::AuditQuery::latest(10))
        .expect("query the real audit store")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .collect::<Vec<_>>();

    // Applied, then Authorized, then Requested -- latest-first.
    assert_eq!(records.len(), 3);
    assert_eq!(
        records[0].action_kind,
        crate::audit::AuditActionKind::CommandApprove
    );
    assert_eq!(records[0].outcome, crate::audit::AuditOutcome::Applied);
    assert_eq!(
        records[1].action_kind,
        crate::audit::AuditActionKind::CommandApprove
    );
    assert_eq!(records[1].outcome, crate::audit::AuditOutcome::Authorized);
    assert_eq!(records[0].operation_id, records[1].operation_id);
    assert_eq!(
        records[2].action_kind,
        crate::audit::AuditActionKind::CommandRequest
    );
    assert_eq!(records[2].outcome, crate::audit::AuditOutcome::Requested);
    assert!(records.iter().all(|record| {
        record.family == crate::audit::AuditEventFamily::CommandApproval
            && record.approval_id.is_some()
            && record.risk_level == Some(crate::audit::AuditRiskLevel::Low)
            && record.terminal_id.is_none()
            && record.subject_kind.is_none()
    }));
}

/// **The sentinel privacy test.** No command text, argv, cwd, or intent
/// text may appear anywhere in the durable audit store -- including the
/// quoted, escaped `display_command` this slice constructs specifically
/// for human display. A proposal built from unmistakable sentinel
/// strings is received, approved, rejected (a second one), and
/// edited-and-approved (a third), and the entire raw store file plus
/// every queried record's `Debug` output is checked for every sentinel.
#[test]
fn sentinel_command_text_never_reaches_the_durable_audit_store() {
    const SENTINEL_ARG: &str = "PRIVATE-COMMAND-SENTINEL-4f8b2c";
    const SENTINEL_CWD: &str = "/home/user/PRIVATE-CWD-SENTINEL-9a1e7d";

    let mut coordinator = ApprovalCoordinator::new();
    let mut test_audit = TestAudit::new("sentinel-privacy");
    // Real UUID-shaped ids, not `AgentRunId::for_test` -- this test reads
    // the raw store file back, and `for_test`'s short sequence-based ids
    // cannot round-trip through `from_persisted`'s UUID-suffix check.
    let run_approved = AgentRunId::new_uuid();
    let run_rejected = AgentRunId::new_uuid();
    let run_edited = AgentRunId::new_uuid();

    // Approved.
    let approved_proposal = proposal(
        "proposal-approved",
        &["echo", SENTINEL_ARG, "approved-branch"],
        SENTINEL_CWD,
    );
    let (ReceiveOutcome::Created { .. }, _peer1) = receive(
        &mut coordinator,
        &run_approved,
        SENTINEL_CWD,
        &approved_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };
    coordinator.decide(
        &run_approved,
        approved_proposal.proposal_id(),
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
    );

    // Rejected.
    let rejected_proposal = proposal(
        "proposal-rejected",
        &["echo", SENTINEL_ARG, "rejected-branch"],
        SENTINEL_CWD,
    );
    let (ReceiveOutcome::Created { .. }, _peer2) = receive(
        &mut coordinator,
        &run_rejected,
        SENTINEL_CWD,
        &rejected_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };
    coordinator.decide(
        &run_rejected,
        rejected_proposal.proposal_id(),
        SimpleDecision::Rejected,
        &mut test_audit.coordinator(),
    );

    // Edited and approved -- both the original AND the edited argv carry
    // (different) sentinels, so the sentinel test covers both.
    let edited_proposal = proposal(
        "proposal-edited",
        &["echo", SENTINEL_ARG, "original-branch"],
        SENTINEL_CWD,
    );
    let (ReceiveOutcome::Created { .. }, _peer3) = receive(
        &mut coordinator,
        &run_edited,
        SENTINEL_CWD,
        &edited_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };
    coordinator.decide_with_edited_argv(
        &run_edited,
        edited_proposal.proposal_id(),
        vec![
            "echo".to_string(),
            format!("{SENTINEL_ARG}-EDITED"),
            "edited-branch".to_string(),
        ],
        &VerifiedCwd::for_test(SENTINEL_CWD),
        Path::new(SENTINEL_CWD),
        Path::new(STATE_ROOT),
        &mut test_audit.coordinator(),
    );

    let records = test_audit
        .store
        .query(&crate::audit::AuditQuery::latest(20))
        .expect("query the real audit store")
        .records;
    assert!(
        records.len() >= 5,
        "expected at least command_request + authorize + applied for each of three \
         proposals (minus reject's single write), got {}",
        records.len()
    );
    let debug_dump = format!("{records:?}");
    assert!(!debug_dump.contains(SENTINEL_ARG));
    assert!(!debug_dump.contains(SENTINEL_CWD));
    assert!(!debug_dump.contains("original-branch"));
    assert!(!debug_dump.contains("edited-branch"));
    assert!(!debug_dump.contains("rejected-branch"));

    // Also check the raw on-disk file, not just the typed query result --
    // the query path re-parses the store's own encoding, so this is an
    // independent check that nothing leaked into the bytes on disk.
    //
    // Post-closeout defect 1 (recorded 2026-08-10): the store is still
    // open here and runs in WAL mode, so records just written live in
    // `audit.sqlite3-wal` until the connection closes and SQLite
    // checkpoints -- reading `database_file()` on an open store scans a
    // 4 KiB header page holding none of them, the exact blind spot
    // response 152 found in RFC-017 PR-017-F's first sentinel. Fixed the
    // same way: capture the directory, drop the store so it checkpoints,
    // scan every file rather than only the database file, and assert a
    // positive control so the negative assertions below can't pass
    // merely because nothing was read.
    let audit_dir = test_audit.store.storage_path().audit_dir().to_path_buf();
    drop(test_audit);

    let raw_text = read_every_file_in_dir(&audit_dir);
    assert!(
        raw_text.contains(run_approved.as_str()),
        "the scan must reach a real, persisted field -- otherwise the sentinel assertions \
         below pass merely because nothing was read at all"
    );
    assert!(!raw_text.contains(SENTINEL_ARG));
    assert!(!raw_text.contains(SENTINEL_CWD));
}

/// Recursively reads every file under `dir` into one string, the same
/// shape RFC-018 PR-018-D's sentinel test uses -- robust to SQLite's
/// sidecar set (`-wal`, `-shm`) changing, unlike reading a single named
/// file.
fn read_every_file_in_dir(dir: &Path) -> String {
    let mut contents = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                contents.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
    }
    contents
}

/// Response 116 Required 2: a proposal's claimed `cwd` differing from
/// `verified_cwd` is now compared and recorded as a best-effort
/// `command_cwd_mismatch` audit anomaly -- converted from response 114/115's
/// known-limitation fixture (which documented the absence of this check)
/// to assert its presence, per response 116's explicit instruction.
#[test]
fn a_cwd_mismatch_is_compared_and_recorded_as_a_best_effort_audit_anomaly() {
    let mut coordinator = ApprovalCoordinator::new();
    // A real UUID-shaped id -- this test queries a real store afterward.
    let agent_run_id = AgentRunId::new_uuid();
    // The proposal claims cwd = "/", but the caller supplies the real,
    // separately-sourced PROJECT_ROOT as verified_cwd -- a genuine
    // mismatch between the two.
    let command_proposal = proposal("proposal-1", &["git", "status"], "/");
    let mut test_audit = TestAudit::new("cwd-mismatch");

    let (
        ReceiveOutcome::Created {
            request,
            cwd_mismatch_audit,
            ..
        },
        _peer,
    ) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    )
    else {
        panic!("must create a request");
    };
    // The request is still classified and stored using verified_cwd
    // (correct, per this module's core guarantee -- never the claim,
    // point 1 of the module doc) -- the mismatch is a *separate*,
    // observability-only signal alongside that.
    assert_eq!(request.cwd, Path::new(PROJECT_ROOT));
    assert_ne!(
        command_proposal.cwd(),
        request.cwd,
        "test precondition: the proposal's claim and the verified cwd must actually differ"
    );
    assert_eq!(
        cwd_mismatch_audit,
        Some(crate::audit::AuditObservationStatus::Persisted)
    );

    let records = test_audit
        .store
        .query(&crate::audit::AuditQuery::latest(10))
        .expect("query the real audit store")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .collect::<Vec<_>>();
    let anomaly = records
        .iter()
        .find(|record| record.action_kind == crate::audit::AuditActionKind::CommandCwdMismatch)
        .expect("a command_cwd_mismatch anomaly record must exist");
    assert_eq!(anomaly.outcome, crate::audit::AuditOutcome::Anomaly);
    assert_eq!(
        anomaly.family,
        crate::audit::AuditEventFamily::CommandApproval
    );
    assert_eq!(anomaly.operation_id, None);
    assert_eq!(anomaly.approval_id, Some(request.id.clone()));
}

/// A proposal whose claimed `cwd` genuinely matches `verified_cwd` must
/// **not** produce an anomaly record -- otherwise every ordinary proposal
/// would be flagged, and the signal would be useless.
#[test]
fn a_matching_cwd_produces_no_mismatch_anomaly() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let mut test_audit = TestAudit::new("cwd-match");

    let (
        ReceiveOutcome::Created {
            cwd_mismatch_audit, ..
        },
        _peer,
    ) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    )
    else {
        panic!("must create a request");
    };
    assert_eq!(cwd_mismatch_audit, None);
}

/// Response 116 Required 1's core defect, reproduced and closed: the
/// `Authorized` record for an edit-and-approve must carry the *edited*
/// command's risk level, not the risk level of what was originally
/// proposed -- reclassification now happens before authorization, not
/// after.
#[test]
fn edit_and_approve_authorized_record_carries_the_edited_risk_level_not_the_proposed_one() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::new_uuid();
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let mut test_audit = TestAudit::new("edit-and-approve-audit-level");

    let (ReceiveOutcome::Created { .. }, _peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };

    let outcome = coordinator.decide_with_edited_argv(
        &agent_run_id,
        command_proposal.proposal_id(),
        vec!["rm".to_string(), "-rf".to_string(), "/etc".to_string()],
        &VerifiedCwd::for_test(PROJECT_ROOT),
        Path::new(PROJECT_ROOT),
        Path::new(STATE_ROOT),
        &mut test_audit.coordinator(),
    );
    assert!(matches!(outcome, DecideOutcome::Decided { .. }));

    let records = test_audit
        .store
        .query(&crate::audit::AuditQuery::latest(10))
        .expect("query the real audit store")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .collect::<Vec<_>>();
    let authorized = records
        .iter()
        .find(|record| {
            record.action_kind == crate::audit::AuditActionKind::CommandEditAndApprove
                && record.outcome == crate::audit::AuditOutcome::Authorized
        })
        .expect("an Authorized command_edit_and_approve record must exist");
    assert_eq!(
        authorized.risk_level,
        Some(crate::audit::AuditRiskLevel::Destructive),
        "the Authorized record must describe the EDITED command (rm -rf /etc), not the \
         originally-proposed Low-risk `git status`"
    );
}

// Companion property to the fixture above: on `AuditBlocked`, the stored
// request must remain describing the *original* proposal, never a
// half-applied edit. That specific scenario (a real `AuditCoordinator`
// failing authorization mid-edit-and-approve) cannot be constructed from
// this module alone -- `approval::tests` has no path to a fake failing
// `AuditRecordWriter` (that trait, and every type implementing it, live in
// `audit::integration`/`audit::tests::integration`, neither of which is
// reachable here; see this module's doc). It is proven instead in
// `audit/tests/integration.rs`'s
// `edit_and_approve_audit_block_leaves_the_stored_request_describing_the_original_proposal`.

/// RFC-021's required follow-up at closeout: the fail-closed matrix's
/// headline guarantee, "no response -> pending indefinitely; **no
/// timeout approves**," was verified structurally at PR-021-F
/// (`coordinator.rs` contains no `Duration`/`Instant`/`elapsed`/
/// `SystemTime`/`now()`, reads no timestamp anywhere) but had no
/// regression test -- the guarantee was held by the *absence* of code,
/// which does not survive a future edit visibly (the same class as a
/// fixture corpus that cannot fail, response 110). Someone will one day
/// add a timeout for a good-sounding reason -- a stuck-proposal cleanup,
/// a UI hint -- and nothing will object without this test.
///
/// Does not sleep for a meaningful duration: the point is not to wait
/// out a real timeout (there is none to wait out) but to prove no
/// time-based path exists at all. A short elapsed interval plus the
/// structural assertions below is what the README's spec calls for.
#[test]
fn no_timeout_approves_a_pending_proposal_regardless_of_elapsed_time() {
    let mut coordinator = ApprovalCoordinator::new();
    let agent_run_id = AgentRunId::for_test(1);
    let command_proposal = proposal("proposal-1", &["git", "status"], PROJECT_ROOT);
    let mut test_audit = TestAudit::new("no-timeout-approves");

    let (ReceiveOutcome::Created { request, .. }, mut peer) = receive(
        &mut coordinator,
        &agent_run_id,
        PROJECT_ROOT,
        &command_proposal,
        &mut test_audit.coordinator(),
    ) else {
        panic!("must create a request");
    };
    assert_eq!(request.decision, ApprovalDecision::Pending);

    // A short elapsed interval -- not waiting out a timeout, since there
    // is none; only demonstrating that time passing has no effect.
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 1. Still Pending, unchanged, via a fresh lookup rather than the
    //    stale `request` value above.
    let still_pending = coordinator
        .find(&agent_run_id, command_proposal.proposal_id())
        .expect("the request must still exist after elapsed time");
    assert_eq!(still_pending.decision, ApprovalDecision::Pending);
    assert_eq!(still_pending.display_command, request.display_command);
    assert_eq!(still_pending.risk_level, request.risk_level);

    // 2. No decision was sent over the connection -- nothing to read,
    //    not even after elapsed time, and no code path could have
    //    produced one without an explicit `decide` call.
    peer.set_read_timeout(Some(std::time::Duration::from_millis(100)))
        .expect("set a short read timeout for the check");
    let mut buffer = [0_u8; 1];
    let result = peer.read(&mut buffer);
    assert!(
        result.is_err(),
        "no decision may be sent for a proposal nobody decided, elapsed time or not"
    );

    // 3. The request was not silently abandoned either -- `decide`
    //    afterwards still succeeds normally, exactly as if no time had
    //    passed at all.
    let outcome = coordinator.decide(
        &agent_run_id,
        command_proposal.proposal_id(),
        SimpleDecision::ApprovedOnce,
        &mut test_audit.coordinator(),
    );
    assert!(
        matches!(outcome, DecideOutcome::Decided { .. }),
        "a proposal left pending across elapsed time must still be decidable normally: {outcome:?}"
    );
}
