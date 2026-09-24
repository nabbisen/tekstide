use std::fmt;
use std::path::{Path, PathBuf};

use crate::content::{
    ExternalChangeDecision, SaveDecision, TextCursor, TextDocument, TextDocumentEditError,
    TextDocumentOpenError, TextDocumentOpenPolicy, TextDocumentRefreshError, TextDocumentSaveError,
    TextDocumentState,
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
    active_document: Option<TextDocument>,
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
        self.active_document.as_ref()
    }

    pub fn active_file_launch_assessment(&self) -> ProjectActiveFileLaunchAssessment {
        let Some(document) = self.active_document.as_ref() else {
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
    ) -> Result<(), ProjectContentError> {
        let selected_relative_path = selected_relative_path.as_ref().to_path_buf();

        match TextDocument::open(root, &selected_relative_path, policy) {
            Ok(document) => {
                self.selected_explorer_path = selected_relative_path;
                self.active_document = Some(document);
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
        let Some(document) = self.active_document.as_mut() else {
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
        let Some(document) = self.active_document.as_mut() else {
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
        let Some(document) = self.active_document.as_mut() else {
            return Err(ProjectContentError::NoActiveDocument);
        };
        document.set_cursor(cursor);
        Ok(())
    }

    pub fn refresh_active_document(
        &mut self,
        root: &ProjectRootHandle,
        policy: TextDocumentOpenPolicy,
    ) -> Result<ExternalChangeDecision, ProjectContentError> {
        let Some(document) = self.active_document.as_mut() else {
            self.status = ProjectContentStatus::RefreshError {
                message: "no active text document".to_owned(),
            };
            return Err(ProjectContentError::NoActiveDocument);
        };

        match document.refresh_external_state(root, policy) {
            Ok(decision) => {
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
                    ExternalChangeDecision::ExternalChanged => {
                        ProjectContentStatus::ExternalChanged
                    }
                    ExternalChangeDecision::Conflict => ProjectContentStatus::Conflict,
                };
                Ok(decision)
            }
            Err(error) => {
                self.status = ProjectContentStatus::RefreshError {
                    message: error.to_string(),
                };
                Err(ProjectContentError::Refresh(error))
            }
        }
    }

    pub fn open_buffer_count(&self) -> u32 {
        u32::from(self.active_document.is_some())
    }

    pub fn dirty_file_count(&self) -> u32 {
        u32::from(
            self.active_document
                .as_ref()
                .is_some_and(TextDocument::is_dirty),
        )
    }

    pub fn active_path_hint(&self) -> Option<PathBuf> {
        self.active_document
            .as_ref()
            .map(|document| document.target().selected_relative_path.clone())
    }
}

impl Default for ProjectContentWorkspace {
    fn default() -> Self {
        Self {
            selected_explorer_path: PathBuf::new(),
            explorer_tree: ExplorerTree::default(),
            explorer_status: ProjectExplorerStatus::Empty,
            active_document: None,
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
    Saved { decision: SaveDecision },
    ExternalChanged,
    Conflict,
    OpenError { message: String },
    EditError { message: String },
    SaveError { message: String },
    RefreshError { message: String },
}

impl ProjectContentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Opened => "open",
            Self::Edited => "edited",
            Self::Saved { .. } => "saved",
            Self::ExternalChanged => "external changed",
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
        }
    }
}

impl std::error::Error for ProjectContentError {}
