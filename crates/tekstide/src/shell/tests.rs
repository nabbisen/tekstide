use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tekstide_core::shell::ApplicationShell;

use super::{
    AgentRunLaunchRefusal, ApprovalDialog, ApprovalDialogButton, ExternalChangeButton, Message,
    ModalButton, ModalContent, PasteConfirmButton, State, TerminalPasteRefusal,
    agent_run_launch_refusal_text, attempt_agent_run_launch_with_profile, content_within_bound,
    evaluate_promotion, focus_marker, main_area_key, main_area_label, modal_scrim_style,
    open_real_audit_store, poll_approval_channels, sidebar_label, status_bar_summary,
    terminal_paste_refusal_text, trusted_ui_state, zone_style,
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
fn layer_demo_focus(modal: &Option<ModalContent>) -> Option<ModalButton> {
    match modal {
        Some(ModalContent::LayerDemo { focus }) => Some(*focus),
        _ => None,
    }
}

#[test]
fn modal_focus_cycling_never_touches_the_shell_focus_cycle() {
    let mut state = state_with(ApplicationShell::new());
    state.modal = Some(ModalContent::default());
    let focus_before = state.focus;

    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(
        layer_demo_focus(&state.modal),
        Some(ModalButton::Acknowledge),
        "ModalFocusNext must cycle the modal's own focus"
    );
    assert_eq!(
        state.focus, focus_before,
        "the shell's own focus cycle must not move while a modal is shown"
    );

    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(layer_demo_focus(&state.modal), Some(ModalButton::Dismiss));
    assert_eq!(state.focus, focus_before);

    let _ = super::update(&mut state, Message::ModalFocusPrevious);
    assert_eq!(
        layer_demo_focus(&state.modal),
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

/// RFC-018 PR-018-G: the scrim style's own output, in isolation --
/// mirrors `zone_style_changes_both_border_colour_and_width_when_focused`'s
/// shape (call the closure directly, inspect the returned
/// `container::Style`), the same technique already established here for
/// testing a style function without constructing a real `Element`.
#[test]
fn modal_scrim_style_paints_the_theme_s_scrim_colour() {
    let theme = crate::theme::Theme::default();
    let base = iced::Theme::Light;

    let style = modal_scrim_style(theme)(&base);

    assert_eq!(
        style.background,
        Some(iced::Background::Color(theme.scrim())),
        "the modal layer's background must be exactly the theme's scrim colour"
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

/// RFC-018 PR-018-G's own review gate: "a test that the scrim is present
/// whenever the paste modal is open, at the layer where the two are
/// bound together, so a future modal added without a scrim fails by
/// name." `view`'s modal branch is that one layer -- every `ModalContent`
/// variant already flows through the same `stack![base, opaque(scrim)]`
/// line unconditionally (no per-variant branching decides whether the
/// scrim applies), so the binding this test needs to prove is that the
/// single shared line still calls `modal_scrim_style` at all, not that
/// each variant does so separately.
///
/// `iced::Element` cannot be introspected after construction (no public
/// API exposes a built widget's configured style), so this uses the same
/// source-scan technique `no_raw_color_construction_anywhere_in_the_crate`
/// already established in this file for an identical class of property
/// -- "is X actually wired at this call site," not "does X exist
/// somewhere." Heuristic, not a full parse: extracts `view`'s own source
/// text between its signature and the next top-level `pub fn`, matching
/// this file's existing "heuristic scan, not a full parser" convention.
///
/// **Ablation** (per this slice's review gate): delete `.style(modal_scrim_style(state.theme))`
/// from `view`'s modal branch in `shell.rs`, rerun -- this test fails,
/// naming `view`. Reverted before committing.
///
/// **What this test cannot see (response 194, Finding 3).** It proves the
/// text `.style(modal_scrim_style(` occurs inside `view`'s body -- it
/// does not prove that style sits on the *full-window* container
/// specifically. Someone later moving the same `.style(...)` call onto
/// an inner, content-sized container (rather than `center(modal_view)`,
/// which fills `Length::Fill`) would keep this test green while the
/// content-independence argument silently died -- the screenshots in
/// `qa-evidence.md`'s PR-018-G section would become the only thing still
/// holding that property, not this test.
#[test]
fn modal_layer_always_applies_the_scrim_style() {
    let source = std::fs::read_to_string(crate_src_dir().join("shell.rs"))
        .expect("shell.rs must be readable");

    let start = source
        .find("pub fn view(state: &State) -> Element<'_, Message> {")
        .expect("shell.rs must still define `view` with this exact signature");
    let after_signature = &source[start..];
    let end = after_signature
        .match_indices("\npub fn ")
        .next()
        .map(|(index, _)| index)
        .unwrap_or(after_signature.len());
    let view_body = &after_signature[..end];

    assert!(
        view_body.contains(".style(modal_scrim_style("),
        "view's modal branch must apply modal_scrim_style to the layer every ModalContent \
         variant shares -- a future modal added here without it would render with no scrim, \
         silently, until someone happened to notice a screenshot"
    );
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

/// RFC-017 Amendment 1, PR-A1-C: the old tick handler polled every
/// tracked pane in one `Message::TerminalPollTick`; the real replacement
/// (`Message::TerminalWoke`) is per-pane, driven by that pane's own wake
/// firing. This helper drives the real handler for every pane
/// `state.terminal_panes` currently tracks, matching the same "poll
/// everything" test convenience the old single-message call gave,
/// without pretending a fixed tick still exists.
fn poll_all_via_real_update(state: &mut State) {
    let terminal_ids: Vec<_> = state
        .terminal_panes
        .iter()
        .map(|pane| pane.terminal_id().clone())
        .collect();
    for terminal_id in terminal_ids {
        let _ = super::update(state, Message::TerminalWoke(terminal_id));
    }
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
/// A hidden pane is still polled -- `poll_all_via_real_update` drives
/// the real `Message::TerminalWoke` handler for every pane in
/// `state.terminal_panes`, not only the visible ones -- and its content
/// is retained across a later slot reassignment, proving "hidden" means
/// "not currently displayed," not "torn down."
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

/// **Ablated**: if the real wake-driven handler only polled visible
/// panes (the alternative to the decision this slice made), the hidden
/// pane's content would never appear. Simulated here by polling only
/// the non-hidden panes directly, confirming the hidden pane's own
/// content is absent -- the failure mode the real, every-tracked-pane
/// subscription (`terminal_wake_subscriptions`) exists to avoid.
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

/// **RFC-018 PR-018-D's required sentinel test**, same shape as
/// PR-017-F's above: a sentinel string checked against **raw on-disk
/// bytes** after the store is dropped (response 152's fix, reused
/// rather than rediscovered -- see that test's own doc comment for why
/// an open, WAL-mode store's `database_file()` alone is not
/// sufficient), with a positive control proving the scan reaches real,
/// persisted content.
///
/// The sentinel is driven through a **real** `TerminalInputPolicy::evaluate`
/// call first, so it genuinely exists as real, classified paste content
/// in scope -- not a decorative local never touched by anything real --
/// before `record_paste_blocked` is called. `record_paste_blocked`'s own
/// signature has no parameter a sentinel could be passed through at all
/// (`project_id`/`terminal_id` only), so this test proves that
/// structural absence holds in practice, the response 152 lesson: do
/// not trust that a type looks safe on paper.
#[test]
fn sentinel_pasted_content_never_reaches_the_durable_audit_store() {
    const SENTINEL_PASTE_CONTENT: &str = "SENTINEL-PASTE-9c4e2a71-blocked-paste-content";

    let target = tekstide_core::domain::TerminalId::new_uuid();
    let project_id = tekstide_core::project::ProjectId::new_uuid();
    let handle =
        tekstide_core::runtime::terminal::TerminalRuntimeHandle::new(target.clone(), project_id);

    // A real, control-containing paste carrying the sentinel -- the
    // real class this producer exists for -- classified by the real
    // `evaluate`, exactly as `Message::TerminalPasteResolved`'s handler
    // does, so the sentinel is genuinely real content in scope at the
    // point the producer is called, not a value never touched by
    // anything.
    let decision = tekstide_core::runtime::terminal::TerminalInputPolicy.evaluate(
        &handle,
        Some(&handle),
        tekstide_core::runtime::terminal::TerminalInputSource::Paste,
        format!("echo \x1b[31m{SENTINEL_PASTE_CONTENT}\x1b[0m").as_bytes(),
        tekstide_core::runtime::terminal::TerminalTrustedUiState::Inactive,
    );
    assert!(
        !format!("{decision:?}").contains(SENTINEL_PASTE_CONTENT),
        "the decision itself must not carry the paste's raw content (paste.rs's own precedent)"
    );

    let mut store = super::open_audit_store(&temp_audit_state_dir("paste-sentinel"), Vec::new())
        .expect("open a real, temp-dir-backed audit store");
    let mut health = tekstide_core::audit::AuditHealth::default();
    tekstide_core::audit::AuditCoordinator::new(&mut store, &mut health)
        .record_paste_blocked(handle.project_id.clone(), target.clone());

    let audit_dir = store.storage_path().audit_dir().to_path_buf();
    drop(store);

    let raw_text = read_every_file_in_dir(&audit_dir);
    assert!(
        raw_text.contains(target.as_str()),
        "the scan must reach the real record this test just wrote -- otherwise the sentinel \
         assertion below passes merely because nothing was read at all"
    );
    assert!(
        !raw_text.contains(SENTINEL_PASTE_CONTENT),
        "pasted content must never reach the raw on-disk audit store"
    );
}

/// RFC-017 PR-017-G response 158, **revised for RFC-017 Amendment 1
/// PR-A1-D** per PR-A1-C's own note that this benchmark's fixed 50ms
/// sampling loop was a stand-in, not a claim about the new mechanism.
/// Still a **headless** benchmark, no GUI or GPU at all -- `poll()`'s
/// own cost (a real PTY read plus `Processor::advance` through
/// `SecurityFilter`) under a real, un-fork-bound flood -- but now driven
/// by the *real* wake mechanism (`WakeNotifier::block_until_woken`, a
/// real `poll(2)` park) instead of an artificial timer, so every sample
/// is a genuine production-shaped wake-to-`poll()` cost rather than a
/// cost sampled once per 50ms regardless of how often real wakes fire.
///
/// **Why this, not the live GUI, is this slice's trustworthy source for
/// wake-handling cost**: two live `TEKSTIDE_MEASURE_CRITERION=terminal_flood`
/// runs during this slice reproduced the exact confound signature
/// PR-017-G's responses 155/156 diagnosed on this same shared machine
/// (swap in the mid-20s GiB both times; one run showed plausible
/// tens-of-microsecond-to-millisecond figures, the other showed
/// `input`/`echo` samples landing on suspicious round ~1s/2s/3s/4s
/// plateaus -- the same major-page-fault signature, not a code defect).
/// `record_tick_handler`'s own `tick` samples, by contrast, stayed
/// consistent across both confounded and non-confounded live runs (low
/// single-digit microseconds p50 both times) precisely because they
/// never touch `iced`'s event loop or the GPU present path -- pure CPU
/// timing, the same property that makes *this* headless benchmark
/// trustworthy where the GUI numbers are not. See `qa-evidence.md`'s
/// PR-A1-D section for the full live-run numbers and the disclosure.
///
/// The only assertion is a generous, order-of-magnitude regression
/// guard -- this is a diagnostic report, not `NFR-PERF-004`'s
/// acceptance test, and asserting a tight bound here would make this
/// flaky against ordinary machine noise.
#[test]
fn terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark() {
    let project_id = tekstide_core::project::ProjectId::new_uuid();
    let (mut pane, _session) = crate::surface::terminal::TerminalPane::launch(
        project_id,
        "headless flood benchmark",
        fresh_project_dir("headless-wake-flood-benchmark"),
        PathBuf::from("/bin/sh"),
    )
    .expect("launch a real shell for the headless benchmark");
    let notifier = pane
        .wake_notifier()
        .expect("a freshly launched pane must be able to clone a wake notifier");

    pane.write_input(super::FLOOD_SCRIPT.as_bytes());

    let benchmark_window = std::time::Duration::from_secs(2);
    let benchmark_started = std::time::Instant::now();
    let mut tick_micros: Vec<u128> = Vec::new();
    while benchmark_started.elapsed() < benchmark_window {
        if !notifier.block_until_woken() {
            break;
        }
        let tick_started = std::time::Instant::now();
        pane.poll();
        tick_micros.push(tick_started.elapsed().as_micros());
    }

    tick_micros.sort_unstable();
    let sample_count = tick_micros.len();
    let p50_micros = tick_micros[sample_count / 2];
    let p99_micros = tick_micros[sample_count * 99 / 100];
    let max_micros = *tick_micros
        .last()
        .expect("at least one wake must have fired");
    let bytes_read_total = pane.bytes_read_total();
    let observed_bytes_per_sec =
        bytes_read_total as f64 / benchmark_started.elapsed().as_secs_f64();

    eprintln!(
        "terminal_poll_wake_driven_headless_benchmark samples={sample_count} p50_us={p50_micros} \
         p99_us={p99_micros} max_us={max_micros} bytes_read_total={bytes_read_total} \
         observed_bytes_per_sec={observed_bytes_per_sec:.0}"
    );

    assert!(
        max_micros < 50_000,
        "a single wake-driven poll() cost blew past a sane bound: max={max_micros}us -- this \
         is a real regression, not measurement noise, since 50ms is already generous headroom \
         over the tens-of-microsecond costs a real read plus VTE advance should take"
    );
}

/// RFC-017 Amendment 1, PR-A1-D, response 209: `terminal_session_limit`'s
/// re-derivation, headless -- "the limit is a throughput/keep-up
/// question, not a paint question," so this deliberately does not touch
/// the GUI runs the rest of this slice found unreliable on this machine.
/// `Some(3)`'s own derivation (`ProjectResourceLimits::default`'s doc)
/// was `~10.1ms/pane linear against a 50ms shared tick period` -- there
/// is no longer a shared tick period to divide by, since PR-A1-C made
/// each pane's wake independent, so this measures the analogous new
/// question directly: launch N real panes, each running
/// [`super::FLOOD_SCRIPT`] concurrently, and drive all N through **one**
/// single-threaded round-robin `poll()` loop -- deliberately not N
/// threads each servicing its own pane, since that would prove nothing
/// about the real constraint: `iced`'s own `update()` is single-threaded
/// in production, so every pane's wake ultimately funnels through one
/// consumer regardless of how many panes exist.
///
/// For each `N` tried, a 1s window reports: per-poll cost distribution
/// (does it rise off the single-pane sub-microsecond floor the benchmark
/// above establishes?) and aggregate observed throughput (does it keep
/// scaling with `N`, or does a shared bottleneck cap the total regardless
/// of how many panes are flooding?). No hard pass/fail assertion on the
/// scaling itself -- the point is the reported numbers, which
/// `qa-evidence.md` states the conclusion from, matching this project's
/// own "measure, don't declare" discipline for this exact kind of
/// capacity question.
#[test]
fn terminal_session_limit_headless_n_pane_wake_throughput_benchmark() {
    for pane_count in [1_usize, 3, 6, 8, 10] {
        let mut panes = Vec::with_capacity(pane_count);
        let mut notifiers = Vec::with_capacity(pane_count);
        for index in 0..pane_count {
            let project_id = tekstide_core::project::ProjectId::new_uuid();
            let (mut pane, _session) = crate::surface::terminal::TerminalPane::launch(
                project_id,
                format!("session-limit benchmark pane {index}"),
                fresh_project_dir(&format!("session-limit-benchmark-{pane_count}-{index}")),
                PathBuf::from("/bin/sh"),
            )
            .expect("launch a real shell for the session-limit benchmark");
            let notifier = pane
                .wake_notifier()
                .expect("a freshly launched pane must be able to clone a wake notifier");
            pane.write_input(super::FLOOD_SCRIPT.as_bytes());
            panes.push(pane);
            notifiers.push(notifier);
        }

        let benchmark_window = std::time::Duration::from_secs(1);
        let benchmark_started = std::time::Instant::now();
        let mut poll_micros: Vec<u128> = Vec::new();
        // Round-robin: one `block_until_woken` per pane per pass, not a
        // single shared wait -- there is no "wait on any of N" primitive
        // exposed, and this shape (poll every pane once, then poll every
        // pane again) is itself a fair proxy for one thread fairly
        // servicing N wake sources, which is what matters here.
        while benchmark_started.elapsed() < benchmark_window {
            for (pane, notifier) in panes.iter_mut().zip(notifiers.iter()) {
                if !notifier.block_until_woken() {
                    continue;
                }
                let poll_started = std::time::Instant::now();
                pane.poll();
                poll_micros.push(poll_started.elapsed().as_micros());
            }
        }

        poll_micros.sort_unstable();
        let sample_count = poll_micros.len();
        let p50_micros = poll_micros
            .get(sample_count / 2)
            .copied()
            .unwrap_or_default();
        let p99_micros = poll_micros
            .get(sample_count * 99 / 100)
            .copied()
            .unwrap_or_default();
        let max_micros = poll_micros.last().copied().unwrap_or_default();
        let aggregate_bytes: u64 = panes.iter().map(|pane| pane.bytes_read_total()).sum();
        let aggregate_bytes_per_sec =
            aggregate_bytes as f64 / benchmark_started.elapsed().as_secs_f64();

        eprintln!(
            "terminal_session_limit_n_pane_benchmark panes={pane_count} samples={sample_count} \
             p50_us={p50_micros} p99_us={p99_micros} max_us={max_micros} \
             aggregate_bytes={aggregate_bytes} aggregate_bytes_per_sec={aggregate_bytes_per_sec:.0}"
        );
    }
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

/// RFC-022 PR-022-D: the real `Ctrl+Alt+A` path against a freshly opened
/// project -- `WorkspaceTrust::Restricted` by default, and nothing in
/// this crate (production or test) can grant trust yet (`grant_trust` is
/// `pub(crate)` to `tekstide-core` alone). `claude_code_linux_default`'s
/// honest `MayDiscoverWorkspaceFiles` policy is therefore refused here
/// every time, regardless of whether an AI CLI happens to be installed
/// on the machine running this suite -- the real, current, disclosed
/// behaviour of this keybinding today, not a gap in this test. Still
/// switches to Terminal Immersion, the same "refused but still lands
/// where the notice is visible" shape `launch_terminal`'s own dispatch
/// arm uses.
#[test]
fn agent_run_launch_shell_input_switches_to_terminal_immersion_and_shows_the_real_trust_refusal() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("agent-run-dispatch");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");

    let mut state = state_with(app_shell);
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::LaunchAgentRun,
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
        "a refused agent run launch must still land in Terminal Immersion, where the notice \
         is visible"
    );
    assert_eq!(
        state.terminal_panes.len(),
        0,
        "a refusal must not add a pane"
    );
    assert!(
        matches!(
            state.agent_run_launch_notice,
            Some(AgentRunLaunchRefusal::Validation(
                tekstide_core::agent::AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked { .. }
            ))
        ),
        "a fresh, untrusted project must refuse with WorkspaceDiscoveryBlocked: {:?}",
        state.agent_run_launch_notice
    );
    let notice = state.agent_run_launch_notice.as_ref().unwrap();
    let notice_text = agent_run_launch_refusal_text(&state.catalog, notice);
    assert!(
        notice_text.to_lowercase().contains("trust"),
        "the refusal a user sees must say this is a trust problem, not a generic error: \
         {notice_text:?}"
    );
}

/// The other side of the same refusal: a fake profile whose executable
/// genuinely does not exist, but whose workspace-discovery policy is
/// `NoKnownWorkspaceDiscovery` (the default `AiCliProfile::new` sets) so
/// the launch reaches executable resolution at all under the default
/// `Restricted` test project -- isolating "no AI CLI found" from the
/// trust gate proven above. Proves `agent_run_launch_refusal_text`
/// renders response 218's own honest first-run message, not a generic
/// one, for the refusal type a real Claude Code profile cannot currently
/// reach (see the previous test) but the refusal machinery itself must
/// still render correctly for.
#[test]
fn agent_run_launch_refusal_text_renders_the_not_found_reason_honestly() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("agent-run-not-found");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let empty_lookup_dir = fresh_project_dir("agent-run-not-found-empty-bin");
    let profile = tekstide_core::agent::AiCliProfile::new(
        "definitely-absent-ai-cli",
        "Definitely Absent AI CLI",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::PathLookup {
            command: "definitely-absent-ai-cli".to_owned(),
            lookup_paths: vec![tekstide_core::agent::ExecutableLookupPath::reviewed_system(
                empty_lookup_dir,
            )],
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    let refusal = attempt_agent_run_launch_with_profile(&mut state, profile)
        .expect_err("an executable that genuinely does not exist must be refused");
    assert!(
        matches!(
            refusal,
            AgentRunLaunchRefusal::Validation(
                tekstide_core::agent::AgentRunLaunchValidationError::ExecutableUnavailable { .. }
            )
        ),
        "expected ExecutableUnavailable, got {refusal:?}"
    );
    let notice_text = agent_run_launch_refusal_text(&state.catalog, &refusal);
    assert!(
        notice_text.to_lowercase().contains("no ai cli found"),
        "response 218: 'no AI CLI found' is the honest, common first-run message, not a \
         generic error: {notice_text:?}"
    );
}

/// **The GUI-side production plumbing this slice's gate names**:
/// `attempt_agent_run_launch`'s downstream chain (`AppState::launch_agent_run_with_runtime`,
/// `TerminalPane::from_launched`, pane registration) genuinely spawns,
/// registers, and selects a real agent run -- proven against a
/// controlled, in-repo test executable via `attempt_agent_run_launch_with_profile`
/// (the same "real spawn machinery, controlled test artifact" shape
/// `tekstide-core`'s own agent tests use), not the real, live Claude
/// Code CLI: the real product needs interactive auth and makes real
/// network calls, which is unsafe and unbounded for an automated test.
/// The profile's `NoKnownWorkspaceDiscovery` policy (the `AiCliProfile::new`
/// default) is what makes this reachable in the default `Restricted`
/// test project without needing trust-granting, which this crate cannot
/// do at all yet (see the dispatch test above).
#[test]
fn attempt_agent_run_launch_with_profile_spawns_registers_and_selects_a_real_run() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("agent-run-launch-real");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let bin_dir = fresh_project_dir("agent-run-launch-real-bin");
    let executable = bin_dir.join("fake-ai-cli");
    std::fs::write(&executable, "#!/bin/sh\n").expect("test executable should be written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("test executable metadata should be readable")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&executable, permissions)
        .expect("test executable permissions should be set");

    let profile = tekstide_core::agent::AiCliProfile::new(
        "fake-ai-cli",
        "Fake AI CLI",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: executable,
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    attempt_agent_run_launch_with_profile(&mut state, profile)
        .expect("a resolvable, trust-compatible profile should launch for real");

    assert_eq!(
        state.terminal_panes.len(),
        1,
        "exactly one real pane must be launched"
    );
    assert!(
        state.agent_run_launch_notice.is_none(),
        "a successful launch must not leave a stale refusal notice"
    );

    let project = state.app_shell.state().active_project().unwrap();
    assert_eq!(project.agent_runs().len(), 1);
    let run = &project.agent_runs()[0];
    assert_eq!(
        run.status,
        tekstide_core::domain::AgentRunStatus::Running,
        "a freshly launched run must already be Running, not left at Preparing forever"
    );
    assert_eq!(
        project.selected_agent_run(),
        Some(&run.id),
        "the just-launched run must become the selected one"
    );
}

/// RFC-022 PR-022-E ("the arrival model"): the reference adapter, real
/// and compiled, spawned through the actual `Managed` launch path this
/// GUI crate now supports end to end -- `structured_action_approval:
/// true` on a `DisabledByLaunch` profile (bypasses the trust gate
/// PR-022-D's own `claude_code_linux_default` cannot get past, the same
/// way `tekstide-core`'s own `built_in_profile` test helper does).
/// Deliberately never the real, live product this pathway is modelled
/// on -- see every other real-process test in this RFC for why.
fn reference_adapter_binary_path() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_reference_adapter") {
        return std::path::PathBuf::from(path);
    }
    let test_exe = std::env::current_exe().expect("current_exe should resolve for a running test");
    let profile_dir = test_exe
        .parent()
        .and_then(Path::parent)
        .expect("test binary should live under target/<profile>/deps");
    let candidate = profile_dir.join("reference_adapter");
    assert!(
        candidate.is_file(),
        "expected the reference_adapter binary at {}; the [[bin]] target may not have built",
        candidate.display()
    );
    candidate
}

/// Launches the real reference adapter through the real, production
/// `Managed` path (`attempt_agent_run_launch_with_profile` ->
/// `AppState::launch_agent_run_with_runtime` -> `register_approval_channel`)
/// and returns the launched run's id. The adapter proposes its own
/// `DEFAULT_PROPOSAL_ARGV` (`echo tekstide-reference-adapter-default-proposal`)
/// -- nothing in the production profile/launch path injects a custom
/// argv (`AiCliPromptPolicy::Argument` has no implementation wiring it
/// into the spawned command line), so every real proposal this helper's
/// callers receive classifies `Low`. Tests that need a `High`/
/// `Destructive` proposal to exercise promotion get a real, received,
/// still-live proposal from this helper first, then override only the
/// GUI-mirrored copy's `risk_level` via `replace_approval_request` --
/// the underlying wire connection and the coordinator's own liveness
/// tracking stay entirely real and untouched, only the locally-cached
/// classification is adjusted, so `is_still_answerable` (what
/// `evaluate_promotion` itself re-checks before promoting) still answers
/// truthfully.
///
/// **Response 228 Required 1**: this override methodology proves
/// promotion reads the mirrored `risk_level` correctly, but never joins
/// real classification to real promotion in the same test -- see
/// `launch_real_managed_agent_run_with_executable` and
/// `a_genuinely_destructive_real_proposal_is_classified_and_promoted_end_to_end`
/// for the one test that does.
fn launch_real_managed_agent_run(state: &mut State) -> tekstide_core::domain::AgentRunId {
    launch_real_managed_agent_run_with_executable(state, reference_adapter_binary_path())
}

/// The same real, production `Managed` launch path as
/// `launch_real_managed_agent_run`, but pointed at a caller-chosen
/// executable rather than always the plain `reference_adapter` binary --
/// what `a_genuinely_destructive_real_proposal_is_classified_and_promoted_end_to_end`
/// uses to launch a thin wrapper script instead, so the adapter's own
/// argv (and therefore the real classifier's input) is test-controlled
/// without touching any production spawn code.
fn launch_real_managed_agent_run_with_executable(
    state: &mut State,
    executable_path: std::path::PathBuf,
) -> tekstide_core::domain::AgentRunId {
    let mut profile = tekstide_core::agent::AiCliProfile::new(
        "reference-adapter",
        "Reference Adapter (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: executable_path,
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Managed,
    );
    profile.adapter_capabilities = tekstide_core::agent::AiCliAdapterCapabilities {
        structured_action_approval: true,
    };
    profile.workspace_discovery_policy =
        tekstide_core::agent::AiCliWorkspaceDiscoveryPolicy::DisabledByLaunch {
            evidence: "test: bypasses the trust gate the same way tekstide-core's own \
                       built_in_profile test helper does"
                .to_owned(),
        };

    attempt_agent_run_launch_with_profile(state, profile)
        .expect("a resolvable Managed profile should launch the real reference adapter");
    let project = state.app_shell.state().active_project().unwrap();
    project.agent_runs().last().unwrap().id.clone()
}

/// Response 228 Required 1: the production launch path never appends
/// arguments to the profile's executable (`spawn_adapter` in
/// `tekstide-core`'s `runtime/terminal/launch.rs` calls
/// `Command::new(&spec.shell)` with no `.arg`/`.args`), so there is no
/// way to inject a custom proposal argv through it directly. What *is*
/// test-controlled is which executable the profile names at all -- this
/// writes a tiny `#!/bin/sh` wrapper that hardcodes a genuinely
/// destructive argv (`rm -rf <marker-path>`, never actually executed:
/// the reference adapter only ever proposes and prints the decision, it
/// never runs the argv it sends) and `exec`s the real
/// `reference_adapter` binary with it. Pointing a profile's executable
/// at this script reaches the real classifier with a real `Destructive`
/// command through the unmodified production spawn path.
///
/// **This safety claim is pinned, not just asserted here in prose**:
/// `reference_adapter_binary_never_executes_the_argv_it_proposes`
/// (`crates/tekstide-core/src/approval/tests/reference_adapter.rs`)
/// fails by name if the reference adapter ever grows a real
/// `Command`/`exec` call site, per response 229.
fn destructive_reference_adapter_wrapper_path() -> std::path::PathBuf {
    let script_path = std::env::temp_dir().join(format!(
        "tekstide-destructive-reference-adapter-wrapper-{}-{}.sh",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let script = format!(
        "#!/bin/sh\nexec {} rm -rf /nonexistent/tekstide-test-destructive-marker\n",
        reference_adapter_binary_path().display()
    );
    std::fs::write(&script_path, script).expect("writing the wrapper script should succeed");
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))
        .expect("marking the wrapper script executable should succeed");
    script_path
}

/// The real, freshly spawned adapter needs a moment to connect and send
/// its proposal over the real socket -- not instantaneous, the same
/// reason `poll_demo_pane_until` retries rather than polling exactly
/// once.
fn poll_approval_channels_until(
    state: &mut State,
    mut condition: impl FnMut(&State) -> bool,
) -> bool {
    for _ in 0..200 {
        poll_approval_channels(state);
        if condition(state) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

/// RFC-022 PR-022-E ("the arrival model"): the real receive pipeline,
/// end to end -- a real `Managed` launch, a real spawned reference
/// adapter, a real proposal over a real socket, received by the real
/// `ApprovalCoordinator` and mirrored into the real `ProjectSession`.
/// `Low` risk (the adapter's own unconfigurable default proposal) must
/// **not** promote -- it stays a queued, `Pending`, live entry.
#[test]
fn a_real_low_risk_proposal_is_received_mirrored_and_stays_queued_without_promoting() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-real-receive");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let agent_run_id = launch_real_managed_agent_run(&mut state);
    assert_eq!(
        state.approval_channels.len(),
        1,
        "a real Managed launch must register a real approval channel"
    );
    let received = poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    assert!(
        received,
        "the real adapter should send its default proposal within the poll window"
    );

    let project = state.app_shell.state().active_project().unwrap();
    assert_eq!(project.approval_requests().len(), 1);
    let request = &project.approval_requests()[0];
    assert_eq!(request.agent_run_id, Some(agent_run_id));
    assert_eq!(request.risk_level, tekstide_core::domain::RiskLevel::Low);
    assert_eq!(
        request.decision,
        tekstide_core::domain::ApprovalDecision::Pending
    );
    assert_eq!(
        state.approval_proposal_ids.len(),
        1,
        "the ApprovalId -> ProposalId bridge must be populated on receipt"
    );
    assert!(
        state.modal.is_none(),
        "a Low-risk proposal must not promote to a modal"
    );
}

/// RFC-022 PR-022-E, response 227's own correction: promotion is not
/// only a point-in-time arrival check. A real, live, received proposal
/// (proven real above) whose GUI-mirrored copy is then classified
/// `Destructive` must promote -- with focus defaulting to `Reject`
/// (`what-the-dialog-must-not-lie-about.md` §"one stray keystroke can
/// only reject") and the post-promotion input-ignore window armed.
#[test]
fn a_destructive_risk_level_promotes_with_focus_defaulting_to_reject() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-promote-destructive");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });

    // Override only the GUI-mirrored copy's classification -- the real
    // wire connection and the coordinator's own liveness tracking are
    // untouched (see `launch_real_managed_agent_run`'s own doc comment).
    let project_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .id()
        .clone();
    let mut request = state
        .app_shell
        .state()
        .project(&project_id)
        .unwrap()
        .approval_requests()[0]
        .clone();
    request.risk_level = tekstide_core::domain::RiskLevel::Destructive;
    state
        .app_shell
        .state_mut()
        .project_mut(&project_id)
        .unwrap()
        .replace_approval_request(request)
        .unwrap();

    evaluate_promotion(&mut state);

    match state.modal {
        Some(ModalContent::Approval(ref dialog)) => {
            assert_eq!(
                dialog.focus,
                ApprovalDialogButton::Reject,
                "focus must default to Reject"
            );
            assert!(
                dialog.ignore_input_until.is_some(),
                "a promoted dialog must arm the post-promotion input-ignore window"
            );
        }
        ref other => panic!("expected a promoted approval dialog, got {other:?}"),
    }
}

/// Response 228 Required 1: every other promotion test overrides only
/// the GUI-mirrored `ApprovalRequest.risk_level`, leaving the
/// coordinator's own copy at whatever the adapter's unconfigurable
/// default (`Low`) actually is -- proving promotion reads the mirror
/// correctly, but never proving the real classifier and the real
/// promotion predicate are joined end to end, and constructing a state
/// (mirror `Destructive`, coordinator `Low`) that can never occur in
/// production. This test does not override anything: a wrapper script
/// (`destructive_reference_adapter_wrapper_path`) makes the real
/// reference adapter propose a real `rm -rf` argv over the real socket,
/// the real `ApprovalCoordinator::receive_proposal` classifies it
/// through the real `approval::risk::classify`, and `evaluate_promotion`
/// promotes from that real, received value alone.
#[test]
fn a_genuinely_destructive_real_proposal_is_classified_and_promoted_end_to_end() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-real-destructive");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    launch_real_managed_agent_run_with_executable(
        &mut state,
        destructive_reference_adapter_wrapper_path(),
    );
    let received = poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    assert!(
        received,
        "the wrapped adapter should send its rm -rf proposal within the poll window"
    );

    let request = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    assert_eq!(
        request.risk_level,
        tekstide_core::domain::RiskLevel::Destructive,
        "the real classifier must reach Destructive for a real `rm -rf` argv, with nothing \
         in this test overriding the mirrored copy -- got {:?}",
        request.risk_level
    );

    evaluate_promotion(&mut state);

    match state.modal {
        Some(ModalContent::Approval(ref dialog)) => {
            assert_eq!(
                dialog.focus,
                ApprovalDialogButton::Reject,
                "focus must default to Reject"
            );
        }
        ref other => panic!("expected a promoted approval dialog, got {other:?}"),
    }
}

/// RFC-022 PR-022-E: the real `decide` round trip -- `ModalActivate`,
/// driven through the real `update()`, must send a real decision over
/// the real socket and mirror the coordinator's own authoritative
/// post-decision value back into `ProjectSession`. Constructs the
/// promoted dialog directly (`ignore_input_until: None`) rather than via
/// `evaluate_promotion`, so this test exercises the decide path in
/// isolation from the input-ignore window, which
/// `modal_input_is_ignored_within_the_post_promotion_window` proves
/// separately.
#[test]
fn deciding_the_promoted_dialog_sends_a_real_decision_and_updates_the_stored_request() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-decide-real");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    let request = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    let proposal_id = state.approval_proposal_ids[&request.id].clone();

    state.modal = Some(ModalContent::Approval(Box::new(ApprovalDialog::for_test(
        request.clone(),
        proposal_id,
        ApprovalDialogButton::Reject,
    ))));

    let _ = super::update(&mut state, Message::ModalActivate);

    assert!(
        state.modal.is_none(),
        "activating must close the dialog regardless of outcome"
    );
    let stored = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    assert_eq!(
        stored.decision,
        tekstide_core::domain::ApprovalDecision::Rejected,
        "the real decide round trip must reach Decided, not Undeliverable/AuditBlocked -- \
         got {stored:?}"
    );
    assert!(stored.decided_at.is_some());
    assert!(
        !state.approval_proposal_ids.contains_key(&request.id),
        "response 228 Required 2: the ApprovalId -> ProposalId bridge entry must be pruned \
         once a decision is real, not left to outlive its own usefulness"
    );
}

/// Response 229, priority item 2: the `command_approval` audit family
/// (`CommandRequest`/`CommandApprove`/`CommandEditAndApprove`/
/// `CommandReject`/`CommandCwdMismatch`, `AuditActionKind`) has been
/// wired with no producer since RFC-021 -- `receive_approval_proposal`/
/// `decide_approval` passing a real `AuditCoordinator` *implies* this
/// pipeline is its first real one, but nothing before this test queried
/// the real store and checked. This does: a real receive (through the
/// real reference adapter) followed by a real `ApprovedOnce` decision
/// (through real `update()`/`ModalActivate` routing, the same path a
/// user's own click drives) must each leave their own durable record
/// behind, identifiable by this run's own `AgentRunId` and the
/// request's own `ApprovalId` -- not just "a record exists somewhere,"
/// which a record for an unrelated action would also satisfy.
#[test]
fn command_approval_family_produces_real_durable_audit_records_through_the_pipeline() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-audit-records");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let agent_run_id = launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    let request = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    let proposal_id = state.approval_proposal_ids[&request.id].clone();

    state.modal = Some(ModalContent::Approval(Box::new(ApprovalDialog::for_test(
        request.clone(),
        proposal_id,
        ApprovalDialogButton::ApproveOnce,
    ))));
    let _ = super::update(&mut state, Message::ModalActivate);
    let stored = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    assert_eq!(
        stored.decision,
        tekstide_core::domain::ApprovalDecision::ApprovedOnce,
        "this test's own records assertion below depends on a real Decided outcome having \
         happened -- got {stored:?}"
    );

    let audit_store =
        open_real_audit_store(&state.app_shell).expect("the real audit store must open");
    let records = audit_store
        .query(&tekstide_core::audit::AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| record.agent_run_id.as_ref() == Some(&agent_run_id))
        .collect::<Vec<_>>();

    let request_record = records
        .iter()
        .find(|record| record.action_kind == tekstide_core::audit::AuditActionKind::CommandRequest)
        .unwrap_or_else(|| {
            panic!("expected a real CommandRequest record for this agent run: {records:?}")
        });
    assert_eq!(request_record.approval_id, Some(request.id.clone()));
    assert_eq!(
        request_record.outcome,
        tekstide_core::audit::AuditOutcome::Requested,
        "a proposal's arrival must be recorded as Requested -- got {:?}",
        request_record.outcome
    );

    // `AuditCoordinator::authorize_command_decision` writes an
    // `Authorized` record first (the actual authorization gate -- a
    // failure here must block the decision entirely, per its own doc
    // comment), then `record_command_decision_outcome` writes a second,
    // best-effort record confirming whether the decision was actually
    // delivered back to the adapter (`Applied`) or not (`Failed`). Both
    // share `action_kind: CommandApprove` for an `ApprovedOnce` decision
    // -- asserting on both, not just "a CommandApprove record exists,"
    // is what proves the real socket delivery happened, not only that
    // the decision was authorized in principle.
    let approve_records: Vec<_> = records
        .iter()
        .filter(|record| {
            record.action_kind == tekstide_core::audit::AuditActionKind::CommandApprove
        })
        .collect();
    assert!(
        approve_records.iter().any(|record| {
            record.outcome == tekstide_core::audit::AuditOutcome::Authorized
                && record.approval_id == Some(request.id.clone())
        }),
        "expected a real, Authorized CommandApprove record for this decision: {records:?}"
    );
    assert!(
        approve_records.iter().any(|record| {
            record.outcome == tekstide_core::audit::AuditOutcome::Applied
                && record.approval_id == Some(request.id.clone())
        }),
        "expected a real, Applied CommandApprove record confirming the decision was actually \
         delivered back to the real adapter over the real socket, not just authorized: \
         {records:?}"
    );
}

/// Response 228 Required 2, the other pruning route. Deliberately uses
/// an **expired**, not decided, first entry: `decide_approval` already
/// prunes its own entry immediately (proven above), so a decided entry
/// would already be gone from the bridge before eviction ever ran,
/// exercising nothing new. Expiry is the one path that leaves a bridge
/// entry in place on purpose (see `approval_proposal_ids`'s own doc
/// comment) while still marking the underlying request terminal/
/// evictable -- so it is the only real way to prove the eviction-side
/// pruning branch specifically, independent of the decide-side one.
#[test]
fn approval_proposal_ids_bridge_entry_is_pruned_when_history_eviction_removes_it() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-bridge-eviction");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let project_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .id()
        .clone();

    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    let first_request = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    assert!(state.approval_proposal_ids.contains_key(&first_request.id));

    {
        let project = state
            .app_shell
            .state_mut()
            .project_mut(&project_id)
            .unwrap();
        project.mark_approval_expired(&first_request.id).unwrap();
    }
    assert!(
        state.approval_proposal_ids.contains_key(&first_request.id),
        "expiry deliberately does not prune the bridge entry -- confirming the test's own \
         premise before eviction is exercised"
    );

    // Force the very next admission to evict: at capacity 1, with the
    // one retained entry now expired (terminal), a second arrival must
    // evict it to make room.
    {
        let project = state
            .app_shell
            .state_mut()
            .project_mut(&project_id)
            .unwrap();
        project.set_resource_limits(tekstide_core::project::ProjectResourceLimits {
            approval_history_limit: Some(1),
            ..project.resource_limits()
        });
    }

    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| {
                project.approval_requests().len() == 1
                    && project.approval_requests()[0].id != first_request.id
            })
    });

    let second_request = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    assert_ne!(second_request.id, first_request.id);
    assert!(
        !state.approval_proposal_ids.contains_key(&first_request.id),
        "the evicted (first) entry's bridge mapping must not reappear or persist"
    );
    assert!(
        state.approval_proposal_ids.contains_key(&second_request.id),
        "the admitted (second) entry's bridge mapping must be present"
    );
    assert_eq!(
        state.approval_proposal_ids.len(),
        1,
        "the bridge must not grow past what is actually retained"
    );
}

