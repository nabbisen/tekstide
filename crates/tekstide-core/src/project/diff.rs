//! RFC-024 PR-024-B/PR-024-C/PR-024-D: gating, bounds, bounded content
//! access, and baseline staleness.
//!
//! **This module does not compute a diff.** `gate_diff_content_read`
//! decides whether reading a path's content for diff preview is allowed
//! at all, and whether that content may be attempted as text; `read_diff_content`
//! performs the bounded read the gate approved; `diff_content_is_stale`
//! answers whether the disk state a `DiffContent` was read from still
//! matches disk now. None of the three compute a two-sided comparison --
//! the diff algorithm itself is out of this RFC's scope (Decision 6).
//!
//! **Nothing here retains anything.** All three public functions borrow,
//! read a bounded amount, and return by value -- there is no field
//! anywhere in this module a caller could accidentally hold past the
//! call, and `DiffContent` (PR-024-C) derives neither `Clone` nor
//! `Serialize`: see `read_diff_content`'s own doc comment for why that
//! specific omission is load-bearing, not incidental.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::change_detection::{ChangeLifecycle, ChangePathKind, DetectedChanges};
use super::root::{
    FileAccessBlockedReason, FileAccessError, FileAccessTarget, ProjectFileAccessPolicy,
    ProjectRootHandle,
};
use crate::content::FileSnapshot;

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

/// The two lifecycles a path can actually have once `gate_diff_content_read`
/// reaches its size/binary checks at all -- deliberately narrower than
/// `ChangeLifecycle`. A `Deleted` lifecycle is already returned as
/// `DiffGateDecision::Deleted` before either check runs (see the function
/// below), so `Readable`/`NonTextContent` carrying the full three-variant
/// `ChangeLifecycle` would let `Readable { lifecycle: Deleted }` exist as a
/// representable-but-meaningless value -- the same class of bug RFC-012
/// Amendment 1 fixed for `ChangePathKind` carrying its own `Deleted`,
/// reproduced one level up if not narrowed here too. The conversion from
/// `ChangeLifecycle` happens once, in an exhaustive match with the
/// `Deleted` arm returning early -- not a runtime assumption checked by an
/// `unreachable!()` later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentLifecycle {
    Added,
    Modified,
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
    Readable { lifecycle: ContentLifecycle },
    /// A `File`, not deleted, within the bound, but the first
    /// `BINARY_SNIFF_BYTES` contain a NUL byte. No diff is attempted; the
    /// caller reports a non-text change with this length.
    NonTextContent {
        len: u64,
        lifecycle: ContentLifecycle,
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

/// Private mirror of `DiffGateDecision`, carrying the already-resolved
/// `FileAccessTarget` alongside `Readable` so `read_diff_content` (below)
/// can perform its bounded read against the exact path the gate already
/// checked, without a second, independent `resolve_existing` call. Not
/// public: `DiffGateDecision` stays exactly the compact, already-reviewed
/// shape PR-024-B's own review accepted -- a resolved filesystem target is
/// an implementation detail this function needs internally, not something
/// `gate_diff_content_read`'s own callers should see or could hold onto.
enum GateEvaluation {
    Readable {
        lifecycle: ContentLifecycle,
        target: FileAccessTarget,
    },
    NonTextContent {
        len: u64,
        lifecycle: ContentLifecycle,
        target: FileAccessTarget,
    },
    Deleted {
        kind: ChangePathKind,
    },
    NonFile {
        kind: ChangePathKind,
    },
}

impl GateEvaluation {
    fn into_decision(self) -> DiffGateDecision {
        match self {
            Self::Readable { lifecycle, .. } => DiffGateDecision::Readable { lifecycle },
            Self::NonTextContent { len, lifecycle, .. } => {
                DiffGateDecision::NonTextContent { len, lifecycle }
            }
            Self::Deleted { kind } => DiffGateDecision::Deleted { kind },
            Self::NonFile { kind } => DiffGateDecision::NonFile { kind },
        }
    }
}

/// The gate's real body. Order, matching Decision 2 → Decision 4 exactly,
/// with RFC-012 Amendment 1's lifecycle check ahead of both: confirm the
/// path was already detected → if its lifecycle is `Deleted`, stop --
/// nothing on disk to resolve → resolve it safely (root/symlink) → check
/// its size against metadata alone → sniff a bounded prefix for binary
/// content. A full read is never attempted by this function; `Readable`
/// is the furthest it goes -- `read_diff_content` is where the resolved
/// target this function already produced gets used for that read.
fn evaluate_gate(
    detected: &DetectedChanges,
    root: &ProjectRootHandle,
    selected_relative_path: impl AsRef<Path>,
    policy: DiffPreviewPolicy,
) -> Result<GateEvaluation, DiffGateRefusal> {
    let selected_relative_path = selected_relative_path.as_ref();

    let Some(changed) = detected
        .changed_paths
        .iter()
        .find(|changed| changed.relative_path == selected_relative_path)
    else {
        return Err(DiffGateRefusal::PathNotDetected);
    };

    let lifecycle = match changed.lifecycle {
        ChangeLifecycle::Deleted => {
            return Ok(GateEvaluation::Deleted { kind: changed.kind });
        }
        ChangeLifecycle::Added => ContentLifecycle::Added,
        ChangeLifecycle::Modified => ContentLifecycle::Modified,
    };

    if changed.kind != ChangePathKind::File {
        return Ok(GateEvaluation::NonFile { kind: changed.kind });
    }

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
        return Ok(GateEvaluation::NonTextContent {
            len,
            lifecycle,
            target,
        });
    }

    Ok(GateEvaluation::Readable { lifecycle, target })
}

