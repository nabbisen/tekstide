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
//!    - **Peer credentials** (`SO_PEERCRED` on Linux, `LOCAL_PEERCRED` on
//!      macOS -- RFC-028 D4, both kernel-verified and unforgeable by the
//!      connecting process): the connecting process must run as the same
//!      Unix user as Tekstide itself. This rejects a different *user
//!      account* entirely.
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
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
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

/// Hard cap on connections being authenticated at once by
/// `serve_concurrently` (response 114 Required 4). Fixing the
/// one-at-a-time serialization denial (response 113 Q2) by spawning a
/// thread per connection introduced a different denial: nothing capped
/// how many threads a same-user process could make this endpoint spawn by
/// opening connections and sending nothing, each holding a thread for the
/// full `PROPOSAL_READ_TIMEOUT`. A connection beyond this cap is refused
/// silently -- dropped without being read from or written to -- consistent
/// with the fail-closed-without-a-dialog requirement; nothing here should
/// ever cause a diagnostic to reach an unauthenticated peer. Generous for
/// a local, single-user desktop tool: legitimate concurrent proposals
/// across a handful of `AgentRun`s come nowhere near this.
const MAX_CONCURRENT_CONNECTIONS: usize = 32;

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
///
/// **`channel_dir` is not always under `state_root`.** Linux:
/// `state_root.join("approval")`, same as always, and `state_root_fd`
/// anchors `bind()`'s own `openat`. macOS (review 600): a short, per-user
/// temp location instead (see [`ApprovalChannelPathResolver::resolve`]'s
/// own doc for the arithmetic) -- `sockaddr_un`'s `sun_path` cannot hold
/// the real `AgentRunId`-based filename under a state root as long as
/// `~/Library/Application Support/tekstide/approval`, and no bind-time
/// trick (`pthread_fchdir_np` included) shortens the path the *adapter*
/// must later `connect()` to, since that one is never resolved relative
/// to anything. `channel_anchor_fd` is macOS's own separately-pinned
/// parent-of-`channel_dir` fd for that case, protecting the same
/// ancestor-swap race `state_root_fd` protects Linux's `channel_dir`
/// against -- D3's own `Application Support` decision for *durable*
/// state is untouched; only the socket's own, non-durable directory
/// moves.
#[derive(Clone, Debug)]
pub struct ApprovalChannelDirectory {
    state_root: PathBuf,
    channel_dir: PathBuf,
    state_root_fd: Arc<OwnedFd>,
    #[cfg(target_os = "macos")]
    channel_anchor_fd: Arc<OwnedFd>,
}

impl ApprovalChannelDirectory {
    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn channel_dir(&self) -> &Path {
        &self.channel_dir
    }

    /// One socket per `AgentRun`, named by its bare UUID suffix (review
    /// 600 drops the `agent-run-` prefix `AgentRunId`'s own `Display`
    /// carries -- meaningful in a log line, pure overhead in a path
    /// this crate already knows holds nothing else) -- `.sock` appended.
    ///
    /// Unix `sun_path` is bounded (103 usable bytes on macOS, 107 on
    /// Linux -- see `max_socket_path_len`'s own doc for why the two
    /// differ and why the formula computing them reads the field's own
    /// length rather than deriving it). `bind()` binds through a short,
    /// verified-fd-relative path rather than this real one (Linux:
    /// `/proc/self/fd`; macOS: `pthread_fchdir_np` -- see
    /// `enter_bind_context`), so the limit does not surface *there* --
    /// response 113 Required 1 found that this let a too-long path bind
    /// successfully and then fail silently at *connect* time instead, on
    /// the adapter's side, with an endpoint that looks healthy but can
    /// never be reached. `bind()` now checks this exact path's length
    /// explicitly and returns `ApprovalChannelErrorReason::SocketPathTooLong`
    /// before doing anything else, specifically so that failure mode is
    /// closed rather than moved.
    pub fn socket_path(&self, agent_run_id: &AgentRunId) -> PathBuf {
        self.channel_dir
            .join(format!("{}.sock", socket_filename_stem(agent_run_id)))
    }
}

/// `AgentRunId`'s own `Display` carries a `"agent-run-"` prefix meaningful
/// in a log line; stripped here because it is pure, unnecessary length in
/// a socket filename living inside a directory this crate already knows
/// holds nothing but approval sockets. Falls back to the full id if the
/// prefix is somehow absent (it never is, in practice -- `AgentRunId` is
/// only ever constructed with it) rather than panicking on an assumption
/// about a type this module does not own.
fn socket_filename_stem(agent_run_id: &AgentRunId) -> &str {
    agent_run_id
        .as_str()
        .strip_prefix("agent-run-")
        .unwrap_or(agent_run_id.as_str())
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
    /// Review 600, macOS only: `channel_dir`'s own parent (the per-user
    /// temp root, not `state_root`) could not be canonicalised, created,
    /// or pinned as an fd. Distinguished from `InvalidStateRoot` because
    /// it names a different directory -- a reader seeing `InvalidStateRoot`
    /// for a failure that has nothing to do with the state root would be
    /// actively misled, the same reasoning `ThreadChdirUnavailable` was
    /// given its own name for on `ApprovalChannelError`'s side.
    ChannelAnchorUnavailable,
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

        // Response 113 Required 2: pin the state root itself as an open fd,
        // right after resolving its pathname, rather than only pinning the
        // `approval` subdirectory at `bind()` time as the previous
        // revision did. `O_NOFOLLOW` on a single path component protects
        // only that component -- an ancestor swap (replacing `state_root`
        // itself with a symlink sometime between this call and a later
        // `bind()`) was still possible, one directory higher than the race
        // response 112 closed. An fd pins an inode: Linux's `bind()`
        // resolves the `approval` subdirectory via `openat` relative to
        // `state_root_fd` (see `bind()`), so no pathname swap at any level
        // after this point can redirect it, no matter how long a caller
        // waits between `resolve()` and `bind()`.
        fs::create_dir_all(&state_root).map_err(|_error| {
            ApprovalChannelPathError::new(ApprovalChannelPathErrorReason::InvalidStateRoot)
        })?;
        let state_root_fd = open_dir_no_follow(&state_root).map_err(|_error| {
            ApprovalChannelPathError::new(ApprovalChannelPathErrorReason::InvalidStateRoot)
        })?;

        let channel_dir = channel_dir_under(&state_root)?;
        reject_symlink(&channel_dir)?;

        let resolved = ApprovalChannelDirectory {
            state_root,
            channel_dir,
            state_root_fd: Arc::new(state_root_fd),
            #[cfg(target_os = "macos")]
            channel_anchor_fd: Arc::new(channel_anchor_fd()?),
        };
        for project_root in request.project_roots {
            resolved.ensure_project_root_compatible(&project_root)?;
        }
        Ok(resolved)
    }
}

