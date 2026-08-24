use crate::close::CloseResourceSummary;

// RFC-039 PR-039-C, response 311: `#[derive(Default)]` now that
// `CloseResourceSummary` has its own `Default` (provider `Complete`) --
// every other field here already derives to the right value (`false`,
// `0`, `None`). See `CloseResourceSummary`'s own `Default` impl for why
// a freshly constructed project's resource state is genuinely known,
// not exceptional (`provider_missing()`'s own role).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
