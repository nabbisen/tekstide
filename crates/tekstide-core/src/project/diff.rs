//! RFC-024 PR-024-B: the decision layer, before any content is read.
//!
//! **This module does not compute a diff.** It decides whether reading a
//! path's content for diff preview is allowed at all, and whether that
//! content may be attempted as text -- the size check and the binary
//! classification, both refusing rather than truncating or reading past
//! what they need to answer their own question. Content access itself
//! (PR-024-C) and the diff computation are later, separate concerns; the
//! text-vs-non-text and size decisions made here happen before either.
//!
//! **Nothing here retains anything.** `gate_diff_content_read` borrows,
//! reads a bounded prefix or a metadata call, and returns a decision --
//! there is no field anywhere in this module a caller could accidentally
//! hold past the call.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::change_detection::{ChangeLifecycle, ChangePathKind, DetectedChanges};
use super::root::{FileAccessError, ProjectFileAccessPolicy, ProjectRootHandle};

/// RFC-024 Decision 2, Open question 1 -- measured, not estimated
/// (2026-08-11). Holding two text buffers at this size costs
/// approximately 10.2 MiB of real transient RSS (measured via
/// `/proc/self/status` `VmRSS` deltas around allocating two ~4 MiB
/// `String`s built from a repeated realistic line pattern, not a
/// pathological all-zero buffer) -- well inside safe headroom for a
/// single on-demand request that is dropped immediately after
/// (Decision 1's third clause; nothing here is a standing cache).
///
/// Chosen to equal RFC-019's own reviewed `DEFAULT_MAX_EDITABLE_BYTES`
/// rather than an unrelated new number: a diff is fundamentally a
/// comparison of two files, and bounding each side at the same standard
/// a human already edits one file under is a coherent, already-justified
/// choice, not an arbitrary new constant. The bound applies to **each**
/// version independently (Decision 2: "covers both versions"), not their
/// sum -- a 3 MiB "before" against a 5 MiB "after" refuses on the "after"
/// side alone, the same way a 5 MiB "before" against a 1 MiB "after"
/// would refuse on the "before" side. Two versions is a property of what
/// a diff needs, not of this constant, which only bounds one side at a
/// time.
pub const DEFAULT_MAX_DIFF_INPUT_BYTES: u64 = 4 * 1024 * 1024;

/// RFC-024 Decision 4 -- a bounded prefix is enough to answer "is this
/// binary" without reading, or bounding, the whole file. 8000 bytes
/// matches the sniff size common tooling (git, ripgrep) already uses for
/// the same "does a NUL byte appear early" heuristic
/// `content::open::TextDocumentOpenError::ContainsNul` uses over an
/// already-bounded *full* read -- applied here to a small prefix instead,
/// specifically so the size check (which needs only metadata) and the
/// binary check (which needs only a sniff) both happen before a full read
/// is ever attempted, per Decision 4's own ordering.
const BINARY_SNIFF_BYTES: usize = 8000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiffPreviewPolicy {
    pub max_input_bytes: u64,
}

impl DiffPreviewPolicy {
    pub fn linux_mvp() -> Self {
        Self {
            max_input_bytes: DEFAULT_MAX_DIFF_INPUT_BYTES,
        }
    }
}

impl Default for DiffPreviewPolicy {
    fn default() -> Self {
        Self::linux_mvp()
    }
}

