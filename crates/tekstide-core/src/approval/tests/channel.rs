use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::approval::{
    ApprovalChannelDirectory, ApprovalChannelEndpoint, ApprovalChannelErrorReason,
    ApprovalChannelPathErrorReason, ApprovalChannelPathRequest, ApprovalChannelPathResolver,
    CommandDecision, DecisionOutcome,
};
use crate::domain::AgentRunId;
use crate::test_support::{KillOnDropChild, RealProcessLimiter};

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
    crate::test_support::remove_when_this_test_ends(dir.clone());
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

/// **Linux only, review 604**: `channel_dir` deliberately no longer
/// derives from `state_root` on macOS at all (`channel_dir_under`'s own
/// macOS doc, review 600/601) -- this property is false by design
/// there, not a premise that happened to stop holding. See
/// `resolve_creates_the_macos_approval_directory_under_the_temp_anchor`,
/// below, for the property that replaces it.
#[cfg(target_os = "linux")]
#[test]
fn resolve_creates_the_approval_directory_under_the_state_root() {
    let state_root = temp_state_root("create");
    let directory = resolve_directory(&state_root);
    assert!(directory.channel_dir().starts_with(&state_root));
}

/// macOS's own version of the property above: `channel_dir` lives under
/// the short per-user `$TMPDIR` anchor, never under `state_root`, no
/// matter what `state_root` is.
#[cfg(target_os = "macos")]
#[test]
fn resolve_creates_the_macos_approval_directory_under_the_temp_anchor() {
    let state_root = temp_state_root("create");
    let directory = resolve_directory(&state_root);
    assert!(
        !directory.channel_dir().starts_with(&state_root),
        "channel_dir must not be under state_root on macOS: {:?}",
        directory.channel_dir()
    );
    // Review 605 follow-up, found on the real runner: `std::env::
    // temp_dir()` is not canonicalised (macOS's own `/tmp` is itself a
    // symlink to `/private/tmp`), but `channel_dir_under`'s own macOS
    // arm canonicalises before joining `"tekstide"` -- comparing against
    // the raw, uncanonicalised prefix failed on the real runner even
    // though both sides name the same real directory. Canonicalising
    // the expected prefix here too is the fix, not loosening the
    // assertion -- the exact "/tmp vs /private/tmp" trap this whole RFC
    // found 402 times over in review 596's own triage.
    let expected_prefix = std::fs::canonicalize(std::env::temp_dir())
        .expect("$TMPDIR must canonicalise")
        .join("tekstide");
    assert!(
        directory.channel_dir().starts_with(&expected_prefix),
        "channel_dir must be under the $TMPDIR anchor on macOS: {:?}, expected under {:?}",
        directory.channel_dir(),
        expected_prefix
    );
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

/// **Linux only, review 604**: `channel_dir` here is `state_root.
/// join("approval")`, so symlinking that exact path is this property's
/// own setup. On macOS `channel_dir` is `$TMPDIR/tekstide` instead --
/// `resolve_rejects_a_macos_symlinked_approval_directory`, below, tests
/// the same `reject_symlink` call against the path that actually
/// matters there.
#[cfg(target_os = "linux")]
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

/// macOS's own version: `channel_dir` is `<anchor>/tekstide`, not under
/// `state_root` at all, so the symlink has to sit where `channel_dir`
/// actually resolves to. Uses `test_channel_anchor_root` rather than the
/// real, system-wide `$TMPDIR` -- swapping the real one for a symlink
/// would disrupt every other process on the machine, not only this
/// suite's own other threads.
#[cfg(target_os = "macos")]
#[test]
fn resolve_rejects_a_macos_symlinked_approval_directory() {
    let anchor_root = temp_state_root("macos-symlink-anchor");
    let _anchor_guard = super::super::channel::test_channel_anchor_root(&anchor_root);
    let elsewhere = temp_state_root("macos-symlink-target");
    let channel_dir = anchor_root.join("tekstide");
    std::os::unix::fs::symlink(&elsewhere, &channel_dir).expect("create symlink");

    let state_root = temp_state_root("macos-symlink-state-root");
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

/// What `wait_for_path_to_stop_answering` found.
struct ListenerSettleResult {
    /// `true` once `connect()` against the path returned `ConnectionRefused`;
    /// `false` if `budget` ran out first.
    closed: bool,
    /// Human-readable detail for a panic message when `closed` is `false`
    /// (or for a log line even when it's `true`, if a caller wants one) --
    /// timing and, best-effort, who was holding the socket open.
    diagnosis: String,
}

/// Review 576's second instrument (verify the test's own precondition),
/// refined by review 577's parts 1 and 2 once the hypothesis was
/// confirmed live: this process forks real child processes elsewhere in
/// this module (`cross_process_impersonation_with_wrong_token_is_rejected`,
/// `inject_token_into_environment_sets_the_sanctioned_variable_on_a_real_child_process`),
/// and a sibling thread's `Command::spawn` can duplicate this test's
/// listening fd into the forked child's table an instant before
/// `drop(raw_listener)` runs here -- the child's own copy keeps the
/// socket answering `connect()` until the child reaches `exec` (std sets
/// `CLOEXEC`, so the window closes there, not at the child's eventual
/// exit). Measured live, repeatedly, on 2026-10-15: closes within
/// single-digit milliseconds (1.5ms-6.5ms across every catch so far)
/// every time it was caught -- never approaching the 500ms budget below.
/// This function accepts that window explicitly -- polls until the path
/// refuses or `budget` elapses
/// -- rather than letting the test fail on a race already understood and
/// bounded. The common case (no sibling fork in flight) is checked first,
/// cheaply, with no polling or `/proc` work at all.
fn wait_for_path_to_stop_answering(socket_path: &Path, budget: Duration) -> ListenerSettleResult {
    let start = std::time::Instant::now();
    if matches!(
        UnixStream::connect(socket_path),
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused
    ) {
        return ListenerSettleResult {
            closed: true,
            diagnosis: format!("refused on the first attempt, after {:?}", start.elapsed()),
        };
    }

    // Still open: attempt best-effort holder identification on the way,
    // its only real chance before the window closes. The timer keeps
    // running from the very first attempt above, not from here, so this
    // lookup's own latency (a `/proc/net/unix` read and potentially a
    // scan of every process's `/proc/*/fd`) is not mistaken for part of
    // the window being measured.
    let mut holder_while_open = None;
    let closed = loop {
        if holder_while_open.is_none() {
            holder_while_open = holder_of_unix_socket_path(socket_path);
        }
        if matches!(
            UnixStream::connect(socket_path),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused
        ) {
            break true;
        }
        if start.elapsed() >= budget {
            break false;
        }
        std::thread::sleep(Duration::from_micros(200));
    };
    let elapsed = start.elapsed();
    let holder_while_open = holder_while_open.unwrap_or_else(|| {
        "unidentified -- /proc/net/unix had no matching entry by the time it was read, which \
         itself means the window closed faster than a /proc/net/unix read, string-parsed, \
         completes"
            .to_string()
    });
    let diagnosis = if closed {
        format!(
            "the path stopped answering after {elapsed:?} (closed, consistent with the measured \
             fork/exec window rather than a permanent leak). Holder while it was open: \
             {holder_while_open}"
        )
    } else {
        format!(
            "the path never stopped answering within the {budget:?} budget -- longer than any \
             fork/exec window measured for this process (single-digit milliseconds); something \
             else is holding this socket open. Holder: {holder_while_open}"
        )
    };
    // Review 578: the slow path was built to produce a good diagnosis and
    // then only ever showed it on failure -- a window that crept from
    // 5ms toward the 500ms budget would pass in total silence, and the
    // budget would quietly become a blindfold instead of a bound. Printed
    // every time the fast path is skipped, success or not, so a reader
    // watching test output (not just a failure) sees the tolerance
    // actually being used.
    eprintln!("wait_for_path_to_stop_answering: {diagnosis}");
    ListenerSettleResult { closed, diagnosis }
}

/// Reads `/proc/net/unix` to find the kernel inode bound at `socket_path`,
/// then scans `/proc/*/fd` for whichever process holds a `socket:[<inode>]`
/// descriptor open, and reads that process's `/proc/<pid>/comm`. Best
/// effort: any read failure along the way (permissions, the process
/// having already exited and closed it, a parse mismatch) yields `None`
/// rather than panicking -- this exists only to enrich a diagnostic
/// message, never to gate a result.
fn holder_of_unix_socket_path(socket_path: &Path) -> Option<String> {
    let unix_table = std::fs::read_to_string("/proc/net/unix").ok()?;
    // Collects every matching line rather than stopping at the first:
    // `/proc/net/unix` can list more than one entry for the same bound
    // path while a socket is being torn down (one with a real `Inode`,
    // another already showing `0`), and bucket order in the table is not
    // time order. Preferring any non-"0" match over a "0" one is strictly
    // more informative regardless of why both exist.
    let candidates: Vec<String> = unix_table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let inode_field = fields.nth(6)?;
            let path_field = fields.next()?;
            (Path::new(path_field) == socket_path).then(|| inode_field.to_string())
        })
        .collect();
    let inode = candidates
        .iter()
        .find(|inode| inode.as_str() != "0")
        .or(candidates.first())?
        .clone();

    if inode == "0" {
        // A real, observed kernel value, not a parse failure: `/proc/net/
        // unix` can list a socket still bound to its path with `Inode`
        // printed as `0` while the socket is already mid-teardown and has
        // lost its inode association -- no process's `/proc/*/fd` can
        // meaningfully reference inode 0, so there is nothing to scan for.
        return Some(
            "kernel reports this path's socket already has Inode=0 (mid-teardown, inode already \
             detached) -- no fd scan can identify a holder past this point"
                .to_string(),
        );
    }

    let needle = format!("socket:[{inode}]");
    for proc_entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let pid = proc_entry.file_name();
        let Some(pid) = pid
            .to_str()
            .filter(|pid| pid.bytes().all(|byte| byte.is_ascii_digit()))
        else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(proc_entry.path().join("fd")) else {
            continue;
        };
        for fd_entry in fds.flatten() {
            if std::fs::read_link(fd_entry.path())
                .is_ok_and(|link| link.to_string_lossy() == needle)
            {
                let comm = std::fs::read_to_string(proc_entry.path().join("comm"))
                    .unwrap_or_else(|_| "<unreadable>".to_string());
                return Some(format!("pid={pid} comm={}", comm.trim()));
            }
        }
    }
    Some(format!(
        "inode={inode}, no /proc/*/fd holder found (closed between the /proc/net/unix read and \
         the /proc scan)"
    ))
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
    // Review 576's own second instrument: this test's entire premise is
    // that a dropped `UnixListener` leaves nothing listening at its path.
    // Review 577 parts 1-2: that premise is not quite true in *this*
    // process -- it forks real children elsewhere in this module, and a
    // sibling fork can transiently duplicate this listener's fd across
    // `drop()` until the child reaches `exec`. Rather than fail on a race
    // already measured and understood, accept it within a bounded
    // settling window; only a path that is *still* answering well past
    // that window is the different, worse finding worth failing loudly
    // over.
    let settle = wait_for_path_to_stop_answering(&socket_path, Duration::from_millis(500));
    assert!(
        settle.closed,
        "precondition failed: connecting to the path left by a dropped UnixListener never \
         settled to refused -- {}",
        settle.diagnosis
    );

    let (second_endpoint, _token) = ApprovalChannelEndpoint::bind(&directory, &agent_run_id)
        .expect("second bind must clear the stale file and succeed");
    drop(second_endpoint);
    assert!(
        !socket_path.exists(),
        "the second endpoint's own Drop must clean up after itself"
    );
}