/// The gate. See `evaluate_gate` for the real body -- this is the public,
/// already-reviewed (PR-024-B) surface over it.
pub fn gate_diff_content_read(
    detected: &DetectedChanges,
    root: &ProjectRootHandle,
    selected_relative_path: impl AsRef<Path>,
    policy: DiffPreviewPolicy,
) -> Result<DiffGateDecision, DiffGateRefusal> {
    evaluate_gate(detected, root, selected_relative_path, policy).map(GateEvaluation::into_decision)
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

/// RFC-024 PR-024-C: content, for every outcome `gate_diff_content_read`
/// can produce -- `Added`/`Modified` carry the bytes `Readable` approved;
/// `Deleted`/`NonTextContent`/`NonFile` carry exactly what
/// `DiffGateDecision`'s matching variant already did, since none of those
/// three ever had bytes to read in the first place.
///
/// **The Added/Modified distinction is the constructor, not a field.**
/// Both carry the same `Vec<u8>` shape, but a caller pattern-matches which
/// one it received -- there is no shared `Readable { bytes, lifecycle }`
/// shape a caller could destructure once and then forget which arm its
/// `lifecycle` value came from. RFC-024 §Correction's own requirement --
/// modified content must reach the surface "explicitly not a diff" -- is
/// carried by the variant name itself, not a runtime flag a renderer could
/// fail to check.
///
/// `baseline` (PR-024-D) appears on every variant that resolved a real
/// file (`Added`, `Modified`, `NonTextContent`) -- the disk state this
/// content was actually read against, for `diff_content_is_stale` to
/// later compare against disk again. `Deleted` and `NonFile` carry none:
/// neither ever resolved a file to snapshot.
///
/// Deliberately derives neither `Clone` nor `Serialize`. See
/// `read_diff_content`'s doc comment for why.
///
/// **Deliberately does not derive `Debug` either (RFC-041 D3).** An
/// unredacted `Debug` on a type holding real file content is one
/// `dbg!`, one panic message, or one `tracing` field away from putting
/// a user's source into a log -- see the hand-written `impl` below,
/// which prints kind and length only, exactly as `BoundedRuntimeSummary`
/// and `DisplayText` already do for their own untrusted payloads, and
/// never the bytes themselves.
///
/// **The move-out gap stays open, and is documented here rather than
/// closed (RFC-041 D3):** non-retention protects this wrapper -- it
/// cannot be stored, cloned, or serialized -- but not bytes a consumer
/// destructures out of `bytes: Vec<u8>` after a pattern match. Closing
/// that means a lifetime-bound `DiffContent` tied to the read it came
/// from, which is a larger change than RFC-041 takes on. Any future
/// consumer of `Added`/`Modified`/`NonTextContent` that moves `bytes`
/// out of this type inherits Decision 1's non-retention obligation by
/// convention from that point forward, not by anything the compiler
/// enforces past this boundary.
#[derive(Eq, PartialEq)]
pub enum DiffContent {
    /// Whole-file content for a newly added path. Not a diff -- the whole
    /// change, by definition (RFC-024 §Correction).
    Added {
        bytes: Vec<u8>,
        baseline: FileSnapshot,
    },
    /// Current content for a modified path. Explicitly not a diff: this
    /// RFC cannot produce a "before" side for `FilesystemSnapshot`
    /// detection (response 187) -- the before-bytes were never captured
    /// and are gone by request time, not merely unretained.
    Modified {
        bytes: Vec<u8>,
        baseline: FileSnapshot,
    },
    /// The fact of deletion, from metadata alone -- no bytes exist to
    /// read. `kind` reports what the path *was* (RFC-012 Amendment 1).
    Deleted { kind: ChangePathKind },
    /// Sniffed as binary; reported with its real length, no content read
    /// attempted.
    NonTextContent {
        len: u64,
        lifecycle: ContentLifecycle,
        baseline: FileSnapshot,
    },
    /// Present but not a `File` -- `Directory`, `Symlink`, or `Other`.
    NonFile { kind: ChangePathKind },
}

/// RFC-041 D3: kind and length only, matching every other `Debug` this
/// project hand-writes for a type that might hold untrusted or
/// sensitive payload (`BoundedRuntimeSummary`, `DisplayText`) --
/// `bytes` is never printed, for `Added`, `Modified`, or any variant
/// added here in the future. `baseline` (a `FileSnapshot`, metadata
/// only -- no content) is printed via its own derived `Debug`, which is
/// safe by construction.
impl std::fmt::Debug for DiffContent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added { bytes, baseline } => formatter
                .debug_struct("Added")
                .field("len", &bytes.len())
                .field("baseline", baseline)
                .finish(),
            Self::Modified { bytes, baseline } => formatter
                .debug_struct("Modified")
                .field("len", &bytes.len())
                .field("baseline", baseline)
                .finish(),
            Self::Deleted { kind } => formatter
                .debug_struct("Deleted")
                .field("kind", kind)
                .finish(),
            Self::NonTextContent {
                len,
                lifecycle,
                baseline,
            } => formatter
                .debug_struct("NonTextContent")
                .field("len", len)
                .field("lifecycle", lifecycle)
                .field("baseline", baseline)
                .finish(),
            Self::NonFile { kind } => formatter
                .debug_struct("NonFile")
                .field("kind", kind)
                .finish(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiffContentError {
    /// The gate itself refused -- see `DiffGateRefusal`. This is the only
    /// refusal `read_diff_content` produces for a `Readable` decision's
    /// own resolution, since it reuses the gate's already-resolved target
    /// rather than resolving the path a second time (see `evaluate_gate`):
    /// there is no independent second resolution step left to fail on its
    /// own, so there is no separate `Access` case here for one.
    Gate(DiffGateRefusal),
    /// The bounded read failed outright, or observed more than
    /// `policy.max_input_bytes` after the gate already approved the size
    /// from metadata alone moments earlier -- a race between that
    /// metadata check and this read. Refused rather than silently
    /// returning a truncated prefix, matching Decision 2's own "refuse
    /// whole, never truncate" for every other size check in this module.
    ReadFailed { relative_path: PathBuf },
}

/// RFC-024 PR-024-C: the only place in `tekstide-core` that reads a
/// generated change's full content. Reuses `evaluate_gate` (the same
/// checks `gate_diff_content_read` itself calls) rather than
/// reimplementing them (PR-024-C's own review gate: "the
/// Added/Modified/Deleted distinction is read from `ChangeLifecycle`,
/// never inferred from `ChangePathKind`" -- calling the shared gate
/// evaluation is how this function inherits that property instead of
/// re-deriving it) -- and reuses the gate's own resolved `FileAccessTarget`
/// directly for the read, rather than resolving the path a second,
/// independent time.
///
/// **Decision 1's third clause -- content "never retained beyond the
/// request" -- made structural here, not conventional.** `DiffContent`
/// derives neither `Clone` nor `Serialize`. `ProjectSession` derives
/// `Clone` across all of its fields uniformly (`project/session.rs`), so
/// a `DiffContent` field there would not compile; every
/// `AuditCoordinator::record_*` call requires a `Serialize` event
/// (`audit/recovery.rs`'s own persisted-event shapes), so passing this
/// type to one would not compile either. Both are compile errors, not
/// promises kept by not calling an API that remains callable. This
/// function also returns by value with no cache -- nothing in this module
/// holds a previous result across calls.
///
/// Bytes are returned raw (`Vec<u8>`), not decoded to `String`: RFC-024
/// Decision 4 deliberately chose a NUL-byte sniff over "a UTF-8-decode-and-
/// handle-failure", so this function does not perform the stricter check
/// the sniff was chosen to avoid. Also not pre-escaped -- `text_safety`'s
/// bidi/format-character escaping is RFC-020's job at render time, not
/// this function's; escaping here would hide real file content from any
/// consumer that is not a renderer.
pub fn read_diff_content(
    detected: &DetectedChanges,
    root: &ProjectRootHandle,
    selected_relative_path: impl AsRef<Path>,
    policy: DiffPreviewPolicy,
) -> Result<DiffContent, DiffContentError> {
    let evaluation = evaluate_gate(detected, root, selected_relative_path, policy)
        .map_err(DiffContentError::Gate)?;

    match evaluation {
        GateEvaluation::Deleted { kind } => Ok(DiffContent::Deleted { kind }),
        GateEvaluation::NonFile { kind } => Ok(DiffContent::NonFile { kind }),
        GateEvaluation::NonTextContent {
            len,
            lifecycle,
            target,
        } => {
            let baseline =
                capture_baseline_snapshot(&target).map_err(|()| DiffContentError::ReadFailed {
                    relative_path: target.selected_relative_path.clone(),
                })?;
            Ok(DiffContent::NonTextContent {
                len,
                lifecycle,
                baseline,
            })
        }
        GateEvaluation::Readable { lifecycle, target } => {
            let bytes =
                read_bounded(&target.canonical_path, policy.max_input_bytes).map_err(|()| {
                    DiffContentError::ReadFailed {
                        relative_path: target.selected_relative_path.clone(),
                    }
                })?;
            let baseline =
                capture_baseline_snapshot(&target).map_err(|()| DiffContentError::ReadFailed {
                    relative_path: target.selected_relative_path.clone(),
                })?;

            match lifecycle {
                ContentLifecycle::Added => Ok(DiffContent::Added { bytes, baseline }),
                ContentLifecycle::Modified => Ok(DiffContent::Modified { bytes, baseline }),
            }
        }
    }
}

/// RFC-024 Decision 3, PR-024-D: **reuses `content::FileSnapshot`, the
/// exact type `TextDocument::refresh_external_state` already compares to
/// answer "has this changed underneath what I last saw" -- not a second
/// staleness mechanism.** `content_hash` is deliberately left `None` here
/// rather than computed: hashing would mean a second full read of every
/// file this module already read once (`Added`/`Modified`) or, worse, a
/// read of binary content this module's own `NonTextContent` outcome
/// exists specifically to avoid (Decision 4: "no diff is attempted" for a
/// non-text change -- a full read purely to hash it would be exactly
/// that, under a different name). `len` and `modified_at` alone are the
/// same granularity RFC-012's own `changed_paths_between` already uses to
/// decide whether a path changed (`ReviewBaselineEntry`'s `len`/
/// `modified_unix_nanos`, compared by equality) -- reusing that
/// established precedent's precision rather than inventing a stricter one
/// inconsistently only for some outcomes.
fn capture_baseline_snapshot(target: &FileAccessTarget) -> Result<FileSnapshot, ()> {
    let metadata = fs::metadata(&target.canonical_path).map_err(|_| ())?;
    let modified_at = metadata.modified().map_err(|_| ())?;

    Ok(FileSnapshot {
        canonical_path: target.canonical_path.clone(),
        modified_at,
        len: metadata.len(),
        content_hash: None,
    })
}

/// RFC-024 Decision 3: whether the disk state a `DiffContent` was read
/// against (its `baseline`) still matches disk now. **Only two states are
/// meaningful here, not `content::ExternalChangeDecision`'s three.** Diff
/// preview is read-only (RFC-024 §Scope: "Out: any *action* on a change"),
/// so there is no local-edit state a live disk change could *conflict*
/// with the way an open, dirty `TextDocument` can -- reusing the
/// 3-variant type wholesale would let a `Conflict` outcome exist that
/// this read-only flow can never actually produce, the same
/// representable-but-meaningless-state class PR-024-C already caught and
/// fixed once for `ContentLifecycle`. A `bool` is the correct, minimal
/// type for a genuinely binary question here, not an enum with a dead
/// arm.
///
/// Missing/inaccessible-since-capture folds into `Ok(true)` (stale) for
/// the same reason `TextDocument::refresh_external_state` treats a
/// missing current target as `ExternalChanged` rather than an error --
/// the file being gone *is* the change. Mirrors that function's own
/// narrower distinction too: only `FileAccessBlockedReason::MissingPath`
/// folds into "stale"; any other access refusal (root/symlink-escape) is
/// a real policy violation, surfaced as `Err`, not silently swallowed as
/// staleness.
pub fn diff_content_is_stale(
    baseline: &FileSnapshot,
    root: &ProjectRootHandle,
    selected_relative_path: impl AsRef<Path>,
) -> Result<bool, DiffContentError> {
    let selected_relative_path = selected_relative_path.as_ref();

    let target = match ProjectFileAccessPolicy.resolve_existing(root, selected_relative_path) {
        Ok(target) => target,
        Err(error) if error.reason == FileAccessBlockedReason::MissingPath => return Ok(true),
        Err(error) => return Err(DiffContentError::Gate(DiffGateRefusal::Access(error))),
    };

    let Ok(current) = capture_baseline_snapshot(&target) else {
        return Ok(true);
    };

    Ok(current != *baseline)
}

/// Refuses, never truncates -- the same `.take(max + 1)`-then-check shape
/// `content::open`'s own `read_file_bounded` uses (a different module, not
/// literally shared code, but the same idiom): reading one byte past the
/// bound is enough to detect an oversized file without reading the whole
/// thing, and without that extra byte an oversized result would be
/// indistinguishable from a coincidentally-truncated one.
fn read_bounded(canonical_path: &Path, max_bytes: u64) -> Result<Vec<u8>, ()> {
    let file = File::open(canonical_path).map_err(|_| ())?;
    let read_limit = max_bytes.checked_add(1).unwrap_or(max_bytes);
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;

    if bytes.len() as u64 > max_bytes {
        return Err(());
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests;
