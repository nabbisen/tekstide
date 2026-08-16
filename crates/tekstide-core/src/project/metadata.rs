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
    /// RFC-022 PR-022-E ("the arrival model"), response 224: **per
    /// project**, matching this struct's other two limits and its own
    /// name -- not per `AgentRun`, which response 224 corrected after I
    /// initially proposed reading it that way (the same shape of defect
    /// response 216 found in PR-022-C: reusing a field for a different
    /// scope than its container because the name happened to fit).
    /// Bounds how many of a project's approval requests
    /// `ApprovalCoordinator` will hold **live** (`Pending`, connection
    /// still open) at once across all of that project's `AgentRun`s.
    /// The justification is `AcceptedProposal` holding a live
    /// `UnixStream` per pending entry (response 224): the real ceiling
    /// this protects is process file-descriptor exhaustion, which does
    /// not degrade approvals gracefully -- it takes down PTYs, the audit
    /// store, and transcript writers with it, since they all draw on the
    /// same per-process limit.
    ///
    /// **Response 225: this field must not also bound retained
    /// history** (`ProjectSession.approval_requests`'s total length,
    /// including decided and expired entries) -- an expired entry holds
    /// no file descriptor, so the fd rationale that justifies this
    /// field's value does not apply to that quantity. That is a
    /// different cost (memory, and how much history a user can usefully
    /// review) with its own field: [`ProjectResourceLimits::approval_history_limit`].
    /// Reusing this one for both was the third occurrence of the same
    /// defect shape this session found (PR-022-C's state-root reuse,
    /// this field's own original per-run misreading, then this) --
    /// a field's name and its current value travel together; the
    /// *reason* the value was chosen does not travel automatically to a
    /// new use, and has to be checked every time, not assumed.
    pub approval_request_limit: Option<u32>,
    /// RFC-022 PR-022-E ("the arrival model"), response 224: the
    /// genuinely per-`AgentRun` bound the gate asks for ("a looping
    /// adapter must exhaust its own budget, not starve another agent's
    /// proposals") -- a distinct field from `approval_request_limit`
    /// rather than reusing it, since the two have different scopes and
    /// this struct's own convention is one field per scope, not one
    /// field meaning something different from its neighbours.
    pub agent_run_approval_limit: Option<u32>,
    /// RFC-022 PR-022-E ("the arrival model"), response 225: how many
    /// `ApprovalRequest`s `ProjectSession.approval_requests` retains in
    /// total (decided and expired entries included, not only live
    /// ones) -- a **separate** bound from `approval_request_limit`
    /// above, added after response 225 found the two fields were
    /// wrongly sharing one. The reason for *this* number is disclosure
    /// and memory, not file descriptors: an expired or decided entry
    /// costs nothing but the small `ApprovalRequest` struct itself, and
    /// the real question is how much of a project's approval history is
    /// worth keeping in memory for a user to review, not what would
    /// exhaust a process resource. Eviction at this bound is real
    /// disclosure loss (response 225) -- whatever surface renders
    /// `approval_requests()` must say it is showing the most recent N,
    /// not imply the list is complete.
    pub approval_history_limit: Option<u32>,
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
            // RFC-017 Amendment 1, PR-A1-D: raised from `Some(3)`, and
            // re-derived from scratch, not by assumption. The old number
            // was a function of `read_available_bounded_for`'s 10ms
            // `WouldBlock` sleep against a shared 50ms tick -- both gone
            // since PR-A1-A/C. There is no shared tick period to divide
            // by any more: each pane's wake fires independently, and the
            // real constraint is whether `iced`'s single-threaded
            // `update()` can keep servicing every pane's wakes as more
            // panes flood concurrently, not a per-tick budget.
            //
            // Measured headlessly (`terminal_session_limit_headless_n_pane_wake_throughput_benchmark`,
            // `crates/tekstide/src/shell/tests.rs`, deliberately not the
            // live GUI -- this slice found the live path unreliable on a
            // shared, swap-pressured machine, see `qa-evidence.md`'s
            // PR-A1-D section): N real panes, each running an
            // un-fork-bound flood concurrently, drained by one
            // single-threaded round-robin loop (the same shape
            // `update()` imposes in production, since every pane's wake
            // funnels through that one thread regardless of pane count).
            // N=1/3/6 all measured with poll() cost at low single-digit
            // microseconds and aggregate throughput scaling linearly with
            // N (~17MB/s/pane, matching the flood's own standalone rate).
            // Degradation first became measurable at N=8 (poll cost
            // jumped roughly 10x to ~20µs, though throughput was still
            // linear) and was unambiguous at N=10 (poll cost ~130µs+,
            // aggregate throughput falling meaningfully below linear
            // scaling -- the reader genuinely falling behind, not just
            // costing more per call).
            //
            // `6`, not `8`, keeps the same margin-below-first-measurable-
            // degradation philosophy the old `Some(3)` used (headroom
            // below its own ~5-pane saturation point) -- this time backed
            // by real measured headroom rather than the sleep-imposed
            // one it replaces. Revisit deliberately, from a fresh
            // measurement, not by raising it in isolation, if the
            // underlying wake mechanism changes again.
            terminal_session_limit: Some(6),
            agent_run_limit: None,
            // RFC-022 PR-022-E, response 225: grounded against a real
            // `ulimit -n` check, not left as a bare assumption -- the
            // reference dev machine reports a *soft and hard*
            // `RLIMIT_NOFILE` of 1,048,576 (modern systemd-raised
            // default), not the historically-cited "1024 on most
            // distributions" this comment originally claimed without
            // checking. That number is real for *this* machine and not
            // safe to generalize from -- containers and minimal distros
            // commonly still cap at 1024 or lower, and Tekstide has no
            // way to discover the limit its own process is actually
            // running under. `50` is chosen against the *lower,
            // constrained* end of that range, not the measured one: a
            // rough, reasoned baseline (not a live instrumented count,
            // since fd usage here is cheap enough that precision would
            // be theater) puts Tekstide's own steady-state usage at
            // roughly 20-30 fds under typical defaults --
            // `terminal_session_limit`'s own `6` sessions each holding a
            // PTY master plus two `eventfd`s (`runtime::terminal::reader`)
            // is already ~18, plus a handful for the audit store's
            // WAL-mode sqlite files and any open transcript writers.
            // Adding this bound's `50` brings the project comfortably
            // under 100 total, under 10% of even the constrained 1024
            // floor -- conservative enough that further precision would
            // not change the decision. Unlike `terminal_session_limit`
            // above, this is reasoned, not benchmarked: there is no
            // throughput to measure, only a ceiling to stay well clear
            // of.
            approval_request_limit: Some(50),
            // A looping or malfunctioning adapter must exhaust its own
            // budget well before it could meaningfully contribute to the
            // project-wide ceiling above -- `20` leaves room for several
            // agent runs to each hold a real, if unusually large, burst
            // without any single one being able to consume the whole
            // project's budget alone.
            agent_run_approval_limit: Some(20),
            // RFC-022 PR-022-E, response 225: a genuinely different
            // number from `approval_request_limit` above, for a
            // genuinely different reason -- this bounds retained
            // *history* (memory and how much a user can usefully
            // review), not live file descriptors, so it is not derived
            // from the same fd-exhaustion reasoning at all. `100` is a
            // round, generous multiple of the live-queue ceiling,
            // chosen so a legitimately busy session's full approval
            // history stays reviewable without unbounded growth --
            // reasoned, not measured, the same as the field above, but
            // against a different constraint (usefulness to a reader,
            // not a process resource).
            approval_history_limit: Some(100),
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
