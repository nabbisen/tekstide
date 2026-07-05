use crate::project::ProjectOpenSurface;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppCommand {
    OpenProjectBoard,
    OpenActiveProjectWorkspace,
    ToggleActiveProjectMode,
    OpenActiveProjectSurface(ProjectOpenSurface),
}
