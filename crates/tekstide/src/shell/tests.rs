use std::path::{Path, PathBuf};

use tekstide_core::shell::ApplicationShell;

use super::{Message, ModalButton, ModalContent, State, status_bar_summary};
use crate::i18n::{Catalog, LocalePreference};
use crate::input::FocusZone;

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn state_with(app_shell: ApplicationShell) -> State {
    let catalog = Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()));
    State::new(app_shell, catalog)
}

fn fresh_project_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tekstide-shell-test-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The window title comes from the catalog, not a literal -- if the key
/// name were ever mistyped, `Catalog::get`'s "missing key renders as the
/// key itself" fallback would make this assertion fail loudly rather
/// than silently rendering the wrong (but plausible-looking) string.
#[test]
fn window_title_resolves_through_the_catalog_not_a_literal() {
    let state = state_with(ApplicationShell::new());
    assert_eq!(state.window_title(), "Tekstide");
}

/// Fluent's automatic bidi isolation (First Strong Isolate / Pop
/// Directional Isolate, `use_isolating: true` by default -- already
/// documented and asserted in `i18n.rs`'s own tests) wraps each of
/// `status-bar-summary`'s two select-expression placeables, and the
/// inner `{$count}` placeable gets isolated a second time nested inside
/// the outer one -- exactly the accepted, harmless double isolation
/// response 125 ruled on for `CatalogArgs::untrusted`, here arising from
/// two adjacent select expressions instead. Expected literally rather
/// than stripped, matching `i18n::tests`' own convention of asserting
/// isolate marks explicitly so they are a documented property, not a
/// surprise the next reader has to rediscover from a failing comparison.
const ISOLATE_START: &str = "\u{2068}";
const ISOLATE_END: &str = "\u{2069}";

/// No projects, default route: the two-part summary (route label, then
/// a pluralized count) must show the Project Board route and the
/// zero-projects English plural form -- both resolved through real
/// shipped keys, not hardcoded shell text.
#[test]
fn status_bar_summary_reflects_the_default_route_and_zero_projects() {
    let state = state_with(ApplicationShell::new());
    assert_eq!(
        status_bar_summary(&state),
        format!(
            "{ISOLATE_START}Project Board{ISOLATE_END} | \
             {ISOLATE_START}{ISOLATE_START}0{ISOLATE_END} projects{ISOLATE_END}"
        )
    );
}

/// The English plural boundary this module's own key selection must
/// respect: exactly one project is "1 project," not "1 projects" --
/// proving `status_bar_summary` actually passes a genuine
/// `FluentValue::Number` through (plural-category selection), not a
/// pre-formatted string that happens to look right at zero.
#[test]
fn status_bar_summary_pluralizes_a_single_project_correctly() {
    let mut app_shell = ApplicationShell::new();
    let project_root = std::env::temp_dir().join(format!(
        "tekstide-shell-test-single-project-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&project_root).unwrap();
    app_shell
        .add_project_from_path(&project_root)
        .expect("a freshly created directory is a valid project root");

    let state = state_with(app_shell);
    assert_eq!(
        status_bar_summary(&state),
        format!(
            "{ISOLATE_START}Project Board{ISOLATE_END} | \
             {ISOLATE_START}{ISOLATE_START}1{ISOLATE_END} project{ISOLATE_END}"
        ),
        "exactly one project must use the singular form, not \"1 projects\""
    );
}

// --- PR-015-C: input routing and focus model --------------------------

/// `pr-015-c-input-routing.md`'s own required check: a stale (never
/// existed) `TerminalId` must be treated as not-live, not
/// best-effort-accepted.
///
/// **Only the negative path is testable from this crate today.** A
/// positive "genuinely live terminal" fixture needs `AppState::project_mut`
/// to attach a `TerminalSession`, and that method is `#[cfg(test)]`-gated
/// *inside `tekstide-core` itself* with zero production call sites yet
/// (terminal creation is RFC-017's job -- `grep`-confirmed: every
/// `add_terminal_session` caller in the tree is a `tekstide-core` test).
/// `#[cfg(test)]` items do not cross the crate boundary, so `tekstide`'s
/// own tests cannot reach it, and widening `tekstide-core`'s API just to
/// satisfy this test would be exactly the "change to `tekstide-core`
/// state models without raising it first" `implementation-handoff.md`
/// §8 forbids. Recorded as a real, disclosed testability gap in
/// `qa-evidence.md`, not hidden behind a fixture that only proves the
/// easy half.
#[test]
fn a_never_added_terminal_id_is_not_live() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("stale-terminal"))
        .expect("a freshly created directory is a valid project root");
    let stale_id = tekstide_core::domain::TerminalId::new_uuid();
    let stream =
        crate::input::terminal_stream_for_test(stale_id, iced::keyboard::Modifiers::empty());
    assert!(!super::terminal_stream_targets_a_live_terminal(
        &app_shell, &stream
    ));
}

