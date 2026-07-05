#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppRoute {
    #[default]
    ProjectBoard,
    ActiveProjectWorkspace,
}
