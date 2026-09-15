use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use super::{
    TranscriptBudgetScope, TranscriptCaptureMode, TranscriptRetentionLimits,
    TranscriptRetentionState, TranscriptStoragePath,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptWriterConfig {
    pub storage_path: TranscriptStoragePath,
    pub retention_limits: TranscriptRetentionLimits,
    /// RFC-011 Amendment 2: carried alongside the writer's own config so
    /// whoever moves the writer into the reader thread
    /// (`LinuxTerminalRuntime::spawn_output_reader`) has the capture
    /// mode available at the point it constructs the reader's
    /// `TranscriptCapture` -- the mode decides D3's mid-stream failure
    /// policy, which is the reader thread's decision, not the writer's
    /// own. `BoundedTranscriptWriter` itself never reads this field.
    pub mode: TranscriptCaptureMode,
}

impl TranscriptWriterConfig {
    pub fn new(
        storage_path: TranscriptStoragePath,
        retention_limits: TranscriptRetentionLimits,
        mode: TranscriptCaptureMode,
    ) -> Self {
        Self {
            storage_path,
            retention_limits,
            mode,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptWriteSummary {
    pub byte_count: u64,
    pub retention_state: TranscriptRetentionState,
}

impl TranscriptWriteSummary {
    fn active(byte_count: u64) -> Self {
        Self {
            byte_count,
            retention_state: TranscriptRetentionState::Active,
        }
    }

    fn truncated(byte_count: u64) -> Self {
        Self {
            byte_count,
            retention_state: TranscriptRetentionState::Truncated {
                scope: TranscriptBudgetScope::Transcript,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptWriteErrorReason {
    UnboundedRetention,
    InvalidStoragePath,
    CreateDirectoryFailed,
    OpenFileFailed,
    /// RFC-050 D3: another handle holds the file's exclusive lock, so a
    /// writer may still be using it. The writer is not created, and the
    /// file is left exactly as it was.
    LockUnavailable,
    WriteFailed,
    FlushFailed,
}

#[derive(Debug)]
pub struct TranscriptWriteError {
    pub reason: TranscriptWriteErrorReason,
    pub path: PathBuf,
    pub byte_count: u64,
}

impl TranscriptWriteError {
    fn new(reason: TranscriptWriteErrorReason, path: impl Into<PathBuf>, byte_count: u64) -> Self {
        Self {
            reason,
            path: path.into(),
            byte_count,
        }
    }
}

impl Clone for TranscriptWriteError {
    fn clone(&self) -> Self {
        Self {
            reason: self.reason,
            path: self.path.clone(),
            byte_count: self.byte_count,
        }
    }
}

impl PartialEq for TranscriptWriteError {
    fn eq(&self, other: &Self) -> bool {
        self.reason == other.reason
            && self.path == other.path
            && self.byte_count == other.byte_count
    }
}

impl Eq for TranscriptWriteError {}

impl fmt::Display for TranscriptWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "transcript write failed at {} after {} bytes: {:?}",
            self.path.display(),
            self.byte_count,
            self.reason
        )
    }
}

impl std::error::Error for TranscriptWriteError {}

#[derive(Debug)]
pub struct BoundedTranscriptWriter {
    /// Holds the file's exclusive lock (RFC-050 D3) for the writer's whole
    /// lifetime. Dropping the writer closes the handle and releases it, so
    /// a file with no writer reads as free to every other process.
    file: File,
    transcript_file: PathBuf,
    max_bytes: u64,
    byte_count: u64,
    retention_state: TranscriptRetentionState,
}

impl BoundedTranscriptWriter {
    pub fn create(config: TranscriptWriterConfig) -> Result<Self, TranscriptWriteError> {
        if !config.retention_limits.is_bounded() {
            return Err(TranscriptWriteError::new(
                TranscriptWriteErrorReason::UnboundedRetention,
                config.storage_path.transcript_file(),
                0,
            ));
        }
        if !config.storage_path.is_safe_for_write() {
            return Err(TranscriptWriteError::new(
                TranscriptWriteErrorReason::InvalidStoragePath,
                config.storage_path.transcript_file(),
                0,
            ));
        }

        fs::create_dir_all(config.storage_path.transcript_dir()).map_err(|_| {
            TranscriptWriteError::new(
                TranscriptWriteErrorReason::CreateDirectoryFailed,
                config.storage_path.transcript_dir(),
                0,
            )
        })?;

        // RFC-050 D3, narrowed at response 388: open **without** truncating,
        // then decide both the lock and the truncate from **one `fstat` on
        // the opened handle**.
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(config.storage_path.transcript_file())
            .map_err(|_| {
                TranscriptWriteError::new(
                    TranscriptWriteErrorReason::OpenFileFailed,
                    config.storage_path.transcript_file(),
                    0,
                )
            })?;

        // Only a regular file is ever loaded as a transcript (D7), so only a
        // regular file needs a lock to tell live from leftover. A FIFO or a
        // device is neither locked nor truncated: a lock there protects
        // nothing, and on a shared device — every test capturing into
        // `/dev/full` — it serialises unrelated writers. `O_TRUNC` never
        // affected those either. A handle whose type cannot be read is
        // refused rather than guessed at, so a regular file is never written
        // unlocked.
        let is_regular_file = file
            .metadata()
            .map_err(|_| {
                TranscriptWriteError::new(
                    TranscriptWriteErrorReason::OpenFileFailed,
                    config.storage_path.transcript_file(),
                    0,
                )
            })?
            .is_file();
        if is_regular_file {
            // **A writer that cannot lock does not write.** Any failure, not
            // only `WouldBlock`: a filesystem that cannot lock would otherwise
            // produce writers every other process reads as dead.
            if file.try_lock().is_err() {
                return Err(TranscriptWriteError::new(
                    TranscriptWriteErrorReason::LockUnavailable,
                    config.storage_path.transcript_file(),
                    0,
                ));
            }
            // Truncate only once the lock is held. Truncating first would wipe
            // a file another writer still holds — RFC-049 §2's deletion under a
            // live writer, by a different route.
            file.set_len(0).map_err(|_| {
                TranscriptWriteError::new(
                    TranscriptWriteErrorReason::OpenFileFailed,
                    config.storage_path.transcript_file(),
                    0,
                )
            })?;
        }

        Ok(Self {
            file,
            transcript_file: config.storage_path.transcript_file().to_path_buf(),
            max_bytes: config.retention_limits.max_bytes_per_transcript,
            byte_count: 0,
            retention_state: TranscriptRetentionState::Active,
        })
    }

    pub fn append(&mut self, bytes: &[u8]) -> Result<TranscriptWriteSummary, TranscriptWriteError> {
        if bytes.is_empty() {
            return Ok(self.summary());
        }

        let remaining = self.max_bytes.saturating_sub(self.byte_count);
        let write_len = remaining.min(bytes.len() as u64) as usize;

        if write_len > 0 {
            self.file.write_all(&bytes[..write_len]).map_err(|error| {
                self.write_error(error, TranscriptWriteErrorReason::WriteFailed)
            })?;
            self.byte_count += write_len as u64;
        }

        if write_len < bytes.len() {
            self.retention_state = TranscriptRetentionState::Truncated {
                scope: TranscriptBudgetScope::Transcript,
            };
        }

        Ok(self.summary())
    }

    pub fn flush(&mut self) -> Result<TranscriptWriteSummary, TranscriptWriteError> {
        self.file
            .flush()
            .map_err(|error| self.write_error(error, TranscriptWriteErrorReason::FlushFailed))?;
        Ok(self.summary())
    }

    pub fn summary(&self) -> TranscriptWriteSummary {
        match self.retention_state {
            TranscriptRetentionState::Truncated {
                scope: TranscriptBudgetScope::Transcript,
            } => TranscriptWriteSummary::truncated(self.byte_count),
            _ => TranscriptWriteSummary::active(self.byte_count),
        }
    }

    fn write_error(
        &self,
        _error: io::Error,
        reason: TranscriptWriteErrorReason,
    ) -> TranscriptWriteError {
        TranscriptWriteError::new(reason, &self.transcript_file, self.byte_count)
    }
}
