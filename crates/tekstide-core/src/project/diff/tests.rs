use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::domain::{ChangeDetectionSource, ChangeDetectionStatus};
use crate::project::change_detection::{
    ChangeLifecycle, ChangePathKind, DetectedChangedPath, DetectedChanges,
};
use crate::project::root::{ProjectRootHandle, ProjectRootValidator, SymlinkPolicy};
use crate::project::{ProjectId, ProjectSession};

use super::{
    BINARY_SNIFF_BYTES, DEFAULT_MAX_DIFF_INPUT_BYTES, DiffGateDecision, DiffGateRefusal,
    DiffPreviewPolicy, gate_diff_content_read, sniff_is_binary,
};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tekstide-diff-gate-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn write_file(&self, relative_path: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        path
    }

    fn root_handle(&self) -> ProjectRootHandle {
        let valid = ProjectRootValidator
            .validate(&self.root, SymlinkPolicy::FailClosed)
            .expect("sandbox root must validate");
        let project = ProjectSession::new(
            ProjectId::new_uuid(),
            "diff-gate-fixture",
            valid.selected_path,
            valid.canonical_path,
        );
        ProjectRootHandle::from_project_session(&project)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn detected(entries: &[(&str, ChangePathKind, ChangeLifecycle)]) -> DetectedChanges {
    DetectedChanges {
        project_id: ProjectId::new_uuid(),
        source: ChangeDetectionSource::FilesystemSnapshot,
        baseline_snapshot_ref: Some("fixture-baseline".to_string()),
        changed_paths: entries
            .iter()
            .map(|(path, kind, lifecycle)| DetectedChangedPath {
                relative_path: PathBuf::from(path),
                kind: *kind,
                lifecycle: *lifecycle,
            })
            .collect(),
        status: ChangeDetectionStatus::Complete,
        scanned_entry_count: entries.len(),
    }
}

/// A path never reported by RFC-012's own detector is refused before
/// anything else about it is checked -- Decision 1 clause 1: this policy
/// authorises reading content for an already-detected change, not for
/// scanning.
#[test]
fn a_path_absent_from_detected_changes_is_refused_before_any_filesystem_check() {
    let sandbox = Sandbox::new("not-detected");
    sandbox.write_file("real.txt", b"hello");
    let changes = detected(&[]); // nothing detected at all

    let result = gate_diff_content_read(
        &changes,
        &sandbox.root_handle(),
        "real.txt",
        DiffPreviewPolicy::default(),
    );

    assert_eq!(result, Err(DiffGateRefusal::PathNotDetected));
}

/// RFC-012 Amendment 1: a `Deleted` lifecycle is checked *first*, ahead
/// of `kind` -- a deleted path has nothing on disk to resolve, size-check,
/// or sniff, whatever kind of thing it used to be. `kind` still reports
/// what it *was* (from the baseline), since `ChangePathKind` no longer
/// has a `Deleted` variant of its own to conflate with it.
#[test]
fn a_deleted_path_is_reported_without_touching_the_filesystem() {
    let sandbox = Sandbox::new("deleted-lifecycle");
    // Never written to disk at all -- if this reached a filesystem check,
    // it would fail for the wrong reason (missing), not report Deleted.
    let changes = detected(&[("gone.txt", ChangePathKind::File, ChangeLifecycle::Deleted)]);

    let result = gate_diff_content_read(
        &changes,
        &sandbox.root_handle(),
        "gone.txt",
        DiffPreviewPolicy::default(),
    );

    assert_eq!(
        result,
        Ok(DiffGateDecision::Deleted {
            kind: ChangePathKind::File
        })
    );
}

/// The three non-`File` kinds still present at detection time have no
/// text content to bound, sniff, or read -- reported as `NonFile`, not
/// routed through any filesystem check at all. Distinct from `Deleted`:
/// these still exist, they are just not a plain file.
#[test]
fn non_file_kinds_still_present_are_reported_without_any_content_check() {
    let sandbox = Sandbox::new("non-file-kinds");
    let changes = detected(&[
        (
            "a-directory",
            ChangePathKind::Directory,
            ChangeLifecycle::Modified,
        ),
        (
            "a-symlink",
            ChangePathKind::Symlink,
            ChangeLifecycle::Modified,
        ),
        (
            "something-else",
            ChangePathKind::Other,
            ChangeLifecycle::Modified,
        ),
    ]);
    let root = sandbox.root_handle();

    for (path, kind) in [
        ("a-directory", ChangePathKind::Directory),
        ("a-symlink", ChangePathKind::Symlink),
        ("something-else", ChangePathKind::Other),
    ] {
        let result = gate_diff_content_read(&changes, &root, path, DiffPreviewPolicy::default());
        assert_eq!(
            result,
            Ok(DiffGateDecision::NonFile { kind }),
            "{path} must report its own real kind, not be silently skipped"
        );
    }
}

/// A detected `File` that fails root/symlink resolution reuses
/// `ProjectFileAccessPolicy` rather than a second safety check --
/// checked against a path that no longer exists on disk (the detector's
/// own scan is not instantaneous with a diff request; the file may be
/// gone by the time content is asked for).
#[test]
fn a_detected_file_missing_on_disk_reuses_the_real_access_policy_and_refuses() {
    let sandbox = Sandbox::new("missing-on-disk");
    // Never actually written -- `changed_paths` claims it exists (as a
    // real detector scan would have, at scan time), disk disagrees now.
    // Lifecycle `Modified`, not `Deleted`: the detector's own scan really
    // did see it as a live file at scan time, so gating must still try
    // to resolve it (and fail there) rather than short-circuit.
    let changes = detected(&[("gone.txt", ChangePathKind::File, ChangeLifecycle::Modified)]);

    let result = gate_diff_content_read(
        &changes,
        &sandbox.root_handle(),
        "gone.txt",
        DiffPreviewPolicy::default(),
    );

    assert!(
        matches!(result, Err(DiffGateRefusal::Access(_))),
        "expected Access, got {result:?}"
    );
}

/// Decision 2's own boundary, against the real default bound: exactly
/// `DEFAULT_MAX_DIFF_INPUT_BYTES` is accepted, one byte more is refused.
/// Real files on disk, not a substituted small policy -- the same shape
/// `content_within_bound_accepts_content_exactly_at_the_cap` uses for
/// RFC-018's own bound.
#[test]
fn the_boundary_is_exact_not_greater_than_or_equal() {
    let sandbox = Sandbox::new("boundary");
    let at_bound = vec![b'a'; DEFAULT_MAX_DIFF_INPUT_BYTES as usize];
    let over_bound = vec![b'a'; DEFAULT_MAX_DIFF_INPUT_BYTES as usize + 1];
    sandbox.write_file("at-bound.txt", &at_bound);
    sandbox.write_file("over-bound.txt", &over_bound);
    let changes = detected(&[
        (
            "at-bound.txt",
            ChangePathKind::File,
            ChangeLifecycle::Modified,
        ),
        (
            "over-bound.txt",
            ChangePathKind::File,
            ChangeLifecycle::Modified,
        ),
    ]);
    let root = sandbox.root_handle();

    assert_eq!(
        gate_diff_content_read(
            &changes,
            &root,
            "at-bound.txt",
            DiffPreviewPolicy::default()
        ),
        Ok(DiffGateDecision::Readable {
            lifecycle: ChangeLifecycle::Modified
        }),
        "exactly the bound must be accepted"
    );
    assert_eq!(
        gate_diff_content_read(
            &changes,
            &root,
            "over-bound.txt",
            DiffPreviewPolicy::default()
        ),
        Err(DiffGateRefusal::TooLarge {
            relative_path: PathBuf::from("over-bound.txt"),
            len: DEFAULT_MAX_DIFF_INPUT_BYTES + 1,
            max: DEFAULT_MAX_DIFF_INPUT_BYTES,
        }),
        "one byte over the bound must refuse, not silently pass or truncate"
    );
}

/// **The review gate's own required proof**: refusal happens before any
/// content read, not merely ordered correctly in the source. Removing
/// every permission from an oversized file leaves `fs::metadata` (which
/// needs only directory search permission, not permission on the file
/// itself) able to succeed while `File::open` for reading would fail
/// with `EACCES`. A `TooLarge` refusal with the real, accurately measured
/// length is therefore only possible if the size check ran from metadata
/// alone and the function returned *before* ever attempting to open the
/// file -- if it had tried to open it first, this would fail with an
/// access error instead, not a size refusal.
#[test]
fn refusal_happens_from_metadata_alone_before_any_open_is_attempted() {
    let sandbox = Sandbox::new("refuse-before-open");
    let over_bound = vec![b'a'; DEFAULT_MAX_DIFF_INPUT_BYTES as usize + 1];
    let path = sandbox.write_file("unreadable-and-oversized.txt", &over_bound);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let changes = detected(&[(
        "unreadable-and-oversized.txt",
        ChangePathKind::File,
        ChangeLifecycle::Modified,
    )]);

    let result = gate_diff_content_read(
        &changes,
        &sandbox.root_handle(),
        "unreadable-and-oversized.txt",
        DiffPreviewPolicy::default(),
    );

    // Restore permissions before the sandbox's own Drop tries to remove it.
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

    assert_eq!(
        result,
        Err(DiffGateRefusal::TooLarge {
            relative_path: PathBuf::from("unreadable-and-oversized.txt"),
            len: DEFAULT_MAX_DIFF_INPUT_BYTES + 1,
            max: DEFAULT_MAX_DIFF_INPUT_BYTES,
        }),
        "a TooLarge refusal with the real length proves the size check ran from metadata \
         alone -- an attempted open on a 0o000 file would have surfaced as an access \
         failure instead, not this"
    );
}

/// Correctness: a real, small, non-binary file within the bound is
/// `Readable`.
#[test]
fn a_small_text_file_within_bound_is_readable() {
    let sandbox = Sandbox::new("readable-text");
    sandbox.write_file("notes.txt", b"hello world\n");
    let changes = detected(&[("notes.txt", ChangePathKind::File, ChangeLifecycle::Modified)]);

    let result = gate_diff_content_read(
        &changes,
        &sandbox.root_handle(),
        "notes.txt",
        DiffPreviewPolicy::default(),
    );

    assert_eq!(
        result,
        Ok(DiffGateDecision::Readable {
            lifecycle: ChangeLifecycle::Modified
        })
    );
}

/// Decision 4: a NUL byte within the sniff window classifies the file as
/// non-text, reported with its real length -- not attempted as a diff.
#[test]
fn a_nul_byte_in_the_sniff_window_classifies_as_non_text() {
    let sandbox = Sandbox::new("binary-sniff");
    let mut bytes = b"PNG-ish header".to_vec();
    bytes.push(0);
    bytes.extend_from_slice(b"more binary-looking bytes");
    let real_len = bytes.len() as u64;
    sandbox.write_file("image.png", &bytes);
    let changes = detected(&[("image.png", ChangePathKind::File, ChangeLifecycle::Added)]);

    let result = gate_diff_content_read(
        &changes,
        &sandbox.root_handle(),
        "image.png",
        DiffPreviewPolicy::default(),
    );

    assert_eq!(
        result,
        Ok(DiffGateDecision::NonTextContent {
            len: real_len,
            lifecycle: ChangeLifecycle::Added
        })
    );
}

/// **RFC-012 Amendment 1's own point, proven directly**: the real
/// lifecycle a `DetectedChangedPath` carries reaches the decision
/// unaltered, for both cases RFC-024's corrected table treats
/// differently (Added: no "not a diff" label; Modified: labelled).
/// Checked together so a bug that swapped the two would be caught here,
/// not only by each half individually happening to use a different
/// fixture value elsewhere.
#[test]
fn readable_decisions_carry_the_real_lifecycle_through_unaltered() {
    let sandbox = Sandbox::new("lifecycle-passthrough");
    sandbox.write_file("added.txt", b"brand new content\n");
    sandbox.write_file("modified.txt", b"changed content\n");
    let changes = detected(&[
        ("added.txt", ChangePathKind::File, ChangeLifecycle::Added),
        (
            "modified.txt",
            ChangePathKind::File,
            ChangeLifecycle::Modified,
        ),
    ]);
    let root = sandbox.root_handle();

    assert_eq!(
        gate_diff_content_read(&changes, &root, "added.txt", DiffPreviewPolicy::default()),
        Ok(DiffGateDecision::Readable {
            lifecycle: ChangeLifecycle::Added
        })
    );
    assert_eq!(
        gate_diff_content_read(
            &changes,
            &root,
            "modified.txt",
            DiffPreviewPolicy::default()
        ),
        Ok(DiffGateDecision::Readable {
            lifecycle: ChangeLifecycle::Modified
        })
    );
}

/// **The review gate's own required proof, the negative direction**: the
/// sniff never reads past `BINARY_SNIFF_BYTES`, proven rather than
/// asserted. A FIFO whose writer supplies exactly `BINARY_SNIFF_BYTES`
/// (with a `NUL` early on, so classification succeeds) and then blocks
/// without closing means a read that tried to go *past* that boundary
/// would itself block forever on the empty, still-open pipe. Completing
/// within a short timeout is only possible if the read really stopped at
/// the bound -- an unbounded `read_to_end` would hang on this fixture,
/// not return `Ok(true)`.
#[test]
fn the_binary_sniff_never_reads_past_its_own_bound() {
    let sandbox = Sandbox::new("sniff-bounded");
    let fifo_path = sandbox.root.join("blocking.fifo");
    make_fifo(&fifo_path);

    let writer_path = fifo_path.clone();
    let writer = std::thread::spawn(move || {
        // Opening a FIFO for writing blocks until a reader opens it too;
        // this thread's write() call itself is what unblocks once
        // `sniff_is_binary` opens the other end.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&writer_path)
            .expect("writer must open the fifo");
        let mut payload = vec![0u8]; // NUL first byte -- classifies binary immediately
        payload.extend(std::iter::repeat_n(b'x', BINARY_SNIFF_BYTES - 1));
        file.write_all(&payload)
            .expect("writer must supply exactly the sniff window");
        // Deliberately never writes more and never closes `file` here --
        // an unbounded reader on the other end would block waiting for
        // either. Keep the handle alive until the test's own assertions
        // finish so the pipe cannot report EOF early.
        std::thread::sleep(Duration::from_secs(2));
    });

    let (sender, receiver) = mpsc::channel();
    let sniff_path = fifo_path.clone();
    std::thread::spawn(move || {
        let result = sniff_is_binary(&sniff_path);
        let _ = sender.send(result);
    });

    let result = receiver.recv_timeout(Duration::from_millis(500)).expect(
        "sniff_is_binary must return within its own bound, not block reading past it -- \
             a hang here means the sniff is unbounded",
    );

    assert_eq!(
        result,
        Ok(true),
        "the NUL byte at the very start of the fixture must still classify as binary"
    );

    writer.join().expect("writer thread must not panic");
}

fn make_fifo(path: &Path) {
    let c_path = CString::new(path.as_os_str().as_bytes()).expect("fifo path must not contain NUL");
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    assert_eq!(
        result,
        0,
        "mkfifo failed: {}",
        std::io::Error::last_os_error()
    );
}
