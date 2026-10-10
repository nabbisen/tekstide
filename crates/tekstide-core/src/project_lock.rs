//! RFC-058 PR-058-B: the mechanism `REQ-PROJ-009` names for write-sensitive
//! project state. Two Tekstide processes opening the same canonical root
//! used to reuse the same project id (`app.rs`) and therefore the same
//! recovery-record paths (`recovery/record.rs`) with nothing between them --
//! last tick wins, silently. This is the "nothing" that now exists between
//! them.
//!
//! **D3**: process-visible (`REQ-PROJ-009`'s own words rule out an
//! in-memory-only lock), under the state directory, scoped to one project
//! id -- never inside the project itself. **D4**: released the moment its
//! holder's process ends, including a crash, because the primitive is a
//! real OS file lock (`std::fs::File::try_lock`, the same mechanism
//! `transcript::writer::BoundedTranscriptWriter` already uses for RFC-050
//! D3) and the kernel drops that lock when every file descriptor pointing
//! at it closes -- no pid bookkeeping to go stale, no liveness check that
//! pid reuse can fool (the hole RFC-027 D12's own marker discloses).
//! **Read `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`
//! before touching this file**: a descriptor a `fork` duplicated before
//! `exec` keeps an `flock` alive past its own owner's drop, measured there
//! at roughly 1 run in 40 under load. The same residual hazard applies
//! here and is accepted on the same evidence, not newly discovered or newly
//! risked by this module.
//!
//! **D5 is this module's one deliberate difference from that precedent.**
//! `BoundedTranscriptWriter::create` treats *any* lock failure, not only
//! `WouldBlock`, as "do not write" -- correct there, because a transcript
//! nobody can protect is a transcript not worth writing unprotected. A
//! project is not: "a text editor that will not open because a lock file is
//! confusing has chosen the wrong failure" (D5). So only `WouldBlock` -- a
//! clean, unambiguous "another live process holds this" -- is treated as
//! held. Every other outcome (an unreadable lock directory, a filesystem
//! that cannot lock at all, any other I/O error) is [`ProjectLockOutcome::CannotDecide`],
//! and the caller must open the project without claiming protection, never
//! refuse.
//!
//! The lock file's own content (this process's pid) is written only once
//! the lock is held, for naming the holder later (D8/D10) -- it plays no
//! part in the exclusivity decision itself, which is `try_lock`'s alone.
//! Stale content left by a crashed holder is simply overwritten the moment
//! anyone next acquires the (by then unheld) lock; a build that cannot
//! parse that content still decides correctly, because it never needed to
//! parse it to decide.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// `<state_root>/project-locks/<project_id>.lock` -- a sibling of
/// `recovery/`, under the same state directory, never inside the project
/// (D3).
pub fn project_locks_dir(state_root: &Path) -> PathBuf {
    state_root.join("project-locks")
}

pub fn project_lock_file(state_root: &Path, project_id: &str) -> PathBuf {
    project_locks_dir(state_root).join(format!("{project_id}.lock"))
}

/// Held for as long as this project stays open in this process. Dropping it
/// (closing the file) is the whole release mechanism (D4) -- there is
/// nothing else to clean up, and nothing else must be added here that could
/// fail or block a release.
#[derive(Debug)]
pub struct ProjectLock {
    // Never read again -- held only so dropping `ProjectLock` closes it,
    // which is the release itself (D4). An unread field is the honest shape
    // of that; a method that touched it would be reading it for no reason.
    #[allow(dead_code)]
    file: File,
    path: PathBuf,
}

impl ProjectLock {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug)]
pub enum ProjectLockOutcome {
    Acquired(ProjectLock),
    /// `WouldBlock` -- the one outcome `try_lock` reports as unambiguous.
    /// `holder_pid` is the lock file's own content, read on a best-effort
    /// basis (reading never needs the lock another process holds): `None`
    /// means the content was missing or unreadable, not that nobody holds
    /// it -- the file is still locked either way.
    HeldByAnother {
        holder_pid: Option<u32>,
    },
    /// Everything else (D5): the mechanism could not tell held from free,
    /// and the caller must proceed as if unprotected, not refuse to open.
    CannotDecide {
        reason: String,
    },
}

/// Attempts to become the one process holding `project_id`'s write-sensitive
/// state. Never blocks: `try_lock`, not `lock`, because a lock that waited
/// would turn "another instance has this open" into "frozen until it
/// closes", exactly the D5 failure this RFC exists to rule out.
pub fn acquire_project_lock(state_root: &Path, project_id: &str) -> ProjectLockOutcome {
    let dir = project_locks_dir(state_root);
    // `0o700` applies only to a directory this call itself creates -- an
    // already-existing one keeps whatever permissions it has, deliberately
    // not reset here the way `recovery::write_recovery_record`'s own
    // directory is. Resetting it would silently repair a directory D5's own
    // "unreadable state directory" case describes, turning a real ambiguity
    // this function must report as [`ProjectLockOutcome::CannotDecide`] into
    // one it quietly fixed instead -- the exact failure row 1 of
    // `what-the-lock-must-not-do.md` warns against, approached from the
    // other side.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    if let Err(error) = builder.create(&dir) {
        // `DirBuilder::recursive`'s own `AlreadyExists` case is not an error
        // here (D3's directory may already exist from an earlier session) --
        // only a real failure (permission denied, no space, not a directory)
        // reaches here.
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return ProjectLockOutcome::CannotDecide {
                reason: format!("could not create {}: {error}", dir.display()),
            };
        }
    }

    let path = project_lock_file(state_root, project_id);
    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            return ProjectLockOutcome::CannotDecide {
                reason: format!("could not open {}: {error}", path.display()),
            };
        }
    };

    match file.try_lock() {
        Ok(()) => {
            // Held. Overwrite whatever content was there -- our own current
            // pid, and nothing else -- regardless of whether it was a
            // previous holder's clean release, a crashed holder's stale
            // content, or text this build cannot parse at all (D5's three
            // named ambiguous cases: none of them reach this branch unless
            // `try_lock` itself just succeeded, in which case none of them
            // matter any more).
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
            }
            let mut file = file;
            let _ = file.set_len(0);
            let _ = file.write_all(std::process::id().to_string().as_bytes());
            let _ = file.sync_all();
            ProjectLockOutcome::Acquired(ProjectLock { file, path })
        }
        Err(fs::TryLockError::WouldBlock) => {
            let holder_pid = read_holder_pid(&path);
            ProjectLockOutcome::HeldByAnother { holder_pid }
        }
        Err(fs::TryLockError::Error(error)) => ProjectLockOutcome::CannotDecide {
            reason: format!("try_lock on {} failed: {error}", path.display()),
        },
    }
}

/// Best-effort: a missing file, an unreadable one, or content this build
/// cannot parse as a bare pid are all `None`, never an error this module
/// propagates -- naming the holder is a courtesy to the caller (D8), not
/// part of the exclusivity decision `acquire_project_lock` already made by
/// the time this runs.
fn read_holder_pid(path: &Path) -> Option<u32> {
    let mut content = String::new();
    File::open(path).ok()?.read_to_string(&mut content).ok()?;
    content.trim().parse().ok()
}

pub mod attention;

#[cfg(test)]
mod tests;
