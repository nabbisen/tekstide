use crate::close::CloseResourceSummary;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRuntimeSummary {
    pub risk_warning: bool,
    pub pending_approvals: u32,
    pub review_ready_changes: u32,
    pub failed_processes: u32,
    pub running_processes: u32,
    pub dirty_files: u32,
    pub terminal_count: Option<u32>,
    pub agent_run_count: Option<u32>,
    pub close_resources: CloseResourceSummary,
}

impl Default for ProjectRuntimeSummary {
    fn default() -> Self {
        Self {
            risk_warning: false,
            pending_approvals: 0,
            review_ready_changes: 0,
            failed_processes: 0,
            running_processes: 0,
            dirty_files: 0,
            terminal_count: None,
            agent_run_count: None,
            close_resources: CloseResourceSummary::provider_missing(),
        }
    }
}
