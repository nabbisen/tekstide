use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::approval::{
    ApprovalChannelDirectory, ApprovalChannelEndpoint, ApprovalChannelErrorReason,
    ApprovalChannelPathErrorReason, ApprovalChannelPathRequest, ApprovalChannelPathResolver,
    CommandDecision, DecisionOutcome,
};
use crate::domain::AgentRunId;

/// Deliberately short: a Unix `sun_path` is bounded (~108 bytes on
/// Linux), and this module's own socket paths are `<state_root>/approval/
/// <agent_run_id>.sock` -- a verbose, descriptive temp-directory name here
/// would eat into that budget for no reason relevant to what these tests
/// are actually checking. `name` is intentionally unused in the path
/// itself; kept as a parameter so call sites stay self-documenting.
fn temp_state_root(name: &str) -> PathBuf {
    let _ = name;
    // Includes the process id: without it, a fresh test-binary run's
    // counter restarts at 0 and can collide with leftover directories
    // from a *previous* run that were never cleaned up (these tests
    // intentionally don't scrub `/tmp` afterward), which caused a real,
    // observed flake -- symlink creation failing with `AlreadyExists`
    // because a prior run's leftover directory already occupied the path.
    let dir = std::env::temp_dir().join(format!("t{}-{}", std::process::id(), rand_seed()));
    std::fs::create_dir_all(&dir).expect("create temp state root");
    dir.canonicalize().expect("canonicalize temp state root")
}

// A cheap, non-cryptographic sequence source for unique temp directory
// names within a single test process -- not used for anything security
// relevant, only to avoid different tests colliding on the same path.
fn rand_seed() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn resolve_directory(state_root: &std::path::Path) -> ApprovalChannelDirectory {
    ApprovalChannelPathResolver
        .resolve(ApprovalChannelPathRequest::new(state_root, Vec::new()))
        .expect("path resolution must succeed for a plain temp directory")
}

// --- Path resolution ----------------------------------------------

#[test]
fn resolve_creates_the_approval_directory_under_the_state_root() {
    let state_root = temp_state_root("create");
    let directory = resolve_directory(&state_root);
    assert!(directory.channel_dir().starts_with(&state_root));
}

#[test]
fn resolve_rejects_a_relative_state_root() {
    let result = ApprovalChannelPathResolver.resolve(ApprovalChannelPathRequest::new(
        "relative/state/root",
        Vec::new(),
    ));
    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelPathErrorReason::StateRootNotAbsolute
    );
}

#[test]
fn resolve_rejects_a_symlinked_approval_directory() {
    let state_root = temp_state_root("symlink");
    let elsewhere = temp_state_root("symlink-target");
    let approval_path = state_root.join("approval");
    std::os::unix::fs::symlink(&elsewhere, &approval_path).expect("create symlink");

    let result = ApprovalChannelPathResolver
        .resolve(ApprovalChannelPathRequest::new(&state_root, Vec::new()));
    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelPathErrorReason::ChannelPathIsSymlink
    );
}

#[test]
fn resolve_rejects_when_a_project_root_contains_the_state_root() {
    let state_root = temp_state_root("nested-state");
    // A "project root" that is state_root's own parent -- state_root is
    // therefore inside it.
    let project_root = state_root.parent().unwrap().to_path_buf();

    let result = ApprovalChannelPathResolver.resolve(ApprovalChannelPathRequest::new(
        &state_root,
        vec![project_root],
    ));
    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelPathErrorReason::ProjectContainsChannelState
    );
}

// --- Endpoint lifecycle ---------------------------------------------

#[test]
fn bind_creates_a_socket_with_owner_only_permissions() {
    let state_root = temp_state_root("perms");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");

    let socket_path = directory.socket_path(&agent_run_id);
    let mode = std::fs::metadata(&socket_path)
        .expect("socket file must exist")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "socket file must be owner-only, got {mode:o}");

    drop(endpoint);
}

#[test]
fn dropping_the_endpoint_removes_the_socket_file() {
    let state_root = temp_state_root("cleanup");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let socket_path = directory.socket_path(&agent_run_id);
    assert!(socket_path.exists());

    drop(endpoint);
    assert!(
        !socket_path.exists(),
        "no orphaned socket may remain after the endpoint is dropped"
    );
}

