use std::path::{Path, PathBuf};

use tekstide_core::shell::ApplicationShell;

use super::{
    Message, ModalButton, ModalContent, State, focus_marker, main_area_key, main_area_label,
    sidebar_label, status_bar_summary, zone_style,
};
use crate::i18n::{Catalog, LocalePreference};
use crate::input::{FocusZone, SubscriptionMode};

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

/// Response 132 Required: the status bar's project count must agree
/// with the number of rows the Project Board actually renders, even
/// when they are genuinely different collections -- an open project
/// plus a recent-but-not-open one (RFC-005's model; the board lists
/// both, `ApplicationShell::state().projects()` lists only the open
/// one). Before this fix, the status bar counted only open sessions,
/// so this exact scenario would have shown "1 project" in chrome next
/// to two rows on the board beneath it.
#[test]
fn status_bar_project_count_matches_the_board_row_count_including_recent_projects() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("status-bar-parity-open"))
        .expect("a freshly created directory is a valid project root");

    let recent = tekstide_core::project::recent::RecentProject::new(
        tekstide_core::project::ProjectId::new_uuid(),
        "recent-only-project",
        "/tmp/recent-only-project",
        "/tmp/recent-only-project",
        tekstide_core::project::recent::Timestamp::now_utc(),
        "Restricted",
    );
    app_shell.restore_recent_projects(tekstide_core::project::recent::RecentProjectState {
        state_version: tekstide_core::project::recent::RECENT_PROJECT_STATE_VERSION,
        projects: vec![recent],
    });

    let board_row_count = app_shell.project_board().rows.len();
    let open_project_count = app_shell.state().projects().len();
    assert_ne!(
        board_row_count, open_project_count,
        "test precondition: an open project plus a recent-but-not-open one must make the two \
         collections genuinely different sizes, or this test cannot prove the fix matters"
    );

    let state = state_with(app_shell);
    let summary = status_bar_summary(&state);
    assert!(
        summary.contains(&format!(
            "{ISOLATE_START}{ISOLATE_START}{board_row_count}{ISOLATE_END}"
        )),
        "the status bar must count what the board renders ({board_row_count} rows), not just \
         open sessions ({open_project_count}): {summary:?}"
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

/// Response 130 Required 2: `SubscriptionMode::for_modal` -- the branch
/// `shell::subscription` picks -- asserted directly against a real
/// `State`, rather than left as an untested seam between `ModalAbsent`'s
/// own tests (`input::tests`) and `route_non_modal_input` given a proof.
/// This does not, and cannot, prove that `iced` actually tears down the
/// non-modal subscription when the branch flips to `Modal` -- that half
/// is a framework-lifecycle dependency, named in the module docs rather
/// than tested here.
#[test]
fn subscription_mode_reflects_whether_a_modal_is_active() {
    let mut state = state_with(ApplicationShell::new());
    assert!(matches!(
        SubscriptionMode::for_modal(&state.modal),
        SubscriptionMode::NonModal(_)
    ));

    state.modal = Some(ModalContent::default());
    assert_eq!(
        SubscriptionMode::for_modal(&state.modal),
        SubscriptionMode::Modal
    );

    state.modal = None;
    assert!(matches!(
        SubscriptionMode::for_modal(&state.modal),
        SubscriptionMode::NonModal(_)
    ));
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

/// RFC-015 PR-015-E: `ToggleProjectMode`'s real default binding
/// (`KeybindingPolicy::linux_mvp`, `Ctrl+Alt+M`) makes this the second
/// `NavigationAction` with a genuine dispatch path -- proven the same
/// way `OpenProjectBoard`'s dispatch is proven above: a real `ShellInput`
/// routed through the real `update`, asserting the project's actual
/// `ProjectMode` changed, not merely that `update` returned.
#[test]
fn a_toggle_project_mode_shell_input_dispatches_the_real_app_command() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("toggle-project-mode-dispatch"))
        .expect("a freshly created directory is a valid project root");
    assert_eq!(
        app_shell
            .state()
            .active_project()
            .map(tekstide_core::project::ProjectSession::mode),
        Some(tekstide_core::project::ProjectMode::Content),
        "test precondition: a freshly opened project starts in Content Mode"
    );

    let mut state = state_with(app_shell);
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::ToggleProjectMode,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    assert_eq!(
        state
            .app_shell
            .state()
            .active_project()
            .map(tekstide_core::project::ProjectSession::mode),
        Some(tekstide_core::project::ProjectMode::TerminalImmersion),
        "ToggleProjectMode must reach the real AppCommand, not be silently swallowed"
    );
    assert_eq!(
        state.app_shell.route(),
        tekstide_core::route::AppRoute::ActiveProjectWorkspace
    );
}

/// Focus cycling now has somewhere real to go: `FocusZone::Sidebar`
/// (PR-015-E). This replaces the PR-015-C-era version of this test,
/// which only proved `FocusNext`/`FocusPrevious` were legitimate no-ops
/// -- correct then, with one variant; the comment predicted this test
/// would need updating the day a second one arrived, and it did.
#[test]
fn focus_next_and_previous_route_through_update() {
    // RFC-015 PR-015-E: with a second `FocusZone` variant, cycling is a
    // genuine toggle -- this replaces the PR-015-C-era version of this
    // test, which only asserted focus stayed put, correct back when
    // `MainArea` was the only zone and cycling was necessarily a no-op.
    let mut state = state_with(ApplicationShell::new());
    let focus_before = state.focus;
    assert_eq!(focus_before, FocusZone::MainArea);

    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::FocusNext),
    );
    assert_eq!(state.focus, FocusZone::Sidebar);

    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::FocusPrevious),
    );
    assert_eq!(state.focus, focus_before);
}

/// RFC-015 PR-015-E, `NFR-UX-002`: the focus indicator must not rely on
/// colour alone. Proves two independent channels both flip with
/// `state.focus`, not just one -- a border-colour-only change would
/// satisfy neither this test's width assertion nor the marker one below.
#[test]
fn zone_style_changes_both_border_colour_and_width_when_focused() {
    let theme = crate::theme::Theme::default();
    let base = iced::Theme::Light;

    let unfocused = zone_style(theme, false)(&base);
    let focused = zone_style(theme, true)(&base);

    assert_ne!(
        unfocused.border.color, focused.border.color,
        "focus must change the border colour"
    );
    assert_ne!(
        unfocused.border.width, focused.border.width,
        "focus must also change a non-colour channel (border width), per NFR-UX-002"
    );
}

/// The textual channel (`NFR-UX-002`'s second, colour-independent
/// signal), matching the modal's own `"> "`/`"  "` convention.
#[test]
fn focus_marker_differs_and_is_not_colour_dependent() {
    assert_ne!(focus_marker(true), focus_marker(false));
    assert!(focus_marker(true).contains('>'));
}