/// Linux: `state_root.join("approval")`, same as always -- `bind()`
/// anchors its own `openat` on `state_root_fd` (already pinned by the
/// time this runs), so this is purely a pathname, checked here only for
/// the "did my own `.join` escape `state_root`" sanity property response
/// 113 originally added.
#[cfg(target_os = "linux")]
fn channel_dir_under(state_root: &Path) -> Result<PathBuf, ApprovalChannelPathError> {
    let channel_dir = state_root.join("approval");
    if !channel_dir.starts_with(state_root) {
        return Err(ApprovalChannelPathError::new(
            ApprovalChannelPathErrorReason::ChannelPathEscapesStateRoot,
        ));
    }
    Ok(channel_dir)
}

/// macOS (review 600): deliberately **not** under `state_root` at all.
/// `sockaddr_un`'s usable `sun_path` is 103 bytes there (see
/// `max_socket_path_len`'s own doc); a real `AgentRunId`-based socket
/// filename under a state root as long as `~/Library/Application
/// Support/tekstide/approval` does not fit, and no bind-time mechanism
/// shortens the path the adapter must later `connect()` to, since that
/// path is never resolved relative to anything -- it is the literal
/// bytes a `sockaddr_un` carries. `std::env::temp_dir()`'s own per-user,
/// `$TMPDIR`-backed directory is the Apple-sanctioned place for exactly
/// this (non-durable, 0700-able, short): D3's own `Application Support`
/// decision for *durable* state is untouched by this -- only the
/// socket's own directory moves, nothing else this crate persists.
/// `state_root` itself is still resolved and pinned above regardless of
/// platform, since `ensure_project_root_compatible` and this type's own
/// public accessors still mean what they always have.
#[cfg(target_os = "macos")]
fn channel_dir_under(_state_root: &Path) -> Result<PathBuf, ApprovalChannelPathError> {
    let tmp_root = canonicalize_dir(
        &std::env::temp_dir(),
        ApprovalChannelPathErrorReason::ChannelAnchorUnavailable,
    )?;
    Ok(tmp_root.join("tekstide"))
}

/// macOS only: pins `channel_dir`'s own parent (`$TMPDIR`, not
/// `state_root`) as an open fd, the same ancestor-swap protection
/// `state_root_fd` gives Linux's `channel_dir` -- `bind()` resolves
/// `channel_dir`'s own subdirectory (`"tekstide"`) via `openat` relative
/// to this fd, not by re-resolving `$TMPDIR`'s pathname.
#[cfg(target_os = "macos")]
fn channel_anchor_fd() -> Result<OwnedFd, ApprovalChannelPathError> {
    let tmp_root = canonicalize_dir(
        &std::env::temp_dir(),
        ApprovalChannelPathErrorReason::ChannelAnchorUnavailable,
    )?;
    fs::create_dir_all(&tmp_root).map_err(|_error| {
        ApprovalChannelPathError::new(ApprovalChannelPathErrorReason::ChannelAnchorUnavailable)
    })?;
    let anchor_fd = open_dir_no_follow(&tmp_root).map_err(|_error| {
        ApprovalChannelPathError::new(ApprovalChannelPathErrorReason::ChannelAnchorUnavailable)
    })?;
    assert_anchor_is_trustworthy(&anchor_fd).map_err(|_error| {
        ApprovalChannelPathError::new(ApprovalChannelPathErrorReason::ChannelAnchorUnavailable)
    })?;
    Ok(anchor_fd)
}

/// Review 601: `std::env::temp_dir()` returns `$TMPDIR` when set (a
/// per-user, `0700` directory under `/var/folders/…` on a normal macOS
/// session) but falls back to the world-writable, sticky-bit `/tmp`
/// when `TMPDIR` is unset or empty -- a cron job, a stripped
/// environment, a `launchd` context that never set it. Traced what that
/// actually buys another local user before deciding how much to close:
/// a pre-created symlink is already refused by `open_dir_no_follow`'s
/// own `O_NOFOLLOW`, and a pre-created directory owned by someone else
/// already fails the later `fchmod(0o700)` with `EPERM`. The one gap
/// neither of those catches: a directory that genuinely **is** ours (so
/// `fchmod` succeeds) but is group- or world-writable -- left behind by
/// an earlier buggy run, or by anything else running as this same user
/// -- open to another local user dropping entries into it in the window
/// between `mkdirat` and that `fchmod`. Checked directly against the fd
/// this call already opened, not by re-resolving the pathname, so there
/// is no new window between this check and the fd it describes.
#[cfg(target_os = "macos")]
fn assert_anchor_is_trustworthy(anchor_fd: &OwnedFd) -> io::Result<()> {
    // SAFETY: `anchor_fd` is a valid, open fd for the duration of this
    // call; `stat_buf` is a valid, fully-zeroed `libc::stat` the call
    // writes into.
    let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(anchor_fd.as_raw_fd(), &mut stat_buf) } == -1 {
        return Err(io::Error::last_os_error());
    }
    if stat_buf.st_uid != current_euid() {
        return Err(io::Error::other(
            "the approval channel's $TMPDIR anchor is not owned by this process's own user",
        ));
    }
    if stat_buf.st_mode & (libc::S_IWGRP | libc::S_IWOTH) != 0 {
        return Err(io::Error::other(
            "the approval channel's $TMPDIR anchor is group- or world-writable",
        ));
    }
    Ok(())
}