#[test]
fn bind_recovers_from_a_stale_socket_file() {
    let state_root = temp_state_root("stale");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());
    let socket_path = directory.socket_path(&agent_run_id);

    // Simulate a leftover socket file from an ungraceful prior
    // termination: bind a *raw* listener directly at the same path (not
    // through `ApprovalChannelEndpoint`, whose own `Drop` both closes the
    // listener and removes the file) and let its real `Drop` run. That
    // closes the fd -- the kernel now has nothing listening -- while the
    // socket *file* remains on disk, since `std::os::unix::net::
    // UnixListener`'s `Drop` does not remove it. (An earlier version of
    // this test used `mem::forget` on a real `ApprovalChannelEndpoint`,
    // which does not actually close the fd -- the "stale" listener was
    // still live in-process, and the test failed for the wrong reason.)
    std::fs::create_dir_all(directory.channel_dir()).expect("create channel dir for the test");
    {
        let raw_listener =
            std::os::unix::net::UnixListener::bind(&socket_path).expect("raw bind must succeed");
        drop(raw_listener);
    }
    assert!(socket_path.exists(), "precondition: stale file exists");

    let (second_endpoint, _token) = ApprovalChannelEndpoint::bind(&directory, &agent_run_id)
        .expect("second bind must clear the stale file and succeed");
    drop(second_endpoint);
    assert!(
        !socket_path.exists(),
        "the second endpoint's own Drop must clean up after itself"
    );
}

fn valid_proposal_frame(token: &str, proposal_id: &str) -> Vec<u8> {
    let json = serde_json::json!({
        "protocol_version": 1,
        "run_token": token,
        "proposal_id": proposal_id,
        "argv": ["git", "status"],
        "cwd": "/tmp",
        "declared_intent": null,
        "declared_effects": null,
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    frame_with_length_prefix(&bytes)
}

fn frame_with_length_prefix(bytes: &[u8]) -> Vec<u8> {
    let len = u32::try_from(bytes.len()).unwrap();
    let mut framed = len.to_be_bytes().to_vec();
    framed.extend_from_slice(bytes);
    framed
}

#[test]
fn real_proposal_and_decision_round_trip_over_the_socket() {
    let state_root = temp_state_root("roundtrip");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let socket_path = directory.socket_path(&agent_run_id);

    let adapter = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(&socket_path).expect("adapter connect");
        stream
            .write_all(&valid_proposal_frame(&token, "proposal-1"))
            .expect("adapter send proposal");

        let mut len_bytes = [0_u8; 4];
        stream
            .read_exact(&mut len_bytes)
            .expect("adapter read decision length");
        let len = u32::from_be_bytes(len_bytes) as usize;
        let mut buffer = vec![0_u8; len];
        stream
            .read_exact(&mut buffer)
            .expect("adapter read decision body");
        serde_json::from_slice::<serde_json::Value>(&buffer).expect("decision must be valid JSON")
    });

    let mut accepted = endpoint
        .accept_proposal()
        .expect("a correctly-authenticated proposal must be accepted");
    assert_eq!(accepted.proposal.proposal_id().as_str(), "proposal-1");
    assert_eq!(
        accepted.proposal.argv(),
        ["git".to_string(), "status".to_string()]
    );

    let decision = CommandDecision::decode(
        crate::approval::PROTOCOL_VERSION,
        "proposal-1".to_string(),
        DecisionOutcome::ApprovedOnce,
        None,
    )
    .expect("decision must decode");
    accepted
        .send_decision(&decision)
        .expect("sending the decision must succeed");

    let received_decision = adapter.join().expect("adapter thread must not panic");
    assert_eq!(received_decision["outcome"], "approved_once");
    assert_eq!(received_decision["proposal_id"], "proposal-1");
}

#[test]
fn wrong_token_is_rejected_without_a_dialog() {
    let state_root = temp_state_root("wrong-token");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _real_token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let socket_path = directory.socket_path(&agent_run_id);

    let impersonator = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(&socket_path).expect("impersonator connect");
        let wrong_token = "0".repeat(64);
        stream
            .write_all(&valid_proposal_frame(&wrong_token, "proposal-evil"))
            .expect("impersonator send proposal");
    });

    let result = endpoint.accept_proposal();
    impersonator
        .join()
        .expect("impersonator thread must not panic");

    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelErrorReason::TokenMismatch
    );
}

