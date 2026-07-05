use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::project::root::FileAccessTarget;

use super::open::{TextDocumentOpenError, read_file_bounded};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSnapshot {
    pub canonical_path: PathBuf,
    pub modified_at: SystemTime,
    pub len: u64,
    /// In-process comparison aid only. This is not persisted, not
    /// cryptographic, and not a durable file identity.
    pub content_hash: Option<u64>,
}

pub(super) fn file_snapshot_from_opened_bytes(
    target: &FileAccessTarget,
    text: &str,
) -> Result<FileSnapshot, TextDocumentOpenError> {
    let metadata = fs::metadata(&target.canonical_path).map_err(|error| {
        TextDocumentOpenError::ReadFailed {
            target: Box::new(target.clone()),
            kind: error.kind(),
        }
    })?;
    let modified_at = metadata
        .modified()
        .map_err(|error| TextDocumentOpenError::ReadFailed {
            target: Box::new(target.clone()),
            kind: error.kind(),
        })?;

    Ok(FileSnapshot {
        canonical_path: target.canonical_path.clone(),
        modified_at,
        len: metadata.len(),
        content_hash: Some(hash_bytes(text.as_bytes())),
    })
}

pub(super) fn file_snapshot_for_current_disk(
    target: &FileAccessTarget,
    max_editable_bytes: u64,
) -> Result<FileSnapshot, TextDocumentSnapshotError> {
    let metadata =
        fs::metadata(&target.canonical_path).map_err(|error| TextDocumentSnapshotError {
            target: Box::new(target.clone()),
            kind: error.kind(),
        })?;
    let modified_at = metadata
        .modified()
        .map_err(|error| TextDocumentSnapshotError {
            target: Box::new(target.clone()),
            kind: error.kind(),
        })?;
    let content_hash = if metadata.len() <= max_editable_bytes {
        let bytes = read_file_bounded(target, max_editable_bytes).map_err(|error| match error {
            TextDocumentOpenError::ReadFailed { target, kind } => {
                TextDocumentSnapshotError { target, kind }
            }
            TextDocumentOpenError::TooLarge { target, .. } => TextDocumentSnapshotError {
                target,
                kind: io::ErrorKind::InvalidData,
            },
            _ => TextDocumentSnapshotError {
                target: Box::new(target.clone()),
                kind: io::ErrorKind::InvalidData,
            },
        })?;
        Some(hash_bytes(&bytes))
    } else {
        None
    };

    Ok(FileSnapshot {
        canonical_path: target.canonical_path.clone(),
        modified_at,
        len: metadata.len(),
        content_hash,
    })
}

pub(super) fn is_external_snapshot_shape_change(error: &TextDocumentSnapshotError) -> bool {
    matches!(
        error.kind,
        io::ErrorKind::NotFound | io::ErrorKind::InvalidData
    )
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDocumentSnapshotError {
    pub target: Box<FileAccessTarget>,
    pub kind: io::ErrorKind,
}

impl fmt::Display for TextDocumentSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not snapshot {}: {:?}",
            self.target.selected_relative_path.display(),
            self.kind
        )
    }
}

impl std::error::Error for TextDocumentSnapshotError {}
