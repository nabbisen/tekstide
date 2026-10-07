use std::fmt;
use std::path::Path;

use crate::project::root::{
    FileAccessBlockedReason, FileAccessError, FileAccessSymlinkStatus, FileAccessTarget,
    ProjectFileAccessPolicy, ProjectRootHandle,
};

use super::edit::TextDocumentEditError;
use super::open::{
    TextDocumentOpenError, TextDocumentOpenPolicy, enforce_editable_size_cap, metadata_len,
    read_file_bounded,
};
use super::save::{SaveDecision, TextDocumentSaveError, write_text_via_temp_rename};
use super::snapshot::{
    FileSnapshot, TextDocumentSnapshotError, file_snapshot_for_current_disk,
    file_snapshot_from_opened_bytes, is_external_snapshot_shape_change,
};
use super::undo::{EditOperation, UNDO_MAX_DEPTH};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextCursor {
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextViewport {
    pub first_visible_line: usize,
    /// RFC-057 PR-057-C, Q2: the horizontal twin of `first_visible_line`, on
    /// the same D9 rule -- the least movement that keeps the cursor's column
    /// on screen. Columns, not bytes: the same unit `TextCursor.column` uses.
    pub first_visible_column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextDocumentState {
    Clean,
    Dirty,
    ExternalChanged,
    Conflict,
    SaveError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalChangeDecision {
    Unchanged,
    ExternalChanged,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDocument {
    target: FileAccessTarget,
    text: String,
    last_known_snapshot: FileSnapshot,
    state: TextDocumentState,
    cursor: TextCursor,
    viewport: TextViewport,
    /// RFC-057 D3. Empty on every fresh [`TextDocument::open`] -- there is
    /// no separate clearing step for "undo must not cross an
    /// external-change reload" (the risk document's §3 row three) because
    /// a reload replaces this whole struct with a freshly opened one
    /// (`ProjectContentWorkspace::open_text_document`), never patches
    /// `text` on an existing instance.
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
    /// Latches true the first time [`Self::record_edit_operation`] drops
    /// the oldest entry for being over `UNDO_MAX_DEPTH`. Stays true once
    /// set -- the forgotten edit does not come back just because the
    /// stack later has room again.
    undo_bound_reached: bool,
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

        enforce_editable_size_cap(&target, metadata_len(&target)?, policy.max_editable_bytes)?;

        let bytes = read_file_bounded(&target, policy.max_editable_bytes)?;

        if bytes.contains(&0) {
            return Err(TextDocumentOpenError::ContainsNul {
                target: Box::new(target),
            });
        }

        let text = String::from_utf8(bytes).map_err(|_| TextDocumentOpenError::InvalidUtf8 {
            target: Box::new(target.clone()),
        })?;
        let last_known_snapshot = file_snapshot_from_opened_bytes(&target, &text)?;

        Ok(Self {
            target,
            text,
            last_known_snapshot,
            state: TextDocumentState::Clean,
            cursor: TextCursor::default(),
            viewport: TextViewport::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_bound_reached: false,
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
        matches!(
            self.state,
            TextDocumentState::Dirty | TextDocumentState::Conflict | TextDocumentState::SaveError
        )
    }

    pub fn last_known_snapshot(&self) -> &FileSnapshot {
        &self.last_known_snapshot
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
            // RFC-057 D3: text that changed away from the opened content
            // and then back to it is `Clean` again, not permanently
            // `Dirty` because it changed at some point -- the property
            // undoing every edit back to the opened file needs.
            // Guarded to the two ordinary editing states only:
            // `ExternalChanged`/`Conflict`/`SaveError` each need their own
            // resolution (the reload/conflict dialog, or a retried save),
            // never a silent overwrite just because the text coincided
            // with what was last known on disk.
            self.state = match self.state {
                TextDocumentState::Clean | TextDocumentState::Dirty => {
                    if self.last_known_snapshot.matches_content(&self.text) {
                        TextDocumentState::Clean
                    } else {
                        TextDocumentState::Dirty
                    }
                }
                // RFC-026 D8: the disk changed under a clean document, so text that differs from
                // what was opened is local work the disk does not have, which is a conflict. Text
                // back at the opened content has nothing local, so the external change stands. A
                // `Conflict` is not cleared this way: it needs its own resolution (RFC-057).
                TextDocumentState::ExternalChanged => {
                    if self.last_known_snapshot.matches_content(&self.text) {
                        TextDocumentState::ExternalChanged
                    } else {
                        TextDocumentState::Conflict
                    }
                }
                other => other,
            };
        }

        Ok(())
    }

    /// RFC-057 D3: records a real edit for undo -- **not called by undo or
    /// redo themselves**, only by an ordinary edit. An edit made in the
    /// ordinary course of typing clears the redo stack: a new edit after
    /// an undo abandons the old future, the rule every text editor uses.
    /// Depth-bounded at [`UNDO_MAX_DEPTH`]; past the bound the oldest
    /// recorded edit is dropped and [`Self::undo_depth_bound_reached`]
    /// latches true.
    pub fn record_edit_operation(&mut self, operation: EditOperation) {
        self.redo_stack.clear();
        self.undo_stack.push(operation);
        if self.undo_stack.len() > UNDO_MAX_DEPTH {
            self.undo_stack.remove(0);
            self.undo_bound_reached = true;
        }
    }

    /// Pops the most recently recorded (or redone) edit for the caller to
    /// invert, and remembers it on the redo stack. `None` when there is
    /// nothing to undo.
    pub fn undo_operation(&mut self) -> Option<EditOperation> {
        let operation = self.undo_stack.pop()?;
        self.redo_stack.push(operation.clone());
        Some(operation)
    }

    /// The mirror of [`Self::undo_operation`]: the most recently undone
    /// edit, for the caller to reapply. `None` when there is nothing to
    /// redo.
    pub fn redo_operation(&mut self) -> Option<EditOperation> {
        let operation = self.redo_stack.pop()?;
        self.undo_stack.push(operation.clone());
        Some(operation)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// RFC-057 D3: whether an edit has ever been forgotten because the
    /// undo depth was full -- the product's own way of saying so, rather
    /// than silently dropping the oldest edit with nothing on screen to
    /// show for it.
    pub fn undo_depth_bound_reached(&self) -> bool {
        self.undo_bound_reached
    }

    pub fn refresh_external_state(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<ExternalChangeDecision, TextDocumentRefreshError> {
        let current_target = match self.resolve_current_target(root) {
            Ok(target) => target,
            Err(error) if is_missing_current_target(&error) => {
                return Ok(self.record_external_change());
            }
            Err(error) => return Err(TextDocumentRefreshError::Access(error)),
        };

        if !current_target.canonical_path.is_file() {
            return Ok(self.record_external_change());
        }

        let current_snapshot =
            match file_snapshot_for_current_disk(&current_target, policy.max_editable_bytes) {
                Ok(snapshot) => snapshot,
                Err(error) if is_external_snapshot_shape_change(&error) => {
                    return Ok(self.record_external_change());
                }
                Err(error) => return Err(TextDocumentRefreshError::Snapshot(error)),
            };

        if current_snapshot == self.last_known_snapshot {
            return Ok(ExternalChangeDecision::Unchanged);
        }

        if self.is_dirty() {
            self.state = TextDocumentState::Conflict;
            return Ok(ExternalChangeDecision::Conflict);
        }

        self.target = current_target;
        self.state = TextDocumentState::ExternalChanged;
        Ok(ExternalChangeDecision::ExternalChanged)
    }

    pub fn save(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<SaveDecision, TextDocumentSaveError> {
        let current_target = match self.resolve_current_target(root) {
            Ok(target) => target,
            Err(error) if is_missing_current_target(&error) => {
                return Err(self.block_external_change(self.target.clone()));
            }
            Err(error) => {
                self.state = TextDocumentState::SaveError;
                return Err(self.save_access_error(error));
            }
        };

        if !current_target.canonical_path.is_file() {
            return Err(self.block_external_change(current_target));
        }

        if current_target.symlink_status != FileAccessSymlinkStatus::NoSymlink {
            self.state = TextDocumentState::SaveError;
            return Err(TextDocumentSaveError::UnsafeSymlink {
                target: Box::new(current_target),
            });
        }

        let current_snapshot =
            match file_snapshot_for_current_disk(&current_target, policy.max_editable_bytes) {
                Ok(snapshot) => snapshot,
                Err(error) if is_external_snapshot_shape_change(&error) => {
                    return Err(self.block_external_change_from_snapshot(error));
                }
                Err(error) => {
                    self.state = TextDocumentState::SaveError;
                    return Err(TextDocumentSaveError::Snapshot(error));
                }
            };

        if current_snapshot != self.last_known_snapshot {
            return Err(self.block_external_change(current_target));
        }

        if !self.is_dirty() {
            self.target = current_target;
            return Ok(SaveDecision::Saved);
        }

        if let Err(error) = write_text_via_temp_rename(&current_target, &self.text) {
            self.state = TextDocumentState::SaveError;
            return Err(error);
        }

        self.target = current_target;
        self.last_known_snapshot =
            match file_snapshot_for_current_disk(&self.target, policy.max_editable_bytes) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    self.state = TextDocumentState::SaveError;
                    return Err(TextDocumentSaveError::Snapshot(error));
                }
            };
        self.state = TextDocumentState::Clean;

        Ok(SaveDecision::Saved)
    }

    fn resolve_current_target(
        &self,
        root: &ProjectRootHandle,
    ) -> Result<FileAccessTarget, FileAccessError> {
        ProjectFileAccessPolicy.resolve_existing(root, &self.target.selected_relative_path)
    }

    fn record_external_change(&mut self) -> ExternalChangeDecision {
        if self.is_dirty() {
            self.state = TextDocumentState::Conflict;
            ExternalChangeDecision::Conflict
        } else {
            self.state = TextDocumentState::ExternalChanged;
            ExternalChangeDecision::ExternalChanged
        }
    }

    fn block_external_change(&mut self, target: FileAccessTarget) -> TextDocumentSaveError {
        self.state = if self.is_dirty() {
            TextDocumentState::Conflict
        } else {
            TextDocumentState::ExternalChanged
        };
        TextDocumentSaveError::ExternalChange {
            target: Box::new(target),
        }
    }

    fn block_external_change_from_snapshot(
        &mut self,
        error: TextDocumentSnapshotError,
    ) -> TextDocumentSaveError {
        self.block_external_change(*error.target)
    }

    fn save_access_error(&self, error: FileAccessError) -> TextDocumentSaveError {
        match error.reason {
            FileAccessBlockedReason::RootEscape | FileAccessBlockedReason::SymlinkEscape => {
                TextDocumentSaveError::RootEscape(error)
            }
            _ => TextDocumentSaveError::Access(error),
        }
    }
}

fn is_missing_current_target(error: &FileAccessError) -> bool {
    error.reason == FileAccessBlockedReason::MissingPath
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextDocumentRefreshError {
    Access(FileAccessError),
    Snapshot(TextDocumentSnapshotError),
}

impl fmt::Display for TextDocumentRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => write!(formatter, "file access blocked: {error}"),
            Self::Snapshot(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for TextDocumentRefreshError {}