/// RFC-022 PR-022-E, response 224's own required guard, extended to
/// promotion (response 227 asked for this explicitly): a `Destructive`
/// proposal belonging to a project that is **not** the active one must
/// not promote, even though every other condition is met.
#[test]
fn a_destructive_proposal_for_a_background_project_does_not_promote() {
    let mut app_shell = ApplicationShell::new();
    let active_project_dir = fresh_project_dir("approval-cross-project-active");
    app_shell
        .add_project_from_path(&active_project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    let background_project_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .id()
        .clone();
    let mut request = state
        .app_shell
        .state()
        .project(&background_project_id)
        .unwrap()
        .approval_requests()[0]
        .clone();
    request.risk_level = tekstide_core::domain::RiskLevel::Destructive;
    state
        .app_shell
        .state_mut()
        .project_mut(&background_project_id)
        .unwrap()
        .replace_approval_request(request)
        .unwrap();

    // A second project is opened and explicitly switched to -- adding a
    // project alone does not change which one is active unless it is the
    // very first project ever added (`AppState::add_project_session` only
    // auto-activates in that case); `switch_active_project` is the real
    // mechanism, disclosed elsewhere as having no GUI-crate caller yet.
    // The first project (holding the real, live, Destructive proposal) is
    // now the background one.
    let second_project_dir = fresh_project_dir("approval-cross-project-second");
    let outcome = state
        .app_shell
        .add_project_from_path(&second_project_dir)
        .expect("a second freshly created directory is a valid project root");
    let second_project_id = match outcome {
        tekstide_core::app::AddProjectOutcome::Added(project_id) => project_id,
        tekstide_core::app::AddProjectOutcome::FocusedExisting(_) => {
            panic!("a freshly created directory must not collide with an existing project")
        }
    };
    assert!(
        state
            .app_shell
            .state_mut()
            .switch_active_project(&second_project_id),
        "the second project must exist to switch to"
    );
    assert_ne!(
        state.app_shell.state().active_project().unwrap().id(),
        &background_project_id,
        "test precondition: the second project must now be active"
    );

    evaluate_promotion(&mut state);

    assert!(
        state.modal.is_none(),
        "a Destructive proposal for a background project must not promote"
    );
}

/// RFC-022 PR-022-E, response 227's required correction: re-evaluation
/// on modal close. A `Destructive` proposal arriving (via risk-level
/// override, same technique as above) while a *different* modal is open
/// must stay queued, not silently downgraded to "never promotes" by
/// arrival timing -- and must promote the moment that modal closes.
#[test]
fn re_evaluation_promotes_a_queued_destructive_proposal_once_a_different_modal_closes() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-reevaluate-on-close");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    // A different modal is already open -- promotion must not happen
    // while it is.
    state.modal = Some(ModalContent::LayerDemo {
        focus: ModalButton::Dismiss,
    });

    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    let project_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .id()
        .clone();
    let mut request = state
        .app_shell
        .state()
        .project(&project_id)
        .unwrap()
        .approval_requests()[0]
        .clone();
    request.risk_level = tekstide_core::domain::RiskLevel::Destructive;
    state
        .app_shell
        .state_mut()
        .project_mut(&project_id)
        .unwrap()
        .replace_approval_request(request)
        .unwrap();

    // `poll_approval_channels` itself calls `evaluate_promotion` at the
    // end of every tick -- confirm it correctly declined to promote
    // while the layer-demo modal was open.
    poll_approval_channels(&mut state);
    assert!(
        matches!(state.modal, Some(ModalContent::LayerDemo { .. })),
        "a Destructive proposal must not promote over an already-open, different modal"
    );

    let _ = super::update(&mut state, Message::ModalDismiss);

    assert!(
        matches!(state.modal, Some(ModalContent::Approval(_))),
        "the queued Destructive proposal must promote the moment the other modal closes, \
         got {:?}",
        state.modal
    );
}