/// Review 554: the recurring flake in the test above always panicked with
/// `reason: Io, source: None`, which response 231 traced to one of two
/// arms in `clear_stale_socket` -- "the bind path exists and is not a
/// socket" (already covered, without asserting its specific reason, by
/// `stale_socket_clearing_refuses_a_non_socket_file` below) or
/// "`connect()` succeeded, something is really listening" (new coverage
/// here) -- left indistinguishable from each other. This test forces the
/// second arm deterministically (no drop-then-rebind race, the listener
/// stays alive throughout) and asserts the distinct reason it now
/// carries, so the next occurrence of that flake is self-diagnosing
/// instead of reporting `Io` again.
#[test]
fn a_live_listener_at_the_bind_path_is_refused_as_already_listening() {
    let state_root = temp_state_root("listening");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());
    let socket_path = directory.socket_path(&agent_run_id);

    std::fs::create_dir_all(directory.channel_dir()).expect("create channel dir for the test");
    // Kept alive for the whole test, unlike the stale-recovery test above:
    // this is the genuine-conflict arm, not the stale-leftover one, so the
    // listener must still be accepting connections when the second bind
    // runs.
    let _live_listener =
        std::os::unix::net::UnixListener::bind(&socket_path).expect("raw bind must succeed");

    let error = ApprovalChannelEndpoint::bind(&directory, &agent_run_id)
        .expect_err("a live listener at the bind path must be refused, not cleared");
    assert_eq!(
        error.reason,
        ApprovalChannelErrorReason::BindTargetAlreadyListening
    );
    assert!(
        socket_path.exists(),
        "refused, not removed -- a genuine conflict must not have its socket file deleted out \
         from under the process still listening on it"
    );
    // Review 576's own first instrument: `_live_listener` above is owned by
    // this same test process, so the peer credentials when
    // `clear_stale_socket` connects to it must name this process's own
    // pid -- confirming the new diagnostic correctly identifies an
    // in-process answerer, which is exactly what row 1's real,
    // non-deterministic occurrences need distinguished from "something
    // else." **Review 604: `pid={own_pid}` only on Linux's `SO_PEERCRED`.**
    // RFC-028 D4's own `PeerCredentials.pid: Option<u32>` exists precisely
    // because macOS's `LOCAL_PEERCRED`/`xucred` carries no pid at all
    // (confirmed on the real runner here, not a new finding -- the
    // bind-refusal logic above this already passed, so this is the
    // diagnostic's own wording, not the mechanism). `BindTargetAlreadyListening`'s
    // own diagnostic formatting already renders `"unknown"` for that
    // `None` case (`channel.rs`'s own doc comment on the variant), so
    // this asserts the platform-correct expectation instead of a Linux
    // one both platforms were held to.
    let source_message = error
        .source()
        .expect("review 576: BindTargetAlreadyListening must carry the peer credential detail")
        .to_string();
    #[cfg(target_os = "linux")]
    {
        let own_pid = std::process::id();
        assert!(
            source_message.contains(&format!("pid={own_pid}")),
            "the peer connecting to its own just-bound listener is this same test process -- the \
             diagnostic source should name pid={own_pid}, got {source_message:?}"
        );
    }
    #[cfg(target_os = "macos")]
    {
        // Review 605: asserting only the absent field confirms this
        // platform's own gap, not what the test exists to prove -- that
        // the diagnostic identifies an in-process answerer. `xucred`
        // carries no pid, but it does carry a uid, and the peer
        // connecting to its own just-bound listener is this same test
        // process, so that uid must be this process's own real one.
        let real_uid = unsafe { libc::geteuid() };
        assert!(
            source_message.contains("pid=unknown"),
            "macOS's LOCAL_PEERCRED/xucred carries no pid at all -- the diagnostic source should \
             say so rather than name one, got {source_message:?}"
        );
        assert!(
            source_message.contains(&format!("uid={real_uid}")),
            "the peer connecting to its own just-bound listener is this same test process -- the \
             diagnostic source should name uid={real_uid}, got {source_message:?}"
        );
    }
}

