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
const PROPOSAL_READ_TIMEOUT: Duration = Duration::from_secs(30);

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalChannelDirectory {
    state_root: PathBuf,
    channel_dir: PathBuf,
}

impl ApprovalChannelDirectory {
    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn channel_dir(&self) -> &Path {
        &self.channel_dir
    }

    /// One socket per `AgentRun`, named by its id. Unix `sun_path` is
    /// bounded (108 bytes on Linux) -- an unusually long state-root path
    /// can make this exceed that limit, which surfaces as a bind-time
    /// `io::Error` (`ApprovalChannelError::Io`) rather than being
    /// pre-validated here. Recorded as a known limitation rather than
    /// worked around with a hashed/shortened name, since doing that well
    /// would need a hashing dependency this crate does not otherwise need.
    pub fn socket_path(&self, agent_run_id: &AgentRunId) -> PathBuf {
        self.channel_dir.join(format!("{agent_run_id}.sock"))
    }

    fn ensure_created(&self) -> io::Result<()> {
        fs::create_dir_all(&self.channel_dir)
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

        let resolved = ApprovalChannelDirectory {
            state_root,
            channel_dir,
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
        directory
            .ensure_created()
            .map_err(ApprovalChannelError::io)?;

        // Opened with `O_NOFOLLOW` immediately before use (response 112
        // Defect 1): `resolve()` already rejected a symlinked channel
        // directory once, but that check happens earlier and the result
        // can decay -- a same-user process could swap the directory for a
        // symlink in the window between `resolve()` and here. Opening
        // again, right before use, re-checks at the only moment that
        // actually matters. Everything below operates through this fd
        // (via the `/proc/self/fd` magic path) rather than by
        // re-resolving `channel_dir`'s pathname a second time, so nothing
        // after this point can be redirected by a later swap either.
        let dir_fd =
            open_dir_no_follow(directory.channel_dir()).map_err(ApprovalChannelError::io)?;
        fchmod(dir_fd.as_raw_fd(), 0o700).map_err(ApprovalChannelError::io)?;

        let filename = format!("{agent_run_id}.sock");
        let bind_path = magic_fd_path(&dir_fd, &filename);
        let socket_path = directory.socket_path(agent_run_id);

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
    /// So a timeout test does not have to wait the real 30-second
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

/// A path that resolves through the kernel's fd table (Linux's `/proc/
/// self/fd` magic symlinks) rather than by re-resolving `dir_fd`'s
/// directory pathname from scratch. Binding or chmod-ing through this
/// path cannot be redirected by anything that happens to the directory's
/// *pathname* after `dir_fd` was opened, since the fd already refers
/// directly to the verified directory's inode -- this is what actually
/// closes the race window `open_dir_no_follow`'s doc comment describes,
/// rather than merely narrowing it.
fn magic_fd_path(dir_fd: &OwnedFd, filename: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/{filename}", dir_fd.as_raw_fd()))
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