/// RFC-022 PR-022-E, response 227: the post-promotion input-ignore
/// window. A stray `ModalActivate` (Enter) arriving within the window
/// must do nothing -- neither deciding nor closing the dialog.
#[test]
fn modal_input_is_ignored_within_the_post_promotion_window() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-ignore-window");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    let project_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .id()
        .clone();
    let mut request = state
        .app_shell
        .state()
        .project(&project_id)
        .unwrap()
        .approval_requests()[0]
        .clone();
    request.risk_level = tekstide_core::domain::RiskLevel::Destructive;
    state
        .app_shell
        .state_mut()
        .project_mut(&project_id)
        .unwrap()
        .replace_approval_request(request)
        .unwrap();

    evaluate_promotion(&mut state);
    assert!(
        matches!(state.modal, Some(ModalContent::Approval(_))),
        "test precondition: the proposal must have promoted"
    );

    let _ = super::update(&mut state, Message::ModalActivate);

    assert!(
        matches!(state.modal, Some(ModalContent::Approval(_))),
        "a ModalActivate within the ignore window must not close or decide the dialog"
    );
    let stored = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    assert_eq!(
        stored.decision,
        tekstide_core::domain::ApprovalDecision::Pending,
        "no decision may be recorded while the ignore window is still active"
    );
}

