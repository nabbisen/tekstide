// RFC-016 PR-016-B: no caller yet. RFC-015 (the shell that would render
// localized chrome through `i18n::Catalog`) has not been implemented --
// `main.rs` below predates it and only dumps `ApplicationShell`'s plain-
// text debug rendering, which is not RFC-015's rendered surface and is
// not in scope for localization. Matching PR-016-C's `text_safety`
// precedent: proven by this module's own tests, not by a fabricated call
// site (`quote_untrusted` had the identical "no re-review, do not stub
// anything" ruling in response 118 Q1).
//
// `pub`, not a blanket `#[allow(dead_code)]` (response 122 Required 3):
// a module-level allow suppresses the lint for everything added here
// from now on, including code that becomes genuinely dead later --
// exactly where a headless module most needs the compiler's help.
// Making the module `pub` lets the lint keep working for the right
// reason instead.
pub mod i18n;

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