/// RFC-015 PR-015-E: the sidebar's rendered label actually changes when
/// `state.focus` moves onto it -- proven against the real `State`, not
/// just the marker function in isolation, so a wiring mistake (e.g.
/// `sidebar_view` reading the wrong `FocusZone` variant) would be caught.
#[test]
fn sidebar_label_reflects_focus() {
    let mut state = state_with(ApplicationShell::new());
    assert_eq!(state.focus, FocusZone::MainArea);
    let unfocused_label = sidebar_label(&state);

    state.focus = FocusZone::Sidebar;
    let focused_label = sidebar_label(&state);

    assert_ne!(unfocused_label, focused_label);
    assert!(focused_label.starts_with(focus_marker(true)));
    assert!(unfocused_label.starts_with(focus_marker(false)));
}

/// Same property for the main area, plus proving `main_area_key` selects
/// distinct catalog keys per `ProjectMode` -- a regression here would
/// mean Content and Terminal Mode render identical scaffolding text.
#[test]
fn main_area_label_reflects_focus_and_mode() {
    let mut state = state_with(ApplicationShell::new());
    assert_eq!(state.focus, FocusZone::MainArea);

    let content_label = main_area_label(&state, Some(tekstide_core::project::ProjectMode::Content));
    let terminal_label = main_area_label(
        &state,
        Some(tekstide_core::project::ProjectMode::TerminalImmersion),
    );
    assert_ne!(
        content_label, terminal_label,
        "Content and Terminal Mode must render different scaffolding text"
    );

    state.focus = FocusZone::Sidebar;
    let unfocused_content_label =
        main_area_label(&state, Some(tekstide_core::project::ProjectMode::Content));
    assert_ne!(content_label, unfocused_content_label);
}

/// `main_area_key`'s `None` fallback (no active project, which core
/// guards against reaching this route without) renders Content Mode's
/// placeholder, not a panic -- `main_area_key` is the pure decision
/// function `main_area_label`/`main_area_view` both defer to.
#[test]
fn main_area_key_falls_back_to_content_mode_for_no_active_project() {
    assert_eq!(
        main_area_key(None),
        main_area_key(Some(tekstide_core::project::ProjectMode::Content))
    );
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
        // `enforcement.rs` (PR-016-E) is test-support code that names
        // `.label()` in prose describing what it must never call in
        // real rendering code -- exempt for the same reason `tests.rs`
        // is: it talks about the shape, it does not render it.
        //
        // `grid_colors.rs` (RFC-017 PR-017-C's exemption, narrowed here
        // in PR-017-E per review 148's expiry): the colour this scan
        // otherwise forbids is the terminal grid's *own*, PTY-determined
        // ANSI colour -- RFC-016's grid exception ("the terminal grid
        // itself renders untrusted bytes unescaped... the ONLY place
        // this exception applies"). That colour cannot come from
        // `state.theme`: it is per-cell data chosen by whatever program
        // is writing to the PTY, not a chrome role this crate's theme
        // defines. This is not the same carve-out as `theme.rs` (which
        // defines the raw palette chrome draws from) -- it is the other
        // legitimate case this scan's own module doc anticipates: a new
        // file that genuinely needs a literal, raised here rather than
        // silently exempted.
        //
        // **`terminal.rs` and `terminal/session_bar.rs` are deliberately
        // NOT exempt.** PR-017-C's exemption was file-level, covering a
        // file whose only colour call was the grid's; PR-017-E gives
        // `terminal.rs` real chrome (`session_bar`), so the file-level
        // exemption was narrowed to exactly the file whose claim is
        // still true -- the grid-rendering code, moved to
        // `grid_colors.rs` for exactly this reason. `session_bar.rs`
        // sources every colour from `crate::theme::Theme`, same as
        // `shell::zone_style`, and must keep failing this scan if it
        // ever stops.
        Some("theme.rs") | Some("tests.rs") | Some("enforcement.rs") | Some("grid_colors.rs")
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

// RFC-016 PR-016-E: the no-hardcoded-string scan this file introduced in
// PR-015-B (response 128) is now canonical in
// `crate::i18n::enforcement::no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate`,
// which absorbs this file's directory walk rather than duplicating it --
// see that module's doc for why PR-016-E, not PR-015-B, owns this policy
// going forward. Colour and font-size stay here; they are RFC-015's own
// seam, not RFC-016's.

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

/// Response 130's decision, made mechanical: `CountDisplay`/
/// `AttentionState::label()` must never be called from this crate's
/// rendering code -- `surface::board`'s module doc explains why
/// (hardcoded English at the render layer, and the easiest way to
/// quietly fail "Unavailable/NotImplemented never render as 0"). Test
/// fixtures legitimately construct a `.label()` value when building a
/// `ProjectBoardRow` (the field exists on the core type regardless of
/// whether this crate reads it back), so `tests.rs` files are exempt,
/// same as the other scans.
#[test]
fn no_count_display_or_attention_label_is_called_anywhere_in_the_crate() {
    for path in scannable_source_files() {
        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        assert!(
            !source.contains(".label()"),
            "{} calls .label() -- CountDisplay/AttentionState must render through the catalog \
             (CatalogArgs::number/trusted_symbol), never through label()'s hardcoded English",
            path.display()
        );
    }
}

// --- PR-015-F: measurement, discharging R1 -----------------------------

/// `State::is_measuring_typing()` must be `false` with no measurement
/// env var set -- the default for every normal interactive run, and the
/// property `main.rs`'s view-cost timing wrapper and the idle-CPU
/// comparison both depend on. Does not itself set or clear
/// `TEKSTIDE_MEASURE_CRITERION`, relying on the ambient test environment
/// not setting it (true for every gate run in this repo).
#[test]
fn is_measuring_typing_is_false_by_default() {
    let state = state_with(ApplicationShell::new());
    assert!(!state.is_measuring_typing());
}

/// Response 134 Required: measurement and the demo modal must never both
/// be active -- otherwise `subscription()`'s measurement branch (checked
/// ahead of `SubscriptionMode::for_modal` entirely) would skip modal
/// exclusivity while the modal is still on screen. Tests the pure
/// decision function directly (not by setting `TEKSTIDE_LAYER_DEMO`/
/// `TEKSTIDE_MEASURE_CRITERION` env vars, which are process-global and
/// would race against concurrently-running tests that also construct a
/// `State`) -- all four input combinations, not only the one that
/// matters for the fix, so a future change to the *other* branch is
/// caught too.
#[test]
fn measurement_and_the_demo_modal_are_mutually_exclusive() {
    assert!(
        super::modal_for_state(true, true).is_none(),
        "measurement active + layer-demo requested must not open the modal"
    );
    assert!(
        super::modal_for_state(true, false).is_none(),
        "measurement active, no layer-demo requested: still no modal"
    );
    assert!(
        super::modal_for_state(false, true).is_some(),
        "no measurement, layer-demo requested: modal opens as before"
    );
    assert!(
        super::modal_for_state(false, false).is_none(),
        "no measurement, no layer-demo requested: no modal"
    );
}

/// `tail_lines` -- the typing-measurement surface's only rendering
/// logic worth testing in isolation from `iced`'s `Element` tree --
/// keeps exactly the last `count` lines, joined back with `\n`, and
/// does not panic when the document has fewer lines than requested.
#[test]
fn tail_lines_keeps_only_the_last_n_lines() {
    let doc = "one\ntwo\nthree\nfour\nfive";
    assert_eq!(super::tail_lines(doc, 2), "four\nfive");
    assert_eq!(super::tail_lines(doc, 5), doc);
    assert_eq!(
        super::tail_lines(doc, 100),
        doc,
        "must not panic when count exceeds line count"
    );
    assert_eq!(super::tail_lines("", 5), "");
}

// RFC-017 PR-017-D: input. A real, live `TerminalPane` (not a synthetic
// `TerminalId`) is what turns "modal exclusivity holds" and "global
// keybindings win" from a headless proof (`input::tests`, unchanged
// from RFC-015) into a proof against an actual PTY -- the explicit
// requirement review 148/RFC-017 both name: do not assume the headless
// proof transfers.
//
// RFC-017 PR-017-E: the pane launched below is now registered as a real
// `TerminalSession` on the active project (`Primary` slot) -- the
// change response 149 anticipated ("registering for real is what
// PR-017-E's job means"). `terminal_stream_targets_a_live_terminal`
// (the real, core-backed check) is what these fixtures exercise now,
// not the demo-only counterpart PR-017-D added and this slice removes.

fn state_with_a_real_terminal_focused(label: &str) -> State {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir(label))
        .expect("a freshly created directory is a valid project root");
    app_shell.dispatch(tekstide_core::command::AppCommand::ToggleActiveProjectMode);
    assert_eq!(
        app_shell
            .state()
            .active_project()
            .map(tekstide_core::project::ProjectSession::mode),
        Some(tekstide_core::project::ProjectMode::TerminalImmersion),
        "test precondition: the active project must be in Terminal Mode"
    );
    let project_id = app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("test precondition: a project was just added");

    let (pane, session) = crate::surface::terminal::TerminalPane::launch(
        project_id,
        "shell-test pane",
        fresh_project_dir(&format!("{label}-pane")),
        PathBuf::from("/bin/sh"),
    )
    .expect("launch a real shell for a live-terminal input test");
    let terminal_id = session.id.clone();
    app_shell
        .state_mut()
        .attach_terminal_session(session)
        .expect("registering a session on its own project must succeed");
    app_shell
        .state_mut()
        .assign_terminal_visible_slot(&terminal_id, tekstide_core::domain::VisibleSlot::Primary)
        .expect("assigning Primary on a just-registered session must succeed");

    let mut state = state_with(app_shell);
    state.focus = FocusZone::MainArea;
    state.terminal_panes = vec![pane];
    state
}