/// **Review gate**: "the session limit is enforced in core and
/// demonstrated, including what the user sees on refusal." Runs the
/// default limit (`ProjectResourceLimits::default`, 6 as of RFC-017
/// Amendment 1 PR-A1-D -- a real headless N-pane measurement, not
/// assumption, see that doc comment) to exhaustion with real launches,
/// confirms the typed refusal names the real number, and confirms the
/// refusal notice a user would actually see states that number too --
/// not a generic message that could pass whether or not the real limit
/// made it through.
#[test]
fn terminal_session_limit_is_enforced_end_to_end_with_a_visible_notice() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("terminal-session-limit"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    for index in 0..6 {
        super::attempt_terminal_launch(&mut state).unwrap_or_else(|error| {
            panic!("launch {index} must succeed, under the limit: {error:?}")
        });
    }
    assert_eq!(state.terminal_panes.len(), 6);

    let refusal = super::attempt_terminal_launch(&mut state)
        .expect_err("the 7th launch must be refused once the default limit of 6 is reached");
    assert_eq!(
        refusal,
        super::TerminalLaunchRefusal::SessionLimitExceeded { limit: 6 }
    );
    assert_eq!(
        state.terminal_panes.len(),
        6,
        "a refused launch must not add a pane"
    );

    let notice_text = super::terminal_launch_refusal_text(&state.catalog, &refusal);
    assert!(
        notice_text.contains('6'),
        "the refusal notice a user sees must state the real limit, not a generic message: \
         {notice_text:?}"
    );
}