impl ApprovalChannelDirectory {
    /// Refuses if `project_root` would contain either this channel's
    /// socket directory or its durable state root -- a project a user
    /// opens must never be able to read or tamper with either.
    ///
    /// **Review 601: the first clause (`channel_dir.starts_with
    /// (project_root)`) can never fire on macOS**, since `channel_dir`
    /// there is always `$TMPDIR/tekstide` (`channel_dir_under`'s own
    /// macOS doc) and never under anything a `project_root` could name.
    /// That is the guard being satisfied by construction, not a dead or
    /// buggy clause -- the property it checks still holds, just for a
    /// reason this function no longer has to verify itself on that
    /// platform.
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

/// The one and only environment variable through which a
/// `RunCapabilityToken` may reach an adapter process (response 113 Q3 item
/// 1: promoted from launch-path wiring to a security decision this module
/// must own -- "the token must reach the adapter only by this route" is
/// itself part of what makes the token a capability rather than a
/// guessable value, not a coordination detail for whichever code happens
/// to spawn the process).
pub const APPROVAL_TOKEN_ENV_VAR: &str = "TEKSTIDE_APPROVAL_TOKEN";

/// The environment variable through which a bound `ApprovalChannelEndpoint`'s socket path
/// reaches a spawned adapter process (RFC-022 PR-022-C). Unlike `APPROVAL_TOKEN_ENV_VAR`,
/// the path is not a secret -- it is the address the adapter must connect to before it can
/// even attempt to use the token -- but it is still delivered only through the
/// environment, for the same reason RFC-022 gives for the token itself: every alternative
/// (a CLI argument, a well-known file) still bootstraps through the environment or an
/// equally-transparent-to-a-same-user-process channel, and a value with the same lifecycle
/// as the token (generated fresh per bind, meaningless once the endpoint is gone) gains
/// nothing from a second delivery *class*.
pub const APPROVAL_SOCKET_PATH_ENV_VAR: &str = "TEKSTIDE_APPROVAL_SOCKET_PATH";

/// Sets `APPROVAL_TOKEN_ENV_VAR` on `command` to `token`'s value -- the
/// single sanctioned way a `RunCapabilityToken` may be delivered to a
/// spawned adapter process. This exists so there is exactly one, tested
/// choke point for that delivery, rather than each future call site
/// improvising its own `.env(...)` call: nothing here (or, so far as a
/// search of this crate when this was written could confirm, anywhere
/// else in it) writes the token to a file, a log, or any other channel.
///
/// **RFC-022 PR-022-C gives this its first production caller**:
/// `runtime::terminal::launch::spawn_adapter`, a spawn path distinct from
/// `spawn_shell` (which still only ever launches a plain interactive
/// shell with its own fixed, hard-coded environment and never calls this
/// function). `TerminalEnvironmentPolicy::ExplicitAllowlist` remains
/// rejected by the Linux runtime regardless -- delivering this token sets
/// one value Tekstide generated; it does not inherit anything, and
/// `ExplicitAllowlist` could only ever express inheriting names, not
/// setting a generated value, so the two are orthogonal by construction,
/// not by policy this slice had to add.
pub fn inject_token_into_environment(
    command: &mut std::process::Command,
    token: &RunCapabilityToken,
) {
    command.env(APPROVAL_TOKEN_ENV_VAR, token.as_str());
}

// --- The endpoint --------------------------------------------------

/// **Deliberately not `#[non_exhaustive]`, unlike `crates/tekstide`'s own
/// `i18n::LocalePreference`** -- review 571 correction of review 570 (the
/// first draft of this comment named "this crate," but `LocalePreference`
/// lives in the GUI crate, `tekstide`, not here in `tekstide-core`). The
/// precedent genuinely differs rather than merely living elsewhere:
/// `LocalePreference`'s consumers are all in-tree, while this enum is
/// public API of a published library, so a downstream crate's own build
/// is what a bare `match` breaks, not only this workspace's own callers.
/// Adding `BindTargetNotASocket`/`BindTargetAlreadyListening` is a
/// breaking change for any exhaustive `match` over this enum in a
/// downstream crate (see `CHANGELOG.md`'s `Unreleased` entry), and
/// marking it `#[non_exhaustive]` now would make the *next* addition
/// non-breaking at the cost of permanently removing the compiler's
/// ability to force a choice at every call site that matches a reason.
/// That matters concretely here: whether either new variant deserves its
/// own refusal symbol in `agent_run_launch_refusal_symbol` (`shell.rs`) is
/// an open, later question, and today both fall into that match's own
/// `Runtime(_) => "error"` catch-all. Keeping this enum exhaustive-capable
/// means the fix, when that question is taken up, is *removing* the
/// catch-all and letting the compiler enumerate every
/// site that needs a decision -- `#[non_exhaustive]` would foreclose that.
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
    /// failure. Linux only -- see [`Self::ThreadChdirUnavailable`] for
    /// macOS's own, differently-shaped version of the same precondition.
    ProcMagicPathUnavailable,
    /// RFC-028 D4, review 597/599: macOS's own equivalent of
    /// [`Self::ProcMagicPathUnavailable`] -- `/dev/fd` was checked and
    /// confirmed not traversable the way `/proc/self/fd` is (`fdescfs`
    /// re-opens the fd itself rather than supporting path traversal
    /// through a directory descriptor), so this platform binds through
    /// `pthread_fchdir_np` (the calling *thread's* cwd, not the whole
    /// process's) plus a relative path instead. This reason means that
    /// call itself failed -- almost always because the symbol does not
    /// exist on the running macOS version (undocumented officially,
    /// present since OS X 10.5; checked, not assumed, by the postcondition
    /// at the call site, not by a version number).
    ThreadChdirUnavailable,
    /// `clear_stale_socket` found a file at the bind path that exists but
    /// is not a socket (`response 112 Q3`'s own refusal: connecting to a
    /// regular file also yields `ECONNREFUSED`, so this case is refused
    /// outright rather than risking a silent delete of the wrong thing).
    /// Distinguished from a bare `Io`, review 554: this and
    /// [`Self::BindTargetAlreadyListening`] were the two arms response 231
    /// left indistinguishable from each other, which is exactly the shape
    /// `bind_recovers_from_a_stale_socket_file`'s own recurring flake
    /// panic (`reason: Io, source: None`) could never narrow past -- every
    /// other `Io` in this path already carries its source.
    BindTargetNotASocket,
    /// `clear_stale_socket`'s connect probe against the staged bind path
    /// returned `Ok`: something is genuinely listening, so the path is a
    /// live conflict, not a stale leftover, and is refused rather than
    /// removed. Distinguished from a bare `Io` for the same reason as
    /// [`Self::BindTargetNotASocket`] -- review 554's own row-1 re-count.
    /// Review 576: this arm's `source` now also names the peer's
    /// credentials (pid/uid/gid, `pid`/`gid` `"unknown"` on macOS, since
    /// that platform's own mechanism does not report either the same
    /// way -- see this module's private `PeerCredentials` for why),
    /// gathered from the connected stream before it is dropped -- the
    /// refusal is unchanged, only the diagnostic detail is new.
    BindTargetAlreadyListening,
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

