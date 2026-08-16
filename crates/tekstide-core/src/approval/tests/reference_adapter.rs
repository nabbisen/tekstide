//! RFC-022 PR-022-B: the reference adapter's own review gate.
//!
//! Every test here spawns the *compiled* `reference_adapter` binary
//! (`src/bin/reference_adapter.rs`) as a real child process and drives it
//! against the real, unmodified `ApprovalChannelEndpoint` and
//! `ApprovalCoordinator` -- never a mock, never a reimplementation of
//! either side's own logic. `reference_adapter_binary_path()` locates
//! that exact compiled artifact (see its own doc comment for why this is
//! not simply `env!("CARGO_BIN_EXE_reference_adapter")`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::agent::VerifiedCwd;
use crate::approval::{
    APPROVAL_SOCKET_PATH_ENV_VAR, APPROVAL_TOKEN_ENV_VAR, ApprovalChannelDirectory,
    ApprovalChannelEndpoint, ApprovalChannelErrorReason, ApprovalChannelPathRequest,
    ApprovalChannelPathResolver, ApprovalCoordinator, ApprovalQueueLimits, DecideOutcome,
    ReceiveOutcome, SimpleDecision,
};
use crate::audit::{
    AuditCoordinator, AuditHealth, AuditPathRequest, AuditPathResolver, AuditStore,
};
use crate::domain::AgentRunId;
use crate::project::ProjectId;

const PROJECT_ROOT: &str = "/home/user/project";
/// Short by design -- see `unique_temp_dir`'s doc comment.
const STATE_ROOT_LABEL_PREFIX: &str = "ta";

/// A real, sqlite-backed `AuditStore` -- mirrors `coordinator.rs`'s own
/// `TestAudit` (that one is private to a sibling test file, so this is a
/// duplicate rather than a reuse; both construct the same real, public
/// path production code uses).
struct TestAudit {
    store: AuditStore,
    health: AuditHealth,
}

impl TestAudit {
    fn new(name: &str) -> Self {
        let state_root = unique_temp_dir(&format!("audit-{name}"));
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

/// Deliberately short: a Unix `sun_path` is bounded (~107 usable bytes,
/// `ApprovalChannelEndpoint::bind`'s own `max_socket_path_len`), and the
/// full socket path is `<this>/approval/<agent-run-id>.sock` --
/// `AgentRunId::for_test`'s own format (`agent-run-` plus 12 hex digits)
/// already spends 22 of that budget before this directory name is even
/// considered. A timestamp-plus-nanoseconds label (tried first) blew the
/// budget outright; pid-plus-counter is unique enough for one test binary
/// run and comfortably shorter.
fn unique_temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "{STATE_ROOT_LABEL_PREFIX}-{label}-{}-{sequence}",
        std::process::id()
    ))
}

struct TestChannel {
    directory: ApprovalChannelDirectory,
    base: PathBuf,
}

impl TestChannel {
    fn new(label: &str) -> Self {
        let base = unique_temp_dir(label);
        std::fs::create_dir_all(&base).expect("create temp channel state root");
        let directory = ApprovalChannelPathResolver
            .resolve(ApprovalChannelPathRequest::new(base.clone(), Vec::new()))
            .expect("resolve approval channel directory");
        Self { directory, base }
    }

    fn bind(&self, agent_run_id: &AgentRunId) -> (ApprovalChannelEndpoint, String, PathBuf) {
        let (endpoint, raw_token) = ApprovalChannelEndpoint::bind(&self.directory, agent_run_id)
            .expect("bind a real approval channel endpoint");
        let socket_path = self.directory.socket_path(agent_run_id);
        (endpoint, raw_token, socket_path)
    }
}

impl Drop for TestChannel {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// `CARGO_BIN_EXE_<name>` is only guaranteed for genuine integration test
/// targets (`tests/*.rs`), not for a lib's own `#[cfg(test)]` unit tests
/// like this module -- checked directly (it is simply absent here, not a
/// configuration mistake). Falls back to deriving the path from this test
/// binary's own location: Cargo places every crate's test binary at
/// `target/<profile>/deps/<crate>-<hash>` and every `[[bin]]` target
/// (including `reference_adapter`) as a sibling of `deps/` itself, at
/// `target/<profile>/<bin-name>` -- so `current_exe()`'s grandparent
/// directory is exactly where to look.
fn reference_adapter_binary_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_reference_adapter") {
        return PathBuf::from(path);
    }
    let test_exe = std::env::current_exe().expect("current_exe should resolve for a running test");
    let profile_dir = test_exe
        .parent() // .../target/<profile>/deps
        .and_then(Path::parent) // .../target/<profile>
        .expect("test binary should live under target/<profile>/deps");
    let candidate = profile_dir.join("reference_adapter");
    assert!(
        candidate.is_file(),
        "expected the reference_adapter binary at {}; the [[bin]] target may not have built",
        candidate.display()
    );
    candidate
}