/// **Ablation** for the limit above: with the pre-check and the
/// `add_terminal_session` refusal both bypassed, a 7th real process
/// would be spawned -- confirming the assertions above are load-bearing,
/// not passing for an unrelated reason. Simulated here by calling the
/// real spawn machinery directly past where `attempt_terminal_launch`
/// would have stopped, rather than editing production source for this
/// run (`tekstide-core`'s own `terminal_session_limit_is_enforced_with_a_typed_refusal`
/// ablates the enforcement itself at its real call site).
#[test]
fn ablation_a_seventh_real_process_would_spawn_without_the_limit_check() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("terminal-session-limit-ablation"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    for index in 0..6 {
        super::attempt_terminal_launch(&mut state).unwrap_or_else(|error| {
            panic!("launch {index} must succeed, under the limit: {error:?}")
        });
    }

    // The real project-level check tekstide-core::project::tests::collections
    // ablates directly; this proves the *consequence* would be a real,
    // seventh spawned process if that check were absent, by launching one
    // through the same TerminalPane::launch the production path uses,
    // bypassing only the registration (which would itself refuse).
    let root = fresh_project_dir("terminal-session-limit-ablation-7th");
    let (pane, _session) = crate::surface::terminal::TerminalPane::launch(
        tekstide_core::project::ProjectId::new_uuid(),
        "ablation 7th",
        root,
        PathBuf::from("/bin/sh"),
    )
    .expect(
        "a 7th real shell can always be spawned -- nothing in the OS stops it, which is \
             exactly why the application-level limit above is the only thing that does",
    );
    assert!(
        pane.terminal_id().as_str().starts_with("terminal-"),
        "a real, live process was spawned -- the scenario the limit exists to prevent"
    );
}

/// **Review gate**: "exit detection demonstrated: type exit, session
/// bar shows Exited, slot is freed and reusable by a new launch."
/// Drives the real `TerminalWoke` handler against a real, just-launched
/// shell, sending it a real `exit` command through the same
/// `write_input` a routed keystroke would use.
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
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert_eq!(
        status_of(&state),
        tekstide_core::domain::TerminalStatus::Exited,
        "typing exit must be reflected as Exited within a few real wakes, not left at Running"
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
         exact lie the real TerminalWoke handler exists to prevent"
    );
}

/// **Non-blocking recommendation, review response 163.** `check_exit`'s
/// non-blocking claim (`Duration::ZERO` degrades `wait_for_exit`'s retry
/// loop to a single non-blocking `try_wait()`) is a real property of
/// that loop as written, not a contract of it -- if the loop's
/// `elapsed() > timeout` guard ever changed to `>=`, this would
/// silently reintroduce a 10ms blocking sleep per live pane per tick,
/// on top of the tick-budget cost `ProjectResourceLimits::default`'s
/// `terminal_session_limit` doc comment already accounts for. Pins the
/// property with a real timer against a real, still-running shell
/// (never called `exit`), with a bound generous enough not to be flaky
/// under CI load but tight enough that a reintroduced 10ms sleep trips
/// it.
#[test]
fn check_exit_on_a_live_shell_returns_without_blocking() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("terminal-check-exit-non-blocking"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    super::attempt_terminal_launch(&mut state).expect("the launch must succeed");

    let started = std::time::Instant::now();
    let outcome = state.terminal_panes[0].check_exit();
    let elapsed = started.elapsed();

    assert_eq!(
        outcome, None,
        "a shell that was never told to exit must not report an outcome"
    );
    assert!(
        elapsed < std::time::Duration::from_millis(5),
        "check_exit on a live shell took {elapsed:?} -- the 10ms WouldBlock sleep this test \
         exists to catch appears to have leaked back into the Duration::ZERO path"
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

// RFC-018 PR-018-B: paste ingress. `pr-018-b-paste-ingress.md`'s own
// review gate, worked through below in the order it lists them.

/// Shared by the two enumeration tests below so their scanning logic
/// cannot drift apart from each other -- the exact failure mode
/// `pr-018-b-paste-ingress.md` warns against for the write-site guard
/// itself ("two arms, two guards, and the second one drifts") applies
/// just as well to two copies of the same test helper.
fn enclosing_functions_for_call_site(source: &str, needle: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut enclosing_functions = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.contains(needle) {
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
    enclosing_functions
}

/// **Review gate**: "the starting state confirmed: `TerminalInputPolicy`
/// had no production caller before this slice, shown by enumeration."
/// Before this slice, `grep -rn "TerminalInputPolicy" crates/tekstide/src`
/// matched nothing outside this file's own new code -- not testable
/// after the fact, so what this test pins instead is the state that
/// claim left behind: **exactly one** production `.evaluate(` call
/// site, inside the one function that now owns the policy decision. A
/// second real call site -- a classifier growing anywhere else in this
/// crate -- fails this test by name, not by inspection.
#[test]
fn terminal_input_policy_evaluate_has_exactly_one_production_call_site() {
    let shell_rs_path = format!("{}/src/shell.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&shell_rs_path).expect("shell.rs must be readable");
    let enclosing_functions = enclosing_functions_for_call_site(&source, ".evaluate(");

    assert_eq!(
        enclosing_functions,
        vec!["update"],
        "TerminalInputPolicy::evaluate must have exactly one production call site, inside \
         `update` -- any other count or location is a second classifier this crate must not \
         grow: {enclosing_functions:?}"
    );
}

/// RFC-019 PR-019-B's own review gate: "the starting state confirmed:
/// the content-model accessors had no production caller, shown by
/// enumeration." Confirmed before writing any code in this slice --
/// `grep -rn "scan_active_project_explorer_directory\|open_active_project_text_document\|
/// replace_active_project_text\|save_active_project_text_document\|refresh_active_project_text_document"
/// crates/tekstide crates/tekstide-core` matched only `#[cfg(test)]`
/// call sites and the `AppState`/`ApplicationShell` definitions
/// themselves. What this test pins is the state that confirmation left
/// behind, the same shape
/// [`terminal_input_policy_evaluate_has_exactly_one_production_call_site`]
/// uses: **exactly two** named, intentional production
/// `.scan_active_project_explorer_directory(` call sites --
/// [`ensure_explorer_scanned`] (the first scan, triggered on entering
/// Content mode with none yet) and [`handle_explorer_key`] (a rescan,
/// triggered by the user selecting a directory) -- named explicitly
/// rather than hidden, the same shape
/// `write_terminal_input_has_exactly_the_three_named_production_call_sites`
/// uses for a different property. A third call site fails this test by
/// name.
#[test]
fn scan_active_project_explorer_directory_has_exactly_the_two_named_production_call_sites() {
    let shell_rs_path = format!("{}/src/shell.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&shell_rs_path).expect("shell.rs must be readable");
    let enclosing_functions =
        enclosing_functions_for_call_site(&source, ".scan_active_project_explorer_directory(");

    assert_eq!(
        enclosing_functions,
        vec!["ensure_explorer_scanned", "handle_explorer_key"],
        "scan_active_project_explorer_directory must have exactly these two named production \
         call sites: {enclosing_functions:?}"
    );
}

/// RFC-018 PR-018-D: mirrors the two enumeration tests above for the
/// audit producer. Exactly one `.record_paste_blocked(` call site,
/// inside `update`, guarded by the `TerminalPasteRefusal::Blocked`
/// match arm -- never called for `RequiresConfirmation` (not a real
/// policy refusal, just this slice's temporary conservative block) or
/// `TooLarge` (a shell-level bound that never reached `evaluate`, so it
/// has no `TerminalInputDecisionReason` this family's fixed
/// `reason_code` could honestly represent).
#[test]
fn record_paste_blocked_has_exactly_one_production_call_site() {
    let shell_rs_path = format!("{}/src/shell.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&shell_rs_path).expect("shell.rs must be readable");
    let enclosing_functions = enclosing_functions_for_call_site(&source, ".record_paste_blocked(");

    assert_eq!(
        enclosing_functions,
        vec!["update"],
        "record_paste_blocked must have exactly one production call site, inside `update`: \
         {enclosing_functions:?}"
    );
}

/// **Review gate**: "one PTY ingress, enumerated mechanically and
/// ablated -- a synthetic second call site fails the test." Typed
/// keystrokes and a resolved, `Allow`ed paste both write through
/// `write_terminal_input`, the one function real, modal-gated user
/// input reaches a PTY from. Two more call sites already existed before
/// this slice, reviewed under RFC-017 PR-017-G, and are named here so
/// they cannot be mistaken for a second real ingress: `update`'s
/// `MeasuredTerminalInput` arm and `launch_measurement_terminal_pane`'s
/// `FLOOD_SCRIPT` write, both synthetic-measurement paths that
/// deliberately bypass `TextStream`/routing entirely (their own doc
/// comments say so) and were never in scope for this slice to fold in.
/// A parallel `write_paste` on `TerminalPane`, or a second inline
/// `.write_input(` call in a new message arm -- the two traps
/// `pr-018-b-paste-ingress.md` names by name -- would each add a
/// fourth entry here and fail this assertion.
#[test]
fn write_terminal_input_has_exactly_the_three_named_production_call_sites() {
    let shell_rs_path = format!("{}/src/shell.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&shell_rs_path).expect("shell.rs must be readable");
    let enclosing_functions = enclosing_functions_for_call_site(&source, ".write_input(");

    assert_eq!(
        enclosing_functions.len(),
        3,
        "TerminalPane::write_input must have exactly the three named production call sites \
         below -- any other count is either a regressed second real ingress or a caller this \
         test doesn't yet know to name: {enclosing_functions:?}"
    );
    for expected in [
        "write_terminal_input",
        "update",
        "launch_measurement_terminal_pane",
    ] {
        assert!(
            enclosing_functions.iter().any(|name| name == expected),
            "{expected}'s own call site must still exist: {enclosing_functions:?}"
        );
    }
}

/// **Review gate**: "modal exclusivity re-proven with a real paste
/// against a real `TerminalPane`, not headless." Mirrors
/// `modal_open_blocks_pty_write_and_closing_it_resumes_delivery` exactly
/// -- same real pane, same "resumes afterward" second half ruling out
/// "the pane was simply broken" as the reason nothing appeared -- but
/// driving the paste path (`Message::TerminalPasteResolved`) instead of
/// a keystroke.
#[test]
fn modal_open_blocks_paste_write_and_closing_it_resumes_delivery() {
    let mut state = state_with_a_real_terminal_focused("live-paste-modal-exclusivity");
    let real_id = state
        .terminal_panes
        .first()
        .expect("test precondition")
        .terminal_id()
        .clone();
    state.modal = Some(ModalContent::default());

    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target: real_id.clone(),
            content: Some("paste-while-modal-open".to_string()),
        },
    );
    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        !rendered_demo_pane_text(&state).contains("paste-while-modal-open"),
        "a paste resolved while a modal is open must never reach the PTY"
    );

    state.modal = None;
    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target: real_id,
            content: Some("paste-after-modal-closed".to_string()),
        },
    );
    assert!(
        poll_demo_pane_until(&mut state, "paste-after-modal-closed"),
        "the same target, resolved after the modal closes, must reach the PTY -- proving the \
         pane itself was never broken and the earlier silence was the modal check, not a fluke"
    );
}

/// **Review gate**: "no paste classification anywhere in
/// `crates/tekstide`; every decision originates from `evaluate`" and
/// "each `TerminalPasteClass` exercised against real bytes." All four
/// classes, driven through the real `update` handler against a real
/// pane -- `Allow`-class content lands in the PTY, `RequiresConfirmation`
/// and `Block` do not, and each records the visible notice the review
/// gate requires.
#[test]
fn single_line_paste_is_allowed_and_reaches_the_pty() {
    let mut state = state_with_a_real_terminal_focused("paste-class-single-line");
    let target = state.terminal_panes[0].terminal_id().clone();

    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target,
            content: Some("echo single-line-paste".to_string()),
        },
    );
    assert!(
        poll_demo_pane_until(&mut state, "single-line-paste"),
        "a single-line paste must be Allowed and reach the real PTY"
    );
    assert_eq!(
        state.terminal_paste_notice, None,
        "an Allowed paste must never leave a refusal notice behind"
    );
}