    /// A non-`Io` reason with a diagnostic `source` attached -- review
    /// 576's own distinction: the `reason` stays content-free API (the
    /// refusal itself never changes), while `source`'s message carries
    /// whatever extra detail this occurrence could gather, the same way
    /// `Io`'s own source already does for OS errors.
    fn with_source(reason: ApprovalChannelErrorReason, source: io::Error) -> Self {
        Self {
            reason,
            source: Some(source),
        }
    }

    fn io(error: io::Error) -> Self {
        Self::with_source(ApprovalChannelErrorReason::Io, error)
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

    /// RFC-022 PR-022-E ("the arrival model"): a non-blocking,
    /// non-consuming check for whether the adapter on the other end of
    /// this proposal's connection is still there. `MSG_PEEK` reads
    /// without removing the byte from the socket's receive buffer, so a
    /// `true` result leaves `send_decision` free to still work
    /// afterward -- this is a liveness probe, not a read. Raw
    /// `libc::recv` rather than `UnixStream::peek`: peeking a Unix
    /// socket is not yet stable in `std` (`unix_socket_peek`, rust-lang
    /// issue #76923), and this crate already reaches into `libc`
    /// directly elsewhere in this same file for comparable reasons.
    ///
    /// An orderly close (the reference adapter's own 30-second read
    /// timeout, or any other clean exit) surfaces as `recv` returning
    /// `0` (EOF). Any other failure -- a reset connection, or any
    /// `errno` other than `EAGAIN`/`EWOULDBLOCK` -- is treated the same
    /// way: fail-safe, "cannot confirm this is still answerable" wins
    /// over "assume it is." A positive byte count (the adapter sending
    /// something unsolicited, which this protocol never has it do after
    /// a proposal) is treated as "still open" rather than as an
    /// impossible case to panic on -- an unexpected byte from a live
    /// peer is still evidence of a live peer.
    ///
    /// This is `ApprovalCoordinator::decide`'s guard against recording a
    /// decision for a proposal nobody can still deliver it to, and a
    /// queued (not yet promoted) entry's own "is this still answerable"
    /// check.
    pub fn is_connection_still_open(&self) -> bool {
        let fd = self.stream.as_raw_fd();
        let mut probe: u8 = 0;
        // SAFETY: `fd` is a valid, open file descriptor for the
        // lifetime of this call (owned by `self.stream`, borrowed for
        // this call only); `probe` is a valid, correctly-sized buffer
        // for the requested length (1 byte); `MSG_DONTWAIT` makes this
        // call non-blocking regardless of the socket's own blocking
        // mode, so this never toggles or otherwise mutates socket state
        // -- unlike the `set_nonblocking`-based approach this replaced,
        // there is nothing to restore afterward.
        let received = unsafe {
            libc::recv(
                fd,
                (&mut probe as *mut u8).cast::<libc::c_void>(),
                1,
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        match received {
            0 => false,
            n if n > 0 => true,
            _ => matches!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock),
        }
    }

    /// Test-only constructor for `approval::coordinator`'s tests, which
    /// need a real, working `AcceptedProposal` (this type has no public
    /// constructor otherwise -- the only way to obtain one in production
    /// is a full authenticate-over-a-real-socket cycle via
    /// `accept_proposal`/`serve_concurrently`) without paying for a full
    /// bind/listen/connect/authenticate cycle in every coordinator unit
    /// test. `UnixStream::pair()` gives a genuinely connected, real pair
    /// of local sockets -- not a mock -- so `send_decision` on the
    /// returned value still does real I/O; the peer half is handed back
    /// too so a test can read what was actually sent.
    #[cfg(test)]
    pub(crate) fn for_test(proposal: CommandProposal) -> (Self, UnixStream) {
        let (stream, peer) = UnixStream::pair().expect("create a real connected socket pair");
        (Self { proposal, stream }, peer)
    }
}

#[derive(Debug)]
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
        // front, before anything else. Binding below goes through a
        // short, verified-fd-relative path instead (Linux: the `/proc/
        // self/fd` magic path; macOS: a `pthread_fchdir_np`-scoped
        // relative path -- see `enter_bind_context`'s own two doc
        // comments for why that shortness is incidental to closing a
        // TOCTOU race, not a feature), which no longer fails here on a
        // long real path the way the previous revision did -- it would
        // instead bind successfully and leave a real adapter's later
        // `connect()` (which must use the real path) to fail against a
        // healthy-looking, unreachable endpoint. That is a harder
        // failure to diagnose than the one it replaced, so it is closed
        // explicitly here instead.
        if socket_path.as_os_str().len() > max_socket_path_len() {
            return Err(ApprovalChannelError::new(
                ApprovalChannelErrorReason::SocketPathTooLong,
            ));
        }

        // Response 113 Required 2: `mkdirat`/`openat` relative to an
        // already-pinned anchor fd (Linux: `state_root_fd`; macOS,
        // review 600: `channel_anchor_fd`, pinned to `$TMPDIR` rather
        // than `state_root` -- see `channel_dir_under`'s own macOS doc
        // for why `channel_dir` is not under `state_root` there at all),
        // instead of re-resolving `channel_dir`'s pathname here the way
        // the previous revision's `open_dir_no_follow(directory.
        // channel_dir())` did. That call re-resolved the anchor's own
        // pathname on every `bind()`, so a same-user swap of the anchor
        // itself (not just the subdirectory this call opens) between
        // `resolve()` and `bind()` still worked -- the fd was opened
        // once and never re-resolves its own pathname again, closing
        // that too.
        let (anchor_fd, subdir_name) = channel_anchor(directory);
        mkdirat_if_missing(anchor_fd, subdir_name, 0o700).map_err(ApprovalChannelError::io)?;
        let dir_fd =
            openat_dir_no_follow(anchor_fd, subdir_name).map_err(ApprovalChannelError::io)?;
        fchmod(dir_fd.as_raw_fd(), 0o700).map_err(ApprovalChannelError::io)?;

        // Response 113 Q1 item 1 (Linux) / RFC-028 D4 review 597/599
        // (macOS): this fd-relative approach depends on a per-platform
        // precondition -- `/proc/self/fd` being mounted and functional
        // on Linux, `pthread_fchdir_np` succeeding (and its own
        // postcondition holding) on macOS. Checked here, against the fd
        // this call just opened itself, so a failure is unambiguously
        // about that precondition rather than about the eventual bind
        // target -- and surfaces as a distinguished reason instead of a
        // bare `Io` that would otherwise look identical to an unrelated
        // bind failure.
        let filename = format!("{}.sock", socket_filename_stem(agent_run_id));
        let bind_context = enter_bind_context(&dir_fd, &filename)
            .map_err(|_error| ApprovalChannelError::new(bind_context_error_reason()))?;
        let bind_path = &bind_context.path;

        clear_stale_socket(bind_path, &socket_path)?;

        let listener = UnixListener::bind(bind_path).map_err(ApprovalChannelError::io)?;
        // Empirically, `fchmod` on the *listening socket's own fd* does
        // not reliably update the bound special file's mode bits (tried
        // first; the socket file kept its umask-derived default despite
        // `fchmod` reporting success) -- unlike a plain directory fd,
        // where `fchmod` unambiguously works, a socket fd's relationship
        // to the special file's inode metadata is evidently not the same
        // thing. Falling back to a path-based `set_permissions`, but
        // still through `bind_path` (the verified-fd-relative path)
        // rather than the plain path, so this still resolves through the
        // already-verified directory fd instead of re-resolving a
        // separate pathname.
        fs::set_permissions(bind_path, fs::Permissions::from_mode(0o600))
            .map_err(ApprovalChannelError::io)?;
        // `bind_context` drops here, after every operation that needed
        // its path -- on macOS, this is where the calling thread's cwd
        // reverts.
        drop(bind_context);

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
    ///
    /// RFC-036 PR-036-B: not `pub` in production any more -- superseded
    /// there by `serve_concurrently`'s own internal accept loop, which is
    /// what every real adapter connection goes through. `#[cfg(test)]`
    /// because this single-connection, single-thread shape is this
    /// module's own primary vehicle for testing authentication and frame
    /// parsing directly, without `serve_concurrently`'s multi-threaded
    /// machinery in the way -- roughly a dozen tests drive it, and
    /// rewriting each onto `serve_concurrently` to delete one dormant
    /// production entry point would be a materially larger, riskier
    /// change to this module's own security-relevant test coverage than
    /// this triage slice's own scope.
    #[cfg(test)]
    pub fn accept_proposal(&self) -> Result<AcceptedProposal, ApprovalChannelError> {
        let (stream, _addr) = self.listener.accept().map_err(ApprovalChannelError::io)?;
        self.authenticate_and_read(stream)
    }

    /// Response 113 Q2 (promoted from an open PR-021-E question to
    /// required E1 scope, per the reviewer's framing: the timeout bounds
    /// how long one connection can occupy the slot, but does not fix the
    /// underlying denial of a single-slot design). `accept_proposal`
    /// authenticates one connection fully -- including the up-to-
    /// `read_timeout` wait for its first frame -- before `listener.accept`
    /// is ever called again. A same-user peer that connects and stalls
    /// therefore occupies the one in-flight slot repeatedly, delaying the
    /// real adapter's connection behind it in the kernel's listen backlog
    /// for a further `read_timeout` on every repeat.
    ///
    /// This spawns a dedicated thread per accepted connection (bounded by
    /// `MAX_CONCURRENT_CONNECTIONS`, response 114 Required 4) so the
    /// accept loop calls `listener.accept()` again immediately rather than
    /// waiting for the previous connection to finish authenticating.
    /// `read_timeout` still bounds how long any *one* connection can
    /// occupy its own thread; it no longer bounds how quickly the *next*
    /// connection is accepted. Results are delivered through the returned
    /// channel in whatever order authentication actually finishes in, not
    /// necessarily accept order.
    ///
    /// The accept loop holds only a `Weak` reference to `self` (response
    /// 114 Required 3): an earlier version captured a strong `Arc` in the
    /// loop's own closure, which meant the loop itself was always one more
    /// live owner than whatever the caller held -- the endpoint's strong
    /// count could never reach zero from outside, `Drop` could never run,
    /// and the socket file was never removed no matter what the caller
    /// did with its own `Arc`. A `Weak` alone is not enough on its own,
    /// since the loop cannot check anything while blocked inside
    /// `accept()` -- the returned [`ServeShutdown`] is what actually
    /// unblocks it, via a throw-away self-connect, so the loop can notice
    /// the shutdown flag and exit.
    pub fn serve_concurrently(
        self: Arc<Self>,
    ) -> (
        mpsc::Receiver<Result<AcceptedProposal, ApprovalChannelError>>,
        ServeShutdown,
    ) {
        let (sender, receiver) = mpsc::channel();
        let shutting_down = Arc::new(AtomicBool::new(false));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let socket_path = self.socket_path.clone();
        let weak = Arc::downgrade(&self);
        // Deliberately dropped rather than moved into the loop below: the
        // loop must never itself be a strong owner (see the doc comment
        // above). Only a caller-retained clone of the original `Arc` this
        // function was given keeps the endpoint alive from here on.
        drop(self);

        let loop_shutting_down = Arc::clone(&shutting_down);
        let join_handle = thread::spawn(move || {
            loop {
                if loop_shutting_down.load(Ordering::SeqCst) {
                    break;
                }
                let Some(endpoint) = weak.upgrade() else {
                    // Response 115: corrected from an earlier, overstated
                    // comment. This is reachable only in the brief window
                    // between one connection finishing and the next
                    // `accept()` call starting -- while a call is blocked
                    // inside `accept()` below, this check has already
                    // passed and is not reached again until that call
                    // returns, so a caller dropping every strong owner
                    // *during* a blocked `accept()` is NOT caught here.
                    // `ServeShutdown`'s `Drop` (which wakes a blocked
                    // `accept()` via a self-connect before the strong
                    // count can matter) is what actually guarantees
                    // eventual cleanup in that case, not this branch.
                    break;
                };
                let stream = match endpoint.listener.accept() {
                    Ok((stream, _addr)) => stream,
                    Err(_) => break,
                };
                if loop_shutting_down.load(Ordering::SeqCst) {
                    // Either the throw-away self-connect `shutdown()` used
                    // to unblock `accept()`, or a genuine connection that
                    // arrived exactly as shutdown began -- either way,
                    // stop without processing it further.
                    drop(stream);
                    break;
                }
                if in_flight.fetch_add(1, Ordering::SeqCst) >= MAX_CONCURRENT_CONNECTIONS {
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    // Silent refusal (response 114 Required 4): nothing is
                    // read from or written to a connection beyond the cap.
                    drop(stream);
                    continue;
                }
                let sender = sender.clone();
                let in_flight = Arc::clone(&in_flight);
                thread::spawn(move || {
                    let result = endpoint.authenticate_and_read(stream);
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    // The receiver may have been dropped (the caller
                    // stopped listening for results) -- a send failure
                    // here is not this thread's problem to handle further.
                    let _ = sender.send(result);
                });
            }
        });

        (
            receiver,
            ServeShutdown {
                shutting_down,
                socket_path,
                join_handle: Some(join_handle),
            },
        )
    }

    fn authenticate_and_read(
        &self,
        mut stream: UnixStream,
    ) -> Result<AcceptedProposal, ApprovalChannelError> {
        // Response 112 Defect 3: without this, a same-user peer that
        // connects and then sends nothing (or a partial frame) blocks
        // this call -- and, before `serve_concurrently`, every connection
        // after it -- forever.
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

/// Returned by [`ApprovalChannelEndpoint::serve_concurrently`] (response
/// 114 Required 3, corrected by response 115 Required B). The accept loop
/// cannot be stopped by dropping `Arc` clones alone -- it may be blocked
/// indefinitely inside `accept()`, which nothing but an actual event on
/// that listener can unblock, and while a connection is being handled the
/// loop holds its own temporary strong `Arc` across that blocking call, so
/// a caller dropping every *other* strong reference at that exact moment
/// does not bring the count to zero either.
///
/// **This type's `Drop` impl is what actually guarantees cleanup**
/// (response 115 Required B: an earlier version relied on the caller
/// remembering to call `shutdown()`, and simply dropping this value did
/// nothing -- the reviewer's probe showed a caller that dropped
/// everything, including this handle, while the loop was already blocked
/// in `accept()`, left the socket file behind and the loop blocked
/// forever, because nothing had signalled it). Whether cleanup happens via
/// an explicit `shutdown()` call or simply by this value going out of
/// scope, the same signal-wake-join sequence runs exactly once.
pub struct ServeShutdown {
    shutting_down: Arc<AtomicBool>,
    socket_path: PathBuf,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl ServeShutdown {
    fn shutdown_and_join(&mut self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

impl Drop for ServeShutdown {
    fn drop(&mut self) {
        self.shutdown_and_join();
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

    /// Review 577's part 3: lets a test verify the listening socket's
    /// `FD_CLOEXEC` flag directly against the kernel, rather than trusting
    /// that `std::os::unix::net::UnixListener` sets it (it does, but
    /// nothing in this codebase had verified that for RFC-022's own trust
    /// boundary until this).
    pub(crate) fn listener_raw_fd_for_test(&self) -> RawFd {
        self.listener.as_raw_fd()
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
        return Err(ApprovalChannelError::new(
            ApprovalChannelErrorReason::BindTargetNotASocket,
        ));
    }
    match UnixStream::connect(bind_path) {
        Ok(stream) => {
            // Review 576: the refusal itself does not change -- this is
            // strictly more diagnostic detail on the same reason, naming
            // *who* answered the connect so the next occurrence can tell
            // whether the peer is this test process itself (the staging
            // is suspect) or something else (a pid worth chasing).
            let diagnostic = match peer_credentials(&stream) {
                // RFC-028 D4: `pid`/`gid` are `Option` because macOS's own
                // mechanism does not report a pid at all and reports
                // groups rather than one primary gid -- "unknown" on
                // that platform is accurate, not a missing case.
                Ok(cred) => io::Error::other(format!(
                    "connect() succeeded; peer is pid={} uid={} gid={}",
                    cred.pid
                        .map_or_else(|| "unknown".to_owned(), |pid| pid.to_string()),
                    cred.uid,
                    cred.gid
                        .map_or_else(|| "unknown".to_owned(), |gid| gid.to_string()),
                )),
                Err(error) => io::Error::other(format!(
                    "connect() succeeded but reading peer credentials failed: {error}"
                )),
            };
            Err(ApprovalChannelError::with_source(
                ApprovalChannelErrorReason::BindTargetAlreadyListening,
                diagnostic,
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
            fs::remove_file(removal_path).map_err(ApprovalChannelError::io)
        }
        Err(error) => {
            // Any other error (permission denied, etc.) is treated the
            // same as "do not touch it" -- only a confirmed stale
            // listener is cleared.
            //
            // Response 231: this was `ApprovalChannelError::new(..Io)`,
            // discarding `error` -- the real errno a caller would need to
            // tell "permission denied" apart from "too many open files"
            // apart from anything else. That is the exact site
            // `bind_recovers_from_a_stale_socket_file`'s own flake panic
            // (`ApprovalChannelError { reason: Io, source: None }`) comes
            // from: every *other* `Io`-reason error in `bind()`'s call
            // chain already goes through the source-preserving
            // `ApprovalChannelError::io`, so this was the one place the
            // evidence a repeated flake needed was thrown away before it
            // could ever reach the panic that reported it.
            Err(ApprovalChannelError::io(error))
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

/// What `bind()` actually binds `UnixListener` (and the later
/// `set_permissions` call) through -- resolved via the kernel's fd table
/// on Linux (an absolute magic path, no held state) and via a
/// thread-local `chdir` on macOS (a relative path, with a guard that
/// must stay alive for every operation using it and reverts the calling
/// thread's cwd on drop). **Dropping the macOS guard early, before every
/// operation that needs the relative path has finished, would silently
/// resolve later operations against whatever the thread's cwd happens to
/// be instead** -- the field exists specifically to make "keep this
/// alive" a type-level fact rather than a convention to remember. `path`
/// is deliberately the only thing read at any call site; nothing outside
/// this type inspects which platform's shape produced it.
struct BindContext {
    path: PathBuf,
    #[cfg(target_os = "macos")]
    _scope: ThreadDirScope,
}

/// Linux: a path that resolves through the kernel's fd table (`/proc/
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
///
/// Confirms `/proc/self/fd/<fd>` resolves, for the fd this module just
/// opened itself, before relying on the same mechanism to construct a
/// bind path (response 113 Q1 item 1). `/proc` not being mounted is the
/// expected cause of failure here -- rare on Tekstide's Linux desktop
/// target, but not impossible in minimal containers or hardened
/// configurations -- and checking against a fd we know is valid isolates
/// that specific precondition from any other reason a later operation on
/// the constructed path might fail.
#[cfg(target_os = "linux")]
fn enter_bind_context(dir_fd: &OwnedFd, filename: &str) -> io::Result<BindContext> {
    let self_path = PathBuf::from(format!("/proc/self/fd/{}", dir_fd.as_raw_fd()));
    fs::metadata(&self_path)?;
    Ok(BindContext {
        path: PathBuf::from(format!("/proc/self/fd/{}/{filename}", dir_fd.as_raw_fd())),
    })
}

/// macOS: RFC-028 D4, review 597/599. `/dev/fd` was checked on the real
/// runner and confirmed not traversable the way `/proc/self/fd` is
/// (`fdescfs` re-opens the fd itself rather than supporting path
/// traversal through a directory descriptor), so this platform cannot
/// construct an absolute magic path at all. `pthread_fchdir_np` changes
/// the *calling thread's* current directory, not the whole process's --
/// unlike `chdir(2)`/`fchdir(2)`, which would race every other thread in
/// this process doing its own relative file I/O (a different `AgentRun`'s
/// own `bind()` running concurrently, for one) -- so the relative path
/// this returns resolves through `dir_fd`'s own verified inode the same
/// way the Linux magic path does, without the process-wide side effect a
/// plain `chdir` would have.
///
/// `ThreadDirScope::enter` carries its own postcondition (`fstat` on
/// `dir_fd` against `stat(".")` after the call) rather than trusting a
/// zero return alone -- the same "postcondition, not the mechanism"
/// discipline `pty.rs`'s `assert_cloexec` already uses for a different
/// close-on-exec race in this same workspace.
#[cfg(target_os = "macos")]
fn enter_bind_context(dir_fd: &OwnedFd, filename: &str) -> io::Result<BindContext> {
    let scope = ThreadDirScope::enter(dir_fd)?;
    Ok(BindContext {
        path: PathBuf::from(filename),
        _scope: scope,
    })
}

#[cfg(target_os = "linux")]
fn bind_context_error_reason() -> ApprovalChannelErrorReason {
    ApprovalChannelErrorReason::ProcMagicPathUnavailable
}

#[cfg(target_os = "macos")]
fn bind_context_error_reason() -> ApprovalChannelErrorReason {
    ApprovalChannelErrorReason::ThreadChdirUnavailable
}

/// What `bind()`'s own `mkdirat`/`openat` pair resolves `channel_dir`'s
/// subdirectory relative to -- `directory.state_root_fd` naming
/// `"approval"` on Linux, `directory.channel_anchor_fd` naming
/// `"tekstide"` on macOS. See `channel_dir_under`'s own two doc comments
/// for why these differ.
#[cfg(target_os = "linux")]
fn channel_anchor(directory: &ApprovalChannelDirectory) -> (RawFd, &'static str) {
    (directory.state_root_fd.as_raw_fd(), "approval")
}

#[cfg(target_os = "macos")]
fn channel_anchor(directory: &ApprovalChannelDirectory) -> (RawFd, &'static str) {
    (directory.channel_anchor_fd.as_raw_fd(), "tekstide")
}

/// The longest path `bind`/`connect` can accept in a `sockaddr_un`, not
/// counting the mandatory trailing NUL terminator the kernel requires
/// (response 113 Required 1).
///
/// **Review 600: reads `sun_path`'s own array length directly, rather
/// than deriving it by subtracting header fields from the struct's
/// total size.** The previous formula (`size_of::<sockaddr_un>() -
/// size_of::<sa_family_t>()`) assumed `sockaddr_un` has no header field
/// before `sun_family` -- true on Linux (`{ sa_family_t, sun_path }`,
/// formula gives exactly 108) and false on macOS/BSD (`{ sun_len: u8,
/// sun_family: sa_family_t, sun_path }` -- an extra leading byte the
/// formula never subtracted), silently returning 104 there instead of
/// `sun_path`'s real size of 104... minus the one byte the NUL still
/// needs, which the old formula's own final `- 1` then took from the
/// *wrong* starting number: 104 reported as the max instead of the
/// true 103. A 104-byte path passed this check and failed only at the
/// real `bind()` call -- the exact "pre-check disagrees with the
/// kernel" shape this project fixes on sight. Reading the field's own
/// length instead is correct by construction on every platform's own
/// `sockaddr_un`, with no assumption about what else precedes it.
fn max_socket_path_len() -> usize {
    // SAFETY: only used for its own `sun_path` field's array length;
    // never treated as a real address or dereferenced through a raw
    // pointer.
    let probe: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    probe.sun_path.len() - 1
}

/// Declared directly: not in the vendored `libc` crate for any target
/// (checked), undocumented in Apple's own reference but present in the
/// open-source `pthread.c`/`pthread.h` since OS X 10.5 and used by
/// several real projects for exactly this "relative-path-through-a-
/// verified-fd, without a process-wide `chdir`" pattern. `-1` is its own
/// documented sentinel for "revert this thread to the process's shared
/// cwd" -- what [`ThreadDirScope`]'s `Drop` uses, rather than needing to
/// save and restore whatever the thread's previous cwd happened to be.
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn pthread_fchdir_np(fd: libc::c_int) -> libc::c_int;
}

/// RAII guard for a successful [`pthread_fchdir_np`] call -- reverts this
/// thread's cwd on drop regardless of what happened while it was held,
/// including a bind or chmod failure in between, so a single failed
/// `ApprovalChannelEndpoint::bind` can never leave the calling OS thread
/// permanently pointed at an approval directory for whatever unrelated
/// work it does next.
#[cfg(target_os = "macos")]
struct ThreadDirScope;

#[cfg(target_os = "macos")]
impl ThreadDirScope {
    fn enter(dir_fd: &OwnedFd) -> io::Result<Self> {
        // SAFETY: `dir_fd` is a valid, currently-open directory fd owned
        // by the caller for at least the duration of this call.
        if unsafe { pthread_fchdir_np(dir_fd.as_raw_fd()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        if let Err(error) = Self::assert_cwd_is(dir_fd) {
            // SAFETY: `-1` is `pthread_fchdir_np`'s own documented
            // sentinel for "revert to the process's shared cwd"; always
            // legal to call, regardless of this thread's prior state.
            unsafe {
                pthread_fchdir_np(-1);
            }
            return Err(error);
        }
        Ok(Self)
    }

    /// Postcondition, not the mechanism: confirms the calling thread's
    /// cwd now really is `dir_fd`'s own directory (same device and
    /// inode), rather than trusting `pthread_fchdir_np`'s zero return
    /// alone. A mismatch here would mean every subsequent relative-path
    /// operation in this scope silently resolves against the wrong
    /// directory instead of failing loudly -- exactly the class of
    /// defect a postcondition check exists to catch instead of assume.
    fn assert_cwd_is(dir_fd: &OwnedFd) -> io::Result<()> {
        // SAFETY: `dir_fd` is a valid, open fd for the duration of this
        // call; `stat_buf` is a valid, fully-zeroed `libc::stat` the
        // call writes into.
        let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(dir_fd.as_raw_fd(), &mut stat_buf) } == -1 {
            return Err(io::Error::last_os_error());
        }
        let cwd = fs::metadata(".")?;
        if stat_buf.st_dev as u64 == cwd.dev() && stat_buf.st_ino as u64 == cwd.ino() {
            Ok(())
        } else {
            Err(io::Error::other(
                "pthread_fchdir_np reported success but the calling thread's cwd does not match \
                 the directory fd it was given",
            ))
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for ThreadDirScope {
    fn drop(&mut self) {
        // SAFETY: `-1` is `pthread_fchdir_np`'s own documented sentinel
        // for "revert to the process's shared cwd"; always legal to call.
        let result = unsafe { pthread_fchdir_np(-1) };
        // `Drop` cannot return an error, and a failed revert here is the
        // same failure class `enter`'s own postcondition exists to
        // catch: this OS thread would stay scoped to the approval
        // directory it should have left, and every later relative-path
        // operation on it would resolve somewhere unintended --
        // silently, with no error of its own. `debug_assert_eq!`, this
        // crate's own established shape for a best-effort failure that
        // must still be loud (`config/sensitive.rs`,
        // `runtime::terminal::security::parser`), rather than a bare
        // best-effort call with nothing checking it.
        debug_assert_eq!(
            result, 0,
            "pthread_fchdir_np(-1) failed while reverting this thread's cwd after a bind \
             attempt -- this thread is left scoped to the approval directory it should have \
             left"
        );
    }
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

/// RFC-028 D4: this crate's own cross-platform shape for a peer's
/// kernel-verified credentials, independent of either platform's raw
/// struct. `pid` and `gid` are `Option` because the two mechanisms this
/// type abstracts over do not report the same things: macOS's
/// `LOCAL_PEERCRED`/`xucred` carries no pid at all (`LOCAL_PEERPID` is a
/// separate sockopt, not needed here -- the only check this crate ever
/// makes is same-user, which `uid` alone answers) and reports a *list*
/// of groups rather than one primary gid, so `gid` here is the first
/// reported group when macOS provides at least one.
struct PeerCredentials {
    pid: Option<u32>,
    uid: u32,
    gid: Option<u32>,
}

/// Reads the connecting process's real, kernel-verified credentials.
/// This is not something the connecting process can spoof: the kernel
/// fills in the platform's own credentials struct from the actual
/// socket peer at `connect()` time, not from anything the peer
/// transmits. Linux: `SO_PEERCRED`/`struct ucred`. macOS: RFC-028 D4,
/// `LOCAL_PEERCRED`/`struct xucred` -- a different option *level*
/// (`SOL_LOCAL`, not `SOL_SOCKET`), with its own `cr_version` field
/// checked against `XUCRED_VERSION` before trusting the rest of the
/// struct, the same discipline this security boundary already applies
/// to every other kernel-reported value.
#[cfg(target_os = "linux")]
fn peer_credentials(stream: &UnixStream) -> io::Result<PeerCredentials> {
    let fd = stream.as_raw_fd();
    // Review 587's own recommendation, applied for symmetry with the
    // macOS arm's `cr_version` sentinel below: seeded with values no
    // real peer ever has (a negative pid, `u32::MAX` uid/gid) rather
    // than zeroed, so a hypothetical `getsockopt` that reports success
    // without actually writing the buffer cannot be mistaken for pid 0/
    // uid 0 (root) -- it instead produces a uid that will never match
    // `self.owner_uid` and the connection fails closed. No kernel
    // actually does this; this is defence-in-depth, not a live fix.
    let mut cred = libc::ucred {
        pid: -1,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    // SAFETY: `cred` is a plain-old-data struct with no invariants
    // beyond its fields being initialized (done above); `len` is
    // initialized to the struct's size, matching what `getsockopt`
    // expects for a fixed-size option, and is unused if the call fails.
    unsafe {
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
        Ok(PeerCredentials {
            pid: Some(cred.pid as u32),
            uid: cred.uid,
            gid: Some(cred.gid),
        })
    }
}

/// See [`peer_credentials`]'s own doc comment (the Linux arm) for the
/// convention this implements.
#[cfg(target_os = "macos")]
fn peer_credentials(stream: &UnixStream) -> io::Result<PeerCredentials> {
    let fd = stream.as_raw_fd();
    // Review 587: `XUCRED_VERSION` is `0`, the same bit pattern
    // `mem::zeroed()` produces -- without this, a hypothetical
    // `getsockopt` that reports success without writing the buffer
    // would be indistinguishable from a genuine version-0 reply, and
    // the `cr_version` check below would be a tautology rather than a
    // guard. Seeding `cr_version` with a sentinel that is *not*
    // `XUCRED_VERSION` first makes the existing check real: an
    // unwritten struct now fails it, the same way a torn or truncated
    // one should.
    //
    // SAFETY: `cred` is a plain-old-data struct with no invariants
    // beyond its fields being initialized; zeroing first and then
    // overwriting `cr_version` leaves every field initialized before
    // `getsockopt` or the version check reads any of them. `len` is
    // sized to match, fixed-size.
    unsafe {
        let mut cred: libc::xucred = std::mem::zeroed();
        cred.cr_version = u32::MAX;
        let mut len = std::mem::size_of::<libc::xucred>() as libc::socklen_t;
        let result = libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERCRED,
            &mut cred as *mut libc::xucred as *mut libc::c_void,
            &mut len,
        );
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if cred.cr_version != libc::XUCRED_VERSION {
            return Err(io::Error::other(format!(
                "LOCAL_PEERCRED returned cr_version {}, expected {} -- refusing to trust the \
                 rest of the struct",
                cred.cr_version,
                libc::XUCRED_VERSION
            )));
        }
        let gid = (cred.cr_ngroups > 0).then(|| cred.cr_groups[0]);
        Ok(PeerCredentials {
            pid: None,
            uid: cred.cr_uid,
            gid,
        })
    }
}

/// Reads just the connecting process's real, kernel-verified user id --
/// the comparison the token/same-user check above actually needs.
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    peer_credentials(stream).map(|cred| cred.uid)
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