/// Spawns the real compiled adapter binary. `token`/`socket_path`:
/// `None` omits the corresponding env var entirely (the missing-token
/// and missing-socket-path cases); `Some` sets it to exactly that value,
/// whether or not it is the real one (the wrong-token case reuses this
/// same helper). Both travel through the environment, matching PR-022-C's
/// production spawn path exactly -- this binary takes neither as a CLI
/// argument, so a test spawning it directly and the production
/// `spawn_adapter` (`runtime::terminal::launch`) exercise the identical
/// contract.
fn spawn_adapter(
    socket_path: Option<&PathBuf>,
    token: Option<&str>,
    argv: &[&str],
) -> std::process::Child {
    let mut command = Command::new(reference_adapter_binary_path());
    command
        .args(argv)
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(token) = token {
        command.env(APPROVAL_TOKEN_ENV_VAR, token);
    }
    if let Some(socket_path) = socket_path {
        command.env(APPROVAL_SOCKET_PATH_ENV_VAR, socket_path);
    }
    command
        .spawn()
        .expect("reference_adapter binary should spawn")
}

fn finish(child: std::process::Child) -> Output {
    child
        .wait_with_output()
        .expect("reference_adapter binary should exit")
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The full happy path: a real spawned adapter process, a real accepted
/// connection, a real coordinator classifying and deciding, and the
/// decision travelling back over the same real socket to a process that
/// is not this test -- proven by exit code and by what the adapter itself
/// printed after parsing the decision it received, not by inspecting
/// anything on the server side alone.
#[test]
fn a_real_adapter_process_completes_a_full_approve_round_trip() {
    let channel = TestChannel::new("approve");
    let agent_run_id = AgentRunId::for_test(1);
    let (endpoint, raw_token, socket_path) = channel.bind(&agent_run_id);

    let child = spawn_adapter(Some(&socket_path), Some(&raw_token), &["git", "status"]);

    let accepted = endpoint
        .accept_proposal()
        .expect("the real adapter's proposal should authenticate and parse");

    let mut coordinator = ApprovalCoordinator::new();
    let mut audit = TestAudit::new("approve");
    let proposal_id = accepted.proposal.proposal_id().clone();
    let outcome = coordinator.receive_proposal(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        &VerifiedCwd::for_test(PROJECT_ROOT),
        std::path::Path::new(PROJECT_ROOT),
        channel.directory.state_root(),
        accepted,
        ApprovalQueueLimits::default(),
        &mut audit.coordinator(),
    );
    assert!(
        matches!(outcome, ReceiveOutcome::Created { .. }),
        "a first-time proposal from a real adapter should be accepted as Created: {outcome:?}"
    );

    let decide_outcome = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::ApprovedOnce,
        &mut audit.coordinator(),
    );
    let DecideOutcome::Decided { sent, .. } = decide_outcome else {
        panic!("deciding a freshly-created proposal should reach Decided: {decide_outcome:?}");
    };
    sent.expect("sending the decision back over the real connection should succeed");

    let output = finish(child);
    assert_eq!(
        output.status.code(),
        Some(0),
        "an approved_once decision should exit 0; stdout: {}, stderr: {}",
        stdout_text(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout_text(&output).contains("approved_once"),
        "the adapter should report the decision it actually received: {}",
        stdout_text(&output)
    );
}

/// The other half of "both decisions exercised" -- covering only approve
/// proves the easier one. Same real round trip, `SimpleDecision::Rejected`
/// this time.
#[test]
fn a_real_adapter_process_completes_a_full_reject_round_trip() {
    let channel = TestChannel::new("reject");
    let agent_run_id = AgentRunId::for_test(2);
    let (endpoint, raw_token, socket_path) = channel.bind(&agent_run_id);

    let child = spawn_adapter(
        Some(&socket_path),
        Some(&raw_token),
        &["rm", "-rf", "/nonexistent"],
    );

    let accepted = endpoint
        .accept_proposal()
        .expect("the real adapter's proposal should authenticate and parse");

    let mut coordinator = ApprovalCoordinator::new();
    let mut audit = TestAudit::new("reject");
    let proposal_id = accepted.proposal.proposal_id().clone();
    coordinator.receive_proposal(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        &VerifiedCwd::for_test(PROJECT_ROOT),
        std::path::Path::new(PROJECT_ROOT),
        channel.directory.state_root(),
        accepted,
        ApprovalQueueLimits::default(),
        &mut audit.coordinator(),
    );

    let decide_outcome = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::Rejected,
        &mut audit.coordinator(),
    );
    let DecideOutcome::Decided { sent, .. } = decide_outcome else {
        panic!("deciding a freshly-created proposal should reach Decided: {decide_outcome:?}");
    };
    sent.expect("sending the decision back over the real connection should succeed");

    let output = finish(child);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a rejected decision should exit 1; stdout: {}, stderr: {}",
        stdout_text(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout_text(&output).contains("rejected"),
        "the adapter should report the decision it actually received: {}",
        stdout_text(&output)
    );
}