#[test]
fn empty_paste_is_allowed_and_is_a_harmless_no_op() {
    let mut state = state_with_a_real_terminal_focused("paste-class-empty");
    let target = state.terminal_panes[0].terminal_id().clone();

    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target,
            content: None,
        },
    );
    assert_eq!(
        state.terminal_paste_notice, None,
        "an empty clipboard must classify Empty -> Allow, not a refusal"
    );
}

/// RFC-018 PR-018-C: `RequiresConfirmation` now opens the real dialog
/// instead of PR-018-B's temporary block-and-notice. Nothing reaches
/// the PTY yet -- only `ModalActivate` on `Accept` (tested separately)
/// writes -- and no refusal notice is recorded, since this isn't a
/// refusal at all.
#[test]
fn multiline_paste_opens_the_confirmation_dialog_and_writes_nothing_yet() {
    let mut state = state_with_a_real_terminal_focused("paste-class-multiline");
    let target = state.terminal_panes[0].terminal_id().clone();

    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target: target.clone(),
            content: Some("first-line\nsecond-line".to_string()),
        },
    );
    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        !rendered_demo_pane_text(&state).contains("first-line"),
        "opening the dialog must not write anything -- only Accept does"
    );
    assert_eq!(
        state.terminal_paste_notice, None,
        "RequiresConfirmation is not a refusal now that a real dialog exists for it"
    );
    match &state.modal {
        Some(ModalContent::PasteConfirmation(modal)) => {
            assert_eq!(modal.target, target);
            assert_eq!(modal.content, "first-line\nsecond-line");
            assert_eq!(modal.line_count, 2);
            assert_eq!(
                modal.focus,
                PasteConfirmButton::Reject,
                "defaults to the safe target, matching the layer-demo modal's own convention"
            );
        }
        other => panic!("expected a PasteConfirmation modal, got {other:?}"),
    }
}

/// Shared setup for the dialog-behaviour tests below: a real terminal
/// pane, and a real `RequiresConfirmation` decision already turned into
/// an open `PasteConfirmation` modal via the real `TerminalPasteResolved`
/// handler -- not a hand-built `State { modal: Some(...), .. }`, so
/// these tests exercise the same path a real paste actually takes.
fn state_with_paste_dialog_open(
    label: &str,
    content: &str,
) -> (State, tekstide_core::domain::TerminalId) {
    let mut state = state_with_a_real_terminal_focused(label);
    let target = state.terminal_panes[0].terminal_id().clone();
    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target: target.clone(),
            content: Some(content.to_string()),
        },
    );
    assert!(
        matches!(state.modal, Some(ModalContent::PasteConfirmation(_))),
        "test precondition: the dialog must be open"
    );
    (state, target)
}

fn paste_confirm_focus(modal: &Option<ModalContent>) -> Option<PasteConfirmButton> {
    match modal {
        Some(ModalContent::PasteConfirmation(modal)) => Some(modal.focus),
        _ => None,
    }
}

/// **Review gate**: "every dismissal path defaults to not pasting --
/// Escape, click-away, focus loss, and any other exit that is not an
/// explicit accept must leave the PTY untouched. Test each exit path,
/// not one representative." Two real exit paths exist in this shell
/// (`ModalDismiss`/Escape, and `ModalActivate` while focus is on
/// `Reject`) -- "click-away" and "focus loss" have no reachable trigger
/// here at all: this shell has no mouse-click handling anywhere, and
/// modal focus is structurally isolated from `state.focus` (proven by
/// `modal_focus_cycling_never_touches_the_shell_focus_cycle`), so there
/// is no window-blur-equivalent event to test. Both are disclosed
/// non-goals here, not silently skipped.
#[test]
fn escape_dismisses_the_paste_dialog_without_writing_even_with_accept_focused() {
    let (mut state, target) = state_with_paste_dialog_open("paste-dialog-escape", "one\ntwo");
    // Focus Accept first -- proving Escape overrides whatever is
    // focused, not merely that it coincides with the safe default.
    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(
        paste_confirm_focus(&state.modal),
        Some(PasteConfirmButton::Accept)
    );

    let _ = super::update(&mut state, Message::ModalDismiss);
    assert!(state.modal.is_none(), "Escape must close the dialog");

    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        !rendered_demo_pane_text(&state).contains("one"),
        "Escape must never write, even when Accept was the focused button"
    );
    let _ = target;
}

#[test]
fn activating_reject_dismisses_the_paste_dialog_without_writing() {
    let (mut state, _target) = state_with_paste_dialog_open("paste-dialog-reject", "one\ntwo");
    assert_eq!(
        paste_confirm_focus(&state.modal),
        Some(PasteConfirmButton::Reject),
        "test precondition: Reject is the default focus"
    );

    let _ = super::update(&mut state, Message::ModalActivate);
    assert!(
        state.modal.is_none(),
        "activating Reject must close the dialog"
    );

    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!rendered_demo_pane_text(&state).contains("one"));
}

/// **Review gate**: "the accept path is the only thing that writes, and
/// it still goes through PR-018-B's single ingress rather than a new
/// one." Proven against a real pane: the exact content the dialog held
/// reaches the PTY, through `write_terminal_input`
/// (`record_paste_blocked_has_exactly_one_production_call_site`'s
/// sibling enumeration test already pins this as the one write site).
#[test]
fn activating_accept_writes_the_real_pasted_content_and_closes_the_dialog() {
    let (mut state, _target) = state_with_paste_dialog_open(
        "paste-dialog-accept",
        "echo accepted-paste-content\nsecond-line",
    );
    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(
        paste_confirm_focus(&state.modal),
        Some(PasteConfirmButton::Accept)
    );

    let _ = super::update(&mut state, Message::ModalActivate);
    assert!(state.modal.is_none(), "accepting must close the dialog");

    assert!(
        poll_demo_pane_until(&mut state, "accepted-paste-content"),
        "Accept must write the real content the dialog held, through the real PTY"
    );
}

/// **Review gate**: "focus cycle demonstrated across the dialog's
/// controls... proving the cycle returns." Two buttons: one `next`
/// returns to start, matching `modal_focus_cycling_never_touches_the_shell_focus_cycle`'s
/// own convention for the layer-demo modal.
#[test]
fn paste_dialog_focus_cycles_between_accept_and_reject_and_returns() {
    let (mut state, _target) = state_with_paste_dialog_open("paste-dialog-focus-cycle", "one\ntwo");
    assert_eq!(
        paste_confirm_focus(&state.modal),
        Some(PasteConfirmButton::Reject)
    );

    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(
        paste_confirm_focus(&state.modal),
        Some(PasteConfirmButton::Accept)
    );

    let _ = super::update(&mut state, Message::ModalFocusNext);
    assert_eq!(
        paste_confirm_focus(&state.modal),
        Some(PasteConfirmButton::Reject),
        "cycling forward twice through a two-item cycle must return to the start"
    );

    let _ = super::update(&mut state, Message::ModalFocusPrevious);
    assert_eq!(
        paste_confirm_focus(&state.modal),
        Some(PasteConfirmButton::Accept),
        "Shift+Tab must also cycle, in the opposite direction"
    );
}

/// **Review gate**: "pasted content in the dialog goes through
/// `text_safety::quote_untrusted`... test a bidi/control case
/// specifically -- a paste containing `\u{202E}` must render escaped
/// and does not reorder the dialog's own text." Same fixture class this
/// project's own recent-projects state already exercises for real
/// (response 166), driven through the real preview computation rather
/// than asserted structurally.
#[test]
fn paste_preview_escapes_a_bidi_override_character() {
    let raw = "safe\u{202E}spoofed";
    let (preview, truncated) = super::paste_preview(raw);

    assert!(!truncated);
    assert!(
        !preview.contains('\u{202E}'),
        "the raw bidi override character must not survive escaping unescaped"
    );
    assert!(
        preview.contains("safe") && preview.contains("spoofed"),
        "escaping must not delete the surrounding content, only make the override visible: \
         {preview:?}"
    );
}

#[test]
fn paste_preview_truncates_long_content_and_reports_it() {
    let long = "a".repeat(super::PASTE_PREVIEW_CHAR_LIMIT + 50);
    let (preview, truncated) = super::paste_preview(&long);

    assert!(truncated);
    // The isolate wrapper (`quote_untrusted`) adds two characters of its
    // own around the content, so the bound is on the content length, not
    // the wrapped string's.
    assert!(preview.chars().count() <= super::PASTE_PREVIEW_CHAR_LIMIT + 2);
}

#[test]
fn paste_preview_does_not_truncate_content_within_the_limit() {
    let short = "a".repeat(super::PASTE_PREVIEW_CHAR_LIMIT);
    let (_preview, truncated) = super::paste_preview(&short);
    assert!(!truncated);
}

/// **Review gate**: "`NFR-UX-002`: the accept/reject distinction is not
/// colour-only." The textual marker (`"> "` vs `"  "`) is the whole
/// distinguishing channel, the same mechanism the layer-demo modal and
/// the shell's own chrome-level focus indicator already use -- proven
/// here by confirming the marker actually moves between the two labels
/// as focus changes, not merely that it exists once.
#[test]
fn paste_dialog_accept_reject_distinction_is_a_real_textual_marker_not_colour_only() {
    let (mut state, _target) = state_with_paste_dialog_open("paste-dialog-marker", "one\ntwo");
    let accept_label = state.catalog.get("paste-confirm-dialog-accept");
    let reject_label = state.catalog.get("paste-confirm-dialog-reject");

    let render = |state: &State| {
        let Some(ModalContent::PasteConfirmation(modal)) = &state.modal else {
            panic!("test precondition: dialog must be open");
        };
        let marker = |target: PasteConfirmButton| {
            if modal.focus == target { "> " } else { "  " }
        };
        (
            format!("{}{accept_label}", marker(PasteConfirmButton::Accept)),
            format!("{}{reject_label}", marker(PasteConfirmButton::Reject)),
        )
    };

    let (accept_line, reject_line) = render(&state);
    assert!(accept_line.starts_with("  "));
    assert!(reject_line.starts_with("> "));

    let _ = super::update(&mut state, Message::ModalFocusNext);
    let (accept_line, reject_line) = render(&state);
    assert!(
        accept_line.starts_with("> ") && reject_line.starts_with("  "),
        "the marker must move to whichever button is now focused"
    );
}

/// **Review gate**: "every user-facing word goes through `Catalog`; no
/// hardcoded English at the render layer." Structurally true by
/// construction (every string `paste_confirmation_modal_view` renders
/// comes from `state.catalog.get`/`get_with_args`, never a Rust string
/// literal) -- this test is the same kind of check
/// `window_title_resolves_through_the_catalog_not_a_literal` already
/// runs: if a key were mistyped, the catalog's "missing key renders as
/// the key itself" fallback makes the assertion below fail loudly.
#[test]
fn paste_dialog_body_resolves_through_the_catalog_with_the_real_line_count() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let text = catalog.get_with_args(
        "paste-confirm-dialog-body",
        &crate::i18n::CatalogArgs::new().number("line_count", 3u32),
    );
    assert_ne!(text, "paste-confirm-dialog-body");
    assert!(text.contains('3'));
}

#[test]
fn control_containing_paste_is_blocked_outright() {
    let mut state = state_with_a_real_terminal_focused("paste-class-control");
    let target = state.terminal_panes[0].terminal_id().clone();

    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target,
            content: Some("echo \x1b[31mred\x1b[0m".to_string()),
        },
    );
    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        !rendered_demo_pane_text(&state).contains("red"),
        "a control-containing paste must block outright, never confirm"
    );
    let refusal = TerminalPasteRefusal::Blocked(
        tekstide_core::runtime::terminal::TerminalInputDecisionReason::ControlContainingPasteBlocked,
    );
    assert_eq!(state.terminal_paste_notice, Some(refusal.clone()));
    assert!(
        !terminal_paste_refusal_text(&state.catalog, &refusal).is_empty(),
        "the user pressed a key and is owed a visible answer, not an empty string"
    );
}