#[test]
fn malformed_frame_is_rejected_not_partially_parsed() {
    let state_root = temp_state_root("malformed");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let socket_path = directory.socket_path(&agent_run_id);

    let sender = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(&socket_path).expect("connect");
        let garbage = b"this is not json".to_vec();
        stream
            .write_all(&frame_with_length_prefix(&garbage))
            .expect("send garbage frame");
    });

    let result = endpoint.accept_proposal();
    sender.join().expect("sender thread must not panic");

    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelErrorReason::MalformedFrame
    );
}

#[test]
fn oversized_declared_length_is_rejected_before_reading_the_body() {
    let state_root = temp_state_root("oversized");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let socket_path = directory.socket_path(&agent_run_id);

    let sender = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(&socket_path).expect("connect");
        // Declare a length far beyond MAX_MESSAGE_FRAME_BYTES and then
        // deliberately send nothing further -- if the endpoint tried to
        // read this many bytes it would block forever waiting for data
        // that never arrives, rather than rejecting the declared length
        // up front.
        let oversized_len: u32 = u32::MAX;
        let _ = stream.write_all(&oversized_len.to_be_bytes());
    });

    let result = endpoint.accept_proposal();
    sender.join().expect("sender thread must not panic");

    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelErrorReason::OversizedFrame
    );
}

/// Cross-*process* (not just cross-thread) impersonation attempt: a
/// separate `python3` process -- sharing this test process's UID, so the
/// peer-credential layer alone would not stop it -- connects and submits
/// a proposal with the wrong token. This is the layer that has to catch
/// same-user-but-not-the-real-adapter, and it is worth demonstrating
/// against a genuinely separate OS process rather than only a thread in
/// the same process, since the peer-credential check by itself cannot
/// distinguish the two.
#[test]
fn cross_process_impersonation_with_wrong_token_is_rejected() {
    let Ok(python) = which_python3() else {
        eprintln!(
            "skipping cross_process_impersonation_with_wrong_token_is_rejected: python3 not found"
        );
        return;
    };

    let state_root = temp_state_root("cross-process");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _real_token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let socket_path = directory.socket_path(&agent_run_id);

    let path_str = socket_path.to_str().expect("path must be valid UTF-8");
    let script = format!(
        r#"
import socket, json, struct, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect("{path_str}")
payload = json.dumps({{
    "protocol_version": 1,
    "run_token": "f" * 64,
    "proposal_id": "cross-process-evil",
    "argv": ["git", "push", "--force"],
    "cwd": "/tmp",
    "declared_intent": None,
    "declared_effects": None,
}}).encode()
s.sendall(struct.pack(">I", len(payload)) + payload)
s.close()
"#
    );

    let child = Command::new(python)
        .arg("-c")
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn python3 impersonator process");

    let result = endpoint.accept_proposal();
    let output = child
        .wait_with_output()
        .expect("python3 impersonator process must exit");
    assert!(
        output.status.success(),
        "impersonator script itself must run cleanly (its rejection happens on the Rust side): {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelErrorReason::TokenMismatch,
        "a real separate OS process presenting the wrong token must be rejected"
    );
}

/// Response 112 Defect 1: `resolve()` rejected a symlinked channel
/// directory once, but that check can decay -- a same-user process could
/// swap the directory for a symlink in the window between `resolve()` and
/// `bind()`. Reproduces the reviewer's own probe: resolve normally, then
/// perform the swap, then bind, and confirm the swap is caught rather
/// than the endpoint ending up bound inside the attacker's target
/// directory.
#[test]
fn symlink_swap_between_resolve_and_bind_is_rejected() {
    let state_root = temp_state_root("swap");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    // `resolve()` has already run (inside `resolve_directory`) and
    // approved of `channel_dir` as a real directory -- but that directory
    // does not exist on disk yet at this point (creation happens inside
    // `bind()`). Create it first, as an earlier `bind()` call would have,
    // then perform the swap `resolve()` cannot see: replace it with a
    // symlink to somewhere else entirely, simulating a same-user process
    // racing a *second* `bind()` call.
    std::fs::create_dir_all(directory.channel_dir()).expect("create the real approval dir first");
    let attacker_target = temp_state_root("swap-target");
    std::fs::remove_dir(directory.channel_dir()).expect("remove the real approval dir");
    std::os::unix::fs::symlink(&attacker_target, directory.channel_dir())
        .expect("swap in a symlink");

    let result = ApprovalChannelEndpoint::bind(&directory, &agent_run_id);
    assert!(
        result.is_err(),
        "bind() must refuse a channel directory that resolves to a symlink at bind time, \
         even though resolve() checked and approved it earlier"
    );

    // Confirm the attacker's directory was not touched (no socket placed
    // there, no permissions changed) and nothing was bound outside the
    // real state root.
    let leaked_socket = attacker_target.join(format!("{agent_run_id}.sock"));
    assert!(
        !leaked_socket.exists(),
        "no socket may be created inside the symlink target"
    );
}

