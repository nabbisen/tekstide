//! RFC-011 Amendment 1: a bounded, read-only transcript reader.
//!
//! **D1 -- a bounded window over the tail, not a refusal.** RFC-024
//! chose refuse-never-truncate for diff content; this deliberately
//! chooses differently, because a transcript is an append-only log
//! consumed newest-first, and refusing a large one withholds the feature
//! exactly when a long-running agent makes it most valuable. See
//! [`DEFAULT_TRANSCRIPT_WINDOW_BYTES`] for the measured size.
//!
//! **D2 -- the window boundary is outside the property RFC-017's filter
//! was proven against.** `P4` (stream-position independence) covers
//! chunking where every byte arrives; a tail window drops the prefix,
//! which is a different operation P4 says nothing about. [`read_window`]
//! resynchronizes: it scans from the transcript's real start (never from
//! the raw requested offset), using [`next_token_len`] -- the exact
//! boundary logic `runtime::terminal::security::parser`'s own `parse`
//! loop already uses, reused rather than duplicated -- and reports the
//! delivered start offset, which may differ from the requested one.
//!
//! **D3 -- raw bytes out.** This module never calls
//! `text_safety::quote_untrusted`. Escaping is `crates/tekstide`'s job,
//! at the widget, per the window-boundary handoff.
//!
//! **D4 -- read-only.** This is the only module in `tekstide-core` that
//! opens a transcript file for reading; see
//! `only_this_module_opens_a_transcript_file_for_reading` in this
//! module's own tests for the enumeration proof. Nothing here deletes,
//! truncates, or updates retention metadata.
//!
//! **D5 -- complete vs. still-being-written, in the type.** [`TranscriptWindow`]
//! has two constructors, matching `DiffContent`'s own precedent
//! (RFC-024 PR-024-C) rather than a boolean field a caller could ignore.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use crate::runtime::terminal::next_token_len;

use super::TranscriptStoragePath;

/// RFC-011 Amendment 1, D1: measured against the real 32 MiB retention
/// ceiling (`DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES`), not estimated --
/// this project has twice shipped an estimated bound that was wrong once
/// measured (RFC-024's own diff bound, RFC-017's terminal I/O tick), and
/// a third time (this figure, first draft) measured the wrong quantity:
/// the original sweep varied only the window in isolation and never
/// allocated the mandatory `MAX_SCAN_BYTES` scan buffer `read_window`
/// always fills, so it understated real peak RSS by roughly an order of
/// magnitude. Corrected per response 198, Finding 2.
///
/// A real Rust harness (`rustc -O`, `/proc/self/status` `VmRSS` deltas)
/// now reproduces `read_window`'s actual allocation shape: a real
/// on-disk file at the writer's own 32 MiB retention ceiling, filled
/// with realistic PTY-output-shaped content -- repeated SGR-styled
/// lines, not a pathological all-zero buffer -- opened and
/// `read_to_end`'d into a `Vec::with_capacity(total_len)` scan buffer
/// exactly as `read_window` does, plus the window's own `content` copy,
/// plus a simulated escaped copy alongside it (the widget's own
/// transient state). Swept across candidate window sizes:
///
/// ```text
/// mib=1 scan_len=33554432 content_len=1048576 escaped_len=1081344 rss_delta_kb=34988
/// mib=2 scan_len=33554432 content_len=2097152 escaped_len=2162688 rss_delta_kb=38860
/// mib=4 scan_len=33554432 content_len=4194304 escaped_len=4325376 rss_delta_kb=43020
/// mib=8 scan_len=33554432 content_len=8388608 escaped_len=8650752 rss_delta_kb=51692
/// ```
///
/// **Real peak for a full-size transcript is ~33-50 MiB, dominated by
/// the fixed 32 MiB scan buffer** -- every call pays that cost
/// regardless of the requested window, because `MAX_SCAN_BYTES` bounds
/// what `read_window` reads, not what it returns. The window's own
/// marginal cost (content copy + escaped copy) is a few MiB at most.
///
/// **1 MiB remains the right choice**, though not for the "trivial
/// memory cost" reason the original (wrong) figure gave: since the scan
/// buffer's fixed 32 MiB dominates every candidate size, the window
/// choice cannot meaningfully change *peak* memory -- it changes only
/// the smaller marginal cost on top, where 1 MiB is still the cheapest
/// of the sizes measured. The window is still chosen for what it always
/// was chosen for: 1/32nd of the retention ceiling is meaningfully a
/// *window*, not "basically the whole transcript," and at ordinary PTY
/// text density is on the order of tens of thousands of lines, far more
/// than a report view could usefully show on one screen. Unlike
/// RFC-024's bound, this is not reused from an existing reviewed
/// standard: a transcript tail is not shaped like a whole edited file
/// (RFC-019's editable bound) or a single paste (RFC-018's bound), so
/// this is a fresh number, measured rather than borrowed by analogy.
pub const DEFAULT_TRANSCRIPT_WINDOW_BYTES: u64 = 1024 * 1024;