/// **Review gate**: "the real `TerminalTrustedUiState` is passed,
/// derived in one place, never hardcoded `Inactive`." A paste whose
/// `target` names a real, live pane but does **not** match the
/// terminal `active_terminal_focus` resolves to right now must be
/// blocked by `evaluate`'s own cross-terminal check -- proving the
/// value passed is a real, freshly-derived handle, not a constant that
/// would trivially agree with itself.
#[test]
fn a_paste_targeting_a_different_terminal_than_the_one_focused_now_is_blocked() {
    let mut state = state_with_a_real_terminal_focused("paste-wrong-terminal");
    let stale_target = tekstide_core::domain::TerminalId::new_uuid();

    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target: stale_target,
            content: Some("should-not-appear".to_string()),
        },
    );
    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!rendered_demo_pane_text(&state).contains("should-not-appear"));
    assert_eq!(
        state.terminal_paste_notice,
        Some(TerminalPasteRefusal::Blocked(
            tekstide_core::runtime::terminal::TerminalInputDecisionReason::WrongTerminal
        ))
    );
}

#[test]
fn trusted_ui_state_is_inactive_without_a_modal() {
    let state = state_with(ApplicationShell::new());
    assert_eq!(
        trusted_ui_state(&state),
        tekstide_core::runtime::terminal::TerminalTrustedUiState::Inactive
    );
}

#[test]
fn trusted_ui_state_is_active_with_a_modal_open() {
    let mut state = state_with(ApplicationShell::new());
    state.modal = Some(ModalContent::default());
    assert_ne!(
        trusted_ui_state(&state),
        tekstide_core::runtime::terminal::TerminalTrustedUiState::Inactive,
        "any open modal must produce an active TerminalTrustedUiState, never Inactive"
    );
}

/// **Review gate**: "clipboard read is bounded; a very large clipboard
/// cannot become an unbounded write or render." **Response 169
/// Required**: bounded by refusal, not truncation -- an over-cap paste
/// must never reach `evaluate` at all, since classifying a truncated
/// prefix can change the classification itself and would silently
/// write a prefix of what the user actually copied.
#[test]
fn content_within_bound_passes_short_content_through_unchanged() {
    assert_eq!(
        content_within_bound(Some("hello".to_string())),
        Some("hello".to_string())
    );
}

#[test]
fn content_within_bound_treats_a_missing_clipboard_as_empty() {
    assert_eq!(content_within_bound(None), Some(String::new()));
}

#[test]
fn content_within_bound_accepts_content_exactly_at_the_cap() {
    let exact = "a".repeat(256 * 1024);
    assert_eq!(
        content_within_bound(Some(exact)).map(|content| content.len()),
        Some(256 * 1024)
    );
}

#[test]
fn content_within_bound_refuses_content_over_the_cap() {
    let oversized = "a".repeat(256 * 1024 + 1);
    assert_eq!(
        content_within_bound(Some(oversized)),
        None,
        "content one byte over the cap must be refused whole, not truncated to the cap"
    );
}

/// The end-to-end proof the unit test above cannot give alone: a real,
/// over-cap paste resolved against a real pane never reaches
/// `evaluate` and never writes a byte -- refused, not truncated and
/// classified.
#[test]
fn an_oversized_paste_is_refused_whole_and_reaches_neither_evaluate_nor_the_pty() {
    let mut state = state_with_a_real_terminal_focused("paste-too-large");
    let target = state.terminal_panes[0].terminal_id().clone();
    let oversized = "echo ".to_string() + &"a".repeat(256 * 1024 + 1);

    let _ = super::update(
        &mut state,
        Message::TerminalPasteResolved {
            target,
            content: Some(oversized),
        },
    );
    for _ in 0..20 {
        for pane in &mut state.terminal_panes {
            pane.poll();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        !rendered_demo_pane_text(&state).contains("echo"),
        "an over-cap paste must write nothing at all, not a truncated prefix"
    );
    assert_eq!(
        state.terminal_paste_notice,
        Some(TerminalPasteRefusal::TooLarge)
    );
}

/// **Review gate**: "the paste keybinding collides with nothing" is
/// `navigation::tests`'s job (mechanical, against the whole table); this
/// proves the *shell* side of the same binding does nothing destructive
/// when there is nowhere to paste -- no active project, so no terminal
/// can possibly be focused.
#[test]
fn paste_keybinding_with_no_terminal_focused_is_a_silent_noop() {
    let mut state = state_with(ApplicationShell::new());
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(
            crate::input::shell_input_for_test(
                tekstide_core::navigation::NavigationAction::PasteIntoTerminal,
            ),
        )),
    );
    assert_eq!(state.terminal_paste_notice, None);
}

// --- RFC-019 PR-019-D: editing and save ---

fn state_with_an_open_document(label: &str, contents: &str) -> (State, PathBuf) {
    let dir = fresh_project_dir(label);
    std::fs::write(dir.join("file.txt"), contents).unwrap();
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(&dir)
        .expect("a freshly created directory is a valid project root");
    app_shell
        .open_active_project_text_document("file.txt")
        .expect("the fixture file must open");
    let mut state = state_with(app_shell);
    state.focus = FocusZone::MainArea;
    (state, dir)
}

fn active_document_text(state: &State) -> String {
    state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| project.content_workspace().active_document())
        .expect("an active document must exist")
        .text()
        .to_string()
}

/// A real key, routed by the real router (not `apply_edit_key` called
/// directly, and not a hand-built `SurfaceInput` -- that type has no
/// test constructor, deliberately, per `input`'s own module doc) reaches
/// the active document through `handle_editor_key`. Proves the
/// `FocusZone::MainArea` wiring this slice added, the same shape
/// `tab_cycles_shell_focus_with_a_real_terminal_focused_and_writes_nothing`
/// already uses for its own zone. A freshly opened document's real
/// cursor is `(0, 0)` (RFC-006 Amendment 1) -- cursor-aware insertion at
/// that real position produces `"!hello"`, not `"hello!"`; the append
/// that append-only editing always produced is gone.
#[test]
fn a_typed_key_edits_the_real_active_document_through_real_routing() {
    let (mut state, _dir) = state_with_an_open_document("editor-real-typed-key", "hello");
    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key: iced::keyboard::Key::Character("!".into()),
        modifiers: iced::keyboard::Modifiers::empty(),
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed =
        crate::input::route_non_modal_input(proof, &policy, state.focus, None, press.clone());
    assert_eq!(
        routed,
        crate::input::RoutedInput::Surface(crate::input::surface_input_for_test(
            FocusZone::MainArea,
            press
        )),
        "an ordinary character with MainArea focused and no terminal focus must route to Surface"
    );

    let _ = super::update(&mut state, Message::Input(routed));

    assert_eq!(active_document_text(&state), "!hello");
}

fn active_document_cursor(state: &State) -> tekstide_core::content::TextCursor {
    state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| project.content_workspace().active_document())
        .expect("an active document must exist")
        .cursor()
}

/// **RFC-006 Amendment 1's own point**: real mid-buffer editing, driven
/// entirely through real keys and the real router -- not the pure
/// `apply_edit_key`/`navigate_cursor` functions called directly (those
/// have their own unit tests in `surface/editor/tests.rs`). A real
/// ArrowRight moves the real cursor via `set_active_project_cursor`;
/// the real character typed next inserts exactly there, not at the end
/// -- the property append-only editing structurally could not have.
#[test]
fn arrow_navigation_then_typing_inserts_in_the_middle_through_real_routing() {
    let (mut state, _dir) = state_with_an_open_document("editor-real-mid-insert", "hello");
    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();

    let arrow_right = crate::input::KeyPress {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight),
        modifiers: iced::keyboard::Modifiers::empty(),
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed =
        crate::input::route_non_modal_input(proof, &policy, state.focus, None, arrow_right);
    let _ = super::update(&mut state, Message::Input(routed));

    assert_eq!(
        active_document_cursor(&state),
        tekstide_core::content::TextCursor { line: 0, column: 1 },
        "a real ArrowRight must move the real cursor past the first character"
    );
    assert_eq!(
        active_document_text(&state),
        "hello",
        "navigation alone must never edit the text"
    );

    let typed = crate::input::KeyPress {
        key: iced::keyboard::Key::Character("X".into()),
        modifiers: iced::keyboard::Modifiers::empty(),
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, typed);
    let _ = super::update(&mut state, Message::Input(routed));

    assert_eq!(
        active_document_text(&state),
        "hXello",
        "the typed character must insert exactly where the real cursor moved to"
    );
}

/// `Ctrl+S` is a real global keybinding (`navigation::linux_mvp`), reaches
/// `attempt_save_active_document`, and the bytes actually land on disk --
/// not merely that `ProjectContentStatus` reports success.
#[test]
fn ctrl_s_saves_the_real_edited_document_to_disk() {
    let (mut state, dir) = state_with_an_open_document("editor-real-ctrl-s-save", "hello");
    let _ = state.app_shell.replace_active_project_text("hello!");
    assert_eq!(
        std::fs::read_to_string(dir.join("file.txt")).unwrap(),
        "hello",
        "test precondition: the edit must not have reached disk yet"
    );

    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key: iced::keyboard::Key::Character("s".into()),
        modifiers: iced::keyboard::Modifiers::CTRL,
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
    assert!(
        matches!(routed, crate::input::RoutedInput::Shell(_)),
        "Ctrl+S must be a real global keybinding, not fall through to Surface: {routed:?}"
    );

    let _ = super::update(&mut state, Message::Input(routed));

    assert_eq!(
        std::fs::read_to_string(dir.join("file.txt")).unwrap(),
        "hello!"
    );
    assert!(
        state.modal.is_none(),
        "an ordinary save must never open the conflict modal"
    );
}

fn external_change_focus(modal: &Option<ModalContent>) -> Option<ExternalChangeButton> {
    match modal {
        Some(ModalContent::ExternalChange(external_change)) => Some(external_change.focus),
        _ => None,
    }
}

/// **The review gate's own required proof**: a real file changed
/// underneath a real open buffer -- not a synthesised `SaveDecision` --
/// triggers the conflict modal, and Reload discards the local edit in
/// favour of what is actually on disk. `TextDocument::save()` has no
/// force-overwrite bypass (`content::document`'s own module doc);
/// this is the *only* path back from a conflict, and it is exercised
/// here against a real save that really failed.
#[test]
fn saving_over_a_real_external_change_opens_the_conflict_modal_and_reload_takes_the_disk_content() {
    let (mut state, dir) = state_with_an_open_document("editor-real-conflict-reload", "original");
    let _ = state.app_shell.replace_active_project_text("tekstide edit");
    std::fs::write(dir.join("file.txt"), "external edit").unwrap();

    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key: iced::keyboard::Key::Character("s".into()),
        modifiers: iced::keyboard::Modifiers::CTRL,
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
    let _ = super::update(&mut state, Message::Input(routed));

    assert_eq!(
        external_change_focus(&state.modal),
        Some(ExternalChangeButton::Dismiss),
        "a refused save must open the real conflict modal, defaulting to the non-destructive button"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("file.txt")).unwrap(),
        "external edit",
        "a refused save must never have written the local edit to disk"
    );

    let _ = super::update(
        &mut state,
        Message::ModalFocusNext, // Dismiss -> Reload (two-item cycle)
    );
    assert_eq!(
        external_change_focus(&state.modal),
        Some(ExternalChangeButton::Reload)
    );
    let _ = super::update(&mut state, Message::ModalActivate);

    assert!(state.modal.is_none(), "Reload must close the modal");
    assert_eq!(
        active_document_text(&state),
        "external edit",
        "Reload must take disk's real current content, discarding the local edit"
    );
}