/// Response 112 Defect 3: a same-user peer that connects and then sends
/// nothing must not block `accept_proposal` (and every connection after
/// it) forever. Uses `set_read_timeout_for_test` so this test takes a
/// bounded fraction of a second rather than the real 30-second
/// `PROPOSAL_READ_TIMEOUT`.
#[test]
fn silent_peer_triggers_a_read_timeout_rather_than_blocking_forever() {
    let state_root = temp_state_root("silent-peer");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (mut endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    endpoint.set_read_timeout_for_test(Duration::from_millis(200));
    let socket_path = directory.socket_path(&agent_run_id);

    let silent_peer = std::thread::spawn(move || {
        let stream = UnixStream::connect(&socket_path).expect("silent peer connect");
        // Deliberately send nothing and hold the connection open long
        // enough that, without a timeout, `accept_proposal` would still
        // be blocked when this thread finishes.
        std::thread::sleep(Duration::from_millis(500));
        drop(stream);
    });

    let started = std::time::Instant::now();
    let result = endpoint.accept_proposal();
    let elapsed = started.elapsed();

    silent_peer
        .join()
        .expect("silent peer thread must not panic");

    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelErrorReason::ReadTimedOut
    );
    assert!(
        elapsed < Duration::from_millis(400),
        "accept_proposal must return once its own read timeout elapses, \
         not wait for the peer -- took {elapsed:?}"
    );
}

/// Response 112 Q1: exercises the `peer_uid != self.owner_uid` comparison
/// by varying `owner_uid` (via a test-only setter) rather than the peer's
/// real UID, since no second real user account is available in this
/// environment. See `set_owner_uid_for_test`'s doc comment for exactly
/// what this does and does not prove.
#[test]
fn peer_credential_mismatch_is_rejected() {
    let state_root = temp_state_root("peer-cred");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (mut endpoint, token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let real_uid = unsafe { libc::geteuid() };
    endpoint.set_owner_uid_for_test(real_uid + 1);
    let socket_path = directory.socket_path(&agent_run_id);

    let peer = std::thread::spawn(move || {
        let mut stream = UnixStream::connect(&socket_path).expect("peer connect");
        // A perfectly valid proposal with the correct token -- the point
        // is that peer-credential rejection happens before the token is
        // even inspected. The write itself may race against the main
        // thread's rejection (which drops the connection as soon as the
        // peer-credential check fails, possibly before this write
        // completes) -- a `BrokenPipe` here is an expected side effect of
        // fast, correct rejection, not a test failure, so errors are
        // deliberately ignored rather than `.expect()`-ed.
        let _ = stream.write_all(&valid_proposal_frame(&token, "proposal-1"));
    });

    let result = endpoint.accept_proposal();
    peer.join().expect("peer thread must not panic");

    assert_eq!(
        result.unwrap_err().reason,
        ApprovalChannelErrorReason::PeerCredentialMismatch
    );
}

/// Response 112 Q3: a regular file (never a socket) sitting at the target
/// path must be refused outright, not silently deleted on the assumption
/// that `ECONNREFUSED`-from-connecting-to-it means "stale socket, safe to
/// clear" -- a plain file yields the same `ECONNREFUSED`-shaped failure
/// from a connect attempt, which the pre-fix code could not tell apart
/// from a genuinely stale socket.
#[test]
fn stale_socket_clearing_refuses_a_non_socket_file() {
    let state_root = temp_state_root("non-socket");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());
    let socket_path = directory.socket_path(&agent_run_id);

    std::fs::create_dir_all(directory.channel_dir()).expect("create channel dir for the test");
    std::fs::write(&socket_path, b"not a socket").expect("create a plain regular file");

    let result = ApprovalChannelEndpoint::bind(&directory, &agent_run_id);
    assert!(
        result.is_err(),
        "bind() must refuse when a non-socket file occupies the target path"
    );
    assert!(
        socket_path.exists(),
        "the regular file must not have been deleted"
    );
    assert_eq!(
        std::fs::read(&socket_path).expect("read back the file"),
        b"not a socket",
        "the regular file's contents must be untouched"
    );
}