fn rendered_demo_pane_text(state: &State) -> String {
    state
        .terminal_panes
        .first()
        .expect("test precondition: a demo pane must exist")
        .rendered_text()
}

fn poll_demo_pane_until(state: &mut State, needle: &str) -> bool {
    for _ in 0..200 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        if rendered_demo_pane_text(state).contains(needle) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

/// `active_terminal_focus` is `Some` only when both halves of its own
/// stated condition hold (`FocusZone::MainArea` *and* `TerminalImmersion`
/// mode) -- proven against all four combinations with one real pane,
/// not assumed from reading the `&&`.
#[test]
fn active_terminal_focus_requires_both_main_area_and_terminal_mode() {
    let mut state = state_with_a_real_terminal_focused("active-terminal-focus");
    let real_id = state
        .terminal_panes
        .first()
        .expect("test precondition")
        .terminal_id()
        .clone();

    assert_eq!(super::active_terminal_focus(&state), Some(real_id));

    state.focus = FocusZone::Sidebar;
    assert_eq!(
        super::active_terminal_focus(&state),
        None,
        "Sidebar focused: the terminal is not the focused zone's content"
    );

    state.focus = FocusZone::MainArea;
    state
        .app_shell
        .dispatch(tekstide_core::command::AppCommand::ToggleActiveProjectMode);
    assert_eq!(
        super::active_terminal_focus(&state),
        None,
        "back in Content Mode: MainArea is focused, but it is not showing the terminal"
    );
}

/// RFC-017 PR-017-E: `terminal_stream_targets_a_live_terminal` (the
/// real, core-backed check `#[allow(dead_code)]` in PR-017-D) now has a
/// real, positive case to recognize -- the demo pane's session is a
/// genuinely registered `TerminalSession` on the active project. This
/// is the behaviour change response 149 named as this slice's job.
#[test]
fn terminal_stream_targets_a_live_terminal_recognizes_the_registered_demo_session() {
    let state = state_with_a_real_terminal_focused("demo-session-now-registered");
    let real_id = state
        .terminal_panes
        .first()
        .expect("test precondition")
        .terminal_id()
        .clone();

    let matching =
        crate::input::terminal_stream_for_test(real_id, iced::keyboard::Modifiers::empty());
    assert!(super::terminal_stream_targets_a_live_terminal(
        &state.app_shell,
        &matching
    ));

    let other_id = tekstide_core::domain::TerminalId::new_uuid();
    let mismatched =
        crate::input::terminal_stream_for_test(other_id, iced::keyboard::Modifiers::empty());
    assert!(!super::terminal_stream_targets_a_live_terminal(
        &state.app_shell,
        &mismatched
    ));
}

/// The accept path, end to end: a `TextStream` addressed to the real,
/// live pane's own id, delivered through the real `update`, actually
/// reaches the PTY -- confirmed by polling the pane's own rendered
/// output for the character sent, not by inspecting `update`'s return
/// value.
#[test]
fn a_text_stream_targeting_the_real_pane_writes_to_it() {
    let mut state = state_with_a_real_terminal_focused("live-input-accept");
    let real_id = state
        .terminal_panes
        .first()
        .expect("test precondition")
        .terminal_id()
        .clone();
    let stream =
        crate::input::terminal_stream_for_test(real_id, iced::keyboard::Modifiers::empty());

    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Terminal(stream)),
    );

    assert!(
        poll_demo_pane_until(&mut state, "x"),
        "the character carried by the TextStream must reach the real PTY and render"
    );
}