/// **Every dismissal path defaults to not overwriting** (PR-018-C's own
/// convention, tested per-path there too): Escape on the conflict
/// dialog must leave the externally-written file exactly as the "other
/// process" left it -- proving Dismiss never writes, not merely that it
/// closes the modal.
#[test]
fn dismissing_the_conflict_modal_never_writes_the_local_edit_to_disk() {
    let (mut state, dir) = state_with_an_open_document("editor-real-conflict-dismiss", "original");
    let _ = state.app_shell.replace_active_project_text("tekstide edit");
    std::fs::write(dir.join("file.txt"), "external edit").unwrap();

    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key: iced::keyboard::Key::Character("s".into()),
        modifiers: iced::keyboard::Modifiers::CTRL,
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
    let _ = super::update(&mut state, Message::Input(routed));
    assert!(
        state.modal.is_some(),
        "test precondition: the conflict modal must be open"
    );

    let _ = super::update(&mut state, Message::ModalDismiss);

    assert!(state.modal.is_none());
    assert_eq!(
        std::fs::read_to_string(dir.join("file.txt")).unwrap(),
        "external edit",
        "Escape must leave the file exactly as the external process wrote it"
    );
}

/// **The bidi-override case for the conflict dialog, tested specifically**
/// -- `relative_path` is the same attacker-influenced class as
/// `chrome_line`'s own path (a real file's own name), escaped the same
/// way before it reaches the catalog. Ablated by temporarily replacing
/// `external_change_dialog_body`'s escaping call with a raw
/// `format!("{} changed...", relative_path.display())` during review:
/// this assertion failed with the raw override character present in the
/// panic's own printed value, confirming the test actually exercises the
/// escaping path rather than passing vacuously; reverted before commit.
#[test]
fn external_change_dialog_body_escapes_a_bidi_override_in_the_path() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let relative_path = Path::new("proj\u{202E}gpj.exe");

    let body = super::external_change_dialog_body(&catalog, relative_path, true);

    assert!(
        body.contains("<U+202E>"),
        "expected the escaped marker in {body:?}"
    );
    assert!(
        !body.contains('\u{202E}'),
        "the raw override character must never reach the conflict dialog, got {body:?}"
    );
}

/// **Found live during PR-019-E closeout, not reasoned about in the
/// abstract**: `ProjectContentStatus::Conflict` is set for two different
/// real situations -- a genuinely dirty buffer that would lose local
/// edits on Reload, and a *clean* document that merely changed on disk
/// (`content status: conflict | document: external changed | dirty
/// files: 0`, observed against a real save). The dialog must not claim
/// "your local changes will be discarded" in the second case, since
/// there are none.
#[test]
fn external_change_dialog_body_does_not_claim_discarded_changes_without_local_edits() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let relative_path = Path::new("notes.txt");

    let with_edits = super::external_change_dialog_body(&catalog, relative_path, true);
    let without_edits = super::external_change_dialog_body(&catalog, relative_path, false);

    assert!(
        with_edits.contains("discarded"),
        "a genuine conflict must still warn that local edits are lost: {with_edits:?}"
    );
    assert!(
        !without_edits.contains("discarded"),
        "a clean document must never claim changes will be discarded when it has none: \
         {without_edits:?}"
    );
    assert_ne!(
        with_edits, without_edits,
        "the two real situations must render distinguishably, not the same text"
    );
}

fn approval_request_fixture(
    display_command: impl Into<String>,
    cwd: impl Into<PathBuf>,
    risk_level: tekstide_core::domain::RiskLevel,
) -> tekstide_core::domain::ApprovalRequest {
    tekstide_core::domain::ApprovalRequest::pending(
        tekstide_core::project::ProjectId::new_uuid(),
        None,
        "command_execution",
        display_command,
        risk_level,
        Vec::new(),
        cwd,
    )
}

/// **response 221's own finding, tested**: `cwd` arrives on
/// `ApprovalRequest` raw, straight from the adapter, and is the field
/// this slice's own widget escapes for the first time -- `argv`'s
/// escaping is RFC-021's, already proven by its own ten-probe suite,
/// cited rather than repeated here.
///
/// **Ablated**: temporarily replaced `approval_dialog_body`'s
/// `quote_untrusted(&request.cwd.display().to_string())` call with a raw
/// `request.cwd.display().to_string()`, ran this test -- it failed, with
/// the raw override character present in the panic's own printed body
/// text, confirming the assertion actually exercises the escaping path
/// rather than passing vacuously. Reverted before commit.
#[test]
fn approval_dialog_body_escapes_a_bidi_override_in_the_cwd() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let request = approval_request_fixture(
        "cat notes.txt",
        "/home/user/proj\u{202E}gpj",
        tekstide_core::domain::RiskLevel::Low,
    );

    let body = super::approval_dialog_body(&catalog, &request);

    assert!(
        body.contains("<U+202E>"),
        "expected the escaped marker in {body:?}"
    );
    assert!(
        !body.contains('\u{202E}'),
        "the raw override character must never reach the approval dialog, got {body:?}"
    );
}

/// **No double-escaping (`what-the-dialog-must-not-lie-about.md` §1)**:
/// a `cwd` containing the *literal* text `<U+202E>` (no real override
/// character anywhere in it) must pass through unchanged, not be further
/// mangled by a second escaping pass -- `text_safety::escape_untrusted_chars`
/// never touches `<`, `U`, `+`, hex digits, or `>`, so this is a real
/// idempotency property of the escaping function, not merely hoped for.
#[test]
fn approval_dialog_body_does_not_double_escape_literal_marker_text_in_the_cwd() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let request = approval_request_fixture(
        "cat notes.txt",
        "/home/user/<U+202E>-literally-not-an-override",
        tekstide_core::domain::RiskLevel::Low,
    );

    let body = super::approval_dialog_body(&catalog, &request);

    assert!(
        body.contains("<U+202E>-literally-not-an-override"),
        "literal marker-shaped text must survive unmangled, got {body:?}"
    );
}

/// **`argv`'s escaping is inherited from RFC-021, not re-derived here**
/// (response 221): `display_command` arrives on `ApprovalRequest`
/// already escaped by `approval::coordinator::display_argv`. This proves
/// the widget's isolation-wrapping does not corrupt that already-escaped
/// text -- a marker `display_argv` itself produced must survive
/// unchanged through `approval_dialog_body`, the same
/// no-double-escaping property proven for `cwd` above, applied to the
/// field that already carried it in.
#[test]
fn approval_dialog_body_does_not_mangle_argvs_already_escaped_marker() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let request = approval_request_fixture(
        "rm -rf impor<U+200B>tant.txt",
        "/home/user/project",
        tekstide_core::domain::RiskLevel::Destructive,
    );

    let body = super::approval_dialog_body(&catalog, &request);

    assert!(
        body.contains("impor<U+200B>tant.txt"),
        "an already-escaped marker from the model must survive unmangled, got {body:?}"
    );
}

/// The four `RiskLevel` variants each render as their own, distinct
/// word -- a `Destructive` proposal must not be able to read as `Low`
/// because a selector arm was missed.
#[test]
fn approval_dialog_body_renders_each_risk_level_distinguishably() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let levels = [
        (tekstide_core::domain::RiskLevel::Low, "Low"),
        (tekstide_core::domain::RiskLevel::Medium, "Medium"),
        (tekstide_core::domain::RiskLevel::High, "High"),
        (tekstide_core::domain::RiskLevel::Destructive, "Destructive"),
    ];
    let mut rendered = Vec::new();
    for (level, word) in levels {
        let request = approval_request_fixture("ls", "/home/user/project", level);
        let body = super::approval_dialog_body(&catalog, &request);
        assert!(
            body.contains(word),
            "RiskLevel::{level:?} must render as {word:?}, got {body:?}"
        );
        rendered.push(body);
    }
    let unique: std::collections::HashSet<_> = rendered.iter().collect();
    assert_eq!(
        unique.len(),
        rendered.len(),
        "all four risk levels must render distinguishably from each other: {rendered:?}"
    );
}

/// **`what-the-dialog-must-not-lie-about.md` §2**: "the highest-consequence
/// sentence in this RFC." Asserts all three things this dialog must
/// never let a user assume by omission are actually stated in words, not
/// only documented: that a decision here does not stop execution, that
/// approving does not make the command safe, and that the command shown
/// is all the adapter will do. Response 222: the third non-claim was
/// initially deferred pending open question 3, then added here once the
/// reviewer pointed out it does not depend on that answer -- it is about
/// a single dialog's authority, true whenever the dialog appears.
#[test]
fn approval_dialog_cooperative_notice_states_all_three_required_non_claims() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let notice = catalog.get("approval-dialog-cooperative-notice");

    assert!(
        notice.to_lowercase().contains("cannot stop"),
        "must state that a decision here does not stop execution: {notice:?}"
    );
    assert!(
        notice
            .to_lowercase()
            .contains("does not make the command safe"),
        "must state that approving does not make the command safe: {notice:?}"
    );
    assert!(
        notice.to_lowercase().contains("only one request"),
        "must state that the shown command is not all the adapter will do: {notice:?}"
    );
}

/// **The real, end-to-end proof the unit test above cannot give alone**:
/// a document with no local edits, changed externally, saved for real
/// via a real `Ctrl+S` through `update` -- the modal must open with the
/// non-discarding wording, not the genuine-conflict one. Reproduces
/// exactly the scenario found live: open, do not edit, external write,
/// save.
#[test]
fn saving_a_clean_document_over_a_real_external_change_does_not_claim_discarded_changes() {
    let (mut state, dir) =
        state_with_an_open_document("editor-real-clean-external-change", "original");
    // No local edit -- the document stays Clean.
    std::fs::write(dir.join("file.txt"), "external edit").unwrap();

    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key: iced::keyboard::Key::Character("s".into()),
        modifiers: iced::keyboard::Modifiers::CTRL,
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
    let _ = super::update(&mut state, Message::Input(routed));

    let Some(ModalContent::ExternalChange(modal)) = &state.modal else {
        panic!("a real refused save over a clean document must still open the modal");
    };
    assert!(
        !modal.had_local_edits,
        "test precondition: the document was never edited"
    );
    let rendered_body = super::external_change_dialog_body(
        &state.catalog,
        &modal.relative_path,
        modal.had_local_edits,
    );
    assert!(
        !rendered_body.contains("discarded"),
        "the real modal must not claim discarded changes when there were none: \
         {rendered_body:?}"
    );
}

/// **Response 205 constraint 5**: "Prove the bridging thread is not
/// recreated on every rebuild." Drives `iced`'s own real deduplication
/// path -- [`iced_futures::subscription::Tracker`], the mechanism
/// `Subscription::run_with` relies on in production, not a mock of it --
/// against 50 separate [`super::terminal_wake_subscription`] rebuilds
/// for the *same* `TerminalId`, each carrying its own freshly
/// `try_clone()`'d `WakeNotifier` exactly as a real `subscription()`
/// rebuild would. Only the very first rebuild may ever be handed back as
/// a new future to spawn; every later one must be recognised as already
/// running and discarded (along with its harmlessly duplicated `eventfd`)
/// without spawning a second bridging thread.
#[test]
fn terminal_bridge_thread_count_is_stable_across_many_view_rebuilds() {
    let root = fresh_project_dir("wake-subscription-thread-stability");
    let (pane, _session) = crate::surface::terminal::TerminalPane::launch(
        tekstide_core::project::ProjectId::new_uuid(),
        "wake-stability pane",
        root,
        PathBuf::from("/bin/sh"),
    )
    .expect("a real shell must launch to prove wake-subscription dedup against a real pane");
    let terminal_id = pane.terminal_id().clone();

    let mut tracker = iced_futures::subscription::Tracker::new();
    let (message_sender, _message_receiver) = iced::futures::channel::mpsc::channel::<Message>(16);

    let mut total_new_spawns = 0;
    for _ in 0..50 {
        let notifier = pane
            .wake_notifier()
            .expect("wake notifier must clone against a live pane");
        let subscription = super::terminal_wake_subscription(terminal_id.clone(), notifier);
        let recipes = iced_futures::subscription::into_recipes(subscription);
        let new_futures = tracker.update(recipes.into_iter(), message_sender.clone());
        total_new_spawns += new_futures.len();
    }

    assert_eq!(
        total_new_spawns, 1,
        "the same terminal_id's wake subscription, rebuilt 50 times as a real \
         `subscription()` call would on every view rebuild, must only ever need ONE real \
         spawn -- a count above 1 means iced's own Subscription::run_with identity is not \
         deduping this subscription across rebuilds, which would leak one bridging thread \
         (and one duplicated eventfd) per rebuild in production"
    );
}