/// "Behaviour on a missing ... token is defined and tested" -- the
/// adapter must refuse to propose at all rather than connecting without
/// one and letting the socket decide what happens.
#[test]
fn a_real_adapter_process_refuses_to_run_without_a_token() {
    let channel = TestChannel::new("missing-token");
    let agent_run_id = AgentRunId::for_test(3);
    let (_endpoint, _raw_token, socket_path) = channel.bind(&agent_run_id);

    let child = spawn_adapter(Some(&socket_path), None, &["echo", "hi"]);
    let output = finish(child);

    assert_eq!(
        output.status.code(),
        Some(2),
        "a missing TEKSTIDE_APPROVAL_TOKEN must exit with the defined missing-token code, not \
         hang or attempt to connect; stdout: {}, stderr: {}",
        stdout_text(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(APPROVAL_TOKEN_ENV_VAR),
        "the failure message should name the missing variable, not just fail silently"
    );
}

/// The same treatment for the other required variable, added when
/// PR-022-C moved the socket path from a CLI argument to
/// `APPROVAL_SOCKET_PATH_ENV_VAR`: a missing value must be a defined,
/// immediate refusal, not an attempt to connect to nothing.
#[test]
fn a_real_adapter_process_refuses_to_run_without_a_socket_path() {
    let child = spawn_adapter(None, Some(&"t".repeat(64)), &["echo", "hi"]);
    let output = finish(child);

    assert_eq!(
        output.status.code(),
        Some(5),
        "a missing TEKSTIDE_APPROVAL_SOCKET_PATH must exit with the defined \
         missing-socket-path code, not hang or attempt to connect; stdout: {}, stderr: {}",
        stdout_text(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(APPROVAL_SOCKET_PATH_ENV_VAR),
        "the failure message should name the missing variable, not just fail silently"
    );
}

/// "Behaviour on a ... wrong token is defined and tested." The server
/// rejects a bad token by silently dropping the connection (fail-closed
/// without a dialog, per `approval::channel`'s own design) -- so the
/// adapter cannot distinguish this from any other connection failure, and
/// must not hang waiting for a reply that will never come.
#[test]
fn a_real_adapter_process_exits_distinctly_on_a_rejected_token() {
    let channel = TestChannel::new("wrong-token");
    let agent_run_id = AgentRunId::for_test(4);
    let (endpoint, _raw_token, socket_path) = channel.bind(&agent_run_id);

    let child = spawn_adapter(Some(&socket_path), Some(&"w".repeat(64)), &["echo", "hi"]);

    let server_result = endpoint.accept_proposal();
    assert_eq!(
        server_result.err().map(|error| error.reason),
        Some(ApprovalChannelErrorReason::TokenMismatch),
        "the server side should independently observe the wrong token as a real rejection, \
         not a fluke of the client's own behaviour"
    );

    let output = finish(child);
    assert_eq!(
        output.status.code(),
        Some(3),
        "a rejected token must surface as the defined protocol-failure exit code, not a hang \
         or a panic; stdout: {}, stderr: {}",
        stdout_text(&output),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// **RFC-022 PR-022-E ("the arrival model"): "a decision that can no
/// longer be delivered is not recorded as if it were."** The adapter
/// process is killed for real (`SIGKILL`, then reaped) after it has sent
/// its proposal but before any decision is made -- a genuinely exited
/// process, not a synthesised closed socket, matching the gate's own
/// requirement ("prove it against a real adapter that has actually
/// exited"). No 30-second wait for the adapter's own read timeout is
/// needed: killing the process is itself a real exit, and the kernel
/// tears down its end of the socket as part of that, independent of
/// whether the adapter's own timeout would eventually have fired too.
///
/// `decide` must refuse to authorize or record anything: `Undeliverable`,
/// the request still `Pending`, and no `CommandApprove` audit record at
/// any outcome (`Requested` from `receive_proposal` is the only record
/// that should exist).
#[test]
fn deciding_a_proposal_whose_real_adapter_process_has_already_exited_is_undeliverable() {
    let channel = TestChannel::new("undeliverable");
    // A real UUID-shaped id, not `AgentRunId::for_test` -- this test
    // queries a real store afterward, and `from_persisted` (the decode
    // path every query row goes through) requires the genuine
    // `<prefix>-<uuid>` shape `for_test`'s short, sequence-based ids do
    // not have (same reason `coordinator.rs`'s own
    // `receive_and_approve_persist_the_expected_audit_records` uses it).
    let agent_run_id = AgentRunId::new_uuid();
    let (endpoint, raw_token, socket_path) = channel.bind(&agent_run_id);

    let mut child = spawn_adapter(Some(&socket_path), Some(&raw_token), &["rm", "-rf", "/"]);

    let accepted = endpoint
        .accept_proposal()
        .expect("the real adapter's proposal should authenticate and parse");

    // The real process exits here -- not a dropped `UnixStream`, not a
    // half-close simulated from this test's own side.
    child.kill().expect("kill the real adapter process");
    child.wait().expect("reap the killed adapter process");

    let mut coordinator = ApprovalCoordinator::new();
    let mut audit = TestAudit::new("undeliverable");
    let proposal_id = accepted.proposal.proposal_id().clone();
    let receive_outcome = coordinator.receive_proposal(
        ProjectId::for_test(1),
        agent_run_id.clone(),
        &VerifiedCwd::for_test(PROJECT_ROOT),
        std::path::Path::new(PROJECT_ROOT),
        channel.directory.state_root(),
        accepted,
        ApprovalQueueLimits::default(),
        &mut audit.coordinator(),
    );
    assert!(
        matches!(receive_outcome, ReceiveOutcome::Created { .. }),
        "the proposal itself was received before the process exited, and must still be: {receive_outcome:?}"
    );

    let decide_outcome = coordinator.decide(
        &agent_run_id,
        &proposal_id,
        SimpleDecision::ApprovedOnce,
        &mut audit.coordinator(),
    );
    assert!(
        matches!(decide_outcome, DecideOutcome::Undeliverable),
        "deciding a proposal whose real adapter has already exited must refuse, not \
         authorize-then-fail-to-send: {decide_outcome:?}"
    );

    let stored = coordinator
        .find(&agent_run_id, &proposal_id)
        .expect("the request must still exist");
    assert_eq!(
        stored.decision,
        crate::domain::ApprovalDecision::Pending,
        "nobody decided -- recording anything else would be false"
    );

    let records = audit
        .store
        .query(&crate::audit::AuditQuery::latest(10))
        .expect("query the real audit store")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .collect::<Vec<_>>();
    assert!(
        records
            .iter()
            .all(|record| record.action_kind != crate::audit::AuditActionKind::CommandApprove),
        "no CommandApprove record of any outcome may exist for an undeliverable decision -- \
         got: {records:?}"
    );
}

/// Response 229's suggestion: pins the property
/// `a_genuinely_destructive_real_proposal_is_classified_and_promoted_end_to_end`
/// (`crates/tekstide/src/shell/tests.rs`) depends on for its own safety --
/// that this binary only ever *proposes* the argv it is given, never
/// *runs* it. That test hardcodes a real `rm -rf` argv into a wrapper
/// script and trusts nothing here executes it; this is what makes that
/// trust checkable by name rather than by inspection each time. Same
/// source-scan-for-absence shape `crates/tekstide/src/shell/tests.rs`
/// already uses for `no_raw_color_construction_anywhere_in_the_crate`.
///
/// Scans for the concrete Rust APIs that would actually execute a
/// process (`std::process::Command`, the `exec`/`execvp`/`posix_spawn`
/// libc family, `fork`) rather than a broader lexical net -- narrow
/// enough that this test does not fail on unrelated future code (a
/// string that happens to contain "exec", say), wide enough to catch
/// every ordinary way this file could gain a real spawn path. Not a
/// sandbox: a `#[no_mangle]` FFI trick or an `unsafe` raw syscall built
/// by hand would not match any of these substrings. Proportionate to
/// what a reference/demo binary in this codebase would plausibly grow,
/// not to an adversarial rewrite of it.
#[test]
fn reference_adapter_binary_never_executes_the_argv_it_proposes() {
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/reference_adapter.rs");
    let source = std::fs::read_to_string(&source_path)
        .expect("reference_adapter.rs must be readable from tekstide-core's own crate root");

    for forbidden in [
        "std::process::Command",
        "Command::new",
        "process::Command",
        ".exec(",
        "execvp",
        "execv(",
        "execve",
        "execl",
        "posix_spawn",
        "libc::fork",
    ] {
        assert!(
            !source.contains(forbidden),
            "reference_adapter.rs must never execute the argv it proposes -- found a process-\
             spawning call site (`{forbidden}`) that a real GUI test \
             (a_genuinely_destructive_real_proposal_is_classified_and_promoted_end_to_end) \
             trusts does not exist, since it hardcodes a real `rm -rf` argv on the assumption \
             nothing here runs it"
        );
    }
}