/// The other negative case: no active project at all. Exercises the
/// `active_project()` short-circuit specifically (distinct code path
/// from the "active project exists, terminal not in it" case above).
#[test]
fn with_no_active_project_no_terminal_id_is_ever_live() {
    let app_shell = ApplicationShell::new();
    let some_id = tekstide_core::domain::TerminalId::new_uuid();
    let stream =
        crate::input::terminal_stream_for_test(some_id, iced::keyboard::Modifiers::empty());
    assert!(!super::terminal_stream_targets_a_live_terminal(
        &app_shell, &stream
    ));
}

/// **The real focus-trap test RFC-014 R6 requires**, not a structural
/// argument: while a modal is shown, Tab/Shift+Tab must cycle only the
/// modal's own two targets, and `state.focus` (the shell's own focus
/// cycle) must never move -- proven by dispatching real `Message`
/// values through `update`, not by inspecting the routing code and
/// asserting it "should" hold.
#[test]
fn modal_focus_cycling_never_touches_the_shell_focus_cycle() {
    let mut state = state_with(ApplicationShell::new());
    state.modal = Some(ModalContent::default());
    let focus_before = state.focus;

    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(
        state.modal.as_ref().map(|modal| modal.focus),
        Some(ModalButton::Acknowledge),
        "ModalFocusNext must cycle the modal's own focus"
    );
    assert_eq!(
        state.focus, focus_before,
        "the shell's own focus cycle must not move while a modal is shown"
    );

    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(
        state.modal.as_ref().map(|modal| modal.focus),
        Some(ModalButton::Dismiss)
    );
    assert_eq!(state.focus, focus_before);

    let _ = super::update(&mut state, Message::ModalFocusPrevious);
    assert_eq!(
        state.modal.as_ref().map(|modal| modal.focus),
        Some(ModalButton::Acknowledge)
    );
    assert_eq!(state.focus, focus_before);
}

/// Dismissal (`Enter`/`ModalActivate` or `Escape`/`ModalDismiss`) clears
/// the modal -- and because the shell's own `focus` field was never
/// touched while the modal was shown (proven above), whatever it held
/// before is simply still there afterward. That *is* "focus returns to
/// the invoking element" (UI/UX §18): there is nothing to restore
/// because nothing else was ever allowed to move it.
#[test]
fn dismissing_the_modal_clears_it_and_leaves_shell_focus_undisturbed() {
    let mut state = state_with(ApplicationShell::new());
    state.modal = Some(ModalContent::default());
    let focus_before = state.focus;

    let _ = super::update(&mut state, Message::ModalDismiss);
    assert!(state.modal.is_none());
    assert_eq!(state.focus, focus_before);

    state.modal = Some(ModalContent::default());
    let _ = super::update(&mut state, Message::ModalActivate);
    assert!(state.modal.is_none());
    assert_eq!(state.focus, focus_before);
}

/// A `ShellInput` for `OpenProjectBoard` must actually dispatch
/// `AppCommand::OpenProjectBoard` -- proven by starting from
/// `ActiveProjectWorkspace` (reachable only with a project open and its
/// mode toggled) and confirming the route changes, not merely that
/// `update` returns without panicking.
#[test]
fn a_project_board_shell_input_dispatches_the_real_app_command() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("shell-input-dispatch"))
        .expect("a freshly created directory is a valid project root");
    app_shell.dispatch(tekstide_core::command::AppCommand::ToggleActiveProjectMode);
    assert_eq!(
        app_shell.route(),
        tekstide_core::route::AppRoute::ActiveProjectWorkspace,
        "test precondition: must not already be on the Project Board route"
    );

    let mut state = state_with(app_shell);
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenProjectBoard,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    assert!(status_bar_summary(&state).contains("Project Board"));
}

