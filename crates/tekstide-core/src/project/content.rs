use std::fmt;
use std::path::{Path, PathBuf};

use crate::content::{
    EditOperation, ExternalChangeDecision, SaveDecision, TextCursor, TextDocument,
    TextDocumentEditError, TextDocumentOpenError, TextDocumentOpenPolicy, TextDocumentRefreshError,
    TextDocumentSaveError, TextDocumentState, TextViewport,
};
use crate::project::explorer_tree::{
    ExplorerScanCompleted, ExplorerScanRequest, ExplorerToggle, ExplorerTree,
};
use crate::project::root::{
    ExplorerDirectoryScan, ExplorerNodeKind, ExplorerNodeState, ExplorerScanError,
    FileAccessSymlinkStatus, FileExplorerScanPolicy, FileExplorerScanner, ProjectRootHandle,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContentWorkspace {
    selected_explorer_path: PathBuf,
    explorer_tree: ExplorerTree,
    explorer_status: ProjectExplorerStatus,
    /// RFC-065 D1/D2: the open set. `active_index` names which entry `active_document()`
    /// and every other single-document accessor below reads; `active` keeps its old
    /// meaning exactly, and a plural *open* set is what is new.
    documents: Vec<TextDocument>,
    active_index: Option<usize>,
    status: ProjectContentStatus,
}

impl ProjectContentWorkspace {
    pub fn selected_explorer_path(&self) -> &Path {
        &self.selected_explorer_path
    }

    /// The root directory's scan, if it has loaded. The tree beyond the
    /// root is [`Self::explorer_tree`].
    pub fn explorer_scan(&self) -> Option<&ExplorerDirectoryScan> {
        self.explorer_tree.root_scan()
    }

    /// RFC-052 PR-052-B: the explorer as a tree of expandable folders.
    pub fn explorer_tree(&self) -> &ExplorerTree {
        &self.explorer_tree
    }

    /// RFC-055 D7: `explorer.show_ignored`, applied to this project's tree.
    pub fn set_explorer_show_ignored(&mut self, show_ignored: bool) {
        self.explorer_tree.set_show_ignored(show_ignored);
    }

    /// Marks `path` as needing a scan; a worker runs it later (see
    /// [`ExplorerScanRequest`]). A no-op if one is already in flight.
    pub fn request_explorer_scan(&mut self, path: &Path) {
        self.explorer_tree.request_scan(path);
    }

    /// Expands or collapses the folder at `path`. Expanding requests a scan.
    pub fn toggle_explorer_directory(&mut self, path: &Path) -> ExplorerToggle {
        self.explorer_tree.toggle(path)
    }

    /// The runnable scans for everything pending. Each is `Send + 'static`
    /// and **blocking to run**: hand it to a worker thread.
    pub fn explorer_scan_requests(&self, root: &ProjectRootHandle) -> Vec<ExplorerScanRequest> {
        self.explorer_tree.scan_requests(root)
    }

    /// Applies a finished scan. Returns `false` for a stale one. The root's
    /// outcome also sets [`Self::explorer_status`], as the one-level explorer
    /// always did.
    pub fn apply_explorer_scan(&mut self, completed: ExplorerScanCompleted) -> bool {
        let is_root = completed.path.as_os_str().is_empty();
        let status = match &completed.result {
            Ok(_) => ProjectExplorerStatus::Ready,
            Err(error) => ProjectExplorerStatus::Error {
                message: error.to_string(),
            },
        };
        let applied = self.explorer_tree.apply(completed);
        if applied && is_root {
            self.explorer_status = status;
        }
        applied
    }

    pub fn explorer_status(&self) -> &ProjectExplorerStatus {
        &self.explorer_status
    }

    pub fn active_document(&self) -> Option<&TextDocument> {
        self.active_index
            .and_then(|index| self.documents.get(index))
    }

    fn active_document_mut(&mut self) -> Option<&mut TextDocument> {
        let index = self.active_index?;
        self.documents.get_mut(index)
    }

    /// RFC-065: every open document, active or not, in no particular order beyond
    /// insertion. Read-only: nothing here switches which one is active (PR-065-C) or
    /// bounds how many there are (PR-065-B's D4) -- this is the minimal way to look at
    /// the set the counts below and the open-document repair's own test both need.
    pub fn open_documents(&self) -> impl Iterator<Item = &TextDocument> {
        self.documents.iter()
    }

    /// RFC-026 D1's inputs to the watch scope: the expanded folders and the folders of
    /// every open document, both relative to the project root. RFC-065 PR-065-B: the open
    /// set is plural now, and the watcher's own scope follows it -- `open_documents()`
    /// rather than `active_document()` alone, the measurement RFC-065 itself named
    /// ("`watch_inputs()` already returns `open_document_directories` -- plural, fed by
    /// one. The watcher needs no change in shape, only in count.").
    pub fn watch_inputs(&self) -> (Vec<PathBuf>, Vec<PathBuf>) {
        let expanded = self
            .explorer_tree
            .expanded_directories()
            .map(Path::to_path_buf)
            .collect();
        let open_document_directories = self
            .open_documents()
            .filter_map(|document| {
                document
                    .target()
                    .selected_relative_path
                    .parent()
                    .map(Path::to_path_buf)
            })
            .collect();
        (expanded, open_document_directories)
    }

    pub fn active_file_launch_assessment(&self) -> ProjectActiveFileLaunchAssessment {
        let Some(document) = self.active_document() else {
            return ProjectActiveFileLaunchAssessment {
                active_path_hint: None,
                state: None,
                decision: ProjectActiveFileLaunchDecision::Proceed,
            };
        };

        let state = document.state();
        let decision = match state {
            TextDocumentState::Clean => ProjectActiveFileLaunchDecision::Proceed,
            TextDocumentState::Dirty => {
                ProjectActiveFileLaunchDecision::Blocked(ProjectActiveFileLaunchBlockReason::Dirty)
            }
            TextDocumentState::ExternalChanged => ProjectActiveFileLaunchDecision::Blocked(
                ProjectActiveFileLaunchBlockReason::ExternalChanged,
            ),
            TextDocumentState::Conflict => ProjectActiveFileLaunchDecision::Blocked(
                ProjectActiveFileLaunchBlockReason::Conflict,
            ),
            TextDocumentState::SaveError => ProjectActiveFileLaunchDecision::Blocked(
                ProjectActiveFileLaunchBlockReason::SaveError,
            ),
        };

        ProjectActiveFileLaunchAssessment {
            active_path_hint: Some(document.target().selected_relative_path.clone()),
            state: Some(state),
            decision,
        }
    }

    pub fn status(&self) -> &ProjectContentStatus {
        &self.status
    }

    pub fn scan_explorer_directory(
        &mut self,
        root: &ProjectRootHandle,
        selected_relative_path: impl Into<PathBuf>,
        policy: &FileExplorerScanPolicy,
    ) -> Result<(), ProjectContentError> {
        let selected_relative_path = selected_relative_path.into();

        let result =
            FileExplorerScanner.scan_directory(root, selected_relative_path.clone(), policy);
        match result {
            Ok(scan) => {
                self.selected_explorer_path = scan.directory.selected_relative_path.clone();
                self.explorer_tree
                    .set_scan(&selected_relative_path, Ok(scan));
                self.explorer_status = ProjectExplorerStatus::Ready;
                Ok(())
            }
            Err(error) => {
                self.explorer_tree
                    .set_scan(&selected_relative_path, Err(error.clone()));
                self.explorer_status = ProjectExplorerStatus::Error {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Explorer(error))
            }
        }
    }

    pub fn open_text_document(
        &mut self,
        root: &ProjectRootHandle,
        selected_relative_path: impl AsRef<Path>,
        policy: TextDocumentOpenPolicy,
        open_document_limit: Option<u32>,
    ) -> Result<(), ProjectContentError> {
        let selected_relative_path = selected_relative_path.as_ref().to_path_buf();

        // RFC-065 review 468's ruling: a path already in the open set switches to that
        // entry rather than adding a second. Slice A deliberately left this undone (two
        // entries for one path, neither lost); the review found the hazard it was storing
        // up for slice C -- `save_active_document` saves only the active entry, so once a
        // switcher makes the older entry reachable, saving both in either order silently
        // overwrites one with the other. Switching here, with no disk read, is also what
        // every editor a user has met already does for this case.
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| document.target().selected_relative_path == selected_relative_path)
        {
            self.selected_explorer_path = selected_relative_path;
            self.active_index = Some(index);
            self.status = ProjectContentStatus::Opened;
            return Ok(());
        }

        // RFC-065 D4: the bound, checked after the dedup/ownership question above (a path
        // already open always belongs, no matter how full the set is) and before any disk
        // read -- the same "structural checks before the policy check" order
        // `add_terminal_session`'s own `terminal_session_limit` enforcement uses.
        if let Some(limit) = open_document_limit
            && self.documents.len() as u32 >= limit
        {
            let open = self.documents.len() as u32;
            self.status = ProjectContentStatus::OpenError {
                message: ProjectContentError::OpenSetAtLimit { open, limit }.to_string(),
            };
            return Err(ProjectContentError::OpenSetAtLimit { open, limit });
        }

        match TextDocument::open(root, &selected_relative_path, policy) {
            Ok(document) => {
                self.selected_explorer_path = selected_relative_path;
                // RFC-065 D1, the repair: the open set. Opening a file joins it rather
                // than replacing whatever was there -- there is nothing left to discard,
                // by construction. The set's own shape (true counts, a bound, a
                // switcher) is PR-065-B/C; this is only the line that stops the loss.
                self.documents.push(document);
                self.active_index = Some(self.documents.len() - 1);
                self.status = ProjectContentStatus::Opened;
                Ok(())
            }
            Err(error) => {
                self.status = ProjectContentStatus::OpenError {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Open(error))
            }
        }
    }

    /// RFC-065 PR-065-B: the Reload button's own entry point, split out of
    /// [`Self::open_text_document`] once that method gained the dedup-by-path switch (review
    /// 468). Reload must always take disk's current content for the active document and
    /// discard any local edit -- the one escape `TextDocument::save()` gives past a conflict
    /// -- so it cannot share a path with "open," which now deliberately does the opposite
    /// (switches in place, no disk read) for a path that is already open. Replaces the active
    /// entry's own slot in the set rather than pushing a new one, so its position (and every
    /// other open document's) is undisturbed.
    pub fn reload_active_document(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<(), ProjectContentError> {
        let Some(index) = self.active_index else {
            self.status = ProjectContentStatus::OpenError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };
        let relative_path = self.documents[index]
            .target()
            .selected_relative_path
            .clone();

        match TextDocument::open(root, &relative_path, policy) {
            Ok(document) => {
                self.documents[index] = document;
                self.status = ProjectContentStatus::Opened;
                Ok(())
            }
            Err(error) => {
                self.status = ProjectContentStatus::OpenError {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Open(error))
            }
        }
    }

    pub fn replace_active_text(
        &mut self,
        text: impl Into<String>,
    ) -> Result<(), ProjectContentError> {
        let Some(document) = self.active_document_mut() else {
            self.status = ProjectContentStatus::EditError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };

        match document.replace_text(text) {
            Ok(()) => {
                if document.is_dirty() {
                    self.status = ProjectContentStatus::Edited;
                } else if matches!(self.status, ProjectContentStatus::Empty) {
                    self.status = ProjectContentStatus::Opened;
                }
                Ok(())
            }
            Err(error) => {
                self.status = ProjectContentStatus::EditError {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Edit(error))
            }
        }
    }

    pub fn save_active_document(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<SaveDecision, ProjectContentError> {
        let Some(document) = self.active_document_mut() else {
            self.status = ProjectContentStatus::SaveError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };

        match document.save(root, policy) {
            Ok(decision) => {
                self.status = ProjectContentStatus::Saved { decision };
                Ok(decision)
            }
            Err(error) => {
                self.status = match error.decision() {
                    // `TextDocument::save` already distinguished these two
                    // cases on `self.state` before collapsing both into
                    // `BlockedExternalChange` for this `SaveDecision`
                    // (`content::document`'s `block_external_change`: state
                    // becomes `Conflict` only if the buffer was dirty,
                    // `ExternalChanged` otherwise). Reading `document.state()`
                    // back here recovers that distinction rather than
                    // re-deriving or guessing it -- the same pattern
                    // `refresh_active_document` below already uses, and the
                    // one the shell's own RFC-019 PR-019-E fix reads
                    // independently for its conflict-modal wording.
                    //
                    // RFC-026 D8: a blocked save against a file that is gone is reported as
                    // deleted, not as changed, the same correction `refresh_active_document`
                    // below carries -- a save can be blocked by a disk that went away just as
                    // easily as by one that changed, and the two must not look the same.
                    SaveDecision::BlockedExternalChange
                        if !document.target().canonical_path.exists() =>
                    {
                        ProjectContentStatus::ExternalDeleted
                    }
                    SaveDecision::BlockedExternalChange => match document.state() {
                        TextDocumentState::Conflict => ProjectContentStatus::Conflict,
                        _ => ProjectContentStatus::ExternalChanged,
                    },
                    _ => ProjectContentStatus::SaveError {
                        message: error.to_string(),
                    },
                };
                Err(ProjectContentError::Save(error))
            }
        }
    }

    /// RFC-006 Amendment 1: the one write path `active_document()`'s
    /// read-only shape deliberately left uncovered. Cursor position
    /// participates in no dirty/save/conflict computation anywhere in
    /// `content::document` or `project::content` -- unlike text mutation,
    /// moving the cursor cannot make `self.status` stale, so this never
    /// touches it, in either the success or the no-document case.
    pub fn set_active_cursor(&mut self, cursor: TextCursor) -> Result<(), ProjectContentError> {
        let Some(document) = self.active_document_mut() else {
            return Err(ProjectContentError::NoActiveDocument);
        };
        document.set_cursor(cursor);
        Ok(())
    }

    /// RFC-057 PR-057-B: the viewport's write path, the twin of
    /// [`Self::set_active_cursor`] and for the same reason: the viewport takes
    /// part in no dirty/save/conflict computation, so this never touches
    /// `self.status`.
    pub fn set_active_viewport(
        &mut self,
        viewport: TextViewport,
    ) -> Result<(), ProjectContentError> {
        let Some(document) = self.active_document_mut() else {
            return Err(ProjectContentError::NoActiveDocument);
        };
        document.set_viewport(viewport);
        Ok(())
    }

    /// RFC-057 D3: records a real edit for undo, the twin of
    /// [`Self::set_active_cursor`] for the same reason -- recording an
    /// operation takes part in no dirty/save/conflict computation itself
    /// (the edit's own `replace_active_text` call already updated
    /// `self.status`), so this never touches it either.
    pub fn record_active_edit_operation(
        &mut self,
        operation: EditOperation,
    ) -> Result<(), ProjectContentError> {
        let Some(document) = self.active_document_mut() else {
            return Err(ProjectContentError::NoActiveDocument);
        };
        document.record_edit_operation(operation);
        Ok(())
    }

    /// Pops the active document's most recent undo entry, for the caller
    /// to invert and apply via [`Self::replace_active_text`]/
    /// [`Self::set_active_cursor`] -- the same two-step shape an ordinary
    /// edit already uses. `Ok(None)` (not an error) when there is nothing
    /// to undo.
    pub fn undo_active_document(&mut self) -> Result<Option<EditOperation>, ProjectContentError> {
        let Some(document) = self.active_document_mut() else {
            return Err(ProjectContentError::NoActiveDocument);
        };
        Ok(document.undo_operation())
    }

    /// The mirror of [`Self::undo_active_document`].
    pub fn redo_active_document(&mut self) -> Result<Option<EditOperation>, ProjectContentError> {
        let Some(document) = self.active_document_mut() else {
            return Err(ProjectContentError::NoActiveDocument);
        };
        Ok(document.redo_operation())
    }

    pub fn refresh_active_document(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<ExternalChangeDecision, ProjectContentError> {
        let Some(document) = self.active_document() else {
            self.status = ProjectContentStatus::RefreshError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };
        let canonical_path = document.target().canonical_path.clone();
        self.refresh_document_by_canonical_path(&canonical_path, root, policy)
    }

    /// RFC-065 PR-065-B: refreshes any open document by its own canonical path, not only the
    /// active one. The watcher's own scope already follows the whole open set
    /// (`watch_inputs()`); a notice naming a *background* document's file must reach that
    /// document too, not be silently dropped the way a single `document_touched: bool` keyed
    /// to "the active document" (`shell.rs`'s own `ProjectWatch`, pre-PR-065-B) could only
    /// ever do. [`Self::refresh_active_document`] is this method applied to the active
    /// document's own path.
    ///
    /// Updates `self.status` only when the refreshed document is the active one:
    /// `self.status` is the convenience field the chrome renders for *the document on
    /// screen*, and a background document's own `TextDocumentState` (read through
    /// [`Self::open_documents`]) is already where its own state lives. Overwriting the
    /// active document's displayed status because an unrelated background file changed
    /// would be exactly the kind of cross-document leak PR-065-A's own repair was about not
    /// having.
    pub fn refresh_document_by_canonical_path(
        &mut self,
        canonical_path: &Path,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<ExternalChangeDecision, ProjectContentError> {
        let Some(index) = self
            .documents
            .iter()
            .position(|document| document.target().canonical_path == canonical_path)
        else {
            return Err(ProjectContentError::NoActiveDocument);
        };
        let is_active = self.active_index == Some(index);
        let document = &mut self.documents[index];

        match document.refresh_external_state(root, policy) {
            Ok(decision) => {
                if is_active {
                    self.status = match decision {
                        ExternalChangeDecision::Unchanged
                            if document.state() == TextDocumentState::SaveError =>
                        {
                            ProjectContentStatus::SaveError {
                                message: "active document has save error".to_owned(),
                            }
                        }
                        ExternalChangeDecision::Unchanged if document.is_dirty() => {
                            ProjectContentStatus::Edited
                        }
                        ExternalChangeDecision::Unchanged => ProjectContentStatus::Opened,
                        // RFC-026 D8: a file that is gone is a state the product can say,
                        // distinct from one that changed. The document's text is kept
                        // either way.
                        ExternalChangeDecision::ExternalChanged
                        | ExternalChangeDecision::Conflict
                            if !document.target().canonical_path.exists() =>
                        {
                            ProjectContentStatus::ExternalDeleted
                        }
                        ExternalChangeDecision::ExternalChanged => {
                            ProjectContentStatus::ExternalChanged
                        }
                        ExternalChangeDecision::Conflict => ProjectContentStatus::Conflict,
                    };
                }
                Ok(decision)
            }
            Err(error) => {
                if is_active {
                    self.status = ProjectContentStatus::RefreshError {
                        message: error.to_string(),
                    };
                }
                Err(ProjectContentError::Refresh(error))
            }
        }
    }

    /// RFC-065 D3/D9: counts the whole open set, not only the active document. The set's
    /// own existence (D1's repair) makes this accurate for free; the coverage-row claim
    /// that `REQ-EDIT-004`'s plural is **met**, rather than merely counted correctly, is
    /// PR-065-B's to write, not this slice's.
    pub fn open_buffer_count(&self) -> u32 {
        self.documents.len() as u32
    }

    /// RFC-065 D3/D9: see [`Self::open_buffer_count`] -- the same reasoning, counting
    /// every open document's own dirty state rather than only the active one's.
    pub fn dirty_file_count(&self) -> u32 {
        self.documents
            .iter()
            .filter(|document| document.is_dirty())
            .count() as u32
    }

    pub fn active_path_hint(&self) -> Option<PathBuf> {
        self.active_document()
            .map(|document| document.target().selected_relative_path.clone())
    }
}

impl Default for ProjectContentWorkspace {
    fn default() -> Self {
        Self {
            selected_explorer_path: PathBuf::new(),
            explorer_tree: ExplorerTree::default(),
            explorer_status: ProjectExplorerStatus::Empty,
            documents: Vec::new(),
            active_index: None,
            status: ProjectContentStatus::Empty,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectActiveFileLaunchAssessment {
    pub active_path_hint: Option<PathBuf>,
    pub state: Option<TextDocumentState>,
    pub decision: ProjectActiveFileLaunchDecision,
}

impl ProjectActiveFileLaunchAssessment {
    pub fn allows_launch(&self) -> bool {
        self.decision == ProjectActiveFileLaunchDecision::Proceed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectActiveFileLaunchDecision {
    Proceed,
    Blocked(ProjectActiveFileLaunchBlockReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectActiveFileLaunchBlockReason {
    Dirty,
    ExternalChanged,
    Conflict,
    SaveError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectExplorerStatus {
    Empty,
    Ready,
    Error { message: String },
}

impl ProjectExplorerStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Ready => "ready",
            Self::Error { .. } => "error",
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Error { message } => Some(message),
            _ => None,
        }
    }
}

pub fn explorer_node_kind_label(kind: ExplorerNodeKind) -> &'static str {
    match kind {
        ExplorerNodeKind::File => "file",
        ExplorerNodeKind::Directory => "directory",
        ExplorerNodeKind::Other => "other",
    }
}

pub fn explorer_node_state_label(state: &ExplorerNodeState) -> &'static str {
    match state {
        ExplorerNodeState::Available => "available",
        ExplorerNodeState::Collapsed => "collapsed",
        ExplorerNodeState::Blocked(_) => "blocked",
        ExplorerNodeState::Unreadable => "unreadable",
    }
}

pub fn explorer_symlink_status_label(status: FileAccessSymlinkStatus) -> &'static str {
    match status {
        FileAccessSymlinkStatus::NoSymlink => "none",
        FileAccessSymlinkStatus::InRootSymlink => "in-root symlink",
        FileAccessSymlinkStatus::UnresolvedSymlink => "unresolved symlink",
        FileAccessSymlinkStatus::EscapesRoot => "escapes root",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectContentStatus {
    Empty,
    Opened,
    Edited,
    Saved {
        decision: SaveDecision,
    },
    ExternalChanged,
    /// RFC-026 D8: the open file was deleted on disk. Its text is kept, and nothing is reloaded.
    ExternalDeleted,
    Conflict,
    OpenError {
        message: String,
    },
    EditError {
        message: String,
    },
    SaveError {
        message: String,
    },
    RefreshError {
        message: String,
    },
}

impl ProjectContentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Opened => "open",
            Self::Edited => "edited",
            Self::Saved { .. } => "saved",
            Self::ExternalChanged => "external changed",
            Self::ExternalDeleted => "external deleted",
            Self::Conflict => "conflict",
            Self::OpenError { .. } => "open error",
            Self::EditError { .. } => "edit error",
            Self::SaveError { .. } => "save error",
            Self::RefreshError { .. } => "refresh error",
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::OpenError { message }
            | Self::EditError { message }
            | Self::SaveError { message }
            | Self::RefreshError { message } => Some(message),
            _ => None,
        }
    }
}

pub fn text_document_state_label(state: TextDocumentState) -> &'static str {
    match state {
        TextDocumentState::Clean => "clean",
        TextDocumentState::Dirty => "dirty",
        TextDocumentState::ExternalChanged => "external changed",
        TextDocumentState::Conflict => "conflict",
        TextDocumentState::SaveError => "save error",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectContentError {
    NoActiveProject,
    NoActiveDocument,
    Explorer(ExplorerScanError),
    Open(TextDocumentOpenError),
    Edit(TextDocumentEditError),
    Save(TextDocumentSaveError),
    Refresh(TextDocumentRefreshError),
    /// RFC-065 D4: the open set's own bound, reached. A path already open still switches
    /// (the dedup check runs first), so this is only reachable by opening a path that is
    /// genuinely new while the set is already full.
    OpenSetAtLimit {
        open: u32,
        limit: u32,
    },
}

impl fmt::Display for ProjectContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoActiveProject => write!(formatter, "no active project"),
            Self::NoActiveDocument => write!(formatter, "no active text document"),
            Self::Explorer(error) => write!(formatter, "{error}"),
            Self::Open(error) => write!(formatter, "{error}"),
            Self::Edit(error) => write!(formatter, "{error}"),
            Self::Save(error) => write!(formatter, "{error}"),
            Self::Refresh(error) => write!(formatter, "{error}"),
            Self::OpenSetAtLimit { open, limit } => write!(
                formatter,
                "too many documents are open to open another: {open} are open, limit is {limit}"
            ),
        }
    }
}

impl std::error::Error for ProjectContentError {}
