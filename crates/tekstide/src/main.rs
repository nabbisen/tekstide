// RFC-015 PR-015-B: `crates/tekstide` becomes a real `iced` application.
// The RFC-016 PR-016-B text harness this module used to be (printing
// `shell.render_text()` and exiting) is gone -- `i18n::Catalog` now has
// its real caller, and `shell`/`theme` hold the layer composition,
// chrome, and theme/i18n seams `implementation-handoff.md` describes.
// `pub`, not `#[allow(dead_code)]` (response 122 Required 3 precedent):
// letting the dead-code lint keep working is more useful than a blanket
// suppression, including for whatever in these modules is genuinely
// unused later.
pub mod i18n;
pub mod input;
pub mod measurement;
mod shell;
mod surface;
mod theme;

use std::path::{Path, PathBuf};

use tekstide_core::project::recent::{AppStatePathProvider, RecentProjectStore};
use tekstide_core::shell::ApplicationShell;

fn main() -> iced::Result {
    // Must be the very first statement -- see `measurement`'s module doc
    // on why this is the only honest definition of "process start."
    measurement::mark_process_start();
    iced::application(boot, shell::update, timed_view)
        .title(shell::State::window_title)
        .subscription(shell::subscription)
        .run()
}

/// Wraps `shell::view` with view-build-cost timing (RFC-015 PR-015-F,
/// discharging R1's typing-latency half without `iced::window::frames()`
/// -- see `measurement`'s module doc; extended to `ModeSwitch` in
/// PR-015-E for C4, the same decomposition). `shell::view` itself takes
/// no timing dependency; this wrapper is the one and only place the
/// timing happens, kept out of `shell.rs` entirely so the reviewed
/// layer-composition/routing code is untouched by either slice.
fn timed_view(state: &shell::State) -> iced::Element<'_, shell::Message> {
    if state.is_measuring_view_cost() {
        let start = std::time::Instant::now();
        let element = shell::view(state);
        measurement::record_view_cost(start.elapsed());
        element
    } else {
        shell::view(state)
    }
}

/// `iced::application`'s boot function: called once, with no arguments,
/// so all of this crate's process bootstrapping (recent-project restore,
/// CLI project-path arguments, catalog resolution) has to happen inside
/// it rather than in `main` beforehand -- `iced`'s `BootFn` requires
/// `Fn() -> State`, not `FnOnce`, which rules out constructing state in
/// `main` and moving it in.
fn boot() -> shell::State {
    let mut app_shell = ApplicationShell::new();

    let store = match AppStatePathProvider::linux_default() {
        Ok(path_provider) => Some(RecentProjectStore::new(path_provider)),
        Err(error) => {
            eprintln!("{error}");
            None
        }
    };

    if let Some(store) = &store {
        match store.load() {
            Ok(recent_project_state) => app_shell.restore_recent_projects(recent_project_state),
            Err(error) => eprintln!("{error}"),
        }
    }

    // No window has opened yet at this point, so exiting on an invalid
    // CLI project path preserves the pre-GUI harness's exact behaviour
    // (abort rather than boot with a partially-applied argument list)
    // without needing `iced::Result`/`iced::Error` to express a custom
    // exit code.
    for selected_path in std::env::args_os().skip(1) {
        if let Err(error) = app_shell.add_project_from_path(selected_path) {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }

    if let Some(store) = &store
        && let Err(error) = store.save(&app_shell.recent_project_state())
    {
        eprintln!("{error}");
        std::process::exit(1);
    }

    let catalog = i18n::Catalog::resolve(i18n::LocalePreference::default(), Some(&locales_dir()));
    shell::State::new(app_shell, catalog)
}

fn locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}