/// Response 113 Required 1: binding through the short `/proc/self/fd`-
/// relative path no longer fails on a deep real state root the way the
/// pre-response-113 code did -- the reviewer's probe showed `bind()`
/// succeeding at a 266-byte state-root length while the *real* socket path
/// (327 bytes) exceeded `sun_path`, leaving a healthy-looking endpoint no
/// adapter could ever `connect()` to. `bind()` now checks the real path's
/// length explicitly and must refuse before doing anything else.
#[test]
fn bind_rejects_a_state_root_deep_enough_to_exceed_sun_path() {
    let mut state_root = std::env::temp_dir();
    // A single path component this long, repeated, reliably pushes the
    // *real* socket path (`<state_root>/approval/<agent_run_id>.sock`)
    // past a `sockaddr_un`'s capacity (~108 bytes on Linux) while staying
    // well under any single filesystem's per-component name limit
    // (typically 255 bytes).
    for _ in 0..6 {
        state_root.push("a".repeat(40));
    }
    std::fs::create_dir_all(&state_root).expect("create the deep temp state root");
    let state_root = state_root
        .canonicalize()
        .expect("canonicalize deep state root");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    assert!(
        directory.socket_path(&agent_run_id).as_os_str().len() > 107,
        "test precondition: the real socket path must actually exceed sun_path's capacity"
    );

    let result = ApprovalChannelEndpoint::bind(&directory, &agent_run_id);
    assert_eq!(
        result.err().map(|error| error.reason),
        Some(ApprovalChannelErrorReason::SocketPathTooLong),
        "bind() must refuse a real socket path too long for sockaddr_un, \
         rather than binding through the short /proc/self/fd path and \
         leaving connect() to fail later"
    );
}

/// Response 113 Required 2: `O_NOFOLLOW` on the `approval` subdirectory
/// alone (the pre-response-113 fix) only protects that final path
/// component -- swapping an *ancestor* (`state_root` itself) still placed
/// the socket outside the state root, one directory higher than the race
/// response 112 closed. `resolve()` now pins `state_root` as an fd once,
/// and `bind()` never re-resolves its pathname again, so this swap must
/// now be rejected too.
#[test]
fn ancestor_symlink_swap_of_the_state_root_itself_is_rejected() {
    let state_root = temp_state_root("ancestor-swap");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    // `resolve()` has already pinned `state_root` as an fd by this point.
    // Now perform the swap that fd-pinning is supposed to defeat: replace
    // the state root *itself* with a symlink to somewhere else entirely,
    // simulating a same-user process racing a later `bind()` call.
    let attacker_target = temp_state_root("ancestor-swap-target");
    std::fs::remove_dir(&state_root).expect("remove the real state root");
    std::os::unix::fs::symlink(&attacker_target, &state_root)
        .expect("swap the state root itself for a symlink");

    let result = ApprovalChannelEndpoint::bind(&directory, &agent_run_id);
    assert!(
        result.is_err(),
        "bind() must refuse when the state root itself (not just the approval \
         subdirectory) has been swapped for a symlink since resolve() ran"
    );

    let leaked_socket = attacker_target
        .join("approval")
        .join(format!("{agent_run_id}.sock"));
    assert!(
        !leaked_socket.exists(),
        "no socket may be created inside the symlink target reached via the swapped ancestor"
    );

    // Clean up the symlink so it doesn't get treated as a real directory
    // by anything that later enumerates the parent temp directory.
    let _ = std::fs::remove_file(&state_root);
}

fn which_python3() -> Result<String, ()> {
    let output = Command::new("which")
        .arg("python3")
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|_| ())
}
