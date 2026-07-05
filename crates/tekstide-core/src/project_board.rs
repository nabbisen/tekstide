use crate::app::AppState;
use crate::project::recent::{RecentProjectAvailability, RestoredRecentProject};
use crate::project::{ProjectId, ProjectRuntimeSummary, ProjectSession};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CountDisplay {
    KnownCount(u32),
    Unavailable,
    NotImplemented,
    Unknown,
}

impl CountDisplay {
    pub fn label(self) -> String {
        match self {
            Self::KnownCount(count) => count.to_string(),
            Self::Unavailable => "not available".to_owned(),
            Self::NotImplemented => "not implemented".to_owned(),
            Self::Unknown => "unknown".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttentionState {
    Risk,
    ApprovalNeeded,
    Review,
    Failed,
    Running,
    Dirty,
    Calm,
}

impl AttentionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Risk => "Risk",
            Self::ApprovalNeeded => "Approval needed",
            Self::Review => "Review",
            Self::Failed => "Failed",
            Self::Running => "Running",
            Self::Dirty => "Dirty",
            Self::Calm => "Calm",
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Risk => 0,
            Self::ApprovalNeeded => 1,
            Self::Review => 2,
            Self::Failed => 3,
            Self::Running => 4,
            Self::Dirty => 5,
            Self::Calm => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardRowKind {
    ActiveSession,
    RecentAvailable,
    RecentMissing,
    RecentUnreadable,
    RecentPathChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBoardRow {
    pub project_id: ProjectId,
    pub display_name: String,
    pub root_path_hint: String,
    pub secondary_path_hint: Option<String>,
    pub availability_label: Option<String>,
    pub trust_label: String,
    pub branch_status: CountDisplay,
    pub terminal_count: CountDisplay,
    pub agent_run_count: CountDisplay,
    pub approval_count: CountDisplay,
    pub review_count: CountDisplay,
    pub dirty_file_count: CountDisplay,
    pub attention: AttentionState,
    pub attention_label: String,
    pub row_kind: BoardRowKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBoardViewModel {
    pub rows: Vec<ProjectBoardRow>,
    pub active_project_id: Option<ProjectId>,
    pub empty_state: Option<ProjectBoardEmptyState>,
    pub global_attention_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBoardEmptyState {
    pub heading: String,
    pub primary_action: String,
    pub secondary_action: String,
}

impl ProjectBoardViewModel {
    pub fn from_app_state(state: &AppState) -> Self {
        let mut rows = state
            .projects()
            .iter()
            .map(active_project_row)
            .collect::<Vec<_>>();

        for restored in state.recent_projects() {
            if rows
                .iter()
                .any(|row| row.project_id == restored.recent_project.project_id)
            {
                continue;
            }
            rows.push(recent_project_row(restored));
        }

        rows.sort_by(compare_rows);

        let empty_state = rows.is_empty().then(|| ProjectBoardEmptyState {
            heading: "No projects yet.".to_owned(),
            primary_action: "Add Project".to_owned(),
            secondary_action: "Open from path".to_owned(),
        });

        let global_attention_summary = rows
            .iter()
            .map(|row| row.attention)
            .min_by_key(|attention| attention.priority())
            .unwrap_or(AttentionState::Calm)
            .label()
            .to_owned();

        Self {
            rows,
            active_project_id: state.active_project_id().cloned(),
            empty_state,
            global_attention_summary,
        }
    }
}

pub fn calculate_attention(runtime_summary: &ProjectRuntimeSummary) -> AttentionState {
    if runtime_summary.risk_warning {
        AttentionState::Risk
    } else if runtime_summary.pending_approvals > 0 {
        AttentionState::ApprovalNeeded
    } else if runtime_summary.review_ready_changes > 0 {
        AttentionState::Review
    } else if runtime_summary.failed_processes > 0 {
        AttentionState::Failed
    } else if runtime_summary.running_processes > 0 {
        AttentionState::Running
    } else if runtime_summary.dirty_files > 0 {
        AttentionState::Dirty
    } else {
        AttentionState::Calm
    }
}

fn active_project_row(project: &ProjectSession) -> ProjectBoardRow {
    let runtime_summary = project.runtime_summary();
    let attention = calculate_attention(runtime_summary);

    ProjectBoardRow {
        project_id: project.id().clone(),
        display_name: project.display_name().to_owned(),
        root_path_hint: project.root_path().display().to_string(),
        secondary_path_hint: (project.root_path() != project.canonical_root_path())
            .then(|| project.canonical_root_path().display().to_string()),
        availability_label: None,
        trust_label: project.trust_state().label().to_owned(),
        branch_status: CountDisplay::Unavailable,
        terminal_count: runtime_summary
            .terminal_count
            .map(CountDisplay::KnownCount)
            .unwrap_or(CountDisplay::NotImplemented),
        agent_run_count: runtime_summary
            .agent_run_count
            .map(CountDisplay::KnownCount)
            .unwrap_or(CountDisplay::NotImplemented),
        approval_count: CountDisplay::KnownCount(runtime_summary.pending_approvals),
        review_count: CountDisplay::KnownCount(runtime_summary.review_ready_changes),
        dirty_file_count: CountDisplay::KnownCount(runtime_summary.dirty_files),
        attention,
        attention_label: attention.label().to_owned(),
        row_kind: BoardRowKind::ActiveSession,
    }
}

fn recent_project_row(restored: &RestoredRecentProject) -> ProjectBoardRow {
    let recent_project = &restored.recent_project;
    let availability_label = match restored.availability {
        RecentProjectAvailability::Available => None,
        RecentProjectAvailability::FolderMissing => Some("Folder missing".to_owned()),
        RecentProjectAvailability::CannotReadFolder => Some("Cannot read folder".to_owned()),
        RecentProjectAvailability::PathChanged => Some("Path changed".to_owned()),
    };

    let row_kind = match restored.availability {
        RecentProjectAvailability::Available => BoardRowKind::RecentAvailable,
        RecentProjectAvailability::FolderMissing => BoardRowKind::RecentMissing,
        RecentProjectAvailability::CannotReadFolder => BoardRowKind::RecentUnreadable,
        RecentProjectAvailability::PathChanged => BoardRowKind::RecentPathChanged,
    };

    ProjectBoardRow {
        project_id: recent_project.project_id.clone(),
        display_name: recent_project.display_name.clone(),
        root_path_hint: recent_project.root_path.display().to_string(),
        secondary_path_hint: (recent_project.root_path != recent_project.canonical_root_path)
            .then(|| recent_project.canonical_root_path.display().to_string()),
        availability_label,
        trust_label: "Restricted".to_owned(),
        branch_status: CountDisplay::Unavailable,
        terminal_count: CountDisplay::NotImplemented,
        agent_run_count: CountDisplay::NotImplemented,
        approval_count: CountDisplay::NotImplemented,
        review_count: CountDisplay::NotImplemented,
        dirty_file_count: CountDisplay::NotImplemented,
        attention: AttentionState::Calm,
        attention_label: AttentionState::Calm.label().to_owned(),
        row_kind,
    }
}

fn compare_rows(left: &ProjectBoardRow, right: &ProjectBoardRow) -> std::cmp::Ordering {
    left.attention
        .priority()
        .cmp(&right.attention.priority())
        .then_with(|| row_kind_priority(left.row_kind).cmp(&row_kind_priority(right.row_kind)))
        .then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        })
        .then_with(|| left.project_id.cmp(&right.project_id))
}

fn row_kind_priority(row_kind: BoardRowKind) -> u8 {
    match row_kind {
        BoardRowKind::ActiveSession => 0,
        BoardRowKind::RecentAvailable => 1,
        BoardRowKind::RecentMissing
        | BoardRowKind::RecentUnreadable
        | BoardRowKind::RecentPathChanged => 2,
    }
}

#[cfg(test)]
mod tests;