/// What gating a path decided, when it decided anything at all rather
/// than refusing. `Deleted`, `NonFile`, and `NonTextContent` are all
/// real, expected outcomes RFC-024 itself names -- "a non-text change is
/// reported as a change with its size and kind, and no diff is
/// attempted" -- not errors; `DiffGateRefusal` is for the cases nothing
/// may be reported at all.
///
/// `lifecycle` (RFC-012 Amendment 1) appears only on `Readable` and
/// `NonTextContent` -- the two outcomes RFC-024's corrected §Correction
/// table actually needs it for (Added: no "not a diff" label; Modified:
/// current content, explicitly labelled). `Deleted` and `NonFile` never
/// produce diffable content regardless of lifecycle, so nothing here
/// carries it for them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffGateDecision {
    /// A `File`, not deleted, within the bound, sniffed as text. Safe to
    /// attempt a full bounded read (PR-024-C's job, not this function's).
    Readable { lifecycle: ChangeLifecycle },
    /// A `File`, not deleted, within the bound, but the first
    /// `BINARY_SNIFF_BYTES` contain a NUL byte. No diff is attempted; the
    /// caller reports a non-text change with this length.
    NonTextContent {
        len: u64,
        lifecycle: ChangeLifecycle,
    },
    /// This path's lifecycle is `Deleted` -- checked first, before `kind`
    /// is consulted at all, since a deleted path has nothing on disk to
    /// resolve, size-check, or sniff regardless of what it used to be.
    /// `kind` reports what it *was* (RFC-012 Amendment 1: read from the
    /// baseline, since `ChangePathKind` itself no longer has a `Deleted`
    /// variant to conflate "what kind" with "what happened").
    Deleted { kind: ChangePathKind },
    /// Not deleted, but not a `File` either -- `Directory`, `Symlink`, or
    /// `Other`. None of these have text content to bound, sniff, or
    /// read; a symlink specifically is never resolved or touched by this
    /// function at all, matching this project's consistently cautious
    /// treatment of symlinks elsewhere (`FileAccessSymlinkStatus`, the
    /// explorer's status-not-target decision).
    NonFile { kind: ChangePathKind },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiffGateRefusal {
    /// Decision 1, clause 1: this policy authorises reading content for
    /// an already-detected change, not for scanning. A path absent from
    /// `detected.changed_paths` is refused before anything else is
    /// checked, root-contained or not.
    PathNotDetected,
    /// Root/symlink-escape safety, reused from `ProjectFileAccessPolicy`
    /// rather than re-implemented -- the same policy `TextDocument::open`
    /// resolves an editable path through.
    Access(FileAccessError),
    /// A metadata or sniff read failed *after* `Access` already
    /// succeeded (for example: the file was removed in the window
    /// between resolving it and reading its metadata). Distinct from
    /// `Access` because `ProjectFileAccessPolicy` had already accepted
    /// the path; this is a plain I/O failure on an already-approved one.
    MetadataUnavailable { relative_path: PathBuf },
    /// Decision 2: refused whole, never truncated. `len` is the real,
    /// measured size from metadata -- read before any content, so this
    /// never reflects a partial read.
    TooLarge {
        relative_path: PathBuf,
        len: u64,
        max: u64,
    },
}

/// The gate. Order, matching Decision 2 → Decision 4 exactly, with
/// RFC-012 Amendment 1's lifecycle check ahead of both: confirm the path
/// was already detected → if its lifecycle is `Deleted`, stop -- nothing
/// on disk to resolve → resolve it safely (root/symlink) → check its
/// size against metadata alone → sniff a bounded prefix for binary
/// content. A full read is never attempted by this function; `Readable`
/// is the furthest it goes.
pub fn gate_diff_content_read(
    detected: &DetectedChanges,
    root: &ProjectRootHandle,
    selected_relative_path: impl AsRef<Path>,
    policy: DiffPreviewPolicy,
) -> Result<DiffGateDecision, DiffGateRefusal> {
    let selected_relative_path = selected_relative_path.as_ref();

    let Some(changed) = detected
        .changed_paths
        .iter()
        .find(|changed| changed.relative_path == selected_relative_path)
    else {
        return Err(DiffGateRefusal::PathNotDetected);
    };

    if changed.lifecycle == ChangeLifecycle::Deleted {
        return Ok(DiffGateDecision::Deleted { kind: changed.kind });
    }

    if changed.kind != ChangePathKind::File {
        return Ok(DiffGateDecision::NonFile { kind: changed.kind });
    }
    let lifecycle = changed.lifecycle;

    let target = ProjectFileAccessPolicy
        .resolve_existing(root, selected_relative_path)
        .map_err(DiffGateRefusal::Access)?;

    let len = fs::metadata(&target.canonical_path)
        .map(|metadata| metadata.len())
        .map_err(|_| DiffGateRefusal::MetadataUnavailable {
            relative_path: target.selected_relative_path.clone(),
        })?;

    if len > policy.max_input_bytes {
        return Err(DiffGateRefusal::TooLarge {
            relative_path: target.selected_relative_path.clone(),
            len,
            max: policy.max_input_bytes,
        });
    }

    let is_binary = sniff_is_binary(&target.canonical_path).map_err(|()| {
        DiffGateRefusal::MetadataUnavailable {
            relative_path: target.selected_relative_path.clone(),
        }
    })?;

    if is_binary {
        return Ok(DiffGateDecision::NonTextContent { len, lifecycle });
    }

    Ok(DiffGateDecision::Readable { lifecycle })
}

/// Reads at most `BINARY_SNIFF_BYTES`, never the whole file regardless of
/// its real size -- the sniff itself must stay cheap even for a file this
/// gate is about to refuse for being too large, since the size check
/// above already ran first and this only runs when it passed.
fn sniff_is_binary(canonical_path: &Path) -> Result<bool, ()> {
    let file = File::open(canonical_path).map_err(|_| ())?;
    let mut prefix = Vec::with_capacity(BINARY_SNIFF_BYTES);
    file.take(BINARY_SNIFF_BYTES as u64)
        .read_to_end(&mut prefix)
        .map_err(|_| ())?;
    Ok(prefix.contains(&0))
}

#[cfg(test)]
mod tests;
