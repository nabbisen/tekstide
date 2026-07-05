use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use crate::project::root::{
    FileAccessError, FileAccessTarget, ProjectFileAccessPolicy, ProjectRootHandle,
};

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextCursor {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextViewport {
    pub first_visible_line: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextDocumentState {
    Clean,
    Dirty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDocument {
    target: FileAccessTarget,
    text: String,
    // PR-006-D owns the open/save snapshot fields used for external-change detection.
    state: TextDocumentState,
    cursor: TextCursor,
    viewport: TextViewport,
}

impl TextDocument {
    pub fn open(
        root: &ProjectRootHandle,
        selected_relative_path: impl AsRef<Path>,
        policy: TextDocumentOpenPolicy,
    ) -> Result<Self, TextDocumentOpenError> {
        let target = ProjectFileAccessPolicy
            .resolve_existing(root, selected_relative_path)
            .map_err(TextDocumentOpenError::Access)?;

        if !target.canonical_path.is_file() {
            return Err(TextDocumentOpenError::NotFile {
                target: Box::new(target),
            });
        }

        let metadata = fs::metadata(&target.canonical_path).map_err(|error| {
            TextDocumentOpenError::ReadFailed {
                target: Box::new(target.clone()),
                kind: error.kind(),
            }
        })?;

        enforce_editable_size_cap(&target, metadata.len(), policy.max_editable_bytes)?;

        let bytes = read_file_bounded(&target, policy.max_editable_bytes)?;

        if bytes.contains(&0) {
            return Err(TextDocumentOpenError::ContainsNul {
                target: Box::new(target),
            });
        }

        let text = String::from_utf8(bytes).map_err(|_| TextDocumentOpenError::InvalidUtf8 {
            target: Box::new(target.clone()),
        })?;

        Ok(Self {
            target,
            text,
            state: TextDocumentState::Clean,
            cursor: TextCursor::default(),
            viewport: TextViewport::default(),
        })
    }

    pub fn target(&self) -> &FileAccessTarget {
        &self.target
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn state(&self) -> TextDocumentState {
        self.state
    }

    pub fn is_dirty(&self) -> bool {
        self.state == TextDocumentState::Dirty
    }

    pub fn cursor(&self) -> TextCursor {
        self.cursor
    }

    pub fn viewport(&self) -> TextViewport {
        self.viewport
    }

    pub fn set_cursor(&mut self, cursor: TextCursor) {
        self.cursor = cursor;
    }

    pub fn set_viewport(&mut self, viewport: TextViewport) {
        self.viewport = viewport;
    }

    pub fn replace_text(&mut self, text: impl Into<String>) -> Result<(), TextDocumentEditError> {
        let text = text.into();
        if text.contains('\0') {
            return Err(TextDocumentEditError::ContainsNul);
        }

        if self.text != text {
            self.text = text;
            self.state = TextDocumentState::Dirty;
        }

        Ok(())
    }
}

fn read_file_bounded(
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

fn enforce_editable_size_cap(
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextDocumentEditError {
    ContainsNul,
}

impl fmt::Display for TextDocumentEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContainsNul => write!(formatter, "replacement text contains NUL"),
        }
    }
}

impl std::error::Error for TextDocumentEditError {}

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

#[cfg(test)]
mod tests;
