use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::content::FileSnapshot;
use crate::domain::{ChangeDetectionSource, ChangeDetectionStatus};
use crate::project::change_detection::{
    ChangeLifecycle, ChangePathKind, DetectedChangedPath, DetectedChanges,
};
use crate::project::root::{
    FileAccessBlockedReason, ProjectRootHandle, ProjectRootValidator, SymlinkPolicy,
};
use crate::project::{ProjectId, ProjectSession};

use super::{
    BINARY_SNIFF_BYTES, ContentLifecycle, DEFAULT_MAX_DIFF_INPUT_BYTES, DiffContent,
    DiffContentError, DiffGateDecision, DiffGateRefusal, DiffPreviewPolicy, diff_content_is_stale,
    gate_diff_content_read, read_bounded, read_diff_content, sniff_is_binary,
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
            lifecycle: ContentLifecycle::Modified
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
            lifecycle: ContentLifecycle::Modified
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
            lifecycle: ContentLifecycle::Added
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
            lifecycle: ContentLifecycle::Added
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
            lifecycle: ContentLifecycle::Modified
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

// ---------------------------------------------------------------------
// RFC-024 PR-024-C: content access with a bounded lifetime.
// ---------------------------------------------------------------------

/// RFC-024 §Correction: Added content is the whole change, delivered
/// bounded and gated -- not a diff, since there is no "before" to compare
/// against by definition.
#[test]
fn added_content_is_delivered_whole_bounded_and_gated() {
    let sandbox = Sandbox::new("added-content");
    let bytes = b"brand new file, whole content is the whole change\n".to_vec();
    sandbox.write_file("new.txt", &bytes);
    let changes = detected(&[("new.txt", ChangePathKind::File, ChangeLifecycle::Added)]);

    let result = read_diff_content(
        &changes,
        &sandbox.root_handle(),
        "new.txt",
        DiffPreviewPolicy::default(),
    );

    match result {
        Ok(DiffContent::Added {
            bytes: returned,
            baseline,
        }) => {
            assert_eq!(returned, bytes);
            assert_eq!(baseline.len, bytes.len() as u64);
        }
        other => panic!("expected Added, got {other:?}"),
    }
}

/// RFC-024 §Correction: Modified delivers current content only, and the
/// variant itself -- not a flag or a doc comment -- is what marks it "not
/// a diff". Distinguished from `Added` even when the bytes are identical
/// shape, since the corrected scope table treats the two differently at
/// the surface (Added: no "not a diff" label; Modified: labelled).
#[test]
fn modified_content_is_current_content_explicitly_not_a_diff() {
    let sandbox = Sandbox::new("modified-content");
    let bytes = b"current content only -- this RFC cannot produce a before side\n".to_vec();
    sandbox.write_file("changed.txt", &bytes);
    let changes = detected(&[(
        "changed.txt",
        ChangePathKind::File,
        ChangeLifecycle::Modified,
    )]);

    let result = read_diff_content(
        &changes,
        &sandbox.root_handle(),
        "changed.txt",
        DiffPreviewPolicy::default(),
    );

    match &result {
        Ok(DiffContent::Modified {
            bytes: returned,
            baseline,
        }) => {
            assert_eq!(*returned, bytes);
            assert_eq!(baseline.len, bytes.len() as u64);
        }
        other => panic!("expected Modified, got {other:?}"),
    }
    assert!(
        !matches!(result, Ok(DiffContent::Added { .. })),
        "must not be reachable as Added -- the two are separate constructors, not one \
         shape with a lifecycle tag a caller could misread"
    );
}

/// The corrected table's `Deleted` row: the fact of deletion, from
/// metadata alone -- no bytes exist to read, and none are attempted.
/// `gone.txt` is never written to disk; if this reached a filesystem
/// read, it would fail for the wrong reason (missing), not report
/// `Deleted`.
#[test]
fn deleted_reports_the_fact_of_deletion_without_reading_anything() {
    let sandbox = Sandbox::new("deleted-content");
    let changes = detected(&[("gone.txt", ChangePathKind::File, ChangeLifecycle::Deleted)]);

    let result = read_diff_content(
        &changes,
        &sandbox.root_handle(),
        "gone.txt",
        DiffPreviewPolicy::default(),
    );

    assert_eq!(
        result,
        Ok(DiffContent::Deleted {
            kind: ChangePathKind::File
        })
    );
}

/// `NonTextContent`/`NonFile` pass through from the gate unread -- neither
/// ever had bytes to deliver, so `read_diff_content` performs no read
/// beyond what `gate_diff_content_read` already did for either.
#[test]
fn non_text_and_non_file_decisions_pass_through_without_a_further_read() {
    let sandbox = Sandbox::new("passthrough");
    let mut binary_bytes = vec![0u8];
    binary_bytes.extend_from_slice(b"binary-looking content");
    let real_len = binary_bytes.len() as u64;
    sandbox.write_file("image.png", &binary_bytes);
    let changes = detected(&[
        ("image.png", ChangePathKind::File, ChangeLifecycle::Added),
        (
            "a-directory",
            ChangePathKind::Directory,
            ChangeLifecycle::Modified,
        ),
    ]);
    let root = sandbox.root_handle();

    match read_diff_content(&changes, &root, "image.png", DiffPreviewPolicy::default()) {
        Ok(DiffContent::NonTextContent {
            len,
            lifecycle,
            baseline,
        }) => {
            assert_eq!(len, real_len);
            assert_eq!(lifecycle, ContentLifecycle::Added);
            assert_eq!(baseline.len, real_len);
        }
        other => panic!("expected NonTextContent, got {other:?}"),
    }
    assert_eq!(
        read_diff_content(&changes, &root, "a-directory", DiffPreviewPolicy::default()),
        Ok(DiffContent::NonFile {
            kind: ChangePathKind::Directory
        })
    );
}

/// **The review gate's own required proof**: content is not pre-escaped.
/// A file containing the exact bidi/format-character probes
/// `text_safety`'s own tests use (a right-to-left override, the Hangul
/// filler) must come back byte-for-byte unchanged -- `quote_untrusted`'s
/// visible-marker wrapping is RFC-020's job at render time, not this
/// function's. A model that escaped here would hide real file content
/// from any consumer that is not a renderer.
#[test]
fn content_is_not_pre_escaped_raw_bytes_survive_unaltered() {
    let sandbox = Sandbox::new("not-pre-escaped");
    let mut bytes = b"before ".to_vec();
    bytes.extend_from_slice("\u{202E}".as_bytes()); // RIGHT-TO-LEFT OVERRIDE
    bytes.extend_from_slice("evil.txt".as_bytes());
    bytes.extend_from_slice("\u{202C}".as_bytes()); // POP DIRECTIONAL FORMATTING
    bytes.extend_from_slice(" after".as_bytes());
    sandbox.write_file("bidi.txt", &bytes);
    let changes = detected(&[("bidi.txt", ChangePathKind::File, ChangeLifecycle::Added)]);

    let result = read_diff_content(
        &changes,
        &sandbox.root_handle(),
        "bidi.txt",
        DiffPreviewPolicy::default(),
    );

    match result {
        Ok(DiffContent::Added {
            bytes: returned, ..
        }) => assert_eq!(
            returned, bytes,
            "raw bytes must survive exactly, including the bidi override -- \
             escaping them here would be this function overstepping into RFC-020's job"
        ),
        other => panic!("expected Added with the exact original bytes, got {other:?}"),
    }
}

/// `read_diff_content` reuses the gate rather than re-deriving its
/// refusals: a path absent from `detected.changed_paths` refuses via
/// `DiffContentError::Gate`, carrying the same `DiffGateRefusal` the gate
/// itself would have returned.
#[test]
fn an_undetected_path_refuses_through_the_reused_gate() {
    let sandbox = Sandbox::new("content-undetected");
    sandbox.write_file("real.txt", b"hello");
    let changes = detected(&[]);

    let result = read_diff_content(
        &changes,
        &sandbox.root_handle(),
        "real.txt",
        DiffPreviewPolicy::default(),
    );

    assert_eq!(
        result,
        Err(DiffContentError::Gate(DiffGateRefusal::PathNotDetected))
    );
}

/// A path missing on disk refuses through the reused gate's own
/// resolution -- `read_diff_content` no longer performs a second,
/// independent resolve (see `evaluate_gate`), so there is exactly one
/// place this can fail, and it surfaces as `DiffContentError::Gate`, not
/// a separate access-error case.
#[test]
fn a_file_missing_on_disk_refuses_through_the_single_shared_resolution() {
    let sandbox = Sandbox::new("content-missing-on-disk");
    let changes = detected(&[("gone.txt", ChangePathKind::File, ChangeLifecycle::Modified)]);

    let result = read_diff_content(
        &changes,
        &sandbox.root_handle(),
        "gone.txt",
        DiffPreviewPolicy::default(),
    );

    assert!(
        matches!(
            result,
            Err(DiffContentError::Gate(DiffGateRefusal::Access(_)))
        ),
        "expected Gate(Access(_)), got {result:?}"
    );
}

/// Defense in depth: even though `gate_diff_content_read` already checked
/// size from metadata, the read itself independently refuses rather than
/// truncating if it observes more than the bound -- calling `read_bounded`
/// directly (bypassing the gate) against a real over-bound file, the same
/// way `sniff_is_binary` is tested directly elsewhere in this file.
#[test]
fn the_bounded_read_refuses_rather_than_truncates_when_called_directly() {
    let sandbox = Sandbox::new("bounded-read-direct");
    let over_bound = vec![b'a'; 101];
    let path = sandbox.write_file("over.txt", &over_bound);

    let result = read_bounded(&path, 100);

    assert_eq!(
        result,
        Err(()),
        "a file one byte over the bound must refuse, not return a 100-byte truncated prefix"
    );
}

/// Enumeration test naming every production call site in `tekstide-core`
/// that reads a file's full content as raw bytes -- PR-024-C's own review
/// gate item, using the same recursive-scan-plus-closed-list technique
/// `i18n::enforcement`'s scans use in `crates/tekstide` for a different
/// property. A new raw-content-read call site anywhere else fails this
/// test by file name, rather than being discoverable only by a one-time
/// grep recorded in prose.
///
/// **Closed list, each entry's reason disclosed**: `project/diff.rs`
/// (this module, PR-024-C's own read) and `content/open.rs`
/// (`TextDocument`'s pre-existing editor read, a different feature) both
/// read generated-change/project-file content and are the two paths this
/// RFC's Decision 1 gates. `project/recent/store.rs` and
/// `audit/recovery.rs` also read a whole file's bytes, but neither reads
/// project or generated-change content -- the former is this
/// application's own small recent-projects state file, the latter an
/// audit-store recovery manifest. Both pre-date this RFC and are
/// unrelated to what Decision 1 governs; listed here so the scan's
/// closed list stays accurate rather than silently widening its own
/// definition of "content" to cover them.
const FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT: &[&str] = &[
    "project/diff.rs",
    "content/open.rs",
    "project/recent/store.rs",
    "audit/recovery.rs",
];

fn tekstide_core_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("src dir must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn contains_a_raw_full_file_read(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with("//")
            && (trimmed.contains("read_to_end(")
                || trimmed.contains("fs::read(")
                || trimmed.contains("std::fs::read("))
    })
}

