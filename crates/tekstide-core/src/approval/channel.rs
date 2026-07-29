//! RFC-021 PR-021-D: the sideband approval channel.
//!
//! **This is a security boundary in the same sense PR-021-C is, and the
//! task breakdown says so explicitly: "the slice most likely to contain a
//! real vulnerability."** Two properties matter more than anything else
//! here, and everything below is organized around them:
//!
//! 1. **Never the PTY stream.** This module only ever talks over a Unix
//!    domain socket under the Tekstide state root. There is no code path
//!    here that reads or writes terminal output.
//! 2. **Impersonation resistance.** A process that is not the adapter
//!    Tekstide spawned for *this specific* `AgentRun` must not be able to
//!    submit a proposal that gets treated as legitimate. Two independent
//!    layers enforce this, deliberately redundant:
//!    - **Peer credentials** (`SO_PEERCRED`, kernel-verified, unforgeable
//!      by the connecting process): the connecting process must run as
//!      the same Unix user as Tekstide itself. This rejects a different
//!      *user account* entirely.
//!    - **The per-run capability token**: same-user is not enough on a
//!      machine where the same user runs other processes. The token is
//!      generated fresh per `bind()` call, delivered to the real adapter
//!      only through the environment allowlist, and compared against
//!      every incoming proposal. A mismatch is rejected silently -- see
//!      "fail closed without a dialog" below.
//!
//! Neither layer alone is sufficient, and neither is redundant with the
//! other: same-user-but-wrong-token is the ordinary "another process I
//! happen to run" case; right-token-but-wrong-user should not be
//! reachable at all given the token is never on disk unencrypted, but the
//! peer-credential check means it does not matter if it somehow were.
//!
//! **Path validation is modelled directly on `audit::path`** (RFC-011/
//! RFC-013 discipline: absolute, canonicalized, symlink-rejecting, never
//! inside a project root) rather than inventing a new discipline for this
//! one case.
//!
//! **Fail closed without a dialog.** Per the RFC's fail-closed matrix: an
//! invalid or absent token is rejected before a dialog is ever
//! constructed. A rejection path that renders a dialog is itself an
//! attack surface (a forged proposal could use the dialog's mere
//! appearance to phish a real approval) -- so authentication happens
//! entirely below where a dialog could exist, in this module, before
//! anything reaches `approval::coordinator`.
//!
//! **What this module does not decide:** whether `CommandProposal.cwd()`
//! is the *true* working directory of the run, as opposed to whatever the
//! adapter claims. Response 111 (PR-021-C re-review) flagged this
//! explicitly as a gap between two individually-correct scope decisions
//! (B declined path containment as out of scope for a protocol decoder;
//! C takes paths as given). This module does not resolve it either --
//! establishing the trusted `cwd` is `approval::coordinator`'s job, which
//! has access to the actual `AgentRun`/`ProjectSession` state this module
//! deliberately does not depend on.

use std::ffi::CString;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::AgentRunId;

use super::protocol::{
    CommandDecision, CommandProposal, DecisionOutcome, ProposalValidationError, RunCapabilityToken,
};

/// Hard cap on a single wire message, checked against the length prefix
/// *before* reading that many bytes -- an oversized declared length is
/// rejected without ever allocating a buffer for it. Set well above
/// `MAX_ARGV_TOTAL_LEN` (1 MiB): JSON string escaping can expand a byte up
/// to sixfold (`\u00XX`), so a legitimate proposal near the argv ceiling
/// with heavily-escaped content needs headroom beyond the raw argv bound
/// plus structural overhead, not just a small margin over it (response
/// 112 non-blocking-4).
const MAX_MESSAGE_FRAME_BYTES: u32 = 8 * 1024 * 1024;

/// How long `accept_proposal` waits for the initial proposal frame after a
/// connection is accepted, before giving up (response 112 Defect 3). A
/// same-user peer that connects and then sends nothing -- or a partial
/// frame -- would otherwise block this call forever, queuing every
/// subsequent connection (including the real adapter's) behind it. This
/// is deliberately unrelated to and much shorter than the RFC's "no
/// approval timeout" policy, which governs how long a *human* may take to
/// decide once a proposal has already been received -- that remains
/// unbounded. This timeout only bounds how long the adapter has to
/// finish *transmitting* its proposal, which a real adapter does need
/// generously more than instantly, but never anywhere near indefinitely.
///
/// Response 113 Q2: shortened from an initial 30 seconds to 5. The exact
/// value matters less than the shape of the fix -- a single-threaded
/// accept loop handling one connection at a time means an attacker who
/// loops connect-and-stall still occupies the one slot repeatedly, just
/// in shorter increments; concurrent connection handling (response 113
/// required E1 item) is the actual fix for that class of denial, not this
/// constant. 5 seconds is generous for a local adapter writing one
/// already-composed frame, which needs milliseconds even on a loaded
/// machine. Deliberately not configurable: a user-tunable timeout on a
/// security control invites setting it to something unhelpful, and there
/// is no legitimate reason for a local adapter to need longer.
const PROPOSAL_READ_TIMEOUT: Duration = Duration::from_secs(5);

