use crate::app::AppState;
use crate::project::recent::{RecentProjectAvailability, RestoredRecentProject};
use crate::project::{ProjectId, ProjectRuntimeSummary, ProjectSession};
use crate::security::RestrictedModeSummary;

/// `NotImplemented` and `Unknown` answer different questions and must not
/// be conflated: `NotImplemented` claims the feature does not exist;
/// `Unknown` says the feature exists but nothing has counted it yet (a
/// fresh session before its first collection mutation, or a recent
/// project that has never been opened). Overloading one `Option::None`
/// to mean both was the defect the status-mapping-honesty-fixes handoff
/// found live in `0.7.0` -- a freshly opened project claimed terminals
/// were unimplemented, and the same line silently became a real count
/// the moment one was launched.
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
    pub security_mode_label: String,
    pub restricted_mode: bool,
    pub blocked_automation_count: u32,
    pub blocked_automation_labels: Vec<String>,
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

/// RFC-038 PR-038-E: `primary_action`/`secondary_action` (`"Add Project"`/
/// `"Open from path"`) were removed here -- pre-baked English naming two
/// actions that were never reachable from anywhere, the exact defect
/// `0.12.1` shipped and RFC-038's own path field/browser/one-key-reopen
/// slices existed to fix on the GUI side. **A breaking change to a
/// published crate** (see `CHANGELOG.md`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBoardEmptyState {
    pub heading: String,
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
    let security_summary = RestrictedModeSummary::from_trust(project.trust_state());

    ProjectBoardRow {
        project_id: project.id().clone(),
        display_name: project.display_name().to_owned(),
        root_path_hint: project.root_path().display().to_string(),
        secondary_path_hint: (project.root_path() != project.canonical_root_path())
            .then(|| project.canonical_root_path().display().to_string()),
        availability_label: None,
        trust_label: project.trust_state().label().to_owned(),
        security_mode_label: security_summary.mode_label.to_owned(),
        restricted_mode: security_summary.restricted_mode,
        blocked_automation_count: len_as_u32(security_summary.blocked_features.len()),
        blocked_automation_labels: security_summary
            .blocked_feature_labels()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        branch_status: CountDisplay::Unavailable,
        // `Unknown`, not `NotImplemented`: `terminal_count`/`agent_run_count`
        // are `None` only until `refresh_runtime_summary_from_collections`
        // first runs (session.rs) -- a freshly opened project that has not
        // yet added a terminal or agent run, not a project whose terminals
        // do not exist. See `CountDisplay`'s own doc comment.
        terminal_count: runtime_summary
            .terminal_count
            .map(CountDisplay::KnownCount)
            .unwrap_or(CountDisplay::Unknown),
        agent_run_count: runtime_summary
            .agent_run_count
            .map(CountDisplay::KnownCount)
            .unwrap_or(CountDisplay::Unknown),
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

    // RFC-032: was hardcoded `WorkspaceTrust::Restricted` regardless of
    // what `recent_project.trust_state` (PR-032-B) actually held --
    // flagged in that slice's own evidence as a real, separate gap
    // deliberately left for this one. Reads the real cached value,
    // **except when `availability` is already `PathChanged`**: that
    // means the canonical path this entry was saved against no longer
    // matches what it resolves to now, so `AppState::add_project_session`'s
    // own canonical-path-keyed lookup (PR-032-B) would *not* carry the
    // cached trust over on reopen -- showing it here would claim a grant
    // reopening will not honour.
    //
    // **Disclosed, not fixed**: this is still the *cached* value, not
    // one confirmed against the durable audit store the way
    // `verify_restored_trust` confirms an already-*open* project's trust
    // (PR-032-C, response 245/246) -- that confirmation only runs once a
    // project is actually reopened. A recent-but-unopened row's label is
    // a last-known snapshot, the same status every other recent-only
    // field here already is (`branch_status`, the `Unknown` counts
    // below), not a live, audit-verified fact.
    let trust_state = match restored.availability {
        RecentProjectAvailability::PathChanged => crate::project::WorkspaceTrust::Restricted,
        _ => recent_project.trust_state,
    };
    let security_summary = RestrictedModeSummary::from_trust(trust_state);

    ProjectBoardRow {
        project_id: recent_project.project_id.clone(),
        display_name: recent_project.display_name.clone(),
        root_path_hint: recent_project.root_path.display().to_string(),
        secondary_path_hint: (recent_project.root_path != recent_project.canonical_root_path)
            .then(|| recent_project.canonical_root_path.display().to_string()),
        availability_label,
        trust_label: trust_state.label().to_owned(),
        security_mode_label: security_summary.mode_label.to_owned(),
        restricted_mode: security_summary.restricted_mode,
        blocked_automation_count: len_as_u32(security_summary.blocked_features.len()),
        blocked_automation_labels: security_summary
            .blocked_feature_labels()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        branch_status: CountDisplay::Unavailable,
        // Same fix as `active_project_row`, not a disclosed limitation:
        // a recent-but-unopened project has no `ProjectSession` to count
        // from, which is "nothing has happened yet" -- the same shape as
        // a freshly opened project before its first collection mutation,
        // not a claim that any of these five features do not exist.
        terminal_count: CountDisplay::Unknown,
        agent_run_count: CountDisplay::Unknown,
        approval_count: CountDisplay::Unknown,
        review_count: CountDisplay::Unknown,
        dirty_file_count: CountDisplay::Unknown,
        attention: AttentionState::Calm,
        attention_label: AttentionState::Calm.label().to_owned(),
        row_kind,
    }
}

fn len_as_u32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
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