/// Review 577's part 3: RFC-022's trust boundary names peer credentials
/// and a per-run token as the two layers that keep an unauthorized
/// process from submitting proposals, but neither layer matters if a
/// **spawned AI CLI agent itself** inherits the listening socket past its
/// own `exec` -- it would not need to connect at all, it could `accept()`
/// directly. `std::os::unix::net::UnixListener` is documented to set
/// close-on-exec, but nothing in this codebase had verified that for the
/// actual bind path this channel uses until review 577's own
/// fork/exec-window investigation into row 1 surfaced the question.
/// Verified against the kernel flag directly rather than trusted from
/// std's docs.
#[test]
fn the_approval_listener_is_close_on_exec() {
    let state_root = temp_state_root("cloexec");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");

    // SAFETY: `fd` is the listener's own raw fd, valid for the lifetime of
    // `endpoint`, which outlives this call; `F_GETFD` reads flags and
    // performs no action that could invalidate it.
    let flags = unsafe { libc::fcntl(endpoint.listener_raw_fd_for_test(), libc::F_GETFD) };
    assert!(
        flags >= 0,
        "fcntl(F_GETFD) on the approval listener failed: {}",
        std::io::Error::last_os_error()
    );
    assert!(
        flags & libc::FD_CLOEXEC != 0,
        "the approval channel's listening socket is NOT close-on-exec -- a spawned AI CLI agent \
         process would inherit it past exec and could accept connections on the channel meant \
         only for the real adapter, defeating both the peer-credential and token checks by never \
         needing to connect at all"
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
        // Review 645's own required fix: without this, the race is "server
        // reads the frame" versus "this thread ends and the stream drops",
        // and the client's own close can win on a loaded runner -- on
        // macOS, `set_read_timeout` on the *server's* accepted stream then
        // fails `EINVAL` because the peer is already gone (review 644), so
        // the server reports `ConnectionClosed` instead of exercising the
        // malformed-frame path this test is named for. Set while the peer
        // is certainly still alive, then block on one byte: the server
        // reads the frame, reaches its verdict, and drops its own end,
        // which is what makes this `read` return `0` -- strictly after
        // the server has finished, every time, no `sleep` anywhere.
        //
        // **This thread's own `.expect()` on `set_read_timeout` is sound
        // only because this test's server path always reads first**
        // (`authenticate_and_read` calls `read_frame` before it can reach
        // any verdict here), so the server is strictly slower than this
        // peer and can never drop the stream first. `peer_credential_
        // mismatch_is_rejected` (below) has no such guarantee -- its
        // server's rejection path never reads at all -- which is exactly
        // why that test tolerates this same call failing instead of
        // asserting on it. Copy the bounded-read *shape* freely; copy the
        // `.expect()` only where the server is proven to read first too.
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set the sender's own bound");
        let _ = stream.read(&mut [0_u8; 1]);
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
        // Review 645's own required fix -- see `malformed_frame_is_
        // rejected_not_partially_parsed`'s own identical addition for why:
        // without this, this thread's own close can race ahead of the
        // server's read and win, reporting `ConnectionClosed` instead of
        // exercising the oversized-length rejection this test is named
        // for.
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set the sender's own bound");
        let _ = stream.read(&mut [0_u8; 1]);
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
    // Response 232: real `fork()`+exec, twice (`which`, then the python3
    // impersonator child) -- see the other `RealProcessLimiter::acquire`
    // call in this file for why this is bound first.
    let _real_process_slot = RealProcessLimiter::acquire();
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
# Review 645's own required fix -- same reason as the Rust-side sender
# threads' own identical addition: without this, this process's own exit
# (closing its socket) can race ahead of the server's read and win,
# reporting ConnectionClosed instead of exercising the token-mismatch
# rejection this test is named for. Block on one byte while the peer is
# certainly still alive; the server reads the frame, reaches its
# verdict, and drops its own end, which is what makes this recv return
# b"" -- strictly after the server has finished, every time.
s.settimeout(5)
try:
    s.recv(1)
except socket.timeout:
    pass
s.close()
"#
    );

    // test-process-leak.md: KillOnDropChild, not a bare Child -- this real
    // process must be killed and reaped if this test panics before
    // wait_with_output below runs.
    let child = KillOnDropChild::new(
        Command::new(python)
            .arg("-c")
            .arg(&script)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn python3 impersonator process"),
    );

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
///
/// **Linux only, found on the real runner**: `directory.channel_dir()`
/// on macOS is the *shared* `$TMPDIR/tekstide` anchor -- every test in
/// this binary that resolves a directory there gets the exact same
/// path, unlike Linux's own `state_root`-derived one, which
/// `temp_state_root`'s unique naming already keeps exclusive to this
/// test. `std::fs::remove_dir` on it fails `DirectoryNotEmpty` the
/// moment any other, concurrently-running test has also created a
/// socket under the same shared directory -- confirmed directly, not
/// inferred, by the real runner's own error. `symlink_swap_between_
/// resolve_and_bind_is_rejected_on_macos`, below, is the same property
/// against a private stand-in instead of the shared real one.
#[cfg(target_os = "linux")]
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
    //
    // Review 604: the real filename strips `AgentRunId`'s own
    // `"agent-run-"` prefix (`socket_filename_stem`, review 600/601) --
    // asserting against the unstripped `{agent_run_id}.sock` would check
    // for a file that was never going to exist regardless of whether
    // the swap was actually caught, the same "assertion checks the
    // wrong path" shape this review has already found twice elsewhere
    // in this file.
    let leaked_socket = attacker_target.join(format!(
        "{}.sock",
        super::super::channel::socket_filename_stem(&agent_run_id)
    ));
    assert!(
        !leaked_socket.exists(),
        "no socket may be created inside the symlink target"
    );
}