/// The other half of the liveness check's enforcement: a stream naming
/// a *different* id, delivered through the real `update`, must never
/// reach this pane's PTY -- "a stale or cross-project id is dropped,
/// not best-effort delivered" (`pr-015-c-input-routing.md`) proven end
/// to end, not only against the pure function in isolation.
#[test]
fn a_text_stream_targeting_a_different_id_does_not_write_to_the_pane() {
    let mut state = state_with_a_real_terminal_focused("live-input-wrong-target");
    let other_id = tekstide_core::domain::TerminalId::new_uuid();
    let stream =
        crate::input::terminal_stream_for_test(other_id, iced::keyboard::Modifiers::empty());

    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Terminal(stream)),
    );
    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert!(
        !rendered_demo_pane_text(&state).contains('x'),
        "a TextStream naming a different terminal id must never write to this pane"
    );
}

/// **Modal exclusivity, demonstrated against a real PTY, not argued.**
/// The same `TextStream` that reaches the pane above must produce
/// *zero* bytes while a modal is open, and must resume working the
/// moment it closes -- both halves proven in one test so the "resumes
/// afterward" half rules out "the pane was simply broken" as an
/// alternative explanation for "nothing appeared."
#[test]
fn modal_open_blocks_pty_write_and_closing_it_resumes_delivery() {
    let mut state = state_with_a_real_terminal_focused("live-modal-exclusivity");
    let real_id = state
        .terminal_panes
        .first()
        .expect("test precondition")
        .terminal_id()
        .clone();
    state.modal = Some(ModalContent::default());

    let blocked_stream =
        crate::input::terminal_stream_for_test(real_id.clone(), iced::keyboard::Modifiers::empty());
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Terminal(blocked_stream)),
    );
    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        !rendered_demo_pane_text(&state).contains('x'),
        "a TextStream delivered while a modal is open must never reach the PTY"
    );

    state.modal = None;
    let allowed_stream =
        crate::input::terminal_stream_for_test(real_id, iced::keyboard::Modifiers::empty());
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Terminal(allowed_stream)),
    );
    assert!(
        poll_demo_pane_until(&mut state, "x"),
        "the same stream, sent after the modal closes, must reach the PTY -- proving the pane \
         itself was never broken and the earlier silence was the modal check, not a fluke"
    );
}

/// The Tab decision (recorded in `input`'s module doc), proven against
/// a real live terminal rather than only the synthetic `TerminalId`
/// `input::tests::tab_cycles_focus_even_with_a_terminal_focused` uses:
/// Tab still cycles shell focus, and -- because it never becomes a
/// `TextStream` in the first place -- writes nothing to the real pane.
#[test]
fn tab_cycles_shell_focus_with_a_real_terminal_focused_and_writes_nothing() {
    let mut state = state_with_a_real_terminal_focused("live-tab-escape-hatch");
    let real_id = state
        .terminal_panes
        .first()
        .expect("test precondition")
        .terminal_id()
        .clone();
    let focus_before = state.focus;

    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let tab_press = crate::input::KeyPress {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab),
        modifiers: iced::keyboard::Modifiers::empty(),
    };
    let proof = crate::input::ModalAbsent::check(&state.modal)
        .expect("test precondition: no modal is open");
    let routed =
        crate::input::route_non_modal_input(proof, &policy, state.focus, Some(&real_id), tab_press);
    assert_eq!(
        routed,
        crate::input::RoutedInput::FocusNext,
        "Tab must cycle focus even though a real, live terminal is focused"
    );

    let _ = super::update(&mut state, Message::Input(routed));
    assert_ne!(
        state.focus, focus_before,
        "the real FocusNext message must actually move shell focus"
    );

    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        rendered_demo_pane_text(&state).trim().is_empty()
            || !rendered_demo_pane_text(&state).contains('\t'),
        "Tab must never reach the PTY as literal input"
    );
}

// --- RFC-017 PR-017-E: immersion mode, split policy, session bar ---

/// Three real, launched panes registered on the active project:
/// `Primary`, `Secondary`, and one deliberately `Hidden` from the start
/// -- the same shape `launch_terminal_demo_panes` builds for the real
/// `TEKSTIDE_TERMINAL_DEMO` path, constructed directly here so these
/// tests do not depend on an env var for determinism.
fn state_with_two_visible_and_one_hidden_pane(label: &str) -> State {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir(label))
        .expect("a freshly created directory is a valid project root");
    app_shell.dispatch(tekstide_core::command::AppCommand::ToggleActiveProjectMode);
    let project_id = app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("test precondition: a project was just added");

    let mut panes = Vec::new();
    for (index, slot) in [
        tekstide_core::domain::VisibleSlot::Primary,
        tekstide_core::domain::VisibleSlot::Secondary,
        tekstide_core::domain::VisibleSlot::Hidden,
    ]
    .into_iter()
    .enumerate()
    {
        let (pane, session) = crate::surface::terminal::TerminalPane::launch(
            project_id.clone(),
            format!("{label} pane {index}"),
            fresh_project_dir(&format!("{label}-pane-{index}")),
            PathBuf::from("/bin/sh"),
        )
        .expect("launch a real shell for a multi-pane test");
        let terminal_id = session.id.clone();
        app_shell
            .state_mut()
            .attach_terminal_session(session)
            .expect("registering a session on its own project must succeed");
        app_shell
            .state_mut()
            .assign_terminal_visible_slot(&terminal_id, slot)
            .expect("assigning a slot on a just-registered session must succeed");
        panes.push(pane);
    }

    let mut state = state_with(app_shell);
    state.focus = FocusZone::MainArea;
    state.terminal_panes = panes;
    state
}

fn poll_all_via_real_update(state: &mut State) {
    let _ = super::update(state, Message::TerminalPollTick);
}

/// `active_project_terminal_sessions` must list every registered
/// session -- hidden included -- not only the visible ones: "a session
/// that is producing output, has exited, or is blocked must be
/// distinguishable while hidden" (RFC-017) requires the hidden session
/// to appear somewhere, not silently drop out of the list the session
/// bar renders from.
#[test]
fn active_project_terminal_sessions_lists_hidden_sessions_too() {
    let state = state_with_two_visible_and_one_hidden_pane("sessions-include-hidden");
    let sessions = super::active_project_terminal_sessions(&state);
    assert_eq!(
        sessions.len(),
        3,
        "all three registered sessions must be listed"
    );
    assert!(
        sessions
            .iter()
            .any(|session| session.visible_slot() == tekstide_core::domain::VisibleSlot::Hidden),
        "the hidden session must still appear in the list"
    );
}

