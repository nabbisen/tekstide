#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppRoute {
    #[default]
    ProjectBoard,
    ActiveProjectWorkspace,
    /// RFC-059 D7: a top-level destination, a sibling of the other two
    /// rather than a `ProjectOpenSurface` -- the audit store is app-wide
    /// and must stay viewable with no project open, which no
    /// `ProjectOpenSurface` can do (every one of those requires
    /// `AppState::active_project_mut()` to be `Some`).
    AuditView,
}