/// macOS's own version of the property above, against a private,
/// test-owned stand-in rather than the shared real `$TMPDIR/tekstide`
/// anchor every other concurrently-running test also resolves a
/// directory under -- see the Linux test's own doc comment for why
/// operating on the shared one directly is unsafe here.
#[cfg(target_os = "macos")]
#[test]
fn symlink_swap_between_resolve_and_bind_is_rejected_on_macos() {
    let anchor_root = temp_state_root("macos-dirswap-anchor");
    let _anchor_guard = super::super::channel::test_channel_anchor_root(&anchor_root);
    let state_root = temp_state_root("macos-dirswap-state-root");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    // `resolve()` has already run and approved of `channel_dir` (under
    // this test's own private anchor) as a real directory -- but that
    // directory does not exist on disk yet at this point (creation
    // happens inside `bind()`). Create it first, as an earlier `bind()`
    // call would have, then perform the swap `resolve()` cannot see:
    // replace it with a symlink to somewhere else entirely, simulating
    // a same-user process racing a *second* `bind()` call.
    std::fs::create_dir_all(directory.channel_dir()).expect("create the real approval dir first");
    let attacker_target = temp_state_root("macos-dirswap-target");
    std::fs::remove_dir(directory.channel_dir()).expect("remove the real approval dir");
    std::os::unix::fs::symlink(&attacker_target, directory.channel_dir())
        .expect("swap in a symlink");

    let result = ApprovalChannelEndpoint::bind(&directory, &agent_run_id);
    assert!(
        result.is_err(),
        "bind() must refuse a channel directory that resolves to a symlink at bind time, \
         even though resolve() checked and approved it earlier"
    );

    let leaked_socket = attacker_target.join(format!(
        "{}.sock",
        super::super::channel::socket_filename_stem(&agent_run_id)
    ));
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
        // Review 651 round 3's own required fix -- and round 4's own
        // required correction to it. The *shape* is
        // `malformed_frame_is_rejected_not_partially_parsed`'s: block on a
        // bounded read so this thread cannot return, and drop the stream,
        // before the server has reached its verdict. But that sibling
        // test's server always calls `read_frame` before rejecting, so it
        // is strictly slower than its own peer and cannot finish first --
        // this server's rejection path (`authenticate_and_read`'s
        // `peer_uid` check) never reads at all, so it *can* finish and
        // drop its end before this thread's own `set_read_timeout` runs.
        // When that happens, `set_read_timeout` is itself the call looking
        // at an already-gone peer, and hits the identical macOS `EINVAL`
        // (review 644) this fix exists to avoid on the *server* side --
        // asserting on it here would just move the same intermittent to
        // the peer thread. Tolerated, not asserted: the only measured
        // cause of a failure here is the server already being gone, which
        // is this test's own asserted outcome, so there is nothing left
        // to wait for. Skipping the read on that failure keeps it bounded
        // either way -- the one thing an unconditional `.expect()` was
        // guarding against, a hang against a still-live server, cannot
        // happen, since a *live* server's `set_read_timeout` always
        // succeeds here.
        if stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .is_ok()
        {
            let _ = stream.read(&mut [0_u8; 1]);
        }
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

    let error = ApprovalChannelEndpoint::bind(&directory, &agent_run_id)
        .expect_err("bind() must refuse when a non-socket file occupies the target path");
    // Review 554: distinguished from a bare `Io` so this arm is
    // identifiable rather than indistinguishable from the "already
    // listening" arm this function's other refusal can also produce.
    assert_eq!(
        error.reason,
        ApprovalChannelErrorReason::BindTargetNotASocket
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
///
/// **Linux only, review 600**: this test's whole premise -- a deep
/// `state_root` forces a long *real* socket path -- stopped holding for
/// macOS the moment `channel_dir_under`'s macOS arm stopped deriving
/// `channel_dir` from `state_root` at all (it is always `$TMPDIR/
/// tekstide` there, regardless of how deep `state_root` is). macOS's own
/// version of this property is `the_macos_socket_path_leaves_
/// comfortable_headroom_under_the_real_limit`, below -- a different
/// shape because the thing that can go wrong is different.
#[cfg(target_os = "linux")]
#[test]
fn bind_rejects_a_state_root_deep_enough_to_exceed_sun_path() {
    let mut state_root = std::env::temp_dir();
    // A single path component this long, repeated, reliably pushes the
    // *real* socket path (`<state_root>/approval/<agent_run_id>.sock`)
    // past a `sockaddr_un`'s capacity (~108 bytes on Linux) while staying
    // well under any single filesystem's per-component name limit
    // (typically 255 bytes).
    for index in 0..6 {
        state_root.push("a".repeat(40));
        if index == 0 {
            // The first component is the one that sits in the temp
            // directory; removing it removes the whole chain.
            crate::test_support::remove_when_this_test_ends(state_root.clone());
        }
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

/// Review 600's own required regression: the "fits comfortably" half of
/// the property the pathological test above covers the "refused when it
/// does not fit" half of. A realistic state root -- one padded segment
/// the size of a plausible real directory name, not the six 40-byte
/// segments the other test uses to deliberately force the failure case
/// -- must leave comfortable headroom under `sun_path`'s real 107-byte
/// Linux limit, not merely squeak under it, so a future change
/// lengthening any fixed component this module controls (the
/// `"approval"` dirname, `.sock`, or `AgentRunId`'s own prefix if
/// `socket_filename_stem`'s strip were ever reverted) is caught here
/// before a real user's own, typically longer, home directory pushes a
/// realistic path over the edge.
#[cfg(target_os = "linux")]
#[test]
fn a_realistic_linux_socket_path_leaves_comfortable_headroom() {
    let mut state_root = temp_state_root("realistic-headroom");
    state_root.push("a".repeat(16));
    std::fs::create_dir_all(&state_root).expect("create the realistic state root");
    let state_root = state_root
        .canonicalize()
        .expect("canonicalize realistic state root");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let path_len = directory.socket_path(&agent_run_id).as_os_str().len();
    const LINUX_SUN_PATH_LIMIT: usize = 107;
    const STATED_HEADROOM: usize = 15;
    assert!(
        path_len + STATED_HEADROOM <= LINUX_SUN_PATH_LIMIT,
        "a realistic socket path is {path_len} bytes, leaving less than the stated \
         {STATED_HEADROOM}-byte headroom under Linux's {LINUX_SUN_PATH_LIMIT}-byte sun_path \
         limit -- this module's own fixed overhead may have grown"
    );
}

/// Review 600's own required regression, macOS's shape: `channel_dir`
/// there is always `$TMPDIR/tekstide` (`channel_dir_under`'s macOS arm),
/// never derived from `state_root` at all, so the real, deterministic
/// path can be asserted directly against the real runner's own
/// `$TMPDIR` rather than needing a synthetic stand-in the way Linux's
/// counterpart above does. Catches the same regression class: this
/// module's own fixed overhead (the `"tekstide"` dirname,
/// `socket_filename_stem`'s prefix strip, `.sock`) growing enough to
/// eat the stated headroom under the real 103-byte limit.
/// **Review 651 B1: this test's own doc comment above overstated what it
/// measures.** `ci.yml`'s macOS job runs the whole suite under
/// `TMPDIR="$CI_TMPDIR"` (`/tmp/tci`, 8 bytes) to keep the rest of the
/// suite from reddening on an unrelated `sockaddr_un` path -- which means
/// *this* test, run inside that same job, measures `/tmp/tci`'s own
/// headroom, never the runner's real per-session `$TMPDIR`
/// (`/var/folders/…/T`, canonicalized here to `/private/var/folders/…/T`
/// since `/var` is a symlink on macOS). It is still a real, correct
/// regression check for this module's own fixed overhead -- the thing it
/// is not is a measurement of the real limit, which is what the
/// diagnostic below exists to supply.
#[cfg(target_os = "macos")]
#[test]
fn the_macos_socket_path_leaves_comfortable_headroom_under_the_real_limit() {
    let state_root = temp_state_root("macos-headroom");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let path_len = directory.socket_path(&agent_run_id).as_os_str().len();
    const MACOS_SUN_PATH_LIMIT: usize = 103;
    const STATED_HEADROOM: usize = 10;
    assert!(
        path_len + STATED_HEADROOM <= MACOS_SUN_PATH_LIMIT,
        "the real, resolved macOS socket path is {path_len} bytes, leaving less than the \
         stated {STATED_HEADROOM}-byte headroom under the real {MACOS_SUN_PATH_LIMIT}-byte \
         sun_path limit -- channel_dir_under's own $TMPDIR/tekstide choice, or \
         socket_filename_stem's prefix strip, may have regressed"
    );
}

/// Review 651 B1: a pure measurement, not a check -- asserts nothing,
/// consumes nothing, and exists only to print, against the real runner's
/// own ambient `$TMPDIR` (left alone by this one dedicated CI step,
/// unlike the suite-wide run above), the numbers arithmetic alone
/// predicted might already be over the limit: a typical macOS
/// `/var/folders/<2>/<~30>/T` anchor, canonicalized (the `/private`
/// prefix `channel_dir_under`'s macOS arm pays for), joined with
/// `tekstide` and a 36-byte UUID plus `.sock`. `--nocapture`, its own CI
/// step, the same shape `ci.yml`'s libproc-visibility/kern-proc-session
/// measurements already use, for the same reason: a passing test's
/// stdout/stderr is otherwise discarded.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "a measurement, not a check -- see ci.yml's own dedicated step, review 651 B1"]
fn macos_approval_socket_path_headroom_measurement_against_the_real_ambient_tmpdir() {
    let raw = std::env::temp_dir();
    let canonical = raw.canonicalize();
    eprintln!("MEASURED raw std::env::temp_dir() = {raw:?}");
    eprintln!("MEASURED canonicalized = {canonical:?}");

    let state_root = temp_state_root("macos-real-tmpdir-headroom-measurement");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());
    let path = directory.socket_path(&agent_run_id);
    let path_len = path.as_os_str().len();
    const MACOS_SUN_PATH_LIMIT: usize = 103;
    eprintln!(
        "MEASURED approval socket path = {path:?} ({path_len} bytes, limit \
         {MACOS_SUN_PATH_LIMIT}, headroom {})",
        MACOS_SUN_PATH_LIMIT as isize - path_len as isize
    );
}

/// Response 113 Required 2: `O_NOFOLLOW` on the `approval` subdirectory
/// alone (the pre-response-113 fix) only protects that final path
/// component -- swapping an *ancestor* (`state_root` itself) still placed
/// the socket outside the state root, one directory higher than the race
/// response 112 closed. `resolve()` now pins `state_root` as an fd once,
/// and `bind()` never re-resolves its pathname again, so this swap must
/// now be rejected too.
///
/// **Linux only, review 604**: on macOS `bind()` never reaches the
/// socket through `state_root_fd` at all (`channel_anchor`'s macOS arm
/// uses `channel_anchor_fd`, pinned to `$TMPDIR`, not `state_root_fd`),
/// so swapping `state_root` for a symlink cannot redirect anything
/// there -- the property holds by construction, not by this test's own
/// swap-and-check. `ancestor_symlink_swap_of_the_macos_anchor_is_
/// rejected`, below, is `channel_anchor_fd`'s own version of the same
/// property, against the thing that actually protects macOS's bind
/// path.
#[cfg(target_os = "linux")]
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

    // Review 604: the real filename strips the `"agent-run-"` prefix
    // (`socket_filename_stem`) -- see `symlink_swap_between_resolve_
    // and_bind_is_rejected`'s own comment for why asserting against the
    // unstripped name would check a path that was never real either way.
    let leaked_socket = attacker_target.join("approval").join(format!(
        "{}.sock",
        super::super::channel::socket_filename_stem(&agent_run_id)
    ));
    assert!(
        !leaked_socket.exists(),
        "no socket may be created inside the symlink target reached via the swapped ancestor"
    );

    // Clean up the symlink so it doesn't get treated as a real directory
    // by anything that later enumerates the parent temp directory.
    let _ = std::fs::remove_file(&state_root);
}

/// macOS's own version of the property above, against the thing that
/// actually protects macOS's bind path: `channel_anchor_fd`, pinned to
/// `$TMPDIR` (here, `test_channel_anchor_root`'s own test-owned
/// stand-in -- swapping the real, system-wide `$TMPDIR` would disrupt
/// every other process on the machine, not only this suite's own other
/// threads) at `resolve()` time. Swapping that anchor for a symlink
/// between `resolve()` and `bind()` is the same race
/// `ancestor_symlink_swap_of_the_state_root_itself_is_rejected` closes
/// on Linux, against the fd that actually matters on this platform.
#[cfg(target_os = "macos")]
#[test]
fn ancestor_symlink_swap_of_the_macos_anchor_is_rejected() {
    let anchor_root = temp_state_root("macos-ancestor-swap-anchor");
    let _anchor_guard = super::super::channel::test_channel_anchor_root(&anchor_root);
    let state_root = temp_state_root("macos-ancestor-swap-state-root");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    // `resolve()` has already pinned `anchor_root` (standing in for
    // `$TMPDIR`) as an fd by this point. Now perform the swap that
    // fd-pinning is supposed to defeat: replace the anchor *itself*
    // with a symlink to somewhere else entirely, simulating a
    // same-user process racing a later `bind()` call.
    let attacker_target = temp_state_root("macos-ancestor-swap-target");
    std::fs::remove_dir(&anchor_root).expect("remove the real anchor root");
    std::os::unix::fs::symlink(&attacker_target, &anchor_root)
        .expect("swap the anchor itself for a symlink");

    let result = ApprovalChannelEndpoint::bind(&directory, &agent_run_id);
    assert!(
        result.is_err(),
        "bind() must refuse when the $TMPDIR anchor itself (not just the tekstide \
         subdirectory) has been swapped for a symlink since resolve() ran"
    );

    let leaked_socket = attacker_target.join("tekstide").join(format!(
        "{}.sock",
        super::super::channel::socket_filename_stem(&agent_run_id)
    ));
    assert!(
        !leaked_socket.exists(),
        "no socket may be created inside the symlink target reached via the swapped anchor"
    );

    // Clean up the symlink so it doesn't get treated as a real directory
    // by anything that later enumerates the parent temp directory.
    let _ = std::fs::remove_file(&anchor_root);
}

/// Response 113 Q2 (promoted to required E1 scope): a same-user peer that
/// connects and stalls must not delay acceptance of a second, unrelated,
/// well-behaved connection. Reproduces the exact denial the reviewer
/// described: a slow peer connects first and holds its connection open
/// (sending nothing) for far longer than the well-behaved peer's proposal
/// should take to be served -- if `serve_concurrently` still authenticated
/// one connection at a time, the well-behaved peer's result would not
/// arrive until the slow peer's read timeout released the single slot.
#[test]
fn serve_concurrently_accepts_a_second_connection_while_the_first_stalls() {
    let state_root = temp_state_root("concurrent");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (mut endpoint, token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    endpoint.set_read_timeout_for_test(Duration::from_secs(2));
    let socket_path = directory.socket_path(&agent_run_id);
    let endpoint = std::sync::Arc::new(endpoint);
    let (receiver, shutdown) = std::sync::Arc::clone(&endpoint).serve_concurrently();

    let slow_socket_path = socket_path.clone();
    let slow_peer = std::thread::spawn(move || {
        let stream = UnixStream::connect(&slow_socket_path).expect("slow peer connect");
        // Holds the connection open, sending nothing, for longer than the
        // well-behaved peer below should take to be served -- but well
        // under the 2s read timeout, so this thread's own result arrives
        // (a ReadTimedOut error, ignored by this test) without slowing
        // the test down unnecessarily.
        std::thread::sleep(Duration::from_millis(600));
        drop(stream);
    });
    // Give the slow peer a head start so it is definitely connected and
    // occupying its own authentication thread before the well-behaved
    // peer connects.
    std::thread::sleep(Duration::from_millis(100));

    let well_behaved_socket_path = socket_path;
    let well_behaved_peer = std::thread::spawn(move || {
        let mut stream =
            UnixStream::connect(&well_behaved_socket_path).expect("well-behaved peer connect");
        stream
            .write_all(&valid_proposal_frame(&token, "proposal-1"))
            .expect("well-behaved peer send proposal");
        // Review 645's own pattern, applied here in the only shape that
        // fits this test: a successful `authenticate_and_read` moves the
        // stream into the returned `AcceptedProposal` rather than closing
        // it (the connection stays open for the eventual decision), so
        // there is no "server is done, drops its end" moment for this
        // thread to block on the way the other four fixes do. Returning
        // the stream instead of dropping it here removes the race
        // outright: nothing closes the peer's own end until this test
        // drops it explicitly, well after the server has already read
        // the frame -- the same macOS `EINVAL`-on-a-closed-peer mechanism
        // (review 644) otherwise risks firing on *this* connection's own
        // `set_read_timeout`, which is what a later run of this test
        // showed (`reason: ConnectionClosed` where `proposal-1` was
        // expected). **Same race, same cause, two fixes -- the
        // difference is whose hand the stream ends up in**: a rejected
        // proposal ends with the server dropping its own end, so that
        // peer can block on a bounded read and exit only after the
        // server is done (the other four fixes); a successful one keeps
        // the stream alive in `AcceptedProposal`, so this peer must
        // instead outlive the assertions that use it, which is what
        // returning it here, rather than letting it drop, achieves.
        stream
    });

    let started = std::time::Instant::now();
    let result = receiver.recv_timeout(Duration::from_millis(400)).expect(
        "must receive the well-behaved peer's result promptly -- if this times out, \
             the well-behaved connection was stuck behind the stalling one",
    );
    let elapsed = started.elapsed();

    let well_behaved_stream = well_behaved_peer
        .join()
        .expect("well-behaved peer thread must not panic");
    slow_peer.join().expect("slow peer thread must not panic");

    let accepted = result.expect("the well-behaved peer's proposal must be accepted");
    assert_eq!(accepted.proposal.proposal_id().as_str(), "proposal-1");
    assert!(
        elapsed < Duration::from_millis(400),
        "the well-behaved connection must be served quickly, not wait behind the \
         stalling one -- took {elapsed:?}"
    );
    drop(well_behaved_stream);
    drop(shutdown);
}

/// Response 115 Required B: the previous fix relied on the caller
/// explicitly calling `shutdown()` -- dropping the returned `ServeShutdown`
/// without calling it did nothing, since it had no `Drop` impl. The
/// reviewer's probe showed this leaves the socket file behind and the
/// accept loop blocked forever *specifically* when the loop is already
/// blocked inside `accept()` at the moment everything is dropped (dropping
/// immediately after `serve_concurrently` returns happened to look fine,
/// which is a race, not a guarantee). The delay below is what makes this
/// test prove something: without it, this could pass on the same race the
/// reviewer warned about rather than actually exercising cleanup-via-Drop
/// while a connection-less `accept()` call is genuinely in progress.
#[test]
fn dropping_serve_shutdown_while_the_loop_is_blocked_in_accept_still_cleans_up() {
    let state_root = temp_state_root("drop-blocked");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    let socket_path = directory.socket_path(&agent_run_id);
    let endpoint = std::sync::Arc::new(endpoint);
    let (_receiver, shutdown) = std::sync::Arc::clone(&endpoint).serve_concurrently();

    // Give the accept loop time to actually reach and block inside
    // `accept()` (no connections are made in this test) before anything
    // is dropped.
    std::thread::sleep(Duration::from_millis(200));

    // Dropped, not explicitly shut down -- this must still clean up via
    // `ServeShutdown`'s `Drop` impl.
    drop(shutdown);
    drop(endpoint);

    assert!(
        !socket_path.exists(),
        "the socket file must be removed even when ServeShutdown is only dropped, \
         not explicitly shut down, while the accept loop is blocked in accept()"
    );
    assert!(
        UnixStream::connect(&socket_path).is_err(),
        "the socket path must no longer be connectable"
    );
}

/// Response 114 Required 4: an unbounded thread-per-connection design
/// lets a same-user process exhaust threads by opening connections and
/// sending nothing. Confirms connections beyond `MAX_CONCURRENT_CONNECTIONS`
/// are refused (their stream is closed without a response) rather than
/// each spawning another authentication thread indefinitely.
#[test]
fn serve_concurrently_refuses_connections_beyond_the_concurrency_cap() {
    let state_root = temp_state_root("cap");
    let directory = resolve_directory(&state_root);
    let agent_run_id = AgentRunId::for_test(rand_seed());

    let (mut endpoint, _token) =
        ApprovalChannelEndpoint::bind(&directory, &agent_run_id).expect("bind must succeed");
    // Long enough that none of the held connections below time out during
    // the test, short enough that the test itself stays fast.
    endpoint.set_read_timeout_for_test(Duration::from_secs(5));
    let socket_path = directory.socket_path(&agent_run_id);
    let endpoint = std::sync::Arc::new(endpoint);
    let (_receiver, shutdown) = std::sync::Arc::clone(&endpoint).serve_concurrently();

    // Open well beyond the cap (32) worth of connections, all silent, and
    // hold them open -- if the cap were not enforced, this would spawn
    // one authentication thread per connection with no limit.
    //
    // Review 645's own required fix: the read timeout used to be set
    // *after* the 300ms sleep below, by which time the endpoint had
    // already refused and closed the surplus connections -- on macOS,
    // `set_read_timeout` on a stream whose peer is already gone returns
    // `EINVAL` (review 644's own finding, the same platform rule the
    // production fix is built on), which is exactly what this test's own
    // panic was. Set here instead, in the same loop as `connect`, while
    // every peer this test owns is certainly still alive -- identical
    // semantics either way, since the timeout only governs the later
    // reads below, but now the option can never be set after the close
    // that would make it fail.
    let held: Vec<UnixStream> = (0..40)
        .map(|_| {
            let stream = UnixStream::connect(&socket_path).expect("connect within/beyond the cap");
            stream
                .set_read_timeout(Some(Duration::from_millis(30)))
                .expect("set the check's read timeout while the peer is still connected");
            stream
        })
        .collect();

    // Give the accept loop time to actually accept and process all of
    // them (accept a bounded number at a time; each accept is fast since
    // nothing here sends any data).
    std::thread::sleep(Duration::from_millis(300));

    // The connections beyond the cap must have been closed by the
    // endpoint (refused) rather than left open waiting on a spawned
    // authentication thread -- reading from them must observe EOF
    // (0 bytes) rather than blocking or returning data. The read timeout
    // itself (kept small, 30ms) is what bounds the held-but-accepted
    // connections' own checks below, which directly multiplies by up to
    // 32 held connections -- kept small so the test does not spend most
    // of its time waiting out timeouts on the connections that were
    // correctly accepted rather than refused.
    let mut refused_count = 0;
    for mut stream in held {
        let mut buffer = [0_u8; 1];
        if let Ok(0) = stream.read(&mut buffer) {
            refused_count += 1;
        }
    }
    // `MAX_CONCURRENT_CONNECTIONS` (32) is a private implementation
    // detail, not re-exported -- asserting a generous lower bound rather
    // than an exact count avoids this test depending on the constant's
    // precise value, while still conclusively proving *some* connections
    // were refused rather than every one of the 40 spawning a thread.
    assert!(
        refused_count >= 4,
        "connections beyond the concurrency cap must be refused (closed without a \
         response) -- only {refused_count} of 40 were, expected several once the cap \
         is exceeded"
    );

    drop(shutdown);
}

/// Response 113 Q3 item 1: the capability token must reach a spawned
/// adapter process only through `APPROVAL_TOKEN_ENV_VAR`. Verified against
/// a real, separately spawned child process reading back its own
/// environment -- not merely asserting the `Command` builder was called.
#[test]
fn inject_token_into_environment_sets_the_sanctioned_variable_on_a_real_child_process() {
    // Response 232: this test's own real `fork()`+exec is exactly the
    // mechanism diagnosed for the RFC-021/RFC-022 socket flakes (a
    // forked-not-yet-exec'd child inherits every fd open in the whole
    // process, including sockets another concurrently running test has
    // already `close()`d from its own thread's point of view). Sharing
    // `runtime::terminal::reader::tests`' own limiter narrows how many
    // real spawns can be in flight process-wide at once, not just within
    // this file.
    let _real_process_slot = RealProcessLimiter::acquire();
    let token_value = "a-real-looking-capability-token-0123456789";
    let proposal = crate::approval::CommandProposal::decode(
        crate::approval::PROTOCOL_VERSION,
        token_value.to_string(),
        "proposal-1".to_string(),
        vec!["true".to_string()],
        PathBuf::from("/tmp"),
        None,
        None,
    )
    .expect("test proposal must decode");

    let mut command = Command::new("sh");
    command.arg("-c").arg(format!(
        "printf '%s' \"${}\"",
        crate::approval::APPROVAL_TOKEN_ENV_VAR
    ));
    crate::approval::inject_token_into_environment(&mut command, proposal.run_token());

    let output = command.output().expect("spawn a real child process");
    assert!(output.status.success(), "child process must exit cleanly");
    assert_eq!(
        String::from_utf8(output.stdout).expect("child stdout must be valid UTF-8"),
        token_value,
        "the child process must see the token through APPROVAL_TOKEN_ENV_VAR"
    );
}

/// RFC-022 PR-022-C's own gate: "`inject_token_into_environment` gains
/// its first production caller; the enumeration naming that call site
/// fails by name if a second appears." Scans every non-test `.rs` file
/// under `tekstide-core/src/` for the literal call and asserts the exact,
/// closed list -- one production call site,
/// `runtime/terminal/launch.rs`'s `spawn_adapter`.
///
/// Needle is `"inject_token_into_environment(&mut command"`, not the bare
/// function name followed by `(` -- found live, by running this test
/// before choosing the needle: the bare form also matches this function's
/// own multi-line `pub fn inject_token_into_environment(` *definition*
/// (`&mut command` is textually absent there -- the parameter is written
/// `command: &mut std::process::Command`, name before type, the reverse
/// of a call site's `&mut command`), which would have made this file
/// itself count as a second "call site" it is not.
///
/// Excludes `/tests/`-directory files, not merely files literally named
/// `tests.rs` (the narrower filter an earlier enumeration in this crate
/// used, `reader/tests.rs`'s `count_occurrences_in_crate`) -- this
/// function's own existing test call site
/// (`inject_token_into_environment_sets_the_sanctioned_variable_on_a_real_child_process`,
/// above) lives at `approval/tests/channel.rs`, which the narrower filter
/// would not have excluded.
#[test]
fn inject_token_into_environment_has_exactly_one_production_call_site() {
    let occurrences = count_occurrences_in_crate("inject_token_into_environment(&mut command");
    assert_eq!(
        occurrences,
        vec![("runtime/terminal/launch.rs".to_string(), 1)],
        "exactly this one production call site may ever call inject_token_into_environment -- \
         a second appearing, or this one disappearing, means the token's single sanctioned \
         delivery choke point changed without this test being updated to say so: {occurrences:?}"
    );
}

/// Total occurrences of `needle` across `tekstide-core`'s production
/// `.rs` files, any file under a `tests/` directory or literally named
/// `tests.rs` excluded -- mirrors `project::diff::tests`'s own broader
/// filter (`relative.contains("/tests/") || relative.ends_with("tests.rs")`),
/// not `reader/tests.rs`'s narrower one, per this test's own doc comment
/// above on why the broader filter is required here.
fn count_occurrences_in_crate(needle: &str) -> Vec<(String, usize)> {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    let mut counts: Vec<(String, usize)> = files
        .into_iter()
        .filter_map(|path| {
            let relative = relative_to_src(&path);
            if relative.contains("/tests/") || relative.ends_with("tests.rs") {
                return None;
            }
            let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
            let count = source.matches(needle).count();
            (count > 0).then_some((relative, count))
        })
        .collect();
    counts.sort();
    counts
}

fn crate_src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn relative_to_src(path: &std::path::Path) -> String {
    path.strip_prefix(crate_src_dir())
        .expect("file must be under src/")
        .to_str()
        .expect("path must be valid UTF-8")
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
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

/// RFC-028 D4: this project's own open question, **answered by review
/// 597 on the real runner** -- does `/dev/fd/<fd>/<name>` on macOS have
/// the same TOCTOU-safety property `/proc/self/fd/<fd>/<name>` gives
/// `magic_fd_path` on Linux (reaching the file a verified directory
/// *fd* points to, immune to whatever has happened to the directory's
/// *pathname* since that fd was opened), or does it quietly re-resolve
/// by name instead? **No**: the magic path did not resolve at all
/// (`NOT TRAVERSABLE`, the likelier outcome this test's own assertion
/// already distinguished before the answer existed) -- `fdescfs`
/// re-opens the fd a descriptor names rather than supporting path
/// traversal through a directory descriptor the way `/proc/self/fd`
/// does. That answer is why `enter_bind_context`'s macOS arm
/// (`channel.rs`) uses `pthread_fchdir_np` plus a relative path
/// instead, not `/dev/fd` at all. **Kept as a standing regression
/// check, not deleted**, now that the question it was written to
/// answer is closed: this still exercises a true fact the design
/// decision depends on staying true, the same "rejected alternative
/// recorded, not removed" treatment this RFC has already given
/// `ptsname_r`'s own rejected lock-based answer in `pty.rs`.
///
/// Method: open a directory, create a file in it, record the file's
/// real identity (`fstat`'s `st_dev`/`st_ino`). Then rename the
/// *directory itself* out of the way and put a different directory
/// (containing a same-named file with different content) in its place
/// -- the directory-rename-race shape a TOCTOU attacker would need.
/// Stat the *same* `/dev/fd/<fd>/<name>` path again: if the identity
/// is unchanged, the magic path followed the fd, not the pathname, and
/// the hypothesis holds; if the identity changed (or now points at the
/// decoy), it does not, and `/dev/fd` cannot be used the way
/// `/proc/self/fd` is on Linux without its own, different safeguard.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "Review 641's own required disposition: this is not an open failure to triage, it is \
            a measured platform property with a design consequence already recorded. \
            /dev/fd/<fd>/<name> does not resolve at all on this BSD-derived /dev/fd (fdescfs \
            re-opens the fd itself rather than supporting path traversal through a directory \
            descriptor the way /proc/self/fd does on Linux) -- confirmed, not assumed, by this \
            test's own repeated 'NOT TRAVERSABLE' panic. RFC-028 D4 already answered the design \
            question this test exists to settle (pthread_fchdir_np plus a relative bind(), not \
            /dev/fd), so this test now only re-confirms a closed finding every run. Kept as a \
            standing regression check (run manually: cargo test --ignored \
            dev_fd_magic_path_follows_the_fd_not_a_renamed_pathname), not deleted, the same \
            degrade-and-record shape the APFS work already uses."]
fn dev_fd_magic_path_follows_the_fd_not_a_renamed_pathname() {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::MetadataExt;

    // Review 587: cleans up even if an assertion below panics, so a red
    // run on the real runner doesn't also leak a temp directory into the
    // next one.
    struct CleanupGuard(std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    let base = std::env::temp_dir().join(format!(
        "tekstide-dev-fd-toctou-{}-{}",
        std::process::id(),
        rand_seed()
    ));
    let _cleanup = CleanupGuard(base.clone());
    let real_dir = base.join("real");
    let decoy_dir = base.join("decoy");
    let moved_dir = base.join("moved-aside");
    std::fs::create_dir_all(&real_dir).expect("create real dir");
    std::fs::create_dir_all(&decoy_dir).expect("create decoy dir");

    std::fs::write(real_dir.join("probe"), b"real-content").expect("write real probe");
    std::fs::write(decoy_dir.join("probe"), b"decoy-content").expect("write decoy probe");

    let dir_fd = std::fs::File::open(&real_dir).expect("open real dir for its fd");
    let magic_path = format!("/dev/fd/{}/probe", dir_fd.as_raw_fd());

    // Review 587: the likelier outcome on a BSD-derived `/dev/fd`
    // (`fdescfs`, which re-opens the fd a descriptor names rather than
    // supporting `/proc`-style path traversal *through* a directory
    // descriptor) is that this access fails outright, not that it
    // succeeds unsafely. Named and asserted as its own, distinct
    // outcome -- "not traversable at all" -- rather than folded into
    // the same `.expect()` the later identity check uses, so a red run
    // says which of the two it hit.
    let before = match std::fs::metadata(&magic_path) {
        Ok(metadata) => metadata,
        Err(error) => panic!(
            "NOT TRAVERSABLE: /dev/fd/<fd>/<name> did not resolve at all ({error}) -- this is \
             the likelier outcome on a BSD-derived /dev/fd (fdescfs re-opens the fd itself; it \
             is not obliged to support path traversal through a directory descriptor the way \
             /proc/self/fd does on Linux). See RFC-028 D4's own two named fallbacks \
             (pthread_fchdir_np + a relative bind(), or accepting the already-verified \
             directory fd with the residual documented) -- this is not a bug in this test."
        ),
    };
    assert_eq!(
        std::fs::read(&magic_path).expect("read before the race"),
        b"real-content",
        "precondition: the magic path reads the real file before anything moves"
    );

    // The race: the directory this fd was opened from is renamed away,
    // and a different directory -- same name, different content at the
    // same relative filename -- takes its place.
    std::fs::rename(&real_dir, &moved_dir).expect("rename real dir aside");
    std::fs::rename(&decoy_dir, &real_dir).expect("rename decoy dir into place");

    let after = std::fs::metadata(&magic_path).unwrap_or_else(|error| {
        panic!(
            "the magic path resolved before the rename race but not after it ({error}) -- \
             neither \"not traversable\" nor \"traversable but unsafe\", a third shape worth \
             its own investigation rather than assuming either fallback applies"
        )
    });
    assert_eq!(
        (before.dev(), before.ino()),
        (after.dev(), after.ino()),
        "TRAVERSABLE BUT UNSAFE: dev/fd did not follow the verified fd -- the magic path's \
         target identity changed after the directory holding it was renamed, which means \
         /dev/fd re-resolves by pathname rather than following the fd the way /proc/self/fd \
         does on Linux. See RFC-028 D4's own two named fallbacks."
    );
    assert_eq!(
        std::fs::read(&magic_path).expect("read after the race"),
        b"real-content",
        "TRAVERSABLE BUT UNSAFE: dev/fd read the decoy's content after the rename race -- it \
         followed the pathname, not the fd. See RFC-028 D4's own two named fallbacks."
    );
}
