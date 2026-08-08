use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceTrust {
    Unknown,
    Restricted,
    Trusted,
    Revoked,
}

impl WorkspaceTrust {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Restricted => "Restricted",
            Self::Trusted => "Trusted",
            Self::Revoked => "Revoked",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectOpenSurface {
    ProjectDashboard,
    TextEditor,
    GitStatus,
    AgentRunDetail,
    DiffReview,
    HandoffReport,
    TrustSettings,
}

impl ProjectOpenSurface {
    pub fn label(self) -> &'static str {
        match self {
            Self::ProjectDashboard => "Project Dashboard",
            Self::TextEditor => "Text Editor",
            Self::GitStatus => "Git Status",
            Self::AgentRunDetail => "AgentRun Detail",
            Self::DiffReview => "Diff Review",
            Self::HandoffReport => "Handoff Report",
            Self::TrustSettings => "Trust Settings",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectMode {
    Content,
    TerminalImmersion,
}

impl ProjectMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Content => "Content Mode",
            Self::TerminalImmersion => "Terminal / Agent Immersion Mode",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Content => Self::TerminalImmersion,
            Self::TerminalImmersion => Self::Content,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectResourceLimits {
    pub visible_terminal_limit: Option<u32>,
    pub terminal_session_limit: Option<u32>,
    pub agent_run_limit: Option<u32>,
    pub approval_request_limit: Option<u32>,
}

impl Default for ProjectResourceLimits {
    fn default() -> Self {
        Self {
            visible_terminal_limit: Some(2),
            // Terminal launch UX handoff: previously `None`, i.e.
            // unenforced -- harmless while the only launch path was an
            // env-gated demo that always stopped at three, but a real
            // keybinding a user can hold down would spawn unbounded real
            // shell processes without this.
            //
            // This is a function of poll cost, not process count. Every
            // live session (visible or hidden) is polled sequentially each
            // tick by `Message::TerminalPollTick`, and each `poll()` call
            // carries `read_available_bounded_for`'s hardcoded 10ms
            // `WouldBlock` sleep -- measured linear at ~10.1ms/pane against
            // a 50ms tick period. 5 panes (~50.3ms) saturates the tick; 4
            // (~40.3ms) leaves ~10ms for everything else in the handler,
            // which is tight. 3 (~30.2ms) leaves ~20ms of headroom, and
            // matches the pane count this project already shipped and
            // exercised via the diagnostic demo path before this handoff.
            // Revisit this number deliberately -- not by raising it in
            // isolation -- once readiness-driven terminal I/O
            // (`rfcs/future-work.md` §Readiness-driven terminal I/O)
            // removes the per-poll sleep this bound is defending against.
            terminal_session_limit: Some(3),
            agent_run_limit: None,
            approval_request_limit: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectProviderState {
    Complete,
    Unavailable,
    NotImplemented,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectFileState {
    pub provider_state: ProjectProviderState,
    pub open_buffer_count: u32,
    pub dirty_file_count: u32,
    pub active_path_hint: Option<PathBuf>,
}

impl ProjectFileState {
    pub fn dirty_status(&self) -> ProjectMetadataCountStatus {
        match self.provider_state {
            ProjectProviderState::Complete => {
                ProjectMetadataCountStatus::Known(self.dirty_file_count)
            }
            ProjectProviderState::Unavailable => ProjectMetadataCountStatus::Unavailable,
            ProjectProviderState::NotImplemented => ProjectMetadataCountStatus::NotImplemented,
            ProjectProviderState::Unknown => ProjectMetadataCountStatus::Unknown,
        }
    }
}

impl Default for ProjectFileState {
    fn default() -> Self {
        Self {
            provider_state: ProjectProviderState::NotImplemented,
            open_buffer_count: 0,
            dirty_file_count: 0,
            active_path_hint: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectGitSummary {
    pub provider_state: ProjectProviderState,
    pub branch_name: Option<String>,
    pub changed_file_count: Option<u32>,
    pub ahead_count: Option<u32>,
    pub behind_count: Option<u32>,
}

impl ProjectGitSummary {
    pub fn display_status(&self) -> ProjectGitDisplayStatus {
        match self.provider_state {
            ProjectProviderState::Complete => ProjectGitDisplayStatus::Known {
                branch_name: self.branch_name.clone(),
                changed_file_count: self.changed_file_count,
                ahead_count: self.ahead_count,
                behind_count: self.behind_count,
            },
            ProjectProviderState::Unavailable => ProjectGitDisplayStatus::Unavailable,
            ProjectProviderState::NotImplemented => ProjectGitDisplayStatus::NotImplemented,
            ProjectProviderState::Unknown => ProjectGitDisplayStatus::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectMetadataCountStatus {
    Known(u32),
    Unavailable,
    NotImplemented,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectGitDisplayStatus {
    Known {
        branch_name: Option<String>,
        changed_file_count: Option<u32>,
        ahead_count: Option<u32>,
        behind_count: Option<u32>,
    },
    Unavailable,
    NotImplemented,
    Unknown,
}

impl Default for ProjectGitSummary {
    fn default() -> Self {
        Self {
            provider_state: ProjectProviderState::NotImplemented,
            branch_name: None,
            changed_file_count: None,
            ahead_count: None,
            behind_count: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectWarning {
    pub level: ProjectWarningLevel,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectWarningLevel {
    Info,
    Warning,
    Risk,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectWarningState {
    pub warnings: Vec<ProjectWarning>,
}

impl ProjectWarningState {
    pub fn has_risk_warning(&self) -> bool {
        self.warnings
            .iter()
            .any(|warning| warning.level == ProjectWarningLevel::Risk)
    }
}
