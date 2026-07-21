use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use super::{
    TranscriptBudgetScope, TranscriptRetentionLimits, TranscriptRetentionState,
    TranscriptStoragePath,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptWriterConfig {
    pub storage_path: TranscriptStoragePath,
    pub retention_limits: TranscriptRetentionLimits,
}

impl TranscriptWriterConfig {
    pub fn new(
        storage_path: TranscriptStoragePath,
        retention_limits: TranscriptRetentionLimits,
    ) -> Self {
        Self {
            storage_path,
            retention_limits,
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

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(config.storage_path.transcript_file())
            .map_err(|_| {
                TranscriptWriteError::new(
                    TranscriptWriteErrorReason::OpenFileFailed,
                    config.storage_path.transcript_file(),
                    0,
                )
            })?;

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
