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
pub mod keyboard_help;
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
    handle_informational_flags();
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

/// `-h`/`--help`/`-V`/`--version`, handled before any window opens.
///
/// Until `0.12.1` every argument was treated as a project path, so
/// `tekstide --help` printed `folder does not exist: --help` -- the
/// product's only documented entry point rejecting the universal request
/// for documentation. Nothing else about argument handling changes: an
/// unrecognised `-`-prefixed argument is still passed through as a path,
/// because a real file really can begin with a dash and silently
/// refusing one would trade this defect for a subtler one.
fn handle_informational_flags() {
    let flags: Vec<String> = std::env::args().skip(1).collect();
    let wants_help = flags.iter().any(|arg| arg == "-h" || arg == "--help");
    let wants_version = flags.iter().any(|arg| arg == "-V" || arg == "--version");
    if !wants_help && !wants_version {
        return;
    }

    if wants_version {
        println!("tekstide {}", env!("CARGO_PKG_VERSION"));
    }
    if wants_help {
        let catalog =
            i18n::Catalog::resolve(i18n::LocalePreference::default(), Some(&locales_dir()));
        let executable = std::env::args()
            .next()
            .unwrap_or_else(|| "tekstide".to_owned());
        print!("{}", keyboard_help::usage_text(&catalog, &executable));
    }
    std::process::exit(0);
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
        if let Err(error) = open_cli_project_path_and_record(&mut app_shell, selected_path) {
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

/// RFC-031 PR-031-B: the real, testable open-a-project-from-the-CLI
/// path -- factored out of `boot()`'s loop body (which reads real
/// `std::env::args_os()` and cannot be driven from a test) the same
/// testability-split shape `attempt_agent_run_launch_with_profile_and_state_root`
/// established in `shell.rs`: the exact logic a real CLI argument
/// reaches, callable directly with a controlled path.
///
/// `Added` means `add_project_session` genuinely created a new
/// `ProjectSession` this run -- reusing a remembered `project_id` from
/// `recent_projects` still counts (a real new session exists that did
/// not a moment ago); `FocusedExisting` means nothing new happened (the
/// canonical root already matched a session already open this run) and
/// must not produce a second record for the same open.
/// `restore_recent_projects` (called before this, in `boot()`) never
/// reaches `add_project_session` at all -- it only populates the
/// passive remembered-projects list -- so "restoring on startup" cannot
/// itself fire this producer; only this CLI-argument path can, in the
/// shipped application today.
fn open_cli_project_path_and_record(
    app_shell: &mut ApplicationShell,
    selected_path: impl AsRef<std::path::Path>,
) -> Result<(), tekstide_core::project::root::ProjectRootValidationError> {
    match app_shell.add_project_from_path(selected_path)? {
        tekstide_core::app::AddProjectOutcome::Added(project_id) => {
            record_project_added_if_possible(app_shell, project_id);
        }
        tekstide_core::app::AddProjectOutcome::FocusedExisting(_) => {}
    }
    Ok(())
}

/// RFC-031 PR-031-B: best-effort, matching every other producer call
/// site in this crate (`record_paste_blocked`'s own) -- a failed audit
/// write must never turn a real, already-successful project add into a
/// boot failure. `shell::open_real_audit_store` is the one existing
/// resolution this crate already has for the real audit store, widened
/// from module-private to `pub(crate)` so `boot()` -- the only
/// production caller of `add_project_from_path` in the shipped
/// application -- can reach it, rather than inventing a second
/// resolution here.
fn record_project_added_if_possible(
    app_shell: &ApplicationShell,
    project_id: tekstide_core::project::ProjectId,
) {
    let mut audit_store = shell::open_real_audit_store(app_shell);
    let mut audit_health = tekstide_core::audit::AuditHealth::default();
    if let Some(store) = audit_store.as_mut() {
        let _ = tekstide_core::audit::AuditCoordinator::new(store, &mut audit_health)
            .record_project_added(project_id);
    }
}

fn locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

#[cfg(test)]
mod tests;
