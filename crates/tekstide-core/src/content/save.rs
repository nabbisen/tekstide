use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::project::root::{FileAccessError, FileAccessTarget};

use super::snapshot::TextDocumentSnapshotError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveDecision {
    Saved,
    BlockedExternalChange,
    BlockedRootEscape,
    BlockedUnsafeSymlink,
    WriteFailed,
}

pub(super) fn write_text_via_temp_rename(
    target: &FileAccessTarget,
    text: &str,
) -> Result<(), TextDocumentSaveError> {
    let parent =
        target
            .canonical_path
            .parent()
            .ok_or_else(|| TextDocumentSaveError::WriteFailed {
                target: Box::new(target.clone()),
                kind: io::ErrorKind::InvalidInput,
            })?;
    let temp_path = unique_save_temp_path(parent, &target.selected_relative_path);
    let write_result = File::options()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        });

    if let Err(error) = write_result {
        return Err(write_failed_after_temp_cleanup(
            target,
            &temp_path,
            error.kind(),
        ));
    }

    if let Err(error) = fs::rename(&temp_path, &target.canonical_path) {
        return Err(write_failed_after_temp_cleanup(
            target,
            &temp_path,
            error.kind(),
        ));
    }

    Ok(())
}

pub(super) fn write_failed_after_temp_cleanup(
    target: &FileAccessTarget,
    temp_path: &Path,
    kind: io::ErrorKind,
) -> TextDocumentSaveError {
    let _ = fs::remove_file(temp_path);
    TextDocumentSaveError::WriteFailed {
        target: Box::new(target.clone()),
        kind,
    }
}

fn unique_save_temp_path(parent: &Path, selected_relative_path: &Path) -> PathBuf {
    let target_name = selected_relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("buffer");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    parent.join(format!(
        ".{target_name}.tekstide-save-{}-{nonce}.tmp",
        std::process::id()
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextDocumentSaveError {
    Access(FileAccessError),
    RootEscape(FileAccessError),
    UnsafeSymlink {
        target: Box<FileAccessTarget>,
    },
    ExternalChange {
        target: Box<FileAccessTarget>,
    },
    Snapshot(TextDocumentSnapshotError),
    WriteFailed {
        target: Box<FileAccessTarget>,
        kind: io::ErrorKind,
    },
}

impl TextDocumentSaveError {
    pub fn decision(&self) -> SaveDecision {
        match self {
            Self::Access(_) | Self::Snapshot(_) | Self::WriteFailed { .. } => {
                SaveDecision::WriteFailed
            }
            Self::RootEscape(_) => SaveDecision::BlockedRootEscape,
            Self::UnsafeSymlink { .. } => SaveDecision::BlockedUnsafeSymlink,
            Self::ExternalChange { .. } => SaveDecision::BlockedExternalChange,
        }
    }
}

impl fmt::Display for TextDocumentSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) | Self::RootEscape(error) => {
                write!(formatter, "file access blocked: {error}")
            }
            Self::UnsafeSymlink { target } => write!(
                formatter,
                "save blocked for symlink target: {}",
                target.selected_relative_path.display()
            ),
            Self::ExternalChange { target } => write!(
                formatter,
                "save blocked because file changed externally: {}",
                target.selected_relative_path.display()
            ),
            Self::Snapshot(error) => write!(formatter, "{error}"),
            Self::WriteFailed { target, kind } => write!(
                formatter,
                "could not save {}: {kind:?}",
                target.selected_relative_path.display()
            ),
        }
    }
}

impl std::error::Error for TextDocumentSaveError {}