/// Focus cycling: with a single real `FocusZone` variant today,
/// `FocusNext`/`FocusPrevious` are legitimate no-ops -- proven directly
/// rather than assumed, so the day `FocusZone` grows a second variant
/// (PR-015-E), this test either still passes trivially or fails loudly
/// pointing at exactly what needs updating.
#[test]
fn focus_next_and_previous_route_through_update() {
    let mut state = state_with(ApplicationShell::new());
    let focus_before = state.focus;
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::FocusNext),
    );
    assert_eq!(state.focus, focus_before);
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::FocusPrevious),
    );
    assert_eq!(state.focus, focus_before);
    let _ = FocusZone::MainArea;
}

// --- Mechanical seam scans -------------------------------------------
//
// Response 128 Required: the original scans named `shell.rs` directly
// (`include_str!("../shell.rs")`), which meant PR-015-C's routing module
// and PR-015-D's surface modules would land completely unscanned --
// "the failure mode RFC-016 §Risks names directly: enforcement decays,
// and it decays silently, which is worse than not having a check at
// all, because the green test reads as coverage." Fixed by walking
// `crates/tekstide/src` itself, so a new source file is scanned the
// moment it exists, with no list to fall out of date. Two exemptions,
// both stated rather than implicit:
//
// - `theme.rs` -- the seam's own implementation defines colours; it is
//   what the other files are required to source them *from*, not a
//   violation of the rule.
// - any `tests.rs` -- test code legitimately contains literals (expected
//   strings, this file's own ablation probes) that are not user-facing
//   shell output.

/// `crates/tekstide/src`, the root the seam scans walk.
fn crate_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn is_scan_exempt(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("theme.rs") | Some("tests.rs")
    )
}

/// Every `.rs` file under `crates/tekstide/src`, recursively, minus the
/// stated exemptions above -- a file added anywhere in the tree is
/// picked up automatically, with nothing to remember to add to a list.
fn scannable_source_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);
    files.retain(|path| !is_scan_exempt(path));
    files
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("crate src dir must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Mechanical seam check (response 125's "ideally mechanically" standard,
/// broadened per response 128 to cover the crate's source tree): no
/// scanned file may contain a `text("literal")` call -- every user-facing
/// string must come from `state.catalog.get(...)` or a helper that does.
/// A heuristic scan of each file's own text, not a full parse --
/// ablation-verified in `qa-evidence.md`.
#[test]
fn no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate() {
    for path in scannable_source_files() {
        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!")
            {
                continue;
            }
            assert!(
                !contains_text_call_with_string_literal(line),
                "{}:{} passes a string literal directly to text(...): {line}",
                path.display(),
                line_number + 1
            );
        }
    }
}

fn contains_text_call_with_string_literal(line: &str) -> bool {
    let mut search_from = 0;
    while let Some(relative_index) = line[search_from..].find("text(") {
        let call_start = search_from + relative_index + "text(".len();
        let after_paren = line[call_start..].trim_start();
        if after_paren.starts_with('"') {
            return true;
        }
        search_from = call_start;
    }
    false
}

/// Mechanical seam check for colour, broadened per response 128: no
/// scanned file may construct an `iced::Color` directly
/// (`Color::from_rgb`/`from_rgba`) -- every colour must come from
/// `state.theme`. `theme.rs` is exempt (see above); it is the one
/// legitimate place these calls appear.
#[test]
fn no_raw_color_construction_anywhere_in_the_crate() {
    for path in scannable_source_files() {
        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        assert!(
            !source.contains("Color::from_rgb") && !source.contains("Color::from_rgba"),
            "{} must source every colour from state.theme, not construct one directly",
            path.display()
        );
    }
}

/// Mechanical seam check for font size, broadened per response 128: no
/// scanned file's `.size(...)` calls may read a bare numeric literal --
/// every size must come from `state.theme.font_size_*()`.
#[test]
fn no_raw_font_size_literal_anywhere_in_the_crate() {
    for path in scannable_source_files() {
        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if let Some(relative_index) = line.find(".size(") {
                let call_start = relative_index + ".size(".len();
                let after_paren = line[call_start..].trim_start();
                let starts_with_digit = after_paren
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit());
                assert!(
                    !starts_with_digit,
                    "{}:{} passes a bare numeric literal to .size(...): {line}",
                    path.display(),
                    line_number + 1
                );
            }
        }
    }
}
