use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};

use crate::project::root::{FileAccessError, FileAccessTarget};

pub const DEFAULT_MAX_EDITABLE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextDocumentOpenPolicy {
    pub max_editable_bytes: u64,
}

impl TextDocumentOpenPolicy {
    pub fn linux_mvp() -> Self {
        Self {
            max_editable_bytes: DEFAULT_MAX_EDITABLE_BYTES,
        }
    }
}

impl Default for TextDocumentOpenPolicy {
    fn default() -> Self {
        Self::linux_mvp()
    }
}

pub(super) fn read_file_bounded(
    target: &FileAccessTarget,
    max_editable_bytes: u64,
) -> Result<Vec<u8>, TextDocumentOpenError> {
    let file =
        File::open(&target.canonical_path).map_err(|error| TextDocumentOpenError::ReadFailed {
            target: Box::new(target.clone()),
            kind: error.kind(),
        })?;
    let read_limit = max_editable_bytes
        .checked_add(1)
        .unwrap_or(max_editable_bytes);
    let mut bytes = Vec::new();

    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| TextDocumentOpenError::ReadFailed {
            target: Box::new(target.clone()),
            kind: error.kind(),
        })?;

    enforce_editable_size_cap(target, bytes.len() as u64, max_editable_bytes)?;

    Ok(bytes)
}

pub(super) fn enforce_editable_size_cap(
    target: &FileAccessTarget,
    len: u64,
    max: u64,
) -> Result<(), TextDocumentOpenError> {
    if len > max {
        return Err(TextDocumentOpenError::TooLarge {
            target: Box::new(target.clone()),
            len,
            max,
        });
    }

    Ok(())
}

pub(super) fn metadata_len(target: &FileAccessTarget) -> Result<u64, TextDocumentOpenError> {
    let metadata = fs::metadata(&target.canonical_path).map_err(|error| {
        TextDocumentOpenError::ReadFailed {
            target: Box::new(target.clone()),
            kind: error.kind(),
        }
    })?;
    Ok(metadata.len())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextDocumentOpenError {
    Access(FileAccessError),
    NotFile {
        target: Box<FileAccessTarget>,
    },
    TooLarge {
        target: Box<FileAccessTarget>,
        len: u64,
        max: u64,
    },
    InvalidUtf8 {
        target: Box<FileAccessTarget>,
    },
    ContainsNul {
        target: Box<FileAccessTarget>,
    },
    ReadFailed {
        target: Box<FileAccessTarget>,
        kind: io::ErrorKind,
    },
}

impl fmt::Display for TextDocumentOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => write!(formatter, "file access blocked: {error}"),
            Self::NotFile { target } => {
                write!(
                    formatter,
                    "not an editable file: {}",
                    target.selected_relative_path.display()
                )
            }
            Self::TooLarge { target, len, max } => write!(
                formatter,
                "file is too large to edit: {} is {len} bytes, limit is {max} bytes",
                target.selected_relative_path.display()
            ),
            Self::InvalidUtf8 { target } => {
                write!(
                    formatter,
                    "file is not valid UTF-8: {}",
                    target.selected_relative_path.display()
                )
            }
            Self::ContainsNul { target } => {
                write!(
                    formatter,
                    "file appears to be binary: {}",
                    target.selected_relative_path.display()
                )
            }
            Self::ReadFailed { target, kind } => {
                write!(
                    formatter,
                    "could not read {}: {kind:?}",
                    target.selected_relative_path.display()
                )
            }
        }
    }
}

impl std::error::Error for TextDocumentOpenError {}