/// The reader's own hard ceiling on how much of the file it will ever
/// read into memory in one call, independent of the requested window.
/// Equal to the writer's own per-transcript retention limit
/// (`DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES`) -- the writer already
/// guarantees no transcript grows past this, so this is defense in
/// depth against a file that grew past what the writer would have
/// produced, not a second, independently-chosen number.
const MAX_SCAN_BYTES: u64 = super::DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptReadPolicy {
    pub window_bytes: u64,
}

impl TranscriptReadPolicy {
    pub fn linux_mvp() -> Self {
        Self {
            window_bytes: DEFAULT_TRANSCRIPT_WINDOW_BYTES,
        }
    }
}

impl Default for TranscriptReadPolicy {
    fn default() -> Self {
        Self::linux_mvp()
    }
}

/// RFC-011 Amendment 1, D5. `content` is always raw (D3); `total_len` is
/// the transcript's real, current size, so a caller can tell "this is a
/// tail window" from "this is the whole transcript" without a second
/// call. `requested_start`/`delivered_start` are D2's own required
/// report -- equal for a window that needed no resynchronization,
/// different whenever the raw requested offset landed inside a token
/// the resynchronization skipped past.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranscriptWindow {
    /// The transcript is not being written to by a live process --
    /// whatever this window shows will not change underneath the reader
    /// that asked for it.
    Complete {
        content: Vec<u8>,
        requested_start: u64,
        delivered_start: u64,
        total_len: u64,
    },
    /// An AgentRun this transcript belongs to is still active. The
    /// trailing bytes may be a partial write in progress -- this must
    /// not be presented as a finished record.
    StillBeingWritten {
        content: Vec<u8>,
        requested_start: u64,
        delivered_start: u64,
        total_len: u64,
    },
}

impl TranscriptWindow {
    pub fn content(&self) -> &[u8] {
        match self {
            Self::Complete { content, .. } | Self::StillBeingWritten { content, .. } => content,
        }
    }

    pub fn requested_start(&self) -> u64 {
        match self {
            Self::Complete {
                requested_start, ..
            }
            | Self::StillBeingWritten {
                requested_start, ..
            } => *requested_start,
        }
    }

    pub fn delivered_start(&self) -> u64 {
        match self {
            Self::Complete {
                delivered_start, ..
            }
            | Self::StillBeingWritten {
                delivered_start, ..
            } => *delivered_start,
        }
    }

