use tekstide_core::shell::ApplicationShell;

fn main() -> std::process::ExitCode {
    let mut shell = ApplicationShell::new();
    let store = match tekstide_core::project::recent::AppStatePathProvider::linux_default() {
        Ok(path_provider) => Some(tekstide_core::project::recent::RecentProjectStore::new(
            path_provider,
        )),
        Err(error) => {
            eprintln!("{error}");
            None
        }
    };

    if let Some(store) = &store {
        match store.load() {
            Ok(recent_project_state) => shell.restore_recent_projects(recent_project_state),
            Err(error) => {
                eprintln!("{error}");
            }
        }
    }

    for selected_path in std::env::args_os().skip(1) {
        if let Err(error) = shell.add_project_from_path(selected_path) {
            eprintln!("{error}");
            return std::process::ExitCode::FAILURE;
        }
    }

    if let Some(store) = &store
        && let Err(error) = store.save(&shell.recent_project_state())
    {
        eprintln!("{error}");
        return std::process::ExitCode::FAILURE;
    }

    print!("{}", shell.render_text());
    std::process::ExitCode::SUCCESS
}