/// **The hidden-session grid-state decision, demonstrated, not argued.**
/// A hidden pane is still polled every tick (`Message::TerminalPollTick`
/// iterates every pane in `state.terminal_panes`, not only the visible
/// ones) and its content is retained across a later slot reassignment --
/// proving "hidden" means "not currently displayed," not "torn down."
#[test]
fn a_hidden_pane_keeps_polling_and_retains_its_content_across_a_slot_change() {
    let mut state = state_with_two_visible_and_one_hidden_pane("hidden-pane-retained");
    let hidden_session_id = super::active_project_terminal_sessions(&state)
        .iter()
        .find(|session| session.visible_slot() == tekstide_core::domain::VisibleSlot::Hidden)
        .expect("test precondition: one session must be Hidden")
        .id
        .clone();
    let hidden_pane_index = state
        .terminal_panes
        .iter()
        .position(|pane| pane.terminal_id() == &hidden_session_id)
        .expect("test precondition: the hidden session's pane must be in terminal_demo");

    state.terminal_panes[hidden_pane_index].write_input(b"printf 'HIDDEN_PANE_017E\\n'\n");
    let mut seen = false;
    for _ in 0..200 {
        poll_all_via_real_update(&mut state);
        if state.terminal_panes[hidden_pane_index]
            .rendered_text()
            .contains("HIDDEN_PANE_017E")
        {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        seen,
        "a hidden pane must still be polled and render real output -- retained in memory, not \
         torn down"
    );

    // Reassign the previously-hidden session to Secondary (bumping
    // whatever held Secondary back to Hidden, per
    // `ProjectSession::assign_terminal_visible_slot`'s own enforcement).
    // The content produced while hidden must still be there: retention
    // is a property of the pane, not of momentarily being displayed.
    state
        .app_shell
        .state_mut()
        .assign_terminal_visible_slot(
            &hidden_session_id,
            tekstide_core::domain::VisibleSlot::Secondary,
        )
        .expect("reassigning an existing session's slot must succeed");
    assert!(
        state.terminal_panes[hidden_pane_index]
            .rendered_text()
            .contains("HIDDEN_PANE_017E"),
        "content produced while hidden must survive becoming visible again -- the whole point \
         of retaining rather than rebuilding from scrollback"
    );
}

/// **Ablated**: if `Message::TerminalPollTick` only polled visible
/// panes (the alternative to the decision this slice made), the hidden
/// pane's content would never appear. Simulated here by polling only
/// the non-hidden panes directly, confirming the hidden pane's own
/// content is absent -- the failure mode the real, poll-everything
/// handler exists to avoid.
#[test]
fn ablation_polling_only_visible_panes_would_miss_the_hidden_ones_output() {
    let mut state = state_with_two_visible_and_one_hidden_pane("hidden-pane-ablation");
    let hidden_session_id = super::active_project_terminal_sessions(&state)
        .iter()
        .find(|session| session.visible_slot() == tekstide_core::domain::VisibleSlot::Hidden)
        .expect("test precondition")
        .id
        .clone();
    let hidden_pane_index = state
        .terminal_panes
        .iter()
        .position(|pane| pane.terminal_id() == &hidden_session_id)
        .expect("test precondition");

    state.terminal_panes[hidden_pane_index].write_input(b"printf 'SHOULD_NOT_APPEAR_017E\\n'\n");

    for _ in 0..20 {
        let visible_ids: Vec<tekstide_core::domain::TerminalId> =
            super::active_project_terminal_sessions(&state)
                .iter()
                .filter(|session| {
                    session.visible_slot() != tekstide_core::domain::VisibleSlot::Hidden
                })
                .map(|session| session.id.clone())
                .collect();
        for pane in &mut state.terminal_panes {
            if visible_ids.contains(pane.terminal_id()) {
                pane.poll();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert!(
        !state.terminal_panes[hidden_pane_index]
            .rendered_text()
            .contains("SHOULD_NOT_APPEAR_017E"),
        "polling only visible panes must miss the hidden pane's real output -- confirming the \
         previous test's poll-everything behaviour is what makes retention actually work, not \
         an accident of timing"
    );
}

// --- RFC-017 PR-017-F: plain_terminal_observation audit producer ---

fn temp_audit_state_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tekstide-shell-audit-test-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Concatenates the raw bytes of every regular file under `dir`,
/// recursively, lossily decoded -- used to scan the real on-disk state
/// an `AuditStore` leaves behind rather than naming `audit.sqlite3`
/// alone. Response 152 Required 2: while the store is open in WAL mode,
/// a freshly appended record lives in the `-wal` sidecar, not the main
/// database file, so a check that reads only `database_file()` while
/// the store is still open scans a page that never received the write.
/// Scanning every file (after the caller drops the store, see the
/// sentinel test below) is also robust to SQLite's sidecar set changing
/// -- it doesn't need updating if a new companion file is ever added.
fn read_every_file_in_dir(dir: &std::path::Path) -> String {
    let mut contents = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                contents.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
    }
    contents
}

/// `record_plain_terminal_started` actually persists a `Started` record
/// for a real, launched pane -- proven against a real, file-backed
/// `AuditStore` (`open_audit_store`, the same function
/// `open_real_audit_store` calls with the real `XDG_STATE_HOME`-derived
/// directory), not a mock writer.
#[test]
fn record_plain_terminal_started_persists_against_a_real_store() {
    let project_id = tekstide_core::project::ProjectId::new_uuid();
    let (_pane, session) = crate::surface::terminal::TerminalPane::launch(
        project_id.clone(),
        "audit producer test pane",
        fresh_project_dir("audit-producer-pane"),
        PathBuf::from("/bin/sh"),
    )
    .expect("launch a real shell for the audit producer test");
    let terminal_id = session.id.clone();

    let mut store = super::open_audit_store(&temp_audit_state_dir("started"), Vec::new())
        .expect("open a real, temp-dir-backed audit store");
    let mut health = tekstide_core::audit::AuditHealth::default();
    let status = tekstide_core::audit::AuditCoordinator::new(&mut store, &mut health)
        .record_plain_terminal_started(project_id.clone(), terminal_id.clone());
    assert_eq!(
        status,
        tekstide_core::audit::AuditObservationStatus::Persisted
    );

    let records = store
        .query(&tekstide_core::audit::AuditQuery::latest(10))
        .unwrap()
        .records;
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].record.family,
        tekstide_core::audit::AuditEventFamily::PlainTerminalObservation
    );
    assert_eq!(
        records[0].record.outcome,
        tekstide_core::audit::AuditOutcome::Started
    );
    assert_eq!(records[0].record.project_id, Some(project_id));
    assert_eq!(records[0].record.terminal_id, Some(terminal_id));
}

/// **RFC-017 PR-017-F's required sentinel test.** Matching RFC-021
/// PR-021-E2's shape: sentinel strings baked into the real inputs a
/// plain-terminal launch actually carries (its project root path and
/// its window title -- the closest analogues this family's callers have
/// to PR-021-E2's sentinel argv/cwd) must never reach the durable
/// store, checked against **raw on-disk bytes**, not only the typed
/// query. `DurableAuditRecordV1` has no path/title field for this
/// family at all -- this test is what proves that structural absence
/// holds all the way from a real launch through the real
/// `AuditCoordinator` call this crate makes, not merely that the type
/// looks safe on paper.
///
/// **Response 152 Required 2**: the store runs in WAL mode, and the
/// first version of this test read only `database_file()` while the
/// store was still open -- at that point the just-appended record
/// lives in the `-wal` sidecar, so the assertion scanned a 4096-byte
/// header page that never held the record and passed for the wrong
/// reason (it would have passed unchanged even if the producer wrote
/// the sentinels straight into the schema). The store is dropped here
/// before scanning, which is what makes SQLite checkpoint the WAL and
/// remove the sidecars -- the exact on-disk state a real session
/// leaves behind -- and every file under the audit directory is
/// scanned, not just the named database file.
#[test]
fn sentinel_terminal_derived_text_never_reaches_the_durable_audit_store() {
    const SENTINEL_TITLE: &str = "SENTINEL-TITLE-b23f9b3e-terminal-title";
    const SENTINEL_ROOT_MARKER: &str = "sentinel-root-a71c4d02-terminal-path";

    let project_id = tekstide_core::project::ProjectId::new_uuid();
    let root = fresh_project_dir(SENTINEL_ROOT_MARKER);
    let (_pane, session) = crate::surface::terminal::TerminalPane::launch(
        project_id.clone(),
        SENTINEL_TITLE,
        root.clone(),
        PathBuf::from("/bin/sh"),
    )
    .expect("launch a real shell for the sentinel test");
    let terminal_id = session.id.clone();

    let mut store = super::open_audit_store(&temp_audit_state_dir("sentinel"), Vec::new())
        .expect("open a real, temp-dir-backed audit store");
    let mut health = tekstide_core::audit::AuditHealth::default();
    tekstide_core::audit::AuditCoordinator::new(&mut store, &mut health)
        .record_plain_terminal_started(project_id, terminal_id.clone());

    let records = store
        .query(&tekstide_core::audit::AuditQuery::latest(10))
        .unwrap()
        .records;
    let typed_debug = format!("{records:?}");
    assert!(!typed_debug.contains(SENTINEL_TITLE));
    assert!(!typed_debug.contains(SENTINEL_ROOT_MARKER));
    assert!(!typed_debug.contains(root.to_string_lossy().as_ref()));

    let audit_dir = store.storage_path().audit_dir().to_path_buf();
    // Dropping the store closes its single connection, which is what
    // makes SQLite checkpoint the WAL into the main database file and
    // remove the `-wal`/`-shm` sidecars -- reproducing exactly what a
    // real session leaves on disk, not an open store's transient state.
    drop(store);

    let raw_text = read_every_file_in_dir(&audit_dir);
    // Positive control: `terminal_id` is a real, persisted field, written
    // by this same real producer call. If this failed, the scan would be
    // finding nothing at all (the vacuous-pass failure mode Required 2
    // named), regardless of what the sentinel assertions below say.
    assert!(
        raw_text.contains(terminal_id.as_str()),
        "the scan must reach the real record this test just wrote -- otherwise the sentinel \
         assertions below pass merely because nothing was read at all"
    );
    assert!(
        !raw_text.contains(SENTINEL_TITLE),
        "the terminal's window title must never reach the raw on-disk audit store"
    );
    assert!(
        !raw_text.contains(SENTINEL_ROOT_MARKER),
        "the terminal's project root path must never reach the raw on-disk audit store"
    );
    assert!(
        !raw_text.contains(root.to_string_lossy().as_ref()),
        "the real, full project root path must never reach the raw on-disk audit store"
    );
}

/// RFC-017 PR-017-G response 158: a **headless** benchmark answering the
/// discriminator question response 156 raised, without a GUI or GPU at
/// all -- `poll()`'s own cost (a real PTY read plus `Processor::advance`
/// through `SecurityFilter`) under a real, un-fork-bound flood, against
/// the 50ms tick period `terminal_demo_subscription` polls at in
/// production. `poll()` is pure CPU (no `iced`, no surface, no
/// synthetic input), so this needs none of the machinery the three live
/// GUI attempts kept getting blocked by (swap pressure, then a broken
/// X11/EGL path unrelated to this crate) -- the same real pane-launching
/// machinery `sentinel_terminal_derived_text_never_reaches_the_durable_audit_store`
/// and the hidden-session ablation above already use, just driven on a
/// timer instead of through `shell::update`.
///
/// **Answers three things this benchmark's own output makes legible,
/// not the end-to-end `NFR-PERF-004` figure** (that needs the real
/// event loop and stays parked on the GUI): whether handler cost
/// approaches the 50ms tick period (the saturation hypothesis -- if
/// so, that's a real, structural contributor independent of any
/// environment noise); dropped bytes under a *genuine* flood (still
/// unexamined before this -- the confounded live runs' `0` was, per
/// response 156's re-reading, evidence the flood never reached rate,
/// not evidence of no drops); and observed in-app throughput against
/// the flood script's own ~17.2 MiB/s standalone figure.
///
/// The only assertion is a generous, order-of-magnitude regression
/// guard (10x the tick period) -- this is a diagnostic report, not
/// `NFR-PERF-004`'s acceptance test, and asserting a tight bound here
/// would make this flaky against exactly the kind of shared-machine
/// noise the three GUI attempts already ran into.
#[test]
fn terminal_poll_handler_cost_under_a_real_flood_headless_benchmark() {
    let project_id = tekstide_core::project::ProjectId::new_uuid();
    let (mut pane, _session) = crate::surface::terminal::TerminalPane::launch(
        project_id,
        "headless flood benchmark",
        fresh_project_dir("headless-flood-benchmark"),
        PathBuf::from("/bin/sh"),
    )
    .expect("launch a real shell for the headless benchmark");

    pane.write_input(super::FLOOD_SCRIPT.as_bytes());
    // Give the backgrounded flood a moment to actually start producing
    // before the timed window begins -- otherwise the first samples
    // would measure an empty pipe, not the flood this benchmark exists
    // to observe.
    std::thread::sleep(std::time::Duration::from_millis(200));

    let tick_period = std::time::Duration::from_millis(50);
    let benchmark_window = std::time::Duration::from_secs(2);
    let benchmark_started = std::time::Instant::now();
    let mut tick_micros: Vec<u128> = Vec::new();
    while benchmark_started.elapsed() < benchmark_window {
        let tick_started = std::time::Instant::now();
        pane.poll();
        let elapsed = tick_started.elapsed();
        tick_micros.push(elapsed.as_micros());
        // Sleeping only the remainder of the tick period matches
        // production's `iced::time::every(50ms)` cadence when `poll()`
        // is fast, and correctly busy-polls with no extra delay when
        // `poll()` itself already exceeds the period -- exactly what a
        // saturating update loop would do in production too.
        std::thread::sleep(tick_period.saturating_sub(elapsed));
    }

    tick_micros.sort_unstable();
    let sample_count = tick_micros.len();
    let p50_micros = tick_micros[sample_count / 2];
    let max_micros = *tick_micros.last().expect("at least one tick must have run");
    let bytes_read_total = pane.bytes_read_total();
    let dropped_bytes_total = pane.dropped_bytes_total();
    let observed_bytes_per_sec =
        bytes_read_total as f64 / benchmark_started.elapsed().as_secs_f64();

    eprintln!(
        "terminal_poll_headless_benchmark samples={sample_count} p50_us={p50_micros} \
         max_us={max_micros} bytes_read_total={bytes_read_total} \
         dropped_bytes_total={dropped_bytes_total} \
         observed_bytes_per_sec={observed_bytes_per_sec:.0}"
    );

    assert!(
        max_micros < tick_period.as_micros() * 10,
        "poll() cost blew past a sane, order-of-magnitude-over-the-tick-period bound: \
         max={max_micros}us against a {}us tick period -- this is a real regression, not \
         measurement noise, since the bound is 10x the period it is meant to fit inside",
        tick_period.as_micros()
    );
}

// --- Terminal launch UX handoff ----------------------------------------

/// **The review gate's own first item**: "a user can open a terminal,
/// type in it, and see output." A real `Ctrl+Alt+T` press through the
/// real `update` dispatch must switch to `TerminalImmersion`, launch
/// exactly one real pane rooted in the **real project directory** (not
/// a scratch temp dir, unlike the diagnostic demo/measurement paths --
/// a terminal a user asked for must open where their project is), leave
/// no refusal notice, and the session must already be `Running` with
/// `Primary`'s slot -- not stuck at `Starting` the way every launch path
/// before this handoff left it.
#[test]
fn launch_terminal_shell_input_switches_to_terminal_immersion_and_launches_a_real_session() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("launch-terminal-dispatch");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    assert_eq!(
        app_shell
            .state()
            .active_project()
            .map(tekstide_core::project::ProjectSession::mode),
        Some(tekstide_core::project::ProjectMode::Content),
        "test precondition: a freshly opened project starts in Content Mode"
    );

    let mut state = state_with(app_shell);
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::LaunchTerminal,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    assert_eq!(
        state
            .app_shell
            .state()
            .active_project()
            .map(tekstide_core::project::ProjectSession::mode),
        Some(tekstide_core::project::ProjectMode::TerminalImmersion),
        "launching a terminal must switch into Terminal Immersion, or the user presses a key \
         and nothing appears to happen"
    );
    assert_eq!(
        state.terminal_panes.len(),
        1,
        "exactly one real pane must be launched"
    );
    assert!(
        state.terminal_launch_notice.is_none(),
        "a successful launch must not leave a stale refusal notice"
    );

    let sessions = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .terminal_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].status(),
        tekstide_core::domain::TerminalStatus::Running,
        "a freshly launched session must already be Running, not left at Starting forever"
    );
    assert_eq!(
        sessions[0].visible_slot(),
        tekstide_core::domain::VisibleSlot::Primary,
        "a fresh launch must become Primary so the user can type into it immediately"
    );
    assert_eq!(
        sessions[0].cwd, project_dir,
        "the pane must be rooted in the real project directory, not a scratch temp dir"
    );
}