    pub fn total_len(&self) -> u64 {
        match self {
            Self::Complete { total_len, .. } | Self::StillBeingWritten { total_len, .. } => {
                *total_len
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptReadErrorReason {
    InvalidStoragePath,
    OpenFileFailed,
    MetadataFailed,
    ReadFailed,
    /// Response 198, Finding 1: a transcript larger than `MAX_SCAN_BYTES`
    /// must refuse, not silently read the first `MAX_SCAN_BYTES` and
    /// return a window near the end of *that prefix* -- the middle of
    /// the real file -- mislabelled as the tail. The writer's own
    /// per-transcript limit means this should never happen; a file this
    /// large is exactly the anomalous case `MAX_SCAN_BYTES` exists to
    /// catch, and catching it silently with a wrong answer is worse than
    /// refusing loudly, because nobody would be looking for the wrong
    /// answer.
    TranscriptExceedsScanLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptReadError {
    pub reason: TranscriptReadErrorReason,
    pub path: PathBuf,
}

impl TranscriptReadError {
    fn new(reason: TranscriptReadErrorReason, path: impl Into<PathBuf>) -> Self {
        Self {
            reason,
            path: path.into(),
        }
    }
}

impl std::fmt::Display for TranscriptReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "transcript read failed at {}: {:?}",
            self.path.display(),
            self.reason
        )
    }
}

impl std::error::Error for TranscriptReadError {}

/// The reader. `still_being_written` is caller-supplied rather than
/// inferred from the file: nothing on disk distinguishes "a live
/// process paused between writes" from "a finished transcript" -- only
/// the caller (who tracks the owning `AgentRun`'s own status) can
/// answer that, so D5's distinction is threaded through rather than
/// guessed at here.
///
/// **Always scans from byte 0, never seeks to the requested tail --
/// this is load-bearing, not an unoptimized read.** Resynchronization
/// (see [`resynchronize`]) walks tokens forward from a position
/// *guaranteed* to be a sound parse origin; the file's true start is the
/// only position with that guarantee. Seeking to somewhere near the
/// requested tail before scanning would put the scan's own starting
/// point at an arbitrary, possibly mid-sequence offset -- exactly the
/// defect D2 exists to prevent, reintroduced one level down by the
/// "optimization" meant to avoid it. The cost this buys is a bounded
/// (`<= MAX_SCAN_BYTES`) linear scan on every read, not an unbounded
/// one -- see `MAX_SCAN_BYTES` and the refusal below for what bounds it.
pub fn read_window(
    storage_path: &TranscriptStoragePath,
    policy: TranscriptReadPolicy,
    still_being_written: bool,
) -> Result<TranscriptWindow, TranscriptReadError> {
    if !storage_path.is_safe_for_read() {
        return Err(TranscriptReadError::new(
            TranscriptReadErrorReason::InvalidStoragePath,
            storage_path.transcript_file(),
        ));
    }

    let file = File::open(storage_path.transcript_file()).map_err(|_| {
        TranscriptReadError::new(
            TranscriptReadErrorReason::OpenFileFailed,
            storage_path.transcript_file(),
        )
    })?;

    let total_len = file
        .metadata()
        .map_err(|_| {
            TranscriptReadError::new(
                TranscriptReadErrorReason::MetadataFailed,
                storage_path.transcript_file(),
            )
        })?
        .len();

    // Response 198, Finding 1: refuse rather than silently reading only
    // the first MAX_SCAN_BYTES and returning a window near the end of
    // that prefix -- the middle of the real file, mislabelled as the
    // tail. The writer's own limit means total_len should never exceed
    // this; if it does, that is exactly the anomaly this check exists to
    // catch, not a case to paper over with a wrong-but-plausible answer.
    if total_len > MAX_SCAN_BYTES {
        return Err(TranscriptReadError::new(
            TranscriptReadErrorReason::TranscriptExceedsScanLimit,
            storage_path.transcript_file(),
        ));
    }

    let mut buffer = Vec::with_capacity(total_len as usize);
    file.take(total_len).read_to_end(&mut buffer).map_err(|_| {
        TranscriptReadError::new(
            TranscriptReadErrorReason::ReadFailed,
            storage_path.transcript_file(),
        )
    })?;

    let requested_start = (buffer.len() as u64).saturating_sub(policy.window_bytes);
    let delivered_start = resynchronize(&buffer, requested_start as usize) as u64;
    let content = buffer[delivered_start as usize..].to_vec();

    Ok(if still_being_written {
        TranscriptWindow::StillBeingWritten {
            content,
            requested_start,
            delivered_start,
            total_len,
        }
    } else {
        TranscriptWindow::Complete {
            content,
            requested_start,
            delivered_start,
            total_len,
        }
    })
}

/// RFC-011 Amendment 1, D2's own required operation: advance from
/// `target_start` to the first position where a fresh parse is sound.
///
/// Walks token boundaries from **the buffer's own real start** --
/// `buffer` is always read from byte 0 of the file, never from a raw
/// offset into the middle of it, so there is no discarded prefix to be
/// ambiguous about. Each call to [`next_token_len`] consumes exactly one
/// complete token (a control sequence, a UTF-8 scalar, or a run of
/// plain ASCII), the same unit `runtime::terminal::security::parser`'s
/// own `parse` loop advances by -- so every offset this loop visits is a
/// position where no sequence is open and no UTF-8 scalar is split,
/// genuinely, not by heuristic. The first such offset at or past
/// `target_start` is returned.
///
/// This is the "honest fallback" the window-boundary handoff names --
/// scanning from the real start rather than guessing at a partial
/// resynchronization heuristic for a raw offset -- chosen deliberately:
/// `TerminalSecurityParser::parse` exposes no incremental/streaming
/// state a partial heuristic could reuse, and inventing one for a
/// security-critical property this codebase cannot yet prove correct
/// for OSC/DCS string-control edge cases is a worse trade than a bounded
/// (`&lt;= MAX_SCAN_BYTES`) linear scan.
fn resynchronize(buffer: &[u8], target_start: usize) -> usize {
    let mut offset = 0;
    while offset < target_start && offset < buffer.len() {
        offset += next_token_len(&buffer[offset..]);
    }
    offset.min(buffer.len())
}

#[cfg(test)]
mod tests;
