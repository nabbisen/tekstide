use std::fmt;
use std::path::Path;
use std::time::SystemTime;

use crate::project::root::{
    FileAccessBlockedReason, FileAccessContainmentStatus, FileAccessError, FileAccessSymlinkStatus,
    FileAccessTarget, ProjectFileAccessPolicy, ProjectRootHandle,
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

/// RFC-027 PR-027-C: the fields [`TextDocument::recover`] takes from a recovery record,
/// bundled so the constructor (and [`crate::project::content::ProjectContentWorkspace::recover_text_document`],
/// which forwards it) stay under clippy's argument-count lint without an `#[allow]` --
/// every field here comes from the same `RecoveryRecord`, so grouping them loses no
/// caller-side clarity the way bundling unrelated parameters would.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredBufferInit {
    pub text: String,
    pub cursor: TextCursor,
    pub viewport: TextViewport,
    pub recorded_modified_at: SystemTime,
    pub recorded_len: u64,
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

    /// RFC-027 PR-027-C, D3/D5: builds a document from a recovery record instead of from
    /// disk -- `recovered.text`/`cursor`/`viewport` are the record's own, and undo/redo
    /// start empty (D3: a recovered buffer carries no history; nothing here could
    /// reconstruct it anyway, since it was never persisted). `recovered.recorded_modified_at`/
    /// `recorded_len` are the record's own snapshot of the file *as it stood when the
    /// record was written* -- compared here against the file as it stands now, the same
    /// two fields `TextDocument::save`'s own external-change check already treats as what
    /// changed (never the content hash, which the record does not carry -- see
    /// `RecoveryFileSnapshot`'s own doc).
    ///
    /// A recovered buffer is always local work the file on disk does not have, so this
    /// never produces `Clean` or plain `ExternalChanged`: unchanged restores it `Dirty`,
    /// exactly as it was when captured; changed or gone make it `Conflict`, the same state
    /// `record_external_change` already gives a dirty document whose file moved under it.
    /// A caller deriving a display status from the result should use the identical
    /// `Conflict` + "does the file still exist" check `refresh_document_by_canonical_path`
    /// already uses for `ExternalDeleted` -- this constructor mints no fourth state.
    pub fn recover(
        root: &ProjectRootHandle,
        selected_relative_path: impl AsRef<Path>,
        policy: TextDocumentOpenPolicy,
        recovered: RecoveredBufferInit,
    ) -> Result<Self, TextDocumentOpenError> {
        let RecoveredBufferInit {
            text,
            cursor,
            viewport,
            recorded_modified_at,
            recorded_len,
        } = recovered;
        let selected_relative_path = selected_relative_path.as_ref();

        let resolved = ProjectFileAccessPolicy.resolve_existing(root, selected_relative_path);
        let target = match resolved {
            Ok(target) => target,
            Err(error) if is_missing_current_target(&error) => {
                return Ok(Self::recovered_as_missing(
                    target_for_missing_recovery_path(error),
                    text,
                    cursor,
                    viewport,
                ));
            }
            Err(error) => return Err(TextDocumentOpenError::Access(error)),
        };

        if !target.canonical_path.is_file() {
            // Resolved to something other than a regular file (e.g. a directory now
            // occupies the path) -- there is no file content to compare against, the
            // same "nothing to offer back" shape as an outright-missing path.
            return Ok(Self::recovered_as_missing(target, text, cursor, viewport));
        }

        let current_snapshot =
            match file_snapshot_for_current_disk(&target, policy.max_editable_bytes) {
                Ok(snapshot) => snapshot,
                Err(error) if is_external_snapshot_shape_change(&error) => {
                    return Ok(Self::recovered_as_missing(target, text, cursor, viewport));
                }
                Err(error) => {
                    return Err(TextDocumentOpenError::ReadFailed {
                        target: error.target,
                        kind: error.kind,
                    });
                }
            };

        let unchanged = current_snapshot.modified_at == recorded_modified_at
            && current_snapshot.len == recorded_len;

        // When the file has moved on, `last_known_snapshot` must not become
        // `current_snapshot` as read just above: a *later* `refresh_external_state`/`save`
        // re-reads the disk and compares against whatever is stored here, and comparing a
        // fresh read against itself would report `Unchanged`, silently losing the very
        // divergence this constructor just found (and, with it, the `Conflict`
        // `ProjectContentStatus` the caller's own `refresh_active_text_document` call
        // depends on to show the Reload control -- see `recover_highlighted_offer_item`'s
        // own doc). `content_hash: None` guarantees that later comparison sees a mismatch
        // -- a fresh read's own hash is `Some(_)` whenever the file is within the policy's
        // editable bound, so `None != Some(_)` holds regardless of `modified_at`/`len`.
        let last_known_snapshot = if unchanged {
            current_snapshot
        } else {
            FileSnapshot {
                content_hash: None,
                ..current_snapshot
            }
        };

        Ok(Self {
            target,
            text,
            last_known_snapshot,
            state: if unchanged {
                TextDocumentState::Dirty
            } else {
                TextDocumentState::Conflict
            },
            cursor,
            viewport,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_bound_reached: false,
        })
    }

    /// The `recover` shape for "there is no live file to compare against" (gone, or
    /// resolved to a non-file): always `Conflict` -- a recovered buffer is always local
    /// work, and a dirty document whose file is missing is a conflict by the same rule
    /// `record_external_change` already applies. `last_known_snapshot` is a placeholder:
    /// any future `save`/`refresh_external_state` call resolves the target again first and
    /// hits the identical missing-path branch before ever reaching a snapshot comparison
    /// (see `is_missing_current_target`'s callers), so this value is never compared.
    fn recovered_as_missing(
        target: FileAccessTarget,
        text: String,
        cursor: TextCursor,
        viewport: TextViewport,
    ) -> Self {
        let last_known_snapshot = FileSnapshot {
            canonical_path: target.canonical_path.clone(),
            modified_at: SystemTime::UNIX_EPOCH,
            len: 0,
            content_hash: None,
        };
        Self {
            target,
            text,
            last_known_snapshot,
            state: TextDocumentState::Conflict,
            cursor,
            viewport,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_bound_reached: false,
        }
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

/// RFC-027 PR-027-C: a `FileAccessTarget` for a recovery record whose file is gone,
/// built from the resolve failure's own already-computed fields rather than a fresh
/// `fs::canonicalize` (which cannot succeed against a path that does not exist). This is
/// the same stand-in role an already-open document's own stale `self.target` plays for
/// `save`'s `MissingPath` branch -- a stable identity to display and to re-resolve
/// against later, not a claim that the path was freshly verified.
fn target_for_missing_recovery_path(error: FileAccessError) -> FileAccessTarget {
    FileAccessTarget {
        project_id: error.project_id,
        selected_relative_path: error.selected_relative_path,
        selected_absolute_path: error.selected_absolute_path.clone(),
        canonical_path: error.selected_absolute_path,
        root_canonical_path: error.root_canonical_path,
        symlink_status: FileAccessSymlinkStatus::NoSymlink,
        containment_status: FileAccessContainmentStatus::InsideRoot,
    }
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