/// **Review gate**: "the session limit is enforced in core and
/// demonstrated, including what the user sees on refusal." Runs the
/// default limit (`ProjectResourceLimits::default`, 8) to exhaustion
/// with real launches, confirms the typed refusal names the real
/// number, and confirms the refusal notice a user would actually see
/// states that number too -- not a generic message that could pass
/// whether or not the real limit made it through.
#[test]
fn terminal_session_limit_is_enforced_end_to_end_with_a_visible_notice() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("terminal-session-limit"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    for index in 0..8 {
        super::attempt_terminal_launch(&mut state).unwrap_or_else(|error| {
            panic!("launch {index} must succeed, under the limit: {error:?}")
        });
    }
    assert_eq!(state.terminal_panes.len(), 8);

    let refusal = super::attempt_terminal_launch(&mut state)
        .expect_err("the 9th launch must be refused once the default limit of 8 is reached");
    assert_eq!(
        refusal,
        super::TerminalLaunchRefusal::SessionLimitExceeded { limit: 8 }
    );
    assert_eq!(
        state.terminal_panes.len(),
        8,
        "a refused launch must not add a pane"
    );

    let notice_text = super::terminal_launch_refusal_text(&state.catalog, &refusal);
    assert!(
        notice_text.contains('8'),
        "the refusal notice a user sees must state the real limit, not a generic message: \
         {notice_text:?}"
    );
}