// --- Path resolution, modelled on `audit::path` ---------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalChannelPathRequest {
    pub state_root: PathBuf,
    pub project_roots: Vec<PathBuf>,
}

impl ApprovalChannelPathRequest {
    pub fn new(state_root: impl Into<PathBuf>, project_roots: Vec<PathBuf>) -> Self {
        Self {
            state_root: state_root.into(),
            project_roots,
        }
    }
}

/// `state_root_fd` pins the state root directory as an open fd, captured
/// once in `ApprovalChannelPathResolver::resolve()` (response 113 Required
/// 2). `Arc` rather than a bare `OwnedFd` because `OwnedFd` is not `Clone`
/// and this directory value may be reused across multiple `bind()` calls
/// for different `AgentRun`s. `Eq`/`PartialEq` are deliberately not
/// derived (an fd is not a meaningful value to compare) -- nothing outside
/// this module ever compared two directories anyway.
#[derive(Clone, Debug)]
pub struct ApprovalChannelDirectory {
    state_root: PathBuf,
    channel_dir: PathBuf,
    state_root_fd: Arc<OwnedFd>,
}

impl ApprovalChannelDirectory {
    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn channel_dir(&self) -> &Path {
        &self.channel_dir
    }

    /// One socket per `AgentRun`, named by its id.
    ///
    /// Unix `sun_path` is bounded (~108 bytes on Linux); an unusually long
    /// state-root path can make this exceed that limit. `bind()` binds
    /// through a short `/proc/self/fd`-relative path rather than this real
    /// path (see `magic_fd_path`), so the limit no longer surfaces there --
    /// response 113 Required 1 found that this let a too-long path bind
    /// successfully and then fail silently at *connect* time instead, on
    /// the adapter's side, with an endpoint that looks healthy but can
    /// never be reached. `bind()` now checks this exact path's length
    /// explicitly and returns `ApprovalChannelErrorReason::SocketPathTooLong`
    /// before doing anything else, specifically so that failure mode is
    /// closed rather than moved.
    pub fn socket_path(&self, agent_run_id: &AgentRunId) -> PathBuf {
        self.channel_dir.join(format!("{agent_run_id}.sock"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalChannelPathErrorReason {
    StateRootNotAbsolute,
    InvalidStateRoot,
    InvalidProjectRoot,
    ProjectContainsChannelState,
    ChannelPathEscapesStateRoot,
    ChannelPathIsSymlink,
    ChannelPathTypeInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApprovalChannelPathError {
    pub reason: ApprovalChannelPathErrorReason,
}

impl ApprovalChannelPathError {
    fn new(reason: ApprovalChannelPathErrorReason) -> Self {
        Self { reason }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ApprovalChannelPathResolver;

impl ApprovalChannelPathResolver {
    pub fn resolve(
        self,
        request: ApprovalChannelPathRequest,
    ) -> Result<ApprovalChannelDirectory, ApprovalChannelPathError> {
        if !request.state_root.is_absolute() {
            return Err(ApprovalChannelPathError::new(
                ApprovalChannelPathErrorReason::StateRootNotAbsolute,
            ));
        }
        let state_root = canonicalize_dir(
            &request.state_root,
            ApprovalChannelPathErrorReason::InvalidStateRoot,
        )?;
        let channel_dir = state_root.join("approval");

        if !channel_dir.starts_with(&state_root) {
            return Err(ApprovalChannelPathError::new(
                ApprovalChannelPathErrorReason::ChannelPathEscapesStateRoot,
            ));
        }
        reject_symlink(&channel_dir)?;

        // Response 113 Required 2: pin the state root itself as an open fd,
        // right after resolving its pathname, rather than only pinning the
        // `approval` subdirectory at `bind()` time as the previous
        // revision did. `O_NOFOLLOW` on a single path component protects
        // only that component -- an ancestor swap (replacing `state_root`
        // itself with a symlink sometime between this call and a later
        // `bind()`) was still possible, one directory higher than the race
        // response 112 closed. An fd pins an inode: `bind()` resolves the
        // `approval` subdirectory via `openat` relative to `state_root_fd`
        // (see `bind()`), so no pathname swap at any level after this
        // point can redirect it, no matter how long a caller waits between
        // `resolve()` and `bind()`.
        fs::create_dir_all(&state_root).map_err(|_error| {
            ApprovalChannelPathError::new(ApprovalChannelPathErrorReason::InvalidStateRoot)
        })?;
        let state_root_fd = open_dir_no_follow(&state_root).map_err(|_error| {
            ApprovalChannelPathError::new(ApprovalChannelPathErrorReason::InvalidStateRoot)
        })?;

        let resolved = ApprovalChannelDirectory {
            state_root,
            channel_dir,
            state_root_fd: Arc::new(state_root_fd),
        };
        for project_root in request.project_roots {
            resolved.ensure_project_root_compatible(&project_root)?;
        }
        Ok(resolved)
    }
}

impl ApprovalChannelDirectory {
    pub fn ensure_project_root_compatible(
        &self,
        project_root: &Path,
    ) -> Result<(), ApprovalChannelPathError> {
        let project_root = canonicalize_dir(
            project_root,
            ApprovalChannelPathErrorReason::InvalidProjectRoot,
        )?;
        if self.channel_dir.starts_with(&project_root)
            || self.state_root == project_root
            || self.state_root.starts_with(&project_root)
        {
            return Err(ApprovalChannelPathError::new(
                ApprovalChannelPathErrorReason::ProjectContainsChannelState,
            ));
        }
        Ok(())
    }
}

fn canonicalize_dir(
    path: &Path,
    reason: ApprovalChannelPathErrorReason,
) -> Result<PathBuf, ApprovalChannelPathError> {
    match fs::canonicalize(path) {
        Ok(path) if path.is_dir() => Ok(path),
        Ok(_) => Err(ApprovalChannelPathError::new(reason)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // The directory may not exist yet on first use (this module
            // creates it) -- but its *parent* must, and must itself not
            // be a symlink-redirected path. `create_dir_all` in
            // `ensure_created` will fail loudly if the parent is missing.
            let Some(parent) = path.parent() else {
                return Err(ApprovalChannelPathError::new(reason));
            };
            match fs::canonicalize(parent) {
                Ok(canonical_parent) if canonical_parent.is_dir() => Ok(canonical_parent.join(
                    path.file_name()
                        .ok_or_else(|| ApprovalChannelPathError::new(reason))?,
                )),
                _ => Err(ApprovalChannelPathError::new(reason)),
            }
        }
        Err(_) => Err(ApprovalChannelPathError::new(reason)),
    }
}

fn reject_symlink(path: &Path) -> Result<(), ApprovalChannelPathError> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(ApprovalChannelPathError::new(
                ApprovalChannelPathErrorReason::ChannelPathIsSymlink,
            ));
        }
        if !metadata.is_dir() {
            return Err(ApprovalChannelPathError::new(
                ApprovalChannelPathErrorReason::ChannelPathTypeInvalid,
            ));
        }
    }
    Ok(())
}

// --- Token generation --------------------------------------------------

/// Generates a fresh per-run capability token: two concatenated UUIDv4s
/// (each backed by a real CSPRNG via the already-depended-on `uuid` crate
/// -- reused rather than adding a `rand`/`getrandom` dependency solely for
/// this) for a comfortable margin beyond what a single UUIDv4's ~122 bits
/// would already provide for a short-lived, local-machine credential.
/// Lowercase hex only, so it trivially satisfies `RunCapabilityToken`'s
/// ASCII-graphic bound.
fn generate_capability_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

// --- The endpoint --------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalChannelErrorReason {
    Io,
    OversizedFrame,
    MalformedFrame,
    InvalidProposal,
    TokenMismatch,
    PeerCredentialMismatch,
    ConnectionClosed,
    ReadTimedOut,
    /// Response 113 Required 1: the real `socket_path` (not the short
    /// `/proc/self/fd`-relative path `bind()` actually binds through)
    /// exceeds what a `sockaddr_un` can hold. Distinguished from a bare
    /// `Io` specifically so this fails loudly and identifiably at
    /// `bind()`, rather than binding successfully and leaving every
    /// adapter connect attempt to fail later with an unrelated-looking
    /// error against an endpoint that otherwise looks healthy.
    SocketPathTooLong,
    /// Response 113 Q1 item 1: the `/proc/self/fd/<fd>/<name>` magic path
    /// this module binds through could not be used -- almost always
    /// because `/proc` is not mounted (some minimal containers and
    /// hardened configurations omit it; Tekstide is Linux-only and
    /// desktop-targeted, so this is expected to be rare). Distinguished
    /// from a bare `Io` so this specific, environmental precondition is
    /// identifiable rather than indistinguishable from any other bind
    /// failure.
    ProcMagicPathUnavailable,
}

#[derive(Debug)]
pub struct ApprovalChannelError {
    pub reason: ApprovalChannelErrorReason,
    source: Option<io::Error>,
}

impl ApprovalChannelError {
    fn new(reason: ApprovalChannelErrorReason) -> Self {
        Self {
            reason,
            source: None,
        }
    }

    fn io(error: io::Error) -> Self {
        Self {
            reason: ApprovalChannelErrorReason::Io,
            source: Some(error),
        }
    }

    /// The underlying OS error, when `reason` is `Io`. OS error messages
    /// (e.g. "Connection refused") are not user data, so exposing this for
    /// diagnostics does not conflict with keeping `reason` itself
    /// content-free.
    pub fn source(&self) -> Option<&io::Error> {
        self.source.as_ref()
    }
}

/// A successfully authenticated proposal, still holding the open
/// connection it arrived on -- the same connection is used to send the
/// eventual `CommandDecision` back, since the protocol has no separate
/// mechanism to address a decision to a specific adapter connection.
#[derive(Debug)]
pub struct AcceptedProposal {
    pub proposal: CommandProposal,
    stream: UnixStream,
}

impl AcceptedProposal {
    pub fn send_decision(
        &mut self,
        decision: &CommandDecision,
    ) -> Result<(), ApprovalChannelError> {
        let wire = WireCommandDecision::from_decision(decision);
        let bytes = serde_json::to_vec(&wire).map_err(|_error| {
            ApprovalChannelError::new(ApprovalChannelErrorReason::MalformedFrame)
        })?;
        write_frame(&mut self.stream, &bytes)
    }
}

pub struct ApprovalChannelEndpoint {
    listener: UnixListener,
    socket_path: PathBuf,
    expected_token: RunCapabilityToken,
    owner_uid: u32,
    read_timeout: Duration,
}

impl ApprovalChannelEndpoint {
    /// Creates the endpoint **before** the adapter process starts, per the
    /// RFC's lifecycle. Returns the raw token string alongside `Self` --
    /// the caller (the launch path, not this module) is responsible for
    /// delivering it through the environment allowlist and never writing
    /// it to disk unencrypted or into an audit record.
    pub fn bind(
        directory: &ApprovalChannelDirectory,
        agent_run_id: &AgentRunId,
    ) -> Result<(Self, String), ApprovalChannelError> {
        let socket_path = directory.socket_path(agent_run_id);

        // Response 113 Required 1: checked against the *real* path, up
        // front, before anything else. Binding below goes through a short
        // `/proc/self/fd`-relative path (see `magic_fd_path`'s doc
        // comment for why that shortness is incidental, not a feature),
        // which no longer fails here on a long real path the way the
        // previous revision did -- it would instead bind successfully and
        // leave a real adapter's later `connect()` (which must use the
        // real path) to fail against a healthy-looking, unreachable
        // endpoint. That is a harder failure to diagnose than the one it
        // replaced, so it is closed explicitly here instead.
        if socket_path.as_os_str().len() > max_socket_path_len() {
            return Err(ApprovalChannelError::new(
                ApprovalChannelErrorReason::SocketPathTooLong,
            ));
        }

        // Response 113 Required 2: `mkdirat`/`openat` relative to the
        // state-root fd pinned once in `resolve()`, instead of
        // re-resolving `channel_dir`'s pathname here the way the previous
        // revision's `open_dir_no_follow(directory.channel_dir())` did.
        // That call re-resolved `state_root`'s pathname on every `bind()`,
        // so a same-user swap of `state_root` itself (not just the
        // `approval` subdirectory) between `resolve()` and `bind()` still
        // worked -- `state_root_fd` was opened once and never re-resolves
        // its own pathname again, closing that too.
        let state_root_fd = directory.state_root_fd.as_raw_fd();
        mkdirat_if_missing(state_root_fd, "approval", 0o700).map_err(ApprovalChannelError::io)?;
        let dir_fd =
            openat_dir_no_follow(state_root_fd, "approval").map_err(ApprovalChannelError::io)?;
        fchmod(dir_fd.as_raw_fd(), 0o700).map_err(ApprovalChannelError::io)?;

        // Response 113 Q1 item 1: this fd-relative approach depends on
        // `/proc/self/fd` being mounted and functional. Checked here,
        // against the fd this call just opened itself, so a failure is
        // unambiguously about `/proc` rather than about the eventual bind
        // target -- and surfaces as a distinguished reason instead of a
        // bare `Io` that would otherwise look identical to an unrelated
        // bind failure.
        verify_proc_fd_magic_path_available(&dir_fd).map_err(|_error| {
            ApprovalChannelError::new(ApprovalChannelErrorReason::ProcMagicPathUnavailable)
        })?;

        let filename = format!("{agent_run_id}.sock");
        let bind_path = magic_fd_path(&dir_fd, &filename);

        clear_stale_socket(&bind_path, &socket_path)?;

        let listener = UnixListener::bind(&bind_path).map_err(ApprovalChannelError::io)?;
        // Empirically, `fchmod` on the *listening socket's own fd* does
        // not reliably update the bound special file's mode bits (tried
        // first; the socket file kept its umask-derived default despite
        // `fchmod` reporting success) -- unlike a plain directory fd,
        // where `fchmod` unambiguously works, a socket fd's relationship
        // to the special file's inode metadata is evidently not the same
        // thing. Falling back to a path-based `set_permissions`, but
        // still through `bind_path` (the `/proc/self/fd`-relative path)
        // rather than the plain path, so this still resolves through the
        // already-verified directory fd instead of re-resolving a
        // separate pathname.
        fs::set_permissions(&bind_path, fs::Permissions::from_mode(0o600))
            .map_err(ApprovalChannelError::io)?;

        let raw_token = generate_capability_token();
        let expected_token = super::protocol::validate_token(raw_token.clone())
            .expect("a freshly generated token must satisfy the protocol's own bounds");

        let owner_uid = current_euid();

        Ok((
            Self {
                listener,
                socket_path,
                expected_token,
                owner_uid,
                read_timeout: PROPOSAL_READ_TIMEOUT,
            },
            raw_token,
        ))
    }

    /// Blocks until a peer connects, then authenticates it through both
    /// layers described in the module doc before reading anything the
    /// peer sends further. A connection that fails either layer is
    /// dropped immediately -- no diagnostic is returned to the peer, and
    /// nothing here constructs anything a dialog could render, per the
    /// fail-closed-without-a-dialog requirement.
    pub fn accept_proposal(&self) -> Result<AcceptedProposal, ApprovalChannelError> {
        let (mut stream, _addr) = self.listener.accept().map_err(ApprovalChannelError::io)?;

        // Response 112 Defect 3: without this, a same-user peer that
        // connects and then sends nothing (or a partial frame) blocks
        // this call -- and every connection after it -- forever.
        stream
            .set_read_timeout(Some(self.read_timeout))
            .map_err(ApprovalChannelError::io)?;

        let peer_uid = peer_uid(&stream).map_err(ApprovalChannelError::io)?;
        if peer_uid != self.owner_uid {
            // Deliberately no message is sent back; the connection is
            // simply dropped. Treat as a possible spoofing attempt, per
            // implementation-handoff.md §5.
            return Err(ApprovalChannelError::new(
                ApprovalChannelErrorReason::PeerCredentialMismatch,
            ));
        }

        let frame = read_frame(&mut stream)?;
        let wire: WireCommandProposal = serde_json::from_slice(&frame).map_err(|_error| {
            ApprovalChannelError::new(ApprovalChannelErrorReason::MalformedFrame)
        })?;

        // Response 112 Recommended 7: checked immediately after parsing,
        // before the full `CommandProposal::decode` -- an unauthenticated
        // peer's bytes reach strictly less code this way, and a mismatch
        // is unambiguously reported as `TokenMismatch` rather than
        // whatever `decode` happened to reject first.
        if !self.expected_token.matches_raw(&wire.run_token) {
            return Err(ApprovalChannelError::new(
                ApprovalChannelErrorReason::TokenMismatch,
            ));
        }

        let proposal = wire.into_proposal().map_err(|_error| {
            ApprovalChannelError::new(ApprovalChannelErrorReason::InvalidProposal)
        })?;

        Ok(AcceptedProposal { proposal, stream })
    }
}

#[cfg(test)]
impl ApprovalChannelEndpoint {
    /// So a timeout test does not have to wait the real
    /// `PROPOSAL_READ_TIMEOUT`.
    pub(crate) fn set_read_timeout_for_test(&mut self, timeout: Duration) {
        self.read_timeout = timeout;
    }

    /// Response 112 Q1: there is no second real user account (or root)
    /// available to exercise the peer-credential-*mismatch* branch
    /// end-to-end -- that would require an actually different kernel-
    /// reported UID, which nothing in this process can produce. This
    /// converts an entirely untested branch into one where the
    /// *comparison* (`peer_uid != self.owner_uid`) is exercised for real,
    /// by varying the other operand instead: the peer's real UID (read via
    /// a real `getsockopt(SO_PEERCRED)` call, unchanged and unfaked) is
    /// compared against a deliberately wrong expected value. This does
    /// not prove `getsockopt` reads the kernel's value correctly -- only
    /// a second real account could -- but it is a meaningfully smaller
    /// residual claim than "the comparison is untested."
    pub(crate) fn set_owner_uid_for_test(&mut self, uid: u32) {
        self.owner_uid = uid;
    }
}

impl Drop for ApprovalChannelEndpoint {
    fn drop(&mut self) {
        // Best-effort: destroys the endpoint on AgentRun termination, per
        // the RFC's lifecycle requirement that no orphaned socket remains.
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// If a socket file already exists at this path, determine whether it is
/// a live listener (a genuine conflict -- refuse to touch it) or a stale
/// leftover from an ungraceful prior termination (safe to remove and
/// rebind). `bind_path` (the `/proc/self/fd`-relative path) is used for
/// both the type check and the connect attempt, so both go through the
/// already-verified directory fd rather than re-resolving a plain
/// pathname; `removal_path` is the plain path used only for the final
/// `remove_file`, which is a no-op-if-wrong operation (removing a stale
/// socket by name either succeeds or the subsequent `bind` fails loudly,
/// it cannot silently redirect anything).
fn clear_stale_socket(bind_path: &Path, removal_path: &Path) -> Result<(), ApprovalChannelError> {
    let metadata = match fs::symlink_metadata(bind_path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(()),
    };
    if !metadata.file_type().is_socket() {
        // Response 112 Q3: connecting to a *regular file* also yields
        // `ECONNREFUSED`, which the previous version of this function
        // could not distinguish from a genuinely stale socket -- any
        // non-socket file at this path was silently deleted. Refuse
        // outright rather than attempt to connect to something that was
        // never a socket in the first place.
        return Err(ApprovalChannelError::new(ApprovalChannelErrorReason::Io));
    }
    match UnixStream::connect(bind_path) {
        Ok(_) => Err(ApprovalChannelError::new(ApprovalChannelErrorReason::Io)),
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
            fs::remove_file(removal_path).map_err(ApprovalChannelError::io)
        }
        Err(_) => {
            // Any other error (permission denied, etc.) is treated the
            // same as "do not touch it" -- only a confirmed stale
            // listener is cleared.
            Err(ApprovalChannelError::new(ApprovalChannelErrorReason::Io))
        }
    }
}

/// Opens `path` with `O_NOFOLLOW`: fails if the final path component is a
/// symlink *at the moment of this call*, which is what makes it safe to
/// use as the basis for every subsequent operation on this directory
/// (response 112 Defect 1) -- unlike a plain `fs::symlink_metadata` check
/// followed by a later path-based operation, there is no gap here between
/// "checked" and "used": they are the same fd.
///
/// Used directly only in `resolve()`, to pin `state_root` itself. Opening
/// the `approval` subdirectory later goes through `openat_dir_no_follow`
/// relative to that pinned fd instead (response 113 Required 2) -- calling
/// this function on `channel_dir`'s full pathname, as the previous
/// revision of `bind()` did, re-resolved `state_root`'s pathname every
/// time and left an ancestor-swap race open one directory higher than the
/// `approval` component this originally closed.
fn open_dir_no_follow(path: &Path) -> io::Result<OwnedFd> {
    let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_error| {
        io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte")
    })?;
    // SAFETY: `c_path` is a valid, NUL-terminated C string for the
    // lifetime of this call. `open` either returns a valid owned fd or a
    // negative value with `errno` set; both are handled below.
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_RDONLY,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` was just returned by a successful `open` call above
    // and is not owned anywhere else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Creates `name` as a subdirectory of the directory `dir_fd` refers to,
/// tolerating `AlreadyExists` (this runs on every `bind()` call, not just
/// the first). Relative to an already-open fd rather than a pathname, so
/// this cannot be redirected by any pathname swap of an ancestor directory
/// (response 113 Required 2) -- unlike the plain-path `fs::create_dir_all`
/// the previous revision used here.
fn mkdirat_if_missing(dir_fd: RawFd, name: &str, mode: u32) -> io::Result<()> {
    let c_name = CString::new(name).map_err(|_error| {
        io::Error::new(io::ErrorKind::InvalidInput, "name contains a NUL byte")
    })?;
    // SAFETY: `c_name` is a valid, NUL-terminated C string for the
    // lifetime of this call; `dir_fd` is a valid, currently-open directory
    // fd owned by the caller for at least that long.
    let result = unsafe { libc::mkdirat(dir_fd, c_name.as_ptr(), mode as libc::mode_t) };
    if result != 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
    Ok(())
}

/// `open_dir_no_follow`, but relative to an already-open directory fd
/// (`libc::openat`) rather than resolving a pathname from the process's
/// current working directory. This is the `approval`-subdirectory
/// counterpart to pinning `state_root` in `resolve()`: because `dir_fd`
/// was opened once, at `resolve()` time, and never re-resolves its own
/// pathname again, no pathname swap of `state_root` (or anything above
/// it) after that point can redirect where this ends up opening
/// (response 113 Required 2).
fn openat_dir_no_follow(dir_fd: RawFd, name: &str) -> io::Result<OwnedFd> {
    let c_name = CString::new(name).map_err(|_error| {
        io::Error::new(io::ErrorKind::InvalidInput, "name contains a NUL byte")
    })?;
    // SAFETY: `c_name` is a valid, NUL-terminated C string for the
    // lifetime of this call; `dir_fd` is a valid, currently-open directory
    // fd owned by the caller for at least that long. `openat` either
    // returns a valid owned fd or a negative value with `errno` set; both
    // are handled below.
    let fd = unsafe {
        libc::openat(
            dir_fd,
            c_name.as_ptr(),
            libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_RDONLY,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` was just returned by a successful `openat` call above
    // and is not owned anywhere else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// A path that resolves through the kernel's fd table (Linux's `/proc/
/// self/fd` magic symlinks) rather than by re-resolving `dir_fd`'s
/// directory pathname from scratch. Binding or chmod-ing through this
/// path cannot be redirected by anything that happens to the directory's
/// *pathname* after `dir_fd` was opened, since the fd already refers
/// directly to the verified directory's inode.
///
/// Response 113 Q1 item 3: this path being short is incidental to closing
/// the TOCTOU race, not a feature -- it is what let a `socket_path` longer
/// than `sun_path`'s capacity bind successfully here and fail only later,
/// at `connect()`, on the adapter's side (Required 1). The explicit length
/// check in `bind()` against the *real* `socket_path` is what enforces
/// the limit now; nobody should remove that check on the theory that
/// binding through this short path has lifted it.
fn magic_fd_path(dir_fd: &OwnedFd, filename: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/{filename}", dir_fd.as_raw_fd()))
}

/// The longest path `bind`/`connect` can accept in a `sockaddr_un`, not
/// counting the mandatory trailing NUL terminator the kernel requires
/// (response 113 Required 1).
fn max_socket_path_len() -> usize {
    let sun_path_len =
        std::mem::size_of::<libc::sockaddr_un>() - std::mem::size_of::<libc::sa_family_t>();
    sun_path_len - 1
}

/// Confirms `/proc/self/fd/<fd>` resolves, for the fd this module just
/// opened itself, before relying on the same mechanism to construct a
/// bind path (response 113 Q1 item 1). `/proc` not being mounted is the
/// expected cause of failure here -- rare on Tekstide's Linux desktop
/// target, but not impossible in minimal containers or hardened
/// configurations -- and checking against a fd we know is valid isolates
/// that specific precondition from any other reason a later operation on
/// the constructed path might fail.
fn verify_proc_fd_magic_path_available(dir_fd: &OwnedFd) -> io::Result<()> {
    let self_path = PathBuf::from(format!("/proc/self/fd/{}", dir_fd.as_raw_fd()));
    fs::metadata(&self_path)?;
    Ok(())
}

fn fchmod(fd: RawFd, mode: u32) -> io::Result<()> {
    // SAFETY: `fd` is a valid, currently-open file descriptor for the
    // duration of this call (the caller retains ownership across it).
    let result = unsafe { libc::fchmod(fd, mode as libc::mode_t) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn current_euid() -> u32 {
    // SAFETY: geteuid() takes no arguments, has no preconditions, and
    // cannot fail.
    unsafe { libc::geteuid() }
}

/// Reads the connecting process's real, kernel-verified user id via
/// `SO_PEERCRED`. This is not something the connecting process can spoof:
/// the kernel fills in `ucred` from the actual socket peer at `connect()`
/// time, not from anything the peer transmits.
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let fd = stream.as_raw_fd();
    // SAFETY: `cred` is a plain-old-data struct with no invariants beyond
    // its fields being initialized, which `getsockopt` does on success;
    // `len` is initialized to the struct's size, matching what
    // `getsockopt` expects for a fixed-size option, and is unused if the
    // call fails.
    unsafe {
        let mut cred: libc::ucred = std::mem::zeroed();
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut libc::c_void,
            &mut len,
        );
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(cred.uid)
    }
}

// --- Wire framing --------------------------------------------------

fn write_frame(stream: &mut UnixStream, bytes: &[u8]) -> Result<(), ApprovalChannelError> {
    let len = u32::try_from(bytes.len())
        .map_err(|_error| ApprovalChannelError::new(ApprovalChannelErrorReason::OversizedFrame))?;
    if len > MAX_MESSAGE_FRAME_BYTES {
        return Err(ApprovalChannelError::new(
            ApprovalChannelErrorReason::OversizedFrame,
        ));
    }
    stream
        .write_all(&len.to_be_bytes())
        .map_err(ApprovalChannelError::io)?;
    stream.write_all(bytes).map_err(ApprovalChannelError::io)
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, ApprovalChannelError> {
    let mut len_bytes = [0_u8; 4];
    stream.read_exact(&mut len_bytes).map_err(map_read_error)?;
    let len = u32::from_be_bytes(len_bytes);
    if len > MAX_MESSAGE_FRAME_BYTES {
        return Err(ApprovalChannelError::new(
            ApprovalChannelErrorReason::OversizedFrame,
        ));
    }
    let mut buffer = vec![0_u8; len as usize];
    stream.read_exact(&mut buffer).map_err(map_read_error)?;
    Ok(buffer)
}

/// A read timeout (response 112 Defect 3) can surface as either
/// `WouldBlock` or `TimedOut` depending on platform, so both are treated
/// as the same distinguished, expected outcome rather than a generic I/O
/// error.
fn map_read_error(error: io::Error) -> ApprovalChannelError {
    match error.kind() {
        io::ErrorKind::UnexpectedEof => {
            ApprovalChannelError::new(ApprovalChannelErrorReason::ConnectionClosed)
        }
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
            ApprovalChannelError::new(ApprovalChannelErrorReason::ReadTimedOut)
        }
        _ => ApprovalChannelError::io(error),
    }
}

/// `deny_unknown_fields` (response 112 non-blocking-3): an unrecognized
/// field in an otherwise-valid message sat oddly next to "unknown
/// protocol version rejected without negotiation" -- both are the same
/// kind of "the message shape is not one we agreed to," and both should
/// fail closed the same way.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCommandProposal {
    protocol_version: u32,
    run_token: String,
    proposal_id: String,
    argv: Vec<String>,
    cwd: String,
    declared_intent: Option<String>,
    declared_effects: Option<String>,
}

impl WireCommandProposal {
    fn into_proposal(self) -> Result<CommandProposal, ProposalValidationError> {
        CommandProposal::decode(
            self.protocol_version,
            self.run_token,
            self.proposal_id,
            self.argv,
            PathBuf::from(self.cwd),
            self.declared_intent,
            self.declared_effects,
        )
    }
}

#[derive(Serialize)]
struct WireCommandDecision {
    protocol_version: u32,
    proposal_id: String,
    outcome: &'static str,
    edited_argv: Option<Vec<String>>,
}

impl WireCommandDecision {
    fn from_decision(decision: &CommandDecision) -> Self {
        Self {
            protocol_version: super::protocol::PROTOCOL_VERSION,
            proposal_id: decision.proposal_id().as_str().to_string(),
            outcome: match decision.outcome() {
                DecisionOutcome::ApprovedOnce => "approved_once",
                DecisionOutcome::Rejected => "rejected",
                DecisionOutcome::EditedAndApproved => "edited_and_approved",
            },
            edited_argv: decision.edited_argv().map(|argv| argv.to_vec()),
        }
    }
}
