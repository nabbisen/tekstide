//! RFC-058 PR-058-C, D10: "the holder must be discoverable by another
//! process, and whatever makes it discoverable must not outlive it" --
//! the same sentence D4 already answers for the lock itself, answered here
//! for a second instance's own ability to *ask* the holder's window for
//! attention.
//!
//! **Deliberately not `approval::channel`'s own hardened shape.** That
//! module binds through a `/proc/self/fd`-relative magic path and guards a
//! capability token an untrusted adapter process holds -- proportionate to
//! a channel that decides whether a proposed command runs. This channel
//! carries no token and decides nothing: a spoofed knock only causes an
//! unwanted attention flash on a window the same user already owns, which
//! is also all a *missing* knock costs (D9's own "best-effort, reversible"
//! framing). A plain path-based [`UnixListener::bind`] is proportionate
//! here; the one property reused from that module's own precedent is the
//! socket-path-length check (`max_socket_path_len` below, copied rather
//! than imported -- it is three lines, and a second, independent copy is
//! cheaper than threading a dependency between two otherwise-unrelated
//! modules for it).

use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use super::project_locks_dir;

/// `<state_root>/project-locks/<project_id>.attention.sock` on Linux -- a
/// sibling of the lock file itself, under the same directory D3 already
/// names. RFC-028 D4, review 603's general rule: **every `AF_UNIX` path
/// this product binds derives from a short per-user anchor, never from
/// `state_root`, with a headroom test per socket** -- the approval
/// channel already learned this the hard way (review 600/601) and this
/// socket now inherits the rule instead of rediscovering the ceiling a
/// third time. See `attention_socket_dir`'s own macOS doc for why.
pub fn attention_socket_path(state_root: &Path, project_id: &str) -> PathBuf {
    attention_socket_dir(state_root).join(format!("{project_id}.attention.sock"))
}

/// Linux: unchanged, `state_root`-relative. macOS: `std::env::temp_dir()`
/// joined with a name of its own (`"tekstide-attention"`, a sibling of
/// `approval::channel`'s own `"tekstide"` anchor, not nested inside it --
/// the two modules stay as independent on macOS as `max_socket_path_len`'s
/// own deliberate duplication already keeps them on every platform).
/// `sockaddr_un`'s usable `sun_path` is 103 bytes on macOS; a real
/// `ProjectId`-based path under a state root as long as `~/Library/
/// Application Support/tekstide/project-locks` does not reliably fit,
/// and -- the same lesson the approval channel's own `channel_dir_under`
/// states -- nothing at bind time can shorten the path a knock's own
/// `connect()` must use, since that path is never resolved relative to
/// anything.
///
/// **Deliberately does not add `approval::channel`'s own anchor-
/// trustworthiness check** (owned by this process's `euid`, not group-
/// or world-writable) -- this module's own doc comment already states
/// why its threat model differs: a spoofed knock costs an unwanted
/// attention flash, and this function's own worst case under a
/// `TMPDIR`-less environment (another user's directory already at
/// `/tmp/tekstide-attention`) degrades to exactly the same `None` this
/// module already returns for an ordinary bind failure, not a new risk
/// proportionate to carrying that check.
#[cfg(target_os = "linux")]
fn attention_socket_dir(state_root: &Path) -> PathBuf {
    project_locks_dir(state_root)
}

#[cfg(target_os = "macos")]
fn attention_socket_dir(_state_root: &Path) -> PathBuf {
    std::env::temp_dir().join("tekstide-attention")
}

/// The longest path `bind`/`connect` can accept in a `sockaddr_un`, not
/// counting the mandatory trailing NUL the kernel requires -- the same
/// computation `approval::channel`'s own `max_socket_path_len` already
/// makes, for the reason this module's own doc comment gives for not
/// sharing it directly.
///
/// **Review 602: reads `sun_path`'s own array length directly, rather
/// than deriving it by subtracting header fields from the struct's
/// total size** -- the same fix `approval::channel`'s own copy of this
/// function already got, for the same reason: the subtraction formula
/// assumed `sockaddr_un` has no header field before `sun_family`, true
/// on Linux and false on macOS/BSD's own extra leading `sun_len` byte,
/// silently reporting 104 as the max where the real limit is 103. Two
/// independently-maintained copies of one computation is a defect the
/// day someone edits one without the other -- today was that day, which
/// is why both are fixed in the same commit rather than one now and one
/// later.
fn max_socket_path_len() -> usize {
    // SAFETY: only used for its own `sun_path` field's array length;
    // never treated as a real address or dereferenced through a raw
    // pointer.
    let probe: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    probe.sun_path.len() - 1
}

/// Binds the holder's own attention listener, right alongside acquiring
/// the lock. **Best-effort by construction, not by a caller's discipline**:
/// this returns `None` on *any* failure -- a path too long, a bind error,
/// a permission error -- and has no error variant a caller could mistake
/// for something that must stop the open. Losing this never loses the
/// lock itself; it only means a second instance's knock has nowhere to
/// land, the same degradation D9 already accepts when the compositor
/// itself declines to surface the request.
pub fn bind_attention_listener(state_root: &Path, project_id: &str) -> Option<UnixListener> {
    let path = attention_socket_path(state_root, project_id);
    if path.as_os_str().len() > max_socket_path_len() {
        return None;
    }
    // Self-sufficient regardless of call order. On Linux, in production,
    // this directory already exists (`acquire_project_lock` creates the
    // same `project_locks_dir` first) -- but this function makes no
    // assumption about having been called after that one, which matters
    // for real on macOS now: `attention_socket_dir`'s own anchor there
    // is not `project_locks_dir` at all, so nothing else creates it
    // first.
    std::fs::create_dir_all(attention_socket_dir(state_root)).ok()?;
    // A previous holder that crashed leaves this special file behind --
    // `bind` on an existing path fails `AddrInUse` regardless of whether
    // anything is still listening on it, so clearing it first is what
    // lets the *next* holder bind at all. Harmless if nothing was there.
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Some(listener)
}

/// A second instance's own knock. Connects and lets the stream drop
/// immediately -- the holder's own `accept()` firing is the entire
/// signal; nothing is sent or read over it. Best-effort (D9): returns
/// whether the holder was reached, never blocks waiting for a response
/// that does not exist.
pub fn request_attention(state_root: &Path, project_id: &str) -> bool {
    let path = attention_socket_path(state_root, project_id);
    UnixStream::connect(path).is_ok()
}

/// Removes this project's own attention socket file, best-effort. Not
/// required for correctness (the next holder's own [`bind_attention_listener`]
/// already clears a stale one), but leaves nothing behind after a clean
/// exit, matching [`super::ProjectLock`]'s own file-based housekeeping.
pub fn remove_attention_socket(state_root: &Path, project_id: &str) {
    let _ = std::fs::remove_file(attention_socket_path(state_root, project_id));
}

#[cfg(test)]
mod tests;