/// **Ablation** for the limit above: with the pre-check and the
/// `add_terminal_session` refusal both bypassed, a 9th real process
/// would be spawned -- confirming the assertions above are load-bearing,
/// not passing for an unrelated reason. Simulated here by calling the
/// real spawn machinery directly past where `attempt_terminal_launch`
/// would have stopped, rather than editing production source for this
/// run (`tekstide-core`'s own `terminal_session_limit_is_enforced_with_a_typed_refusal`
/// ablates the enforcement itself at its real call site).
#[test]
fn ablation_a_ninth_real_process_would_spawn_without_the_limit_check() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("terminal-session-limit-ablation"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    for index in 0..8 {
        super::attempt_terminal_launch(&mut state).unwrap_or_else(|error| {
            panic!("launch {index} must succeed, under the limit: {error:?}")
        });
    }

    // The real project-level check tekstide-core::project::tests::collections
    // ablates directly; this proves the *consequence* would be a real,
    // ninth spawned process if that check were absent, by launching one
    // through the same TerminalPane::launch the production path uses,
    // bypassing only the registration (which would itself refuse).
    let root = fresh_project_dir("terminal-session-limit-ablation-9th");
    let (pane, _session) = crate::surface::terminal::TerminalPane::launch(
        tekstide_core::project::ProjectId::new_uuid(),
        "ablation 9th",
        root,
        PathBuf::from("/bin/sh"),
    )
    .expect(
        "a 9th real shell can always be spawned -- nothing in the OS stops it, which is \
             exactly why the application-level limit above is the only thing that does",
    );
    assert!(
        pane.terminal_id().as_str().starts_with("terminal-"),
        "a real, live process was spawned -- the scenario the limit exists to prevent"
    );
}

