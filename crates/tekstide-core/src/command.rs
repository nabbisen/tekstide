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
}