/// Ablation-verified (`qa-evidence.md`): add a `fs::read_to_string(...)`
/// call to an unlisted file, confirm this fails naming it, revert.
#[test]
fn enumeration_confirms_only_the_closed_list_reads_full_file_content() {
    let mut files = Vec::new();
    collect_rs_files(&tekstide_core_src_dir(), &mut files);

    for path in files {
        let relative = path
            .strip_prefix(tekstide_core_src_dir())
            .expect("file must be under src/")
            .to_str()
            .expect("path must be valid UTF-8")
            .to_string();

        // Test files legitimately read fixtures they created themselves.
        if relative.contains("/tests/") || relative.ends_with("tests.rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("scannable file must be readable");
        let reads_full_content = contains_a_raw_full_file_read(&source);
        let is_allowed = FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT.contains(&relative.as_str());

        assert!(
            !reads_full_content || is_allowed,
            "{relative} reads a file's full content but is not in \
             FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT -- a new content-read call site outside \
             the reviewed paths must be added here deliberately, naming what it reads and why, \
             not left to pass silently"
        );
    }

    for &expected in FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT {
        let path = tekstide_core_src_dir().join(expected);
        assert!(
            path.exists(),
            "FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT names {expected:?} but that file does not \
             exist -- stale exemption entry"
        );
    }
}

// ---------------------------------------------------------------------
// RFC-024 PR-024-D: baseline authority.
// ---------------------------------------------------------------------

/// **The review gate's own required proof**: a stale baseline is reported
/// as stale, against a real file changed on disk after capture -- not a
/// synthesised value. Reads content once (capturing its `baseline`),
/// mutates the real file afterward, then asks whether that same baseline
/// is still current.
#[test]
fn a_stale_baseline_is_reported_as_stale_not_silently_diffed() {
    let sandbox = Sandbox::new("stale-baseline");
    sandbox.write_file("changed.txt", b"original content\n");
    let changes = detected(&[(
        "changed.txt",
        ChangePathKind::File,
        ChangeLifecycle::Modified,
    )]);
    let root = sandbox.root_handle();

    let baseline =
        match read_diff_content(&changes, &root, "changed.txt", DiffPreviewPolicy::default()) {
            Ok(DiffContent::Modified { baseline, .. }) => baseline,
            other => panic!("expected Modified, got {other:?}"),
        };

    // A real mutation, not a synthesised FileSnapshot -- the same file,
    // changed on disk, after the baseline above was already captured.
    // Sleep briefly first: some filesystems have coarse mtime resolution,
    // and this property must hold on a real change, not rely on a race.
    std::thread::sleep(Duration::from_millis(10));
    sandbox.write_file("changed.txt", b"a real external mutation\n");

    let stale = diff_content_is_stale(&baseline, &root, "changed.txt");

    assert_eq!(
        stale,
        Ok(true),
        "a file genuinely changed on disk after the baseline was captured must report stale"
    );
}

/// The other half of the same property: a baseline against a file that
/// has not changed reports unchanged, not stale -- proving this is a real
/// comparison, not a function that always answers "stale".
#[test]
fn an_unchanged_baseline_is_reported_as_unchanged() {
    let sandbox = Sandbox::new("unchanged-baseline");
    sandbox.write_file("stable.txt", b"never touched again\n");
    let changes = detected(&[("stable.txt", ChangePathKind::File, ChangeLifecycle::Added)]);
    let root = sandbox.root_handle();

    let baseline =
        match read_diff_content(&changes, &root, "stable.txt", DiffPreviewPolicy::default()) {
            Ok(DiffContent::Added { baseline, .. }) => baseline,
            other => panic!("expected Added, got {other:?}"),
        };

    let stale = diff_content_is_stale(&baseline, &root, "stable.txt");

    assert_eq!(stale, Ok(false));
}

/// A file deleted since the baseline was captured is stale -- the same
/// "the file being gone is itself the change" reasoning
/// `TextDocument::refresh_external_state` already applies to a missing
/// current target, reused here rather than surfaced as an error.
#[test]
fn a_file_deleted_since_capture_is_reported_as_stale() {
    let sandbox = Sandbox::new("deleted-since-capture");
    let path = sandbox.write_file("goes-away.txt", b"here for now\n");
    let changes = detected(&[(
        "goes-away.txt",
        ChangePathKind::File,
        ChangeLifecycle::Modified,
    )]);
    let root = sandbox.root_handle();

    let baseline = match read_diff_content(
        &changes,
        &root,
        "goes-away.txt",
        DiffPreviewPolicy::default(),
    ) {
        Ok(DiffContent::Modified { baseline, .. }) => baseline,
        other => panic!("expected Modified, got {other:?}"),
    };

    fs::remove_file(&path).unwrap();

    let stale = diff_content_is_stale(&baseline, &root, "goes-away.txt");

    assert_eq!(stale, Ok(true));
}

/// **A real policy violation is not silently folded into "stale".** A
/// symlink escaping the project root is a security-relevant refusal
/// (`FileAccessBlockedReason::SymlinkEscape`), not evidence the file
/// "changed" -- it must surface as `Err`, mirroring
/// `TextDocument::refresh_external_state`'s own narrower distinction
/// (only `MissingPath` folds into a changed-state outcome; every other
/// access refusal propagates). **Ablated**: removed the `MissingPath`
/// guard so every `Access` error folded into `Ok(true)` -- this test then
/// failed by returning `Ok(true)` instead of the expected `Err`, proving
/// a real security refusal would otherwise be silently swallowed as
/// ordinary staleness. Reverted before committing.
#[test]
fn a_real_access_violation_surfaces_as_an_error_not_silent_staleness() {
    let sandbox = Sandbox::new("symlink-escape-staleness");
    let outside_file = {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tekstide-diff-staleness-outside-{}-{nonce}.txt",
            std::process::id()
        ));
        fs::write(&path, b"outside the project root\n").unwrap();
        path
    };
    std::os::unix::fs::symlink(&outside_file, sandbox.root.join("escape-link.txt")).unwrap();
    let root = sandbox.root_handle();

    // Any baseline value works here -- resolution fails before it would
    // ever be compared against one.
    let placeholder_baseline = FileSnapshot {
        canonical_path: outside_file.clone(),
        modified_at: SystemTime::now(),
        len: 0,
        content_hash: None,
    };

    let result = diff_content_is_stale(&placeholder_baseline, &root, "escape-link.txt");

    assert!(
        matches!(
            result,
            Err(DiffContentError::Gate(DiffGateRefusal::Access(_)))
        ),
        "expected a real Access error, got {result:?}"
    );
    if let Err(DiffContentError::Gate(DiffGateRefusal::Access(error))) = result {
        assert_eq!(error.reason, FileAccessBlockedReason::SymlinkEscape);
    }

    let _ = fs::remove_file(&outside_file);
}
