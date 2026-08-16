use crate::project::ProjectOpenSurface;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppCommand {
    OpenProjectBoard,
    OpenActiveProjectWorkspace,
    ToggleActiveProjectMode,
    OpenActiveProjectSurface(ProjectOpenSurface),
    /// Terminal launch UX handoff: the route/mode half of opening a
    /// terminal -- lands the active project in `TerminalImmersion`
    /// regardless of which mode it was already in. The actual PTY spawn
    /// and session registration is real I/O and lives in the GUI crate's
    /// own `update`, dispatched alongside this command rather than
    /// inside it (`tekstide-core` has no I/O to spawn a process with).
    LaunchTerminal,
    /// RFC-022 PR-022-D: the route/mode half of starting an agent run --
    /// an AgentRun is a real PTY session the user should be able to see,
    /// so this reuses the same `TerminalImmersion` landing as
    /// `LaunchTerminal` rather than inventing a separate route. Distinct
    /// from `OpenActiveProjectSurface(ProjectOpenSurface::AgentRunDetail)`,
    /// which is the (separate, future) richer detail/report view for an
    /// already-running run -- this command is only about landing on the
    /// live session right after starting it.
    LaunchAgentRun,
}