/// **Review gate**: "exit detection demonstrated: type exit, session
/// bar shows Exited, slot is freed and reusable by a new launch."
/// Drives the real `TerminalPollTick` handler against a real,
/// just-launched shell, sending it a real `exit` command through the
/// same `write_input` a routed keystroke would use.
#[test]
fn a_real_session_exit_updates_status_frees_the_slot_and_is_reusable() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("terminal-exit-detection"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    super::attempt_terminal_launch(&mut state).expect("the first launch must succeed");
    let terminal_id = state.terminal_panes[0].terminal_id().clone();
    let status_of = |state: &State| {
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .terminal_session(&terminal_id)
            .unwrap()
            .status()
    };
    let slot_of = |state: &State| {
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .terminal_session(&terminal_id)
            .unwrap()
            .visible_slot()
    };
    assert_eq!(
        status_of(&state),
        tekstide_core::domain::TerminalStatus::Running
    );
    assert_eq!(slot_of(&state), tekstide_core::domain::VisibleSlot::Primary);

    state.terminal_panes[0].write_input(b"exit\n");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline
        && status_of(&state) != tekstide_core::domain::TerminalStatus::Exited
    {
        let _ = super::update(&mut state, Message::TerminalPollTick);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert_eq!(
        status_of(&state),
        tekstide_core::domain::TerminalStatus::Exited,
        "typing exit must be reflected as Exited within a few real ticks, not left at Running"
    );
    assert_eq!(
        slot_of(&state),
        tekstide_core::domain::VisibleSlot::Hidden,
        "the visible slot must be freed once the session exits"
    );

    super::attempt_terminal_launch(&mut state).expect("a launch after exit must succeed");
    assert_eq!(state.terminal_panes.len(), 2);
    let new_terminal_id = state.terminal_panes[1].terminal_id().clone();
    assert_eq!(
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .terminal_session(&new_terminal_id)
            .unwrap()
            .visible_slot(),
        tekstide_core::domain::VisibleSlot::Primary,
        "the freed Primary slot must be reusable by a new launch"
    );
}

/// **Ablation** for the test above: with `check_exit` never called (the
/// pre-handoff `poll()`-only behaviour), the session must stay
/// `Running` forever even after the real shell has exited -- "a test
/// that passes with the detection removed is the failure mode this
/// project has hit repeatedly," so this proves the removal is
/// observable, not merely that the addition compiles.
#[test]
fn ablation_without_check_exit_a_dead_shell_still_reports_running() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("terminal-exit-detection-ablation"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    super::attempt_terminal_launch(&mut state).expect("the first launch must succeed");
    let terminal_id = state.terminal_panes[0].terminal_id().clone();
    state.terminal_panes[0].write_input(b"exit\n");
    // Give the real shell time to actually exit at the OS level, same
    // real wait the non-ablated test above uses -- only the detection
    // call (`check_exit`, simulated absent below by only calling
    // `poll()`) is what's missing here.
    std::thread::sleep(std::time::Duration::from_millis(500));
    for pane in &mut state.terminal_panes {
        pane.poll();
    }

    let status = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .terminal_session(&terminal_id)
        .unwrap()
        .status();
    assert_eq!(
        status,
        tekstide_core::domain::TerminalStatus::Running,
        "without check_exit, a dead shell must still (wrongly) report Running -- this is the \
         exact lie the real TerminalPollTick handler exists to prevent"
    );
}

/// **Review gate**: "one creation path -- the demo and the keybinding
/// go through the same function, shown by enumeration rather than
/// asserted." Enumerated the same way P1's single-ingress claim is:
/// `TerminalPane::launch` (the one real spawn call) has exactly **two**
/// production callers, named and justified, not an unbounded or
/// undocumented set:
///
/// - `launch_terminal` -- the one ingress this handoff establishes;
///   both `launch_terminal_demo_panes` (`TEKSTIDE_TERMINAL_DEMO`) and
///   `attempt_terminal_launch` (the real `Ctrl+Alt+T` path) call
///   *this*, not `TerminalPane::launch` directly, which is what makes
///   "the demo and the keybinding go through the same function" true
///   rather than asserted.
/// - `launch_measurement_terminal_pane` -- deliberately **not** folded
///   in, reviewed and approved under RFC-017 PR-017-G: it must not open
///   the real audit store `launch_terminal` does, the same
///   non-contamination principle every other measurement criterion
///   follows. A second call site here is the reviewed exception, not a
///   regression to the parallel-construction shape PR-017-B/C spent two
///   slices proving absent.
#[test]
fn terminal_pane_launch_has_exactly_two_named_production_callers() {
    let shell_rs_path = format!("{}/src/shell.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&shell_rs_path).expect("shell.rs must be readable");
    let lines: Vec<&str> = source.lines().collect();

    // The enclosing function name for each call site, found by scanning
    // upward for the nearest preceding `fn ` -- robust to this file's
    // own line numbers shifting (an unrelated edit elsewhere in the
    // file must not fail this test), unlike asserting exact line
    // numbers would be.
    let mut enclosing_functions: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.contains("TerminalPane::launch(") {
            continue;
        }
        let enclosing = lines[..=index]
            .iter()
            .rev()
            .find_map(|candidate| {
                let trimmed = candidate.trim_start();
                trimmed.strip_prefix("fn ").or_else(|| {
                    trimmed
                        .strip_prefix("async fn ")
                        .or_else(|| trimmed.strip_prefix("pub fn "))
                })
            })
            .and_then(|rest| rest.split(['(', '<', ' ']).next())
            .unwrap_or("<unknown>")
            .to_string();
        enclosing_functions.push(enclosing);
    }

    assert_eq!(
        enclosing_functions.len(),
        2,
        "TerminalPane::launch must have exactly the two named, justified production callers \
         below -- any other count is either a regressed parallel construction path or a \
         caller this test doesn't yet know to name: {enclosing_functions:?}"
    );
    assert!(
        enclosing_functions
            .iter()
            .any(|name| name == "launch_terminal"),
        "launch_terminal's own call site must still exist -- losing it would mean the \
         one-ingress function no longer spawns anything: {enclosing_functions:?}"
    );
    assert!(
        enclosing_functions
            .iter()
            .any(|name| name == "launch_measurement_terminal_pane"),
        "launch_measurement_terminal_pane's own, separately-justified call site must still \
         exist: {enclosing_functions:?}"
    );
}
