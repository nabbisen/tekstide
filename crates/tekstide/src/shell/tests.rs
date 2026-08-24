use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tekstide_core::shell::ApplicationShell;

use super::{
    AgentRunLaunchRefusal, ApprovalDialog, ApprovalDialogButton, ExternalChangeButton,
    MAX_PATH_FIELD_CHARS, Message, ModalButton, ModalContent, PasteConfirmButton, PathFieldError,
    State, TerminalPasteRefusal, TranscriptPurgeButton, TrustGrantButton,
    agent_run_launch_refusal_text, attempt_agent_run_launch_with_profile,
    attempt_agent_run_launch_with_profile_and_state_root,
    attempt_agent_run_launch_with_profile_state_root_and_capture, content_within_bound,
    evaluate_promotion, focus_marker, main_area_key, main_area_label, modal_scrim_style,
    open_real_audit_store, path_field_error_text, poll_approval_channels, sidebar_label,
    status_bar_summary, terminal_paste_refusal_text, transcript_local_data_summary_for,
    trust_grant_dialog_body, trusted_ui_state, verify_restored_trust_against, zone_style,
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

/// transcript-capture-evidence handoff: the injectable seam
/// (`attempt_agent_run_launch_with_profile_and_state_root`) needs a
/// state root of its own, separate from `fresh_project_dir` -- a real
/// transcript directory tree gets created under this one, and it must
/// never be the developer's real `$XDG_STATE_HOME`.
///
/// **Deliberately much shorter than `fresh_project_dir`'s naming**, and
/// `label` is not embedded at all: a `Managed` launch binds a real Unix
/// domain socket under `<state root>/approval/<agent-run-id>.sock`,
/// bounded by `sun_path`'s ~108-byte kernel limit
/// (`ApprovalChannelDirectory::socket_path`'s own doc). A verbose,
/// timestamped state root -- the shape every other `fresh_*_dir` helper
/// here uses -- leaves no room for that suffix and fails real launches
/// with `SocketPathTooLong`, discovered by this handoff's own first
/// test run. A short, counter-disambiguated prefix is still unique per
/// call within one test binary, which is all that is required.
fn fresh_state_root_dir() -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tsr-{}-{sequence}", std::process::id()));
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
        tekstide_core::project::WorkspaceTrust::Restricted,
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

/// RFC-022's own explicit, non-optional constraint: "no bulk approval
/// and no multi-select... one decision, one command, read
/// individually" -- a list with risk labels invites triage by label
/// instead of reading commands, the same habituation failure promotion
/// severity already guards against by a different route.
/// `ApprovalHistory`'s own control renders one decision at a time by
/// construction (`Message::OpenApprovalHistoryEntry` takes a single
/// `ApprovalId`, not a collection), but nothing before this test failed
/// by name if a future change reached for the obvious building blocks
/// of a multi-select surface anyway.
///
/// **This is a denylist, not proof of absence** -- the same limitation
/// `reference_adapter_binary_never_executes_the_argv_it_proposes`
/// already discloses for its own scan. It names the concrete shapes a
/// bulk-decide surface would plausibly reach for first (a checkbox
/// widget, a `Vec`-of-ids-shaped decide entry point, an "approve all"/
/// "select all"/"decide all" catalog key) and fails loudly if any
/// appear; it cannot prove no bulk mechanism could ever be built by
/// some other shape entirely.
#[test]
fn no_bulk_approval_or_multi_select_construct_exists_anywhere_in_the_crate() {
    for path in scannable_source_files() {
        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        assert!(
            !source.contains("widget::checkbox") && !source.contains("checkbox("),
            "{} must not introduce a checkbox widget -- RFC-022 requires no multi-select \
             surface for approval decisions",
            path.display()
        );
        for forbidden in [
            "Vec<ApprovalId>",
            "Vec<tekstide_core::domain::ApprovalId>",
            "&[ApprovalId]",
        ] {
            assert!(
                !source.contains(forbidden),
                "{}: found `{forbidden}` -- a collection-of-ids-shaped decide entry point is \
                 exactly the bulk-approval mechanism RFC-022 forbids",
                path.display()
            );
        }
    }

    let en_ftl = std::fs::read_to_string(real_locales_dir().join("en.ftl"))
        .expect("en.ftl must be readable");
    for forbidden in ["approve-all", "select-all", "decide-all", "approve all"] {
        assert!(
            !en_ftl.to_lowercase().contains(forbidden),
            "en.ftl: found {forbidden:?} -- no bulk-decide affordance may exist, in copy or \
             in code"
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

// --- RFC-032 PR-032-C, response 245: the audit store is authoritative -
//
// PR-032-B restores a `Trusted` project from the user-writable
// recent-projects cache on reopen. `verify_restored_trust_against`
// (`shell.rs`) is the gate that confirms it against a real, applied
// `TrustGrant` in the durable audit store before that restoration means
// anything security-relevant.

fn cached_trusted_recent_project(
    project_id: tekstide_core::project::ProjectId,
    canonical_root: PathBuf,
) -> tekstide_core::project::recent::RecentProjectState {
    tekstide_core::project::recent::RecentProjectState {
        state_version: tekstide_core::project::recent::RECENT_PROJECT_STATE_VERSION,
        projects: vec![tekstide_core::project::recent::RecentProject::new(
            project_id,
            "trust-verify-project",
            canonical_root.clone(),
            canonical_root,
            tekstide_core::project::recent::Timestamp::now_utc(),
            tekstide_core::project::WorkspaceTrust::Trusted,
        )],
    }
}

/// The positive case: a real, applied `TrustGrant` genuinely exists in
/// the store for this project -- the restored `Trusted` state must
/// survive verification.
#[test]
fn verify_restored_trust_keeps_trust_when_a_real_grant_is_recorded() {
    let project_dir = fresh_project_dir("verify-trust-confirmed");
    let mut app_shell = ApplicationShell::new();
    app_shell.restore_recent_projects(cached_trusted_recent_project(
        tekstide_core::project::ProjectId::new_uuid(),
        std::fs::canonicalize(&project_dir).unwrap(),
    ));
    let outcome = app_shell
        .add_project_from_path(&project_dir)
        .expect("cached project should reopen");
    assert_eq!(
        app_shell
            .state()
            .project(outcome.project_id())
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "test precondition: PR-032-B's own restoration must have run first"
    );

    let audit_state_dir = temp_audit_state_dir("verify-trust-confirmed");
    let mut store = super::open_audit_store(&audit_state_dir, Vec::new())
        .expect("open a real, temp-dir-backed audit store");
    let project_id = outcome.project_id().clone();
    let mut health = tekstide_core::audit::AuditHealth::default();
    let mut project = app_shell
        .state()
        .project(&project_id)
        .cloned()
        .expect("project must exist to grant trust against");
    tekstide_core::audit::AuditCoordinator::new(&mut store, &mut health)
        .grant_project_trust(&mut project)
        .expect("a real grant against a real store must succeed");
    drop(store);

    verify_restored_trust_against(&mut app_shell, |_shell| {
        super::open_audit_store(&audit_state_dir, Vec::new())
    });

    assert_eq!(
        app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "a real, applied TrustGrant in the store must let the restored trust stand"
    );
}

/// **The finding response 245 required a fix for.** The cache says
/// `Trusted`; the audit store has no record of it at all (as if the
/// cache file had been edited directly, or the grant never actually
/// completed). Verification must demote back to `Restricted`.
#[test]
fn verify_restored_trust_demotes_when_the_store_has_no_matching_grant() {
    let project_dir = fresh_project_dir("verify-trust-unconfirmed");
    let mut app_shell = ApplicationShell::new();
    app_shell.restore_recent_projects(cached_trusted_recent_project(
        tekstide_core::project::ProjectId::new_uuid(),
        std::fs::canonicalize(&project_dir).unwrap(),
    ));
    let outcome = app_shell
        .add_project_from_path(&project_dir)
        .expect("cached project should reopen");
    let project_id = outcome.project_id().clone();
    assert_eq!(
        app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "test precondition: PR-032-B's own restoration must have run first"
    );

    let audit_state_dir = temp_audit_state_dir("verify-trust-unconfirmed");
    // A real store, opened and left genuinely empty of any TrustChange
    // record for this project -- not a missing store, the more common
    // and more important failure mode.
    let _ = super::open_audit_store(&audit_state_dir, Vec::new())
        .expect("open a real, temp-dir-backed audit store");

    verify_restored_trust_against(&mut app_shell, |_shell| {
        super::open_audit_store(&audit_state_dir, Vec::new())
    });

    assert_eq!(
        app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Restricted",
        "a cache-restored Trusted with no confirming record in the durable store must be denied \
         -- this is the security fix response 245 required"
    );
}

/// Fail-closed half of the same requirement: if the store cannot even
/// be opened, every currently-`Trusted` project must still be demoted,
/// not left trusted on the assumption that "no answer" means "yes."
#[test]
fn verify_restored_trust_demotes_when_the_store_cannot_be_opened() {
    let project_dir = fresh_project_dir("verify-trust-no-store");
    let mut app_shell = ApplicationShell::new();
    app_shell.restore_recent_projects(cached_trusted_recent_project(
        tekstide_core::project::ProjectId::new_uuid(),
        std::fs::canonicalize(&project_dir).unwrap(),
    ));
    let outcome = app_shell
        .add_project_from_path(&project_dir)
        .expect("cached project should reopen");
    let project_id = outcome.project_id().clone();

    verify_restored_trust_against(&mut app_shell, |_shell| None);

    assert_eq!(
        app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Restricted",
        "an unopenable audit store must never be treated as silent confirmation"
    );
}

/// The no-op path: nothing is cached as `Trusted`, so verification must
/// not even attempt to open the store -- the "ordinary use does not
/// create this file" discipline `verify_restored_trust`'s own doc names.
/// Proven by passing a closure that panics if called at all, not merely
/// by checking the end state.
#[test]
fn verify_restored_trust_never_opens_the_store_when_nothing_is_cached_trusted() {
    let project_dir = fresh_project_dir("verify-trust-nothing-cached");
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    assert_eq!(
        app_shell.state().projects()[0].trust_state().label(),
        "Restricted",
        "test precondition: an ordinary, never-cached project starts Restricted"
    );

    verify_restored_trust_against(
        &mut app_shell,
        |_shell| -> Option<tekstide_core::audit::AuditStore> {
            panic!("must not open the audit store when nothing is cached as Trusted")
        },
    );
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

// --- Terminal resize handoff, response 243's required fix -------------
//
// Response 242's implementation applied a computed geometry only from
// `Message::WindowResized`'s handler, which fires only on a live drag --
// so a pane launched before the user ever resized the window (the common
// case) stayed at the launch-time `ROWS`/`COLS` default forever, even in
// an already-large window. These tests prove the fix: a real window size
// arriving via `WindowResized`, then a pane launched afterward, must be
// sized immediately, not left at the default until a second drag.

fn launch_a_real_terminal(state: &mut State) {
    let _ = super::update(
        state,
        Message::Input(crate::input::RoutedInput::Shell(
            crate::input::shell_input_for_test(
                tekstide_core::navigation::NavigationAction::LaunchTerminal,
            ),
        )),
    );
}

const LAUNCH_DEFAULT: (u16, u16) = (
    crate::surface::terminal::ROWS as u16,
    crate::surface::terminal::COLS as u16,
);

/// Precondition check, not the regression itself: with no window size
/// known yet (`state.window_size` starts `None`, and nothing in this
/// test dispatches a `WindowResized`), a freshly launched pane has
/// nothing to compute a real size from and must stay at the documented
/// launch-time default -- proving the default is still reachable, not
/// that this test is exercising a no-op path by accident.
#[test]
fn a_pane_launched_before_any_window_size_is_known_stays_at_the_launch_default() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("resize-before-window-size");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    assert!(state.window_size.is_none(), "test precondition");

    launch_a_real_terminal(&mut state);

    let pane = state.terminal_panes.first().expect("one pane launched");
    assert_eq!(
        pane.dimensions(),
        LAUNCH_DEFAULT,
        "with no real window size known yet, the pane has nothing to compute a size from and \
         must stay at the launch-time default"
    );
}

/// `Message::WindowResized` on an already-launched pane: the geometry
/// handoff's own core behaviour, still exercised here as the baseline
/// the launch-site fix is compared against below.
#[test]
fn window_resized_resizes_an_already_launched_pane_away_from_the_default() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("resize-existing-pane");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    launch_a_real_terminal(&mut state);
    assert_eq!(
        state.terminal_panes.first().unwrap().dimensions(),
        LAUNCH_DEFAULT
    );

    let _ = super::update(
        &mut state,
        Message::WindowResized(iced::Size::new(1600.0, 1000.0)),
    );

    let pane = state.terminal_panes.first().expect("still one pane");
    assert_ne!(
        pane.dimensions(),
        LAUNCH_DEFAULT,
        "a real WindowResized event, once state.window_size is known, must resize an \
         already-tracked pane away from the launch-time default"
    );
}

/// **The regression this response-243 fix exists for.** A real window
/// size becomes known first (as it will be in practice, moments after
/// boot, via `Message::WindowOpened`'s handler -- simulated here the
/// same way `TerminalPasteResolved` tests simulate an async result
/// without exercising `iced`'s own I/O plumbing, by dispatching the
/// message the real Task would eventually produce). A terminal launched
/// *afterward* must be sized from that already-known geometry
/// immediately, at launch -- not left at the default until a second,
/// separate resize happens to fire.
#[test]
fn a_pane_launched_after_the_window_size_is_already_known_is_sized_immediately() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("resize-after-window-size");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let _ = super::update(
        &mut state,
        Message::WindowResized(iced::Size::new(1600.0, 1000.0)),
    );
    assert!(
        state.terminal_panes.is_empty(),
        "test precondition: the window size is known before any pane exists"
    );

    launch_a_real_terminal(&mut state);

    let pane = state.terminal_panes.first().expect("one pane launched");
    assert_ne!(
        pane.dimensions(),
        LAUNCH_DEFAULT,
        "a pane launched after the window size is already known must be sized from that \
         geometry immediately -- waiting for a second, separate resize event is the bug this \
         fix closes"
    );
}

/// The agent-run launch call site (`attempt_agent_run_launch_with_profile`)
/// needs the identical fix as the plain-terminal call site above -- a
/// second, independent regression surface, not covered by the plain
/// launch test even though both call `apply_terminal_geometry` the same
/// way. Trust is `Restricted` by default in this test harness (see the
/// neighbouring `agent_run_launch_shell_input...` test's own doc
/// comment), so no session actually launches here -- this test instead
/// calls the resize-relevant sizing function directly against the
/// vector-in/vector-out shape both call sites share, proving
/// `apply_terminal_geometry`'s own "every tracked pane" behaviour
/// (response 242) still holds when triggered from a launch site rather
/// than a `WindowResized` handler.
#[test]
fn apply_terminal_geometry_resizes_every_tracked_pane_when_called_from_a_launch_site() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("resize-multi-pane-launch-site");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let _ = super::update(
        &mut state,
        Message::WindowResized(iced::Size::new(1600.0, 1000.0)),
    );
    launch_a_real_terminal(&mut state);
    let first_dimensions = state.terminal_panes.first().unwrap().dimensions();
    assert_ne!(first_dimensions, LAUNCH_DEFAULT);

    // A second pane, launched later while the same window geometry is
    // already known, must land on the same computed size as the first --
    // both are sized by the same call to `apply_terminal_geometry`
    // inside the launch site, not by a separate per-pane computation.
    launch_a_real_terminal(&mut state);
    assert_eq!(
        state.terminal_panes.len(),
        2,
        "test precondition: a second launch must add a second pane"
    );
    for pane in &state.terminal_panes {
        assert_eq!(
            pane.dimensions(),
            first_dimensions,
            "every tracked pane, not just the most recently launched one, must reflect the same \
             known window geometry"
        );
    }
}

/// RFC-022 PR-022-D: the real `Ctrl+Alt+A` path against a freshly opened
/// project -- `WorkspaceTrust::Restricted` by default, and this test
/// never grants it. `claude_code_linux_default`'s honest
/// `MayDiscoverWorkspaceFiles` policy is therefore refused here every
/// time, regardless of whether an AI CLI happens to be installed on the
/// machine running this suite. Still switches to Terminal Immersion, the
/// same "refused but still lands where the notice is visible" shape
/// `launch_terminal`'s own dispatch arm uses.
///
/// **RFC-032 PR-032-C**: trust can now be genuinely granted through the
/// real GUI route (`Ctrl+Alt+U` -> Enter -> Tab -> Enter,
/// `press_trust_settings_action`) -- see
/// `granting_trust_through_the_real_route_unblocks_a_real_agent_run_launch`
/// below for the other side of this exact refusal, proven to actually
/// clear once trust is granted for real, not merely that the trust flag
/// changed.
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

/// RFC-031 PR-031-A: the restricted-mode-blocked producer's own
/// reachability proof, on top of the same real `Ctrl+Alt+A` path the
/// test above already establishes -- proven from a real key press,
/// through `update`, to a record in the real audit store, not from a
/// dispatched `AppCommand` or a direct call to the producer.
#[test]
fn a_real_workspace_discovery_refusal_writes_a_real_restricted_mode_blocked_record() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("restricted-mode-blocked-reachability");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let project_id = state
        .app_shell
        .state()
        .active_project_id()
        .cloned()
        .unwrap();

    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::LaunchAgentRun,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    assert!(
        matches!(
            state.agent_run_launch_notice,
            Some(AgentRunLaunchRefusal::Validation(
                tekstide_core::agent::AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked { .. }
            ))
        ),
        "test precondition: a fresh, untrusted project must refuse with WorkspaceDiscoveryBlocked: \
         {:?}",
        state.agent_run_launch_notice
    );

    let audit_store =
        open_real_audit_store(&state.app_shell).expect("the real audit store must open");
    let records = audit_store
        .query(&tekstide_core::audit::AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| record.project_id.as_ref() == Some(&project_id))
        .collect::<Vec<_>>();

    let restricted_records: Vec<_> = records
        .iter()
        .filter(|record| {
            record.family == tekstide_core::audit::AuditEventFamily::RestrictedModeBlocked
        })
        .collect();
    assert_eq!(
        restricted_records.len(),
        1,
        "exactly one RestrictedModeBlocked record must exist for this project's real refusal: \
         {records:?}"
    );
    let record = restricted_records[0];
    assert_eq!(
        record.subject_ref, None,
        "what-the-store-may-hold.md: no path-shaped text belongs in this record"
    );
    assert_eq!(
        record.reason_code,
        Some(tekstide_core::audit::AuditReasonCode::RestrictedMode)
    );
}

/// RFC-031 PR-031-A: **both directions** of the discrimination the gate
/// requires -- a record appears for `WorkspaceDiscoveryBlocked` and
/// does not for `RunLimitExceeded` or `ExecutableUnavailable`. Uses
/// `attempt_agent_run_launch_with_profile` directly with controlled
/// profiles (the same shape
/// `granting_trust_through_the_real_route_unblocks_a_real_agent_run_launch`/
/// `agent_run_launch_refusal_text_renders_the_not_found_reason_honestly`
/// already use to isolate one refusal reason deterministically) rather
/// than a real key press -- reachability through a real key press is
/// this same producer's own separate proof, above; this test's job is
/// the narrower discrimination property.
#[test]
fn a_restricted_mode_blocked_record_appears_only_for_workspace_discovery_refusals() {
    // WorkspaceDiscoveryBlocked -> a record appears.
    {
        let (mut state, project_id) =
            state_with_a_real_project("restricted-mode-discrimination-workspace");
        let mut profile = tekstide_core::agent::AiCliProfile::new(
            "discrimination-workspace-discovery",
            "Discrimination Fixture (workspace discovery)",
            tekstide_core::agent::AiCliProfileSource::BuiltIn,
            tekstide_core::agent::AiCliExecutable::Absolute {
                path: transcript_marker_script_path(),
                provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
            },
            tekstide_core::domain::AgentCompatibilityLevel::Supervised,
        );
        profile.workspace_discovery_policy =
            tekstide_core::agent::AiCliWorkspaceDiscoveryPolicy::MayDiscoverWorkspaceFiles {
                summary: "test fixture: reads workspace files".to_owned(),
            };
        let refusal = attempt_agent_run_launch_with_profile(&mut state, profile)
            .expect_err("an untrusted project must refuse a workspace-discovering profile");
        assert!(matches!(
            refusal,
            AgentRunLaunchRefusal::Validation(
                tekstide_core::agent::AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked { .. }
            )
        ));

        super::record_restricted_mode_blocked_if_applicable(&mut state, &refusal);

        let audit_store =
            open_real_audit_store(&state.app_shell).expect("the real audit store must open");
        let found = audit_store
            .query(&tekstide_core::audit::AuditQuery::latest(50))
            .expect("querying the real audit store must succeed")
            .records
            .into_iter()
            .map(|sequenced| sequenced.record)
            .any(|record| {
                record.project_id.as_ref() == Some(&project_id)
                    && record.family
                        == tekstide_core::audit::AuditEventFamily::RestrictedModeBlocked
            });
        assert!(
            found,
            "WorkspaceDiscoveryBlocked must produce a RestrictedModeBlocked record"
        );
    }

    // ExecutableUnavailable -> no record.
    {
        let mut app_shell = ApplicationShell::new();
        app_shell
            .add_project_from_path(fresh_project_dir(
                "restricted-mode-discrimination-not-found",
            ))
            .expect("a freshly created directory is a valid project root");
        let mut state = state_with(app_shell);
        let project_id = state
            .app_shell
            .state()
            .active_project_id()
            .cloned()
            .unwrap();

        let empty_lookup_dir = fresh_project_dir("restricted-mode-discrimination-empty-bin");
        let profile = tekstide_core::agent::AiCliProfile::new(
            "discrimination-absent-ai-cli",
            "Discrimination Fixture (absent)",
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
        assert!(matches!(
            refusal,
            AgentRunLaunchRefusal::Validation(
                tekstide_core::agent::AgentRunLaunchValidationError::ExecutableUnavailable { .. }
            )
        ));

        super::record_restricted_mode_blocked_if_applicable(&mut state, &refusal);

        let audit_store =
            open_real_audit_store(&state.app_shell).expect("the real audit store must open");
        let found = audit_store
            .query(&tekstide_core::audit::AuditQuery::latest(50))
            .expect("querying the real audit store must succeed")
            .records
            .into_iter()
            .map(|sequenced| sequenced.record)
            .any(|record| {
                record.project_id.as_ref() == Some(&project_id)
                    && record.family
                        == tekstide_core::audit::AuditEventFamily::RestrictedModeBlocked
            });
        assert!(
            !found,
            "ExecutableUnavailable must not produce a RestrictedModeBlocked record"
        );
    }

    // RunLimitExceeded -> no record.
    {
        let mut app_shell = ApplicationShell::new();
        app_shell
            .add_project_from_path(fresh_project_dir("restricted-mode-discrimination-limit"))
            .expect("a freshly created directory is a valid project root");
        let mut state = state_with(app_shell);
        let project_id = state
            .app_shell
            .state()
            .active_project_id()
            .cloned()
            .unwrap();
        let limits = tekstide_core::project::ProjectResourceLimits {
            agent_run_limit: Some(0),
            ..Default::default()
        };
        state
            .app_shell
            .state_mut()
            .project_mut(&project_id)
            .unwrap()
            .set_resource_limits(limits);

        let profile = tekstide_core::agent::AiCliProfile::claude_code_linux_default();
        let refusal = attempt_agent_run_launch_with_profile(&mut state, profile)
            .expect_err("a zero agent_run_limit must refuse before anything else is checked");
        assert!(matches!(
            refusal,
            AgentRunLaunchRefusal::RunLimitExceeded { limit: 0 }
        ));

        super::record_restricted_mode_blocked_if_applicable(&mut state, &refusal);

        let audit_store =
            open_real_audit_store(&state.app_shell).expect("the real audit store must open");
        let found = audit_store
            .query(&tekstide_core::audit::AuditQuery::latest(50))
            .expect("querying the real audit store must succeed")
            .records
            .into_iter()
            .map(|sequenced| sequenced.record)
            .any(|record| {
                record.project_id.as_ref() == Some(&project_id)
                    && record.family
                        == tekstide_core::audit::AuditEventFamily::RestrictedModeBlocked
            });
        assert!(
            !found,
            "RunLimitExceeded must not produce a RestrictedModeBlocked record"
        );
    }
}

// --- RFC-032 PR-032-C/D: grant, revoke, the route, and the dialog -----

/// Response 246's enumeration shape (`only_one_production_call_site_ever_restores_a_projects_trust_state`,
/// `tekstide-core`), applied here to this crate's own production call
/// site. Needle has the leading `.` for the same reason that one does:
/// matching call syntax specifically, not `AuditCoordinator::grant_project_trust`'s
/// own `pub fn grant_project_trust(` definition line.
#[test]
fn only_one_production_call_site_ever_grants_workspace_trust() {
    let occurrences = count_occurrences_in_crate(".grant_project_trust(");
    assert_eq!(
        occurrences,
        vec![("shell.rs".to_string(), 1)],
        "exactly this one file may ever call AuditCoordinator::grant_project_trust: {occurrences:?}"
    );
}

#[test]
fn only_one_production_call_site_ever_revokes_workspace_trust() {
    let occurrences = count_occurrences_in_crate(".revoke_project_trust(");
    assert_eq!(
        occurrences,
        vec![("shell.rs".to_string(), 1)],
        "exactly this one file may ever call AuditCoordinator::revoke_project_trust: {occurrences:?}"
    );
}

/// RFC-032: the same needle-counting scan
/// `only_one_production_call_site_ever_restores_a_projects_trust_state`
/// (`tekstide-core`) uses, built on this file's own pre-existing
/// `crate_src_dir`/`collect_rs_files` walk (the "Mechanical seam scans"
/// section above) rather than a second copy of that directory walk.
fn count_occurrences_in_crate(needle: &str) -> Vec<(String, usize)> {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    let mut counts: Vec<(String, usize)> = files
        .into_iter()
        .filter_map(|path| {
            let relative = relative_to_src(&path);
            if relative.contains("/tests/") || relative.ends_with("tests.rs") {
                return None;
            }
            let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
            let count = source.matches(needle).count();
            (count > 0).then_some((relative, count))
        })
        .collect();
    counts.sort();
    counts
}

fn relative_to_src(path: &Path) -> String {
    path.strip_prefix(crate_src_dir())
        .expect("file must be under src/")
        .to_str()
        .expect("path must be valid UTF-8")
        .replace(std::path::MAIN_SEPARATOR, "/")
}

/// The route: `NavigationAction::OpenTrustSettings`, dispatched the real
/// way (through `update`'s `Shell` arm, not a bypass), reaches
/// `ProjectOpenSurface::TrustSettings` and lands in Content mode -- the
/// second real `open_surface`-conditional route after `OpenApprovalHistory`.
#[test]
fn open_trust_settings_shell_input_routes_to_the_trust_settings_surface() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("open-trust-settings");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenTrustSettings,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    let project = state.app_shell.state().active_project().unwrap();
    assert_eq!(
        project.open_surface(),
        tekstide_core::project::ProjectOpenSurface::TrustSettings
    );
    assert_eq!(project.mode(), tekstide_core::project::ProjectMode::Content);
}

/// RFC-033 PR-033-B, the Opt-out checklist's own first requirement:
/// "Reachable from a real key press, not a dispatched command." Real
/// `Ctrl+Alt+U` then Space, through `update`'s `Shell` arm -- proves the
/// route exists before any test relies on it, the same discipline
/// response 248 required for the trust action itself.
#[test]
fn pressing_the_capture_toggle_through_a_real_key_sequence_declines_capture() {
    let (mut state, project_id) = state_with_a_real_project("capture-toggle-reachable");
    assert!(
        !state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .transcript_capture_declined(),
        "test precondition: capture starts on"
    );

    press_transcript_capture_toggle(&mut state);

    assert!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .transcript_capture_declined(),
        "a real Ctrl+Alt+U then Space must decline capture"
    );

    press_transcript_capture_toggle(&mut state);

    assert!(
        !state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .transcript_capture_declined(),
        "pressing it again must toggle back on -- this is a toggle, not a one-way switch"
    );
}

fn state_with_a_real_project(label: &str) -> (State, tekstide_core::project::ProjectId) {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir(label);
    let project_id = match app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root")
    {
        tekstide_core::app::AddProjectOutcome::Added(id) => id,
        tekstide_core::app::AddProjectOutcome::FocusedExisting(id) => id,
    };
    (state_with(app_shell), project_id)
}

/// Response 248's required fix, this file's own real-input helper: the
/// exact key sequence a real keyboard user presses to reach `TrustSettings`
/// and act on it -- `Ctrl+Alt+U` (the real, collision-checked global
/// binding, dispatched through `shell_input_for_test` the same way
/// `LaunchAgentRun`'s own tests reach that action) to open the surface,
/// then Enter (`handle_trust_settings_key`'s own real key, routed
/// through `send_main_area_key` -- the identical helper
/// `arrow_keys_move_the_approval_history_highlight` already uses for
/// that surface's own keyboard access). Which of grant/revoke Enter
/// performs depends on the project's current trust state, exactly as
/// `handle_trust_settings_key`'s own body decides -- this helper does
/// not choose, the same way a real keypress would not either.
///
/// Every test below that needs to reach or act on this surface goes
/// through this helper rather than dispatching `Message::OpenTrustGrantDialog`/
/// `Message::RevokeWorkspaceTrust` directly -- response 248's own
/// finding was that a proof starting one step after the step that does
/// not exist (the missing route) is not a proof of reachability at all.
fn press_trust_settings_action(state: &mut State) {
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenTrustSettings,
    );
    let _ = super::update(
        state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );
    send_main_area_key(
        state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
    );
}

/// RFC-033 PR-033-B: the same real-key-sequence discipline
/// `press_trust_settings_action` establishes, applied to the capture
/// toggle's own key (Space, not Enter -- `handle_trust_settings_key`'s
/// own doc comment explains why the two controls do not share one).
/// Every test below that needs to decline capture through the real
/// route goes through this rather than dispatching
/// `Message::ToggleTranscriptCaptureDeclined` directly, for the same
/// reason response 248 gave for the analogous trust-settings helper.
fn press_transcript_capture_toggle(state: &mut State) {
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenTrustSettings,
    );
    let _ = super::update(
        state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );
    send_main_area_key(
        state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Space),
    );
}

/// RFC-033 PR-033-C: the same real-key-sequence discipline
/// `press_transcript_capture_toggle` establishes, applied to the purge
/// control's own key (Delete, not Space -- `handle_trust_settings_key`'s
/// own doc comment explains why the three controls do not share one).
/// Opens the confirmation dialog only -- a real purge additionally needs
/// `Message::ModalFocusNext`/`Message::ModalActivate`, the same two
/// deliberate acts `trust_grant_dialog_requires_moving_focus_and_activating_to_grant`
/// already requires for the analogous trust-grant dialog, so callers that
/// need the real deletion dispatch those themselves.
fn press_transcript_purge_key(state: &mut State) {
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenTrustSettings,
    );
    let _ = super::update(
        state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );
    send_main_area_key(
        state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete),
    );
}

/// `handle_trust_settings_key`'s own guard: a no-op while a project is
/// open but its `open_surface` is anything other than `TrustSettings`
/// (`ProjectDashboard`, the real default, here) -- the same guard shape
/// `handle_approval_history_key` already uses for its own zone.
#[test]
fn trust_settings_key_is_a_no_op_off_the_trust_settings_surface() {
    let (mut state, project_id) = state_with_a_real_project("trust-settings-key-wrong-surface");
    assert_eq!(
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .open_surface(),
        tekstide_core::project::ProjectOpenSurface::ProjectDashboard,
        "test precondition: not on the TrustSettings surface"
    );

    send_main_area_key(
        &mut state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
    );

    assert!(
        state.modal.is_none(),
        "Enter must do nothing off this surface"
    );
    assert_eq!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Restricted",
        "nothing must be granted from the wrong surface"
    );
}

/// A key other than Enter, on the real `TrustSettings` surface, must do
/// nothing -- `handle_trust_settings_key` only handles one key, by
/// design (there is no list to move a cursor through, unlike
/// `handle_approval_history_key`'s Up/Down).
#[test]
fn trust_settings_key_ignores_keys_other_than_enter() {
    let (mut state, project_id) = state_with_a_real_project("trust-settings-key-other-keys");
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenTrustSettings,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    send_main_area_key(
        &mut state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
    );

    assert!(
        state.modal.is_none(),
        "a non-Enter key must not open the dialog"
    );
    assert_eq!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Restricted"
    );
}

/// `Message::OpenTrustGrantDialog` opens the real dialog, focus
/// defaulting to `Cancel` -- `what-the-trust-dialog-must-say.md` §2, the
/// larger asymmetry than the paste dialog's own default. Activating it
/// on `Cancel` (the default, i.e. Escape's equivalent reachable via
/// Enter too) grants nothing and simply closes.
#[test]
fn trust_grant_dialog_defaults_focus_to_cancel_and_activating_it_grants_nothing() {
    let (mut state, project_id) = state_with_a_real_project("trust-dialog-cancel-default");

    press_trust_settings_action(&mut state);
    match state.modal.as_ref() {
        Some(ModalContent::TrustGrant(modal)) => {
            assert_eq!(modal.focus, TrustGrantButton::Cancel);
        }
        other => panic!("expected an open TrustGrant dialog, got {other:?}"),
    }

    let _ = super::update(&mut state, Message::ModalActivate);

    assert!(state.modal.is_none(), "activating must close the dialog");
    assert_eq!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Restricted",
        "activating on the default Cancel focus must not grant anything"
    );
}

/// The other half: granting needs **two deliberate acts** -- moving
/// focus (`ModalFocusNext`, Tab's real handler) and then activating
/// (`ModalActivate`, Enter's) -- not one. Only then does the real,
/// audited grant happen.
#[test]
fn trust_grant_dialog_requires_moving_focus_and_activating_to_grant() {
    let (mut state, project_id) = state_with_a_real_project("trust-dialog-grant-two-acts");

    press_trust_settings_action(&mut state);
    let _ = super::update(&mut state, Message::ModalFocusNext);
    match state.modal.as_ref() {
        Some(ModalContent::TrustGrant(modal)) => {
            assert_eq!(modal.focus, TrustGrantButton::Grant);
        }
        other => panic!("expected an open TrustGrant dialog, got {other:?}"),
    }
    let _ = super::update(&mut state, Message::ModalActivate);

    assert!(state.modal.is_none(), "activating must close the dialog");
    assert_eq!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "moving focus to Grant and activating must grant trust for real"
    );
}

/// `open_approval_history_entry`'s own "never replace an open modal"
/// rule, applied to this dialog's own manual-open call site.
#[test]
fn open_trust_grant_dialog_does_not_replace_an_already_open_modal() {
    let (mut state, _project_id) = state_with_a_real_project("trust-dialog-modal-exclusivity");
    state.modal = Some(ModalContent::LayerDemo {
        focus: ModalButton::Dismiss,
    });

    press_trust_settings_action(&mut state);

    assert!(
        matches!(state.modal, Some(ModalContent::LayerDemo { .. })),
        "an already-open modal must not be replaced"
    );
}

/// **Audit records queried and asserted, not implied**
/// (`task-breakdown-pr-plan.md`'s own PR-032-C gate item, "the way
/// RFC-022's `command_approval` assertion did"): a real grant through
/// the real route writes both the `Authorized` and `Applied`
/// `TrustGrant` records, sharing one `operation_id`, in that order.
#[test]
fn granting_trust_through_the_real_route_records_both_audit_records() {
    let (mut state, project_id) = state_with_a_real_project("trust-grant-audit-records");

    press_trust_settings_action(&mut state);
    let _ = super::update(&mut state, Message::ModalFocusNext);
    let _ = super::update(&mut state, Message::ModalActivate);

    let audit_store =
        open_real_audit_store(&state.app_shell).expect("the real audit store must open");
    let mut records = audit_store
        .query(&tekstide_core::audit::AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| record.project_id.as_ref() == Some(&project_id))
        .collect::<Vec<_>>();
    // Ascending by the order they were written, since `latest` returns
    // newest-first.
    records.reverse();

    assert_eq!(
        records.len(),
        2,
        "a real grant must write exactly two records for this project: {records:?}"
    );
    assert_eq!(
        records[0].family,
        tekstide_core::audit::AuditEventFamily::TrustChange
    );
    assert_eq!(
        records[0].action_kind,
        tekstide_core::audit::AuditActionKind::TrustGrant
    );
    assert_eq!(
        records[0].outcome,
        tekstide_core::audit::AuditOutcome::Authorized
    );
    assert_eq!(
        records[1].action_kind,
        tekstide_core::audit::AuditActionKind::TrustGrant
    );
    assert_eq!(
        records[1].outcome,
        tekstide_core::audit::AuditOutcome::Applied
    );
    assert_eq!(
        records[0].operation_id, records[1].operation_id,
        "the authorization and its application must share one operation_id"
    );
    assert!(records[0].operation_id.is_some());
}

/// The revoke half: a single `Applied` `TrustRevoke` record, no
/// `Authorized` phase (`revoke_project_trust`'s own single-phase shape,
/// `valid_trust_change`'s own requirement that a `TrustRevoke` record
/// carry no `operation_id`).
#[test]
fn revoking_trust_through_the_real_route_records_a_single_applied_record() {
    let (mut state, project_id) = state_with_a_real_project("trust-revoke-audit-records");
    press_trust_settings_action(&mut state);
    let _ = super::update(&mut state, Message::ModalFocusNext);
    let _ = super::update(&mut state, Message::ModalActivate);
    assert_eq!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "test precondition: the project must be trusted before revoking it"
    );

    press_trust_settings_action(&mut state);

    assert_ne!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "revocation must actually take effect"
    );
    let audit_store =
        open_real_audit_store(&state.app_shell).expect("the real audit store must open");
    let revoke_records = audit_store
        .query(&tekstide_core::audit::AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| {
            record.project_id.as_ref() == Some(&project_id)
                && record.action_kind == tekstide_core::audit::AuditActionKind::TrustRevoke
        })
        .collect::<Vec<_>>();

    assert_eq!(
        revoke_records.len(),
        1,
        "revoking must write exactly one record: {revoke_records:?}"
    );
    assert_eq!(
        revoke_records[0].outcome,
        tekstide_core::audit::AuditOutcome::Applied
    );
    assert!(
        revoke_records[0].operation_id.is_none(),
        "a TrustRevoke record must carry no operation_id"
    );
}

/// **Comparably reachable** (`what-the-trust-dialog-must-say.md` §5):
/// both controls live on the one `TrustSettings` surface, so both are
/// exactly one `NavigationAction::OpenTrustSettings` away -- never both
/// visible at once (nothing to grant while already trusted, nothing to
/// revoke while not), proving the surface actually switches which
/// control it offers rather than requiring a deeper path for one than
/// the other.
#[test]
fn trust_settings_surface_offers_grant_when_restricted_and_revoke_when_trusted() {
    let (mut state, project_id) = state_with_a_real_project("trust-settings-comparable-reach");

    // `trust_settings_view` returns an `iced::Element`, not inspectable
    // text directly -- assert on the catalog-rendered strings its own
    // buttons are built from instead, the same "assert on the resolved
    // text, not the widget tree" shape this crate's other dialog tests
    // already use.
    assert_eq!(
        state.catalog.get("trust-settings-grant-button"),
        "Grant Trust…"
    );
    assert_eq!(
        state.catalog.get("trust-settings-revoke-button"),
        "Revoke Trust"
    );

    press_trust_settings_action(&mut state);
    let _ = super::update(&mut state, Message::ModalFocusNext);
    let _ = super::update(&mut state, Message::ModalActivate);
    assert_eq!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Trusted"
    );
    // Both controls are reached through the identical
    // `NavigationAction::OpenTrustSettings` route -- one action, either
    // direction -- confirmed by dispatching it again post-grant and
    // finding the same surface still open (RFC-032's own "comparably
    // reachable" requirement is about navigation depth to the surface,
    // not about the grant dialog's own two-act confirmation, which is a
    // deliberate asymmetry in the *action*, not the *path*).
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenTrustSettings,
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
            .unwrap()
            .open_surface(),
        tekstide_core::project::ProjectOpenSurface::TrustSettings
    );
}

fn trust_grant_modal_fixture(root_path: &str, canonical_root_path: &str) -> super::TrustGrantModal {
    super::TrustGrantModal {
        project_id: tekstide_core::project::ProjectId::new_uuid(),
        root_path: PathBuf::from(root_path),
        canonical_root_path: PathBuf::from(canonical_root_path),
        focus: TrustGrantButton::Cancel,
    }
}

/// **The falsifiable claim `what-the-trust-dialog-must-say.md` §1 owes
/// evidence for**: a project directory name containing a bidi override
/// renders visibly as an escape marker.
///
/// **Ablated**: temporarily replaced `trust_grant_dialog_paths`'s
/// `quote_untrusted(&modal.canonical_root_path.display().to_string())`
/// call with a raw `.display().to_string()`, ran this test -- it failed,
/// with the raw override character present in the panic's own printed
/// output, confirming the assertion actually exercises the escaping path.
/// Reverted before commit.
#[test]
fn trust_grant_dialog_escapes_a_bidi_override_in_the_canonical_path() {
    let modal = trust_grant_modal_fixture(
        "/home/user/work/safe-project",
        "/home/user/work/safe-project\u{202E}gpj",
    );

    let (canonical, _secondary) = super::trust_grant_dialog_paths(&modal);

    assert!(
        canonical.as_str().contains("<U+202E>"),
        "expected the escaped marker in {canonical:?}"
    );
    assert!(
        !canonical.as_str().contains('\u{202E}'),
        "the raw override character must never reach the dialog, got {canonical:?}"
    );
}

/// No double-escaping: literal marker-shaped text (not a real override
/// character) must survive unmangled, the same idempotency property
/// `approval_dialog_body_does_not_double_escape_literal_marker_text_in_the_cwd`
/// already proves for a different dialog.
#[test]
fn trust_grant_dialog_body_does_not_double_escape_literal_marker_text() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let modal = trust_grant_modal_fixture(
        "/home/user/<U+202E>-literally-not-an-override",
        "/home/user/<U+202E>-literally-not-an-override",
    );

    let body = trust_grant_dialog_body(&catalog, &modal);

    assert!(
        body.contains("<U+202E>-literally-not-an-override"),
        "literal marker-shaped text must survive unmangled, got {body:?}"
    );
}

/// "Show both when they differ": a symlinked project (root path !=
/// canonical path) must show both, escaped independently.
#[test]
fn trust_grant_dialog_paths_shows_both_when_root_and_canonical_differ() {
    let modal = trust_grant_modal_fixture("/home/user/work/link", "/mnt/data/work/real-project");

    let (canonical, secondary) = super::trust_grant_dialog_paths(&modal);

    assert_eq!(
        canonical.as_str(),
        format!("{ISOLATE_START}/mnt/data/work/real-project{ISOLATE_END}")
    );
    assert_eq!(
        secondary.map(|root| root.as_str().to_string()),
        Some(format!("{ISOLATE_START}/home/user/work/link{ISOLATE_END}"))
    );
}

#[test]
fn trust_grant_dialog_paths_shows_only_the_canonical_path_when_they_match() {
    let modal = trust_grant_modal_fixture("/home/user/work/project", "/home/user/work/project");

    let (_canonical, secondary) = super::trust_grant_dialog_paths(&modal);

    assert_eq!(
        secondary, None,
        "an unsymlinked project must not show a second, identical path"
    );
}

/// `what-the-trust-dialog-must-say.md` §3: the canonical sentence,
/// reproduced verbatim from `docs/src/contributors/security-decisions.md`
/// -- "use it, or improve it and change the page too; do not let a
/// second, weaker wording exist alongside it."
#[test]
fn trust_grant_dialog_body_contains_the_canonical_sentence_verbatim() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let modal = trust_grant_modal_fixture("/home/user/project", "/home/user/project");

    let body = trust_grant_dialog_body(&catalog, &modal);

    assert!(
        body.contains(
            "Files inside the trusted folder may configure Tekstide and cause programs to run."
        ),
        "the canonical sentence must appear verbatim, got {body:?}"
    );
}

/// `what-the-trust-dialog-must-say.md` §4: the grant covers files not
/// yet written, including an AI agent's own output, across every future
/// session -- the one consequence a reasonable person would not infer,
/// stated rather than left implicit.
#[test]
fn trust_grant_dialog_body_states_the_present_and_future_consequence() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let modal = trust_grant_modal_fixture("/home/user/project", "/home/user/project");

    let body = trust_grant_dialog_body(&catalog, &modal);

    assert!(
        body.to_lowercase().contains("future")
            && body.to_lowercase().contains("agent")
            && body.to_lowercase().contains("every session"),
        "must name that the grant covers future files (including an agent's own output) across \
         every future session, got {body:?}"
    );
}

/// `what-the-trust-dialog-must-say.md` §3: "do not enumerate the
/// features in the dialog" (nine at the time that document was
/// written; ten as of RFC-023's `WorkspaceConfigLoading`). None of
/// `RestrictedModeFeature::ALL`'s own words appear.
#[test]
fn trust_grant_dialog_body_does_not_enumerate_the_nine_restricted_features() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let modal = trust_grant_modal_fixture("/home/user/project", "/home/user/project");

    let body = trust_grant_dialog_body(&catalog, &modal).to_lowercase();

    for feature in tekstide_core::security::RestrictedModeFeature::ALL {
        assert!(
            !body.contains(&feature.label().to_lowercase()),
            "the dialog must not enumerate individual restricted features -- found {:?} in \
             {body:?}",
            feature.label()
        );
    }
}

/// `what-the-trust-dialog-must-say.md` §6: none of the three forbidden
/// claims -- that trusting is safe, that Tekstide polices what runs, or
/// that revoking undoes what already ran.
#[test]
fn trust_grant_dialog_body_makes_none_of_the_three_forbidden_claims() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let modal = trust_grant_modal_fixture("/home/user/project", "/home/user/project");

    let body = trust_grant_dialog_body(&catalog, &modal).to_lowercase();

    assert!(
        !body.contains("is safe") && !body.contains("safely"),
        "must not claim trusting is safe: {body:?}"
    );
    assert!(
        !body.contains("tekstide will police")
            && !body.contains("tekstide polices")
            && !body.contains("intercept"),
        "must not claim Tekstide polices what runs: {body:?}"
    );
    assert!(
        body.contains("does not undo"),
        "must state plainly that revoking does not undo what already ran, got {body:?}"
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

/// **Response 247's required proof, and response 248's correction to
/// it**: not "the trust flag changed," but the actual chain trust was
/// blocking -- and not from a dispatched `AppCommand`/`Message`, but
/// from the real key sequence a keyboard user presses
/// (`press_trust_settings_action`: the real `Ctrl+Alt+U` global binding,
/// then the real Enter key `handle_trust_settings_key` handles), the
/// same as every other test in this section since response 248 found
/// the route itself was unreachable. Then launch a
/// profile whose `workspace_discovery_policy` is `MayDiscoverWorkspaceFiles`
/// (the same shape `claude_code_linux_default`'s own honest policy uses,
/// which is what `agent_run_launch_shell_input_switches_to_terminal_immersion_and_shows_the_real_trust_refusal`
/// above proves refuses in a fresh, untrusted project) and confirm a
/// real process actually spawns.
///
/// **Before** granting: refused with `WorkspaceDiscoveryBlocked`, the
/// exact same refusal the sibling test above proves for the real
/// `claude_code_linux_default` profile -- this test's own custom profile
/// reaches the identical gate, not a different one, before diverging
/// only in what happens *after* a real grant.
///
/// **After** granting: `attempt_agent_run_launch_with_profile`'s own
/// full production chain (validate, spawn, register, select) succeeds
/// against a real, controlled test executable -- the same "real spawn
/// machinery, controlled test artifact" shape
/// `attempt_agent_run_launch_with_profile_spawns_registers_and_selects_a_real_run`
/// above already uses, not the live product (interactive auth, real
/// network calls, unsafe for an automated test).
#[test]
fn granting_trust_through_the_real_route_unblocks_a_real_agent_run_launch() {
    let (mut state, project_id) = state_with_a_real_project("trust-unblocks-agent-run");

    let bin_dir = fresh_project_dir("trust-unblocks-agent-run-bin");
    let executable = bin_dir.join("fake-ai-cli");
    std::fs::write(&executable, "#!/bin/sh\n").expect("test executable should be written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("test executable metadata should be readable")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&executable, permissions)
        .expect("test executable permissions should be set");

    let mut profile = tekstide_core::agent::AiCliProfile::new(
        "fake-ai-cli-workspace-discovery",
        "Fake AI CLI (workspace discovery)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: executable,
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );
    // The one field that makes this profile trust-gated -- everything
    // else (`source: BuiltIn`, `prompt_policy: Interactive`,
    // `environment_policy: Minimal`, all `AiCliProfile::new`'s own
    // defaults) already passes `validate_profile_source`/
    // `validate_prompt_policy`/`validate_environment_policy` regardless
    // of trust, so this is genuinely isolating the one gate this test
    // means to prove, not incidentally exercising others too.
    profile.workspace_discovery_policy =
        tekstide_core::agent::AiCliWorkspaceDiscoveryPolicy::MayDiscoverWorkspaceFiles {
            summary: "test fixture: reads workspace files".to_owned(),
        };

    let refusal = attempt_agent_run_launch_with_profile(&mut state, profile.clone())
        .expect_err("an untrusted project must still refuse this profile");
    assert!(
        matches!(
            refusal,
            AgentRunLaunchRefusal::Validation(
                tekstide_core::agent::AgentRunLaunchValidationError::WorkspaceDiscoveryBlocked { .. }
            )
        ),
        "test precondition: must refuse with WorkspaceDiscoveryBlocked before granting, got \
         {refusal:?}"
    );
    assert_eq!(
        state.terminal_panes.len(),
        0,
        "a refusal must not add a pane"
    );

    // The real grant, through the real route -- not `grant_trust` called
    // directly, not a test-only bypass.
    press_trust_settings_action(&mut state);
    let _ = super::update(&mut state, Message::ModalFocusNext);
    let _ = super::update(&mut state, Message::ModalActivate);
    assert_eq!(
        state
            .app_shell
            .state()
            .project(&project_id)
            .unwrap()
            .trust_state()
            .label(),
        "Trusted",
        "test precondition: the real grant must have taken effect"
    );

    attempt_agent_run_launch_with_profile(&mut state, profile)
        .expect("the identical profile must now launch for real, once trust is granted");

    assert_eq!(
        state.terminal_panes.len(),
        1,
        "a real grant must genuinely unblock a real launch, not merely flip the trust flag"
    );
    let project = state.app_shell.state().active_project().unwrap();
    assert_eq!(project.agent_runs().len(), 1);
    assert_eq!(
        project.agent_runs()[0].status,
        tekstide_core::domain::AgentRunStatus::Running,
        "the run the previously-blocking gate now allows must actually be running, not just \
         validated"
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

/// RFC-033 PR-033-A's own required gate: a real `Managed` launch with
/// transcript capture disabled must still bind its approval channel --
/// through `attempt_agent_run_launch_with_profile_state_root_and_capture`,
/// the exact seam PR-033-B's real per-project opt-out will drive. Before
/// this slice's fix, the GUI launch call site never set
/// `approval_state_root` explicitly, so a `Managed` launch with no
/// transcript state root configured would have had nothing for its
/// approval channel to bind to either, and failed closed with
/// `AgentAdapterApprovalError::StateRootMissing` -- not reachable in
/// production yet (`claude_code_linux_default` is `Supervised`), which
/// is exactly why this had to land before the opt-out that makes it
/// reachable.
#[test]
fn a_managed_launch_with_capture_disabled_still_binds_its_approval_channel() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("capture-disabled-managed-launch");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();

    let mut profile = tekstide_core::agent::AiCliProfile::new(
        "reference-adapter",
        "Reference Adapter (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: reference_adapter_binary_path(),
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

    attempt_agent_run_launch_with_profile_state_root_and_capture(
        &mut state,
        profile,
        Some(state_root.clone()),
        false,
    )
    .expect(
        "a Managed launch with capture disabled must still bind its approval channel via the \
         explicit approval_state_root this slice adds",
    );

    assert_eq!(
        state.approval_channels.len(),
        1,
        "the approval channel must actually be live, not merely absent an error"
    );
    assert!(
        !state_root.join("transcripts").exists(),
        "capture was disabled -- no transcript directory should have been created"
    );
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

/// change-detection-wiring handoff, Slice C: the gate's own required
/// shape -- a real `ChangeSet`, created in production, from a real
/// completion. `launch_real_managed_agent_run` reaches the exact same
/// production `attempt_agent_run_launch_with_profile` a real
/// `Ctrl+Alt+A` press does (only the executable is test-controlled --
/// see that helper's own doc); this test's job is what this slice
/// specifically adds beyond an already-real launch path: a baseline
/// captured at that launch, and a real detection run at a real exit.
///
/// Writes a real file into the real project directory after the
/// baseline is captured (what an agent changing files looks like from
/// the outside -- the reference adapter itself never touches the
/// filesystem), approves the adapter's own real proposal over the real
/// socket via the same `Message::ModalActivate` route
/// `command_approval_family_produces_real_durable_audit_records_through_the_pipeline`
/// already proves delivers a real decision (which is what makes the
/// real process exit for real, with a real `0` status), then drives the
/// real `Message::TerminalWoke` handler -- the same real exit-detection
/// loop `a_real_session_exit_updates_status_frees_the_slot_and_is_reusable`
/// already establishes for a plain terminal -- until this run reaches a
/// real terminal `AgentRunStatus`.
#[test]
fn a_real_agent_run_exit_creates_a_real_change_set_from_a_real_file_change() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("change-detection-real-agent-run-exit");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let agent_run_id = launch_real_managed_agent_run(&mut state);
    assert!(
        state.agent_run_change_baselines.contains_key(&agent_run_id),
        "a real Managed launch must capture a real filesystem baseline for this run"
    );

    // What an agent changing files looks like from the outside -- a real
    // write to the real project directory, made after the baseline
    // above was already captured, so detection has something real to
    // find.
    std::fs::write(
        project_dir.join("agent-created-file.txt"),
        b"a real change, made after the baseline was captured\n",
    )
    .expect("writing a real file into the real project directory must succeed");

    let received = poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    assert!(
        received,
        "the real adapter should send its proposal within the poll window"
    );

    let request = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .clone();
    let proposal_id = state.approval_proposal_ids[&request.id].clone();
    state.modal = Some(ModalContent::Approval(Box::new(ApprovalDialog::for_test(
        request,
        proposal_id,
        ApprovalDialogButton::ApproveOnce,
    ))));
    // Delivers a real `approved_once` decision over the real socket --
    // the reference adapter's own source (`bin/reference_adapter.rs`)
    // exits `0` immediately once it reads this, which is the real exit
    // this test's own wake loop below waits for.
    let _ = super::update(&mut state, Message::ModalActivate);

    let terminal_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .agent_runs()
        .iter()
        .find(|run| run.id == agent_run_id)
        .unwrap()
        .terminal_id
        .clone()
        .expect("a launched agent run must have a real terminal id");

    let status_of = |state: &State| {
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .agent_runs()
            .iter()
            .find(|run| run.id == agent_run_id)
            .unwrap()
            .status
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline
        && matches!(
            status_of(&state),
            tekstide_core::domain::AgentRunStatus::Running
                | tekstide_core::domain::AgentRunStatus::AwaitingApproval
        )
    {
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert_eq!(
        status_of(&state),
        tekstide_core::domain::AgentRunStatus::Completed,
        "the reference adapter exits 0 on ApprovedOnce, which must land the run at Completed \
         within a few real wakes, through apply_agent_terminal_outcome"
    );
    assert!(
        !state.agent_run_change_baselines.contains_key(&agent_run_id),
        "the baseline must be consumed once detection has been attempted for this run, whether \
         or not a real ChangeSet resulted"
    );

    let project = state.app_shell.state().active_project().unwrap();
    let change_set = project
        .change_sets()
        .iter()
        .find(|change_set| change_set.agent_run_id.as_ref() == Some(&agent_run_id))
        .unwrap_or_else(|| {
            panic!(
                "expected a real ChangeSet strongly associated with this agent run: {:?}",
                project.change_sets()
            )
        });
    assert_eq!(
        change_set.association_confidence,
        tekstide_core::domain::ChangeAssociationConfidence::Strong,
        "a single, unambiguous run with no other run overlapping its baseline must associate \
         Strong, not Ambiguous"
    );
    assert_eq!(
        change_set.changed_files,
        vec![std::path::PathBuf::from("agent-created-file.txt")],
        "the real file written above must be the one real change detected: {:?}",
        change_set.changed_files
    );
}

/// change-detection-wiring handoff, Slice D (D2): the property the
/// handoff's own finding names directly -- a truncated scan and a
/// genuinely clean one both produce **zero** entries in
/// `project.change_sets()` for their run, so that collection alone
/// cannot tell them apart. `state.agent_run_change_detection_status`
/// must be able to, or a truncated result reads as "nothing changed"
/// exactly like the defect the handoff describes for `detect_filesystem_changes`'s
/// own `Vec::new()` -- one layer up, where Slice C's `let _ =` used to
/// discard the distinction silently.
///
/// Two real, separately launched agent runs in the same project, so a
/// stale entry from one could not accidentally satisfy an assertion
/// meant for the other:
///
/// - Run A's real baseline is overwritten with a hand-crafted `Partial`
///   one before detection runs -- forcing genuine truncation would mean
///   creating 16,384+ real files, which this test does not need to do
///   to prove the recording behaviour itself. `attempt_generated_change_detection`
///   is called directly, the same production function
///   `record_terminal_exit` calls after a real exit -- this test's job
///   is the new D2 distinction, not re-proving reachability, which the
///   test above already does from a real key press.
/// - Run B's baseline is the real one Slice C's launch path captures,
///   untouched, with no file changes made -- a genuinely clean,
///   `Complete` scan.
#[test]
fn truncated_and_clean_detections_are_distinguishable_though_both_produce_no_change_set() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("change-detection-truncation-honesty"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    let run_a = launch_real_managed_agent_run(&mut state);
    let run_b = launch_real_managed_agent_run(&mut state);
    let project_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .id()
        .clone();

    let truncated_baseline = tekstide_core::project::ReviewBaseline {
        project_id,
        agent_run_id: Some(run_a.clone()),
        captured_at: tekstide_core::domain::DomainTimestamp::now_utc(),
        source: tekstide_core::domain::ChangeDetectionSource::FilesystemSnapshot,
        baseline_snapshot_ref: "filesystem-snapshot:truncation-honesty-test:0".to_owned(),
        entries: Vec::new(),
        status: tekstide_core::domain::ChangeDetectionStatus::Partial { limit: 1 },
    };
    state
        .agent_run_change_baselines
        .insert(run_a.clone(), truncated_baseline);

    super::attempt_generated_change_detection(&mut state, &run_a);
    super::attempt_generated_change_detection(&mut state, &run_b);

    assert_eq!(
        state.agent_run_change_detection_status.get(&run_a),
        Some(&tekstide_core::domain::ChangeDetectionStatus::Partial { limit: 1 }),
        "the truncated run's status must be recorded exactly, not silently dropped"
    );
    assert_eq!(
        state.agent_run_change_detection_status.get(&run_b),
        Some(&tekstide_core::domain::ChangeDetectionStatus::Complete),
        "the clean run's status must record Complete -- if this is anything else, the contrast \
         below proves nothing"
    );

    let project = state.app_shell.state().active_project().unwrap();
    assert!(
        project
            .change_sets()
            .iter()
            .all(|change_set| change_set.agent_run_id.as_ref() != Some(&run_a)),
        "a truncated scan must not produce a ChangeSet: {:?}",
        project.change_sets()
    );
    assert!(
        project
            .change_sets()
            .iter()
            .all(|change_set| change_set.agent_run_id.as_ref() != Some(&run_b)),
        "a genuinely clean scan must also not produce a ChangeSet -- there is nothing to \
         review: {:?}",
        project.change_sets()
    );

    // The property D2 exists for: both runs produced zero ChangeSets
    // (asserted identically above), and yet their recorded statuses
    // differ -- so a future reader consulting the status map, not just
    // `change_sets()`, can tell "unknown" apart from "genuinely clean."
    assert_ne!(
        state.agent_run_change_detection_status.get(&run_a),
        state.agent_run_change_detection_status.get(&run_b),
        "truncated and clean must never be recorded identically"
    );
}

/// transcript-capture-evidence handoff: `0.10.0` and `0.11.0` both
/// claimed Tekstide writes no transcripts. It writes one for every AI
/// CLI run, and no test on the real launch path ever looked. This one
/// does -- `attempt_agent_run_launch_with_profile_and_state_root` is
/// the exact function `Ctrl+Alt+A` reaches (through
/// `attempt_agent_run_launch_with_profile`, which now delegates to it),
/// pointed at a temporary directory rather than the developer's real
/// `$XDG_STATE_HOME` via the injectable seam this slice's own gate
/// asked for. Uses a `Supervised` profile -- the real
/// `claude_code_linux_default`'s own compatibility level, unlike
/// `launch_real_managed_agent_run`'s `Managed` reference adapter --
/// pointed at a tiny marker-printing script instead, so this test
/// exercises the exact shape of the real production launch rather than
/// the `Managed`-only approval machinery a real AI CLI run never uses.
///
/// Asserts the documented path shape exactly
/// (`<state root>/transcripts/<project>/<agent-run>/transcript.log`,
/// `README.md`'s own *Local Data and Privacy* wording) and that the
/// file contains real content -- the script's own known, printed
/// marker. A file that merely exists but is empty would still pass a
/// weaker assertion; this cannot.
#[test]
fn a_real_agent_run_launch_writes_a_real_transcript_with_real_content() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("transcript-capture-evidence");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();

    let profile = tekstide_core::agent::AiCliProfile::new(
        "transcript-marker-script",
        "Transcript Marker Script (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: transcript_marker_script_path(),
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    attempt_agent_run_launch_with_profile_and_state_root(
        &mut state,
        profile,
        Some(state_root.clone()),
    )
    .expect("a resolvable Supervised profile should launch the real marker script");

    let (project_id, agent_run_id, terminal_id) = capture_evidence_run_identifiers(&state);

    let expected_transcript_file = state_root
        .join("transcripts")
        .join(project_id.as_str())
        .join(agent_run_id.as_str())
        .join("transcript.log");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut transcript_bytes = Vec::new();
    while std::time::Instant::now() < deadline {
        // Real exit detection, the same real route
        // `a_real_session_exit_updates_status_frees_the_slot_and_is_reusable`
        // already establishes -- driving it also drives the terminal
        // reader thread that writes the transcript.
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        if let Ok(bytes) = std::fs::read(&expected_transcript_file) {
            let found_marker = String::from_utf8_lossy(&bytes)
                .contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER");
            transcript_bytes = bytes;
            if found_marker {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert!(
        expected_transcript_file.is_file(),
        "expected a real transcript file at the documented path shape -- README.md's own \
         *Local Data and Privacy* claim -- got nothing at {}",
        expected_transcript_file.display()
    );
    let transcript_text = String::from_utf8_lossy(&transcript_bytes);
    assert!(
        transcript_text.contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER"),
        "expected the real script's own printed marker in the transcript -- an empty or \
         unrelated file must not pass this assertion: {transcript_text:?}"
    );
}

/// Extracted for RFC-033 PR-033-B's own negative gate to share: both
/// tests need the same (project id, agent run id, terminal id) triple
/// after a real launch, to build the documented transcript path and
/// drive the same real exit-detection poll.
fn capture_evidence_run_identifiers(
    state: &State,
) -> (
    tekstide_core::project::ProjectId,
    tekstide_core::domain::AgentRunId,
    tekstide_core::domain::TerminalId,
) {
    let project = state.app_shell.state().active_project().unwrap();
    let run = project.agent_runs().last().unwrap();
    (
        project.id().clone(),
        run.id.clone(),
        run.terminal_id
            .clone()
            .expect("a launched run has a real terminal id"),
    )
}

/// RFC-033 PR-033-B's own required gate, verbatim: "proven from a real
/// key press through to a run that produces NO transcript file,
/// asserted against the real path shape ... not against 'the request
/// said disabled.'" `press_transcript_capture_toggle` reaches the
/// setting the same real way a user would (`Ctrl+Alt+U` then Space);
/// the launch itself goes through `attempt_agent_run_launch_with_profile_and_state_root`
/// -- the same real production wiring `a_real_agent_run_launch_writes_a_real_transcript_with_real_content`
/// proves *writes* a transcript, now proving the opposite once capture
/// has been declined through the real route -- not a direct call
/// naming `capture_enabled: false`, which would only prove the plumbing
/// in isolation, the gate's own stated distinction.
#[test]
fn declining_capture_through_a_real_key_press_produces_no_transcript_file() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("capture-declined-no-transcript");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();

    press_transcript_capture_toggle(&mut state);
    assert!(
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .transcript_capture_declined(),
        "test precondition: the real key press must have declined capture"
    );

    let profile = tekstide_core::agent::AiCliProfile::new(
        "transcript-marker-script",
        "Transcript Marker Script (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: transcript_marker_script_path(),
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    attempt_agent_run_launch_with_profile_and_state_root(
        &mut state,
        profile,
        Some(state_root.clone()),
    )
    .expect("declining capture must not prevent the run itself from launching");

    let (project_id, agent_run_id, terminal_id) = capture_evidence_run_identifiers(&state);
    let expected_transcript_file = state_root
        .join("transcripts")
        .join(project_id.as_str())
        .join(agent_run_id.as_str())
        .join("transcript.log");

    // Same real exit-detection poll as the positive control -- give the
    // run every chance to have written a transcript, so the eventual
    // negative assertion is not merely "we didn't wait long enough."
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert!(
        !expected_transcript_file.exists(),
        "capture was declined through a real key press -- no transcript file may exist at the \
         documented path shape, got one at {}",
        expected_transcript_file.display()
    );
    assert!(
        !state_root.join("transcripts").exists(),
        "not even the transcripts directory should have been created"
    );
}

/// RFC-033 PR-033-C: reachability, the same standard PR-033-B's own
/// capture-toggle reachability test set -- a real Delete keypress on
/// Trust Settings, through `press_transcript_purge_key`, not a
/// dispatched `Message::OpenTranscriptPurgeDialog`.
#[test]
fn pressing_delete_on_trust_settings_opens_the_purge_confirmation_dialog() {
    let (mut state, _project_id) = state_with_a_real_project("purge-dialog-reachable");

    press_transcript_purge_key(&mut state);

    assert!(
        matches!(state.modal, Some(ModalContent::TranscriptPurge(_))),
        "a real Delete keypress on Trust Settings must open the purge confirmation dialog, got \
         {:?}",
        state.modal
    );
}

/// RFC-033 PR-033-C's own required gate, verbatim: "bytes gone from the
/// real filesystem, asserted directly -- not the return value, not the
/// metadata." Real launch -> a real transcript file with real content
/// (the same positive control `a_real_agent_run_launch_writes_a_real_transcript_with_real_content`
/// establishes) -> a real key sequence through the confirmation dialog
/// (Delete opens it, `ModalFocusNext` moves focus to `Purge`,
/// `ModalActivate` confirms, the same two-deliberate-acts shape
/// `trust_grant_dialog_requires_moving_focus_and_activating_to_grant`
/// already requires) -> the file itself must be gone from disk, and the
/// tombstone (`what-purge-must-remove.md`'s own required property) must
/// remain.
#[test]
fn purging_transcripts_through_a_real_key_sequence_removes_the_real_file() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("purge-real-file");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();

    let profile = tekstide_core::agent::AiCliProfile::new(
        "transcript-marker-script",
        "Transcript Marker Script (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: transcript_marker_script_path(),
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    attempt_agent_run_launch_with_profile_and_state_root(
        &mut state,
        profile,
        Some(state_root.clone()),
    )
    .expect("a resolvable Supervised profile should launch the real marker script");

    let (project_id, agent_run_id, terminal_id) = capture_evidence_run_identifiers(&state);
    let expected_transcript_file = state_root
        .join("transcripts")
        .join(project_id.as_str())
        .join(agent_run_id.as_str())
        .join("transcript.log");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        let marker_written = std::fs::read(&expected_transcript_file)
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER")
            })
            .unwrap_or(false);
        if marker_written {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "test precondition: the real transcript file never appeared with its marker"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    press_transcript_purge_key(&mut state);
    assert!(
        matches!(state.modal, Some(ModalContent::TranscriptPurge(_))),
        "test precondition: the purge dialog must be open before moving focus and activating"
    );
    let _ = super::update(&mut state, Message::ModalFocusNext);
    match state.modal.as_ref() {
        Some(ModalContent::TranscriptPurge(modal)) => {
            assert_eq!(modal.focus, TranscriptPurgeButton::Purge);
        }
        other => panic!("expected an open TranscriptPurge dialog, got {other:?}"),
    }
    let _ = super::update(&mut state, Message::ModalActivate);

    assert!(
        state.modal.is_none(),
        "activating the purge decision must close the dialog"
    );
    assert!(
        !expected_transcript_file.exists(),
        "purge was confirmed through a real key sequence -- the real transcript file must be \
         gone from disk, got one still at {}",
        expected_transcript_file.display()
    );
    let project = state.app_shell.state().active_project().unwrap();
    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.agent_run_id.as_ref() == Some(&agent_run_id))
        .expect("the purged transcript's own record must still exist");
    assert!(
        transcript.is_tombstone(),
        "the tombstone must be preserved, per what-purge-must-remove.md"
    );
}

/// RFC-033 PR-033-D: **audit records queried and asserted, not
/// implied** (`granting_trust_through_the_real_route_records_both_audit_records`'s
/// own standard) -- a real purge through the real route (Delete,
/// `ModalFocusNext`, `ModalActivate`) writes exactly one
/// `TranscriptPurge` record, `Completed`, naming this project and
/// nothing else -- `AuditCoordinator::purge_project_transcripts`'s
/// first GUI caller.
#[test]
fn purging_transcripts_through_a_real_key_sequence_records_a_real_audit_record() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("purge-real-audit-record");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();
    let project_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .id()
        .clone();

    let profile = tekstide_core::agent::AiCliProfile::new(
        "transcript-marker-script",
        "Transcript Marker Script (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: transcript_marker_script_path(),
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    attempt_agent_run_launch_with_profile_and_state_root(
        &mut state,
        profile,
        Some(state_root.clone()),
    )
    .expect("a resolvable Supervised profile should launch the real marker script");

    let (project_id_from_run, agent_run_id, terminal_id) = capture_evidence_run_identifiers(&state);
    assert_eq!(project_id_from_run, project_id);
    let expected_transcript_file = state_root
        .join("transcripts")
        .join(project_id.as_str())
        .join(agent_run_id.as_str())
        .join("transcript.log");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        let marker_written = std::fs::read(&expected_transcript_file)
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER")
            })
            .unwrap_or(false);
        if marker_written {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "test precondition: the real transcript file never appeared with its marker"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    press_transcript_purge_key(&mut state);
    let _ = super::update(&mut state, Message::ModalFocusNext);
    let _ = super::update(&mut state, Message::ModalActivate);
    assert!(
        !expected_transcript_file.exists(),
        "test precondition: the purge itself must have succeeded"
    );

    let audit_store =
        open_real_audit_store(&state.app_shell).expect("the real audit store must open");
    let records = audit_store
        .query(&tekstide_core::audit::AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| {
            record.project_id.as_ref() == Some(&project_id)
                && record.family == tekstide_core::audit::AuditEventFamily::TranscriptPurge
        })
        .collect::<Vec<_>>();

    assert_eq!(
        records.len(),
        1,
        "a real project-wide purge must write exactly one TranscriptPurge record for this \
         project: {records:?}"
    );
    let record = &records[0];
    assert_eq!(
        record.family,
        tekstide_core::audit::AuditEventFamily::TranscriptPurge
    );
    assert_eq!(
        record.action_kind,
        tekstide_core::audit::AuditActionKind::TranscriptPurge
    );
    assert_eq!(
        record.outcome,
        tekstide_core::audit::AuditOutcome::Completed
    );
    assert_eq!(
        record.subject_kind,
        Some(tekstide_core::audit::AuditSubjectKind::Transcript)
    );
    assert_eq!(
        record
            .subject_ref
            .as_ref()
            .map(|reference| reference.as_str()),
        Some("project")
    );
}

/// The other half, mirroring `trust_grant_dialog_defaults_focus_to_cancel_and_activating_it_grants_nothing`:
/// activating on the default `Cancel` focus (i.e. Escape's equivalent,
/// reachable via Enter too) must not delete anything -- purging needs
/// the same two deliberate acts real deletion always does in this file.
#[test]
fn cancelling_the_purge_dialog_leaves_the_real_transcript_file_untouched() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("purge-cancel-untouched");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();

    let profile = tekstide_core::agent::AiCliProfile::new(
        "transcript-marker-script",
        "Transcript Marker Script (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: transcript_marker_script_path(),
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    attempt_agent_run_launch_with_profile_and_state_root(
        &mut state,
        profile,
        Some(state_root.clone()),
    )
    .expect("a resolvable Supervised profile should launch the real marker script");

    let (project_id, agent_run_id, terminal_id) = capture_evidence_run_identifiers(&state);
    let expected_transcript_file = state_root
        .join("transcripts")
        .join(project_id.as_str())
        .join(agent_run_id.as_str())
        .join("transcript.log");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        let marker_written = std::fs::read(&expected_transcript_file)
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER")
            })
            .unwrap_or(false);
        if marker_written {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "test precondition: the real transcript file never appeared with its marker"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    press_transcript_purge_key(&mut state);
    match state.modal.as_ref() {
        Some(ModalContent::TranscriptPurge(modal)) => {
            assert_eq!(
                modal.focus,
                TranscriptPurgeButton::Cancel,
                "test precondition: the dialog must default to Cancel"
            );
        }
        other => panic!("expected an open TranscriptPurge dialog, got {other:?}"),
    }
    let _ = super::update(&mut state, Message::ModalActivate);

    assert!(state.modal.is_none(), "activating must close the dialog");
    assert!(
        expected_transcript_file.exists(),
        "activating on the default Cancel focus must not delete anything -- the real transcript \
         file must still be at {}",
        expected_transcript_file.display()
    );
    let project = state.app_shell.state().active_project().unwrap();
    let transcript = project
        .transcripts()
        .iter()
        .find(|transcript| transcript.agent_run_id.as_ref() == Some(&agent_run_id))
        .expect("the transcript's own record must still exist");
    assert!(
        !transcript.is_tombstone(),
        "cancelling must not mark the transcript purged either"
    );
}

/// RFC-033 PR-033-C: `transcript_local_data_summary`'s first real
/// caller, proven against real data -- not merely that the call
/// compiles. The real transcript file's own byte count on disk must
/// match what the wired summary reports, and the count must be exactly
/// one real transcript, not a placeholder.
#[test]
fn retained_transcript_visibility_reflects_a_real_transcripts_real_byte_count() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("purge-visibility-real-bytes");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();

    let profile = tekstide_core::agent::AiCliProfile::new(
        "transcript-marker-script",
        "Transcript Marker Script (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: transcript_marker_script_path(),
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );

    attempt_agent_run_launch_with_profile_and_state_root(
        &mut state,
        profile,
        Some(state_root.clone()),
    )
    .expect("a resolvable Supervised profile should launch the real marker script");

    let (project_id, agent_run_id, terminal_id) = capture_evidence_run_identifiers(&state);
    let expected_transcript_file = state_root
        .join("transcripts")
        .join(project_id.as_str())
        .join(agent_run_id.as_str())
        .join("transcript.log");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let real_byte_count = loop {
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        if let Ok(metadata) = std::fs::metadata(&expected_transcript_file)
            && std::fs::read(&expected_transcript_file)
                .map(|bytes| {
                    String::from_utf8_lossy(&bytes)
                        .contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER")
                })
                .unwrap_or(false)
        {
            break metadata.len();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "test precondition: the real transcript file never appeared with its marker"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(
        real_byte_count > 0,
        "test precondition: the real marker script must have written real bytes"
    );

    let project = state.app_shell.state().active_project().unwrap();
    let summary = transcript_local_data_summary_for(&state, project);

    assert_eq!(
        summary.project_transcript_count, 1,
        "exactly one real transcript exists for this project"
    );
    assert_eq!(
        summary.project_retained_bytes, real_byte_count,
        "the wired summary must report the real file's real byte count, not a placeholder"
    );
}

/// A tiny, real, executable script that prints one known marker line
/// and exits -- the "controlled test executable" the handoff's own
/// gate asks for, so `a_real_agent_run_launch_writes_a_real_transcript_with_real_content`
/// has a real, deliberate byte sequence to assert on rather than
/// trusting that any output at all implies capture worked.
fn transcript_marker_script_path() -> std::path::PathBuf {
    let script_path = std::env::temp_dir().join(format!(
        "tsms-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &script_path,
        "#!/bin/sh\necho TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER\n",
    )
    .expect("writing the marker script should succeed");
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))
        .expect("marking the marker script executable should succeed");
    script_path
}

/// transcript-capture-evidence handoff: `README.md`'s second claim --
/// "a plain terminal (`Ctrl+Alt+T`) is not recorded" -- asserted, not
/// assumed. `attempt_terminal_launch` and the `launch_terminal` it
/// calls take no state-root/transcript parameter at all, structurally
/// unlike `attempt_agent_run_launch_with_profile_and_state_root`, so
/// this checks the domain-level fact a future reader would actually
/// consult: no `Transcript` is ever attached to the project for a plain
/// terminal's session.
#[test]
fn a_plain_terminal_launch_writes_no_transcript() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir(
            "transcript-capture-evidence-plain-terminal",
        ))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    super::attempt_terminal_launch(&mut state).expect("a plain terminal launch must succeed");

    assert!(
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .transcripts()
            .is_empty(),
        "a plain terminal must never attach a transcript"
    );
}

/// pr-020-b-report-surface.md: the gate's own required shape -- **a
/// real key press** (`Ctrl+Alt+R`), through the real `update` dispatch,
/// not a shortcut into `ProjectSession`'s own state -- opens the real
/// surface, and the same production function the view calls
/// (`agent_run_transcript_window`) reaches **a real transcript from a
/// real run**, using the exact seam `transcript-capture-evidence.md`
/// added (`attempt_agent_run_launch_with_profile_and_state_root`, a
/// marker script, a temporary state root) rather than a hand-written
/// fixture file.
#[test]
fn a_real_key_press_opens_the_report_surface_and_reaches_a_real_transcript() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("agent-run-report-reachability"))
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);
    let state_root = fresh_state_root_dir();

    let profile = tekstide_core::agent::AiCliProfile::new(
        "transcript-marker-script",
        "Transcript Marker Script (test-only)",
        tekstide_core::agent::AiCliProfileSource::BuiltIn,
        tekstide_core::agent::AiCliExecutable::Absolute {
            path: transcript_marker_script_path(),
            provenance: tekstide_core::agent::AiCliExecutableProvenance::SystemPathReviewed,
        },
        tekstide_core::domain::AgentCompatibilityLevel::Supervised,
    );
    attempt_agent_run_launch_with_profile_and_state_root(
        &mut state,
        profile,
        Some(state_root.clone()),
    )
    .expect("a resolvable Supervised profile should launch the real marker script");

    let terminal_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .agent_runs()
        .last()
        .unwrap()
        .terminal_id
        .clone()
        .expect("a launched run has a real terminal id");

    // Wait for the marker to actually land on disk through the exact
    // production lookup the view itself calls -- the same real
    // exit-detection loop `a_real_agent_run_launch_writes_a_real_transcript_with_real_content`
    // already establishes, driving the terminal reader thread that
    // writes the transcript.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let _ = super::update(&mut state, Message::TerminalWoke(terminal_id.clone()));
        let project = state.app_shell.state().active_project().unwrap();
        let run = project.agent_runs().last().unwrap();
        if let Ok((_, window)) = super::agent_run_transcript_window_with_state_root(
            project,
            run,
            Some(state_root.clone()),
        ) && String::from_utf8_lossy(window.content())
            .contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER")
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the real marker never landed in the real transcript within the poll window"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // The real key press.
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenCurrentAgentRunDetail,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    let project = state.app_shell.state().active_project().unwrap();
    assert_eq!(
        project.open_surface(),
        tekstide_core::project::ProjectOpenSurface::AgentRunDetail,
        "OpenCurrentAgentRunDetail must reach the real AppCommand and set the real surface"
    );
    assert_eq!(project.mode(), tekstide_core::project::ProjectMode::Content);

    let run = project.agent_runs().last().unwrap();
    let (_, window) =
        super::agent_run_transcript_window_with_state_root(project, run, Some(state_root.clone()))
            .expect("the same production lookup the view calls must still succeed");
    let escaped = super::agent_run_detail_transcript_body(window.content());
    assert!(
        escaped
            .as_str()
            .contains("TEKSTIDE-TRANSCRIPT-CAPTURE-EVIDENCE-MARKER"),
        "the escaped body must still contain the real marker -- plain ASCII text is untouched \
         by escaping: {:?}",
        escaped.as_str()
    );
}

/// `the-window-boundary.md` §2's own required evidence. **What this
/// cannot prove, and does not claim to**: that the two cases render as
/// *different visible text* -- they cannot. `escape_untrusted_chars`
/// passes plain ASCII through unchanged, so a literal 8-character
/// `<U+202E>` string and an escaped real override necessarily produce
/// the identical marker text `<U+202E>`. That is `quote_untrusted`'s
/// own already-proven contract (`text_safety`'s own suite), not
/// something a caller could or should change.
///
/// **What this proves instead, mechanically**: a real override
/// actually becomes a visible marker rather than surviving as a raw
/// directionality control, literal marker-shaped text survives
/// unmodified rather than being stripped or re-escaped, and -- the
/// concrete, checkable shape "double escaping" would take here -- the
/// isolate wrapping (`quote_untrusted`'s own FSI/PDI marks) is never
/// itself visible as escaped text, which is what a second escaping
/// pass over already-escaped content would produce, since FSI/PDI are
/// themselves Unicode Format characters and exactly what this module
/// escapes.
#[test]
fn transcript_body_escapes_a_real_override_and_does_not_double_escape_literal_marker_text() {
    let real_override = "before \u{202E} after".as_bytes();
    let escaped_override = super::agent_run_detail_transcript_body(real_override);
    assert!(
        escaped_override.as_str().contains("<U+202E>"),
        "a real override character must become a visible marker: {:?}",
        escaped_override.as_str()
    );
    assert!(
        !escaped_override.as_str().contains('\u{202E}'),
        "the real override character itself must never survive into the rendered text: {:?}",
        escaped_override.as_str()
    );

    let literal_marker_text = "before <U+202E> after".as_bytes();
    let escaped_literal = super::agent_run_detail_transcript_body(literal_marker_text);
    assert!(
        escaped_literal.as_str().contains("before <U+202E> after"),
        "literal ASCII text that already looks like a marker must pass through unchanged, not \
         be re-escaped or stripped: {:?}",
        escaped_literal.as_str()
    );

    for rendered in [escaped_override.as_str(), escaped_literal.as_str()] {
        assert!(
            !rendered.contains("<U+2068>") && !rendered.contains("<U+2069>"),
            "the isolate marks must never themselves be visible as escaped text -- that would \
             mean this content was escaped more than once: {rendered:?}"
        );
    }
}

/// `the-window-boundary.md`'s own required distinction: a **reader
/// window** (`TranscriptWindow::delivered_start() > 0` -- this is a
/// tail slice of a possibly-larger file) and **writer truncation**
/// (`Transcript.truncation_state` -- RFC-011's own bounded writer
/// stopped capturing before the run's real output ended) are
/// independent facts about the user's data, and must render as
/// independent, never-conflated notices.
#[test]
fn reader_window_and_writer_truncation_render_as_distinct_notices() {
    let catalog = Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()));
    let mut transcript = tekstide_core::domain::Transcript::metadata(
        tekstide_core::project::ProjectId::new_uuid(),
        tekstide_core::domain::TerminalId::new_uuid(),
        None,
        "/tmp/agent-run-detail-notice-fixture",
        "local-bounded-agent-run",
    );

    let full_clean_window = tekstide_core::transcript::TranscriptWindow::Complete {
        content: b"hello".to_vec(),
        requested_start: 0,
        delivered_start: 0,
        total_len: 5,
    };
    let clean_notices = super::agent_run_detail_notices(&catalog, &transcript, &full_clean_window);
    assert_eq!(
        clean_notices.len(),
        2,
        "a Complete, untruncated, full window must produce exactly the status and window \
         notices, no truncation notice: {clean_notices:?}"
    );

    let partial_window = tekstide_core::transcript::TranscriptWindow::Complete {
        content: b"tail".to_vec(),
        requested_start: 4,
        delivered_start: 4,
        total_len: 100,
    };
    let partial_notices = super::agent_run_detail_notices(&catalog, &transcript, &partial_window);
    assert_eq!(partial_notices.len(), 2);
    assert_ne!(
        partial_notices[1], clean_notices[1],
        "a partial reader window must render a different notice than a full one: {partial_notices:?}"
    );

    transcript.truncation_state = tekstide_core::domain::TruncationState::Truncated;
    let truncated_and_partial =
        super::agent_run_detail_notices(&catalog, &transcript, &partial_window);
    assert_eq!(
        truncated_and_partial.len(),
        3,
        "writer truncation must add its own, third notice, not fold into the window notice: \
         {truncated_and_partial:?}"
    );
    assert_ne!(
        truncated_and_partial[1], truncated_and_partial[2],
        "the reader-window notice and the writer-truncation notice must be textually distinct \
         -- conflating them is the exact failure `the-window-boundary.md` names: {truncated_and_partial:?}"
    );
}

/// The precondition [`agent_run_detail_view`]'s own no-runs branch
/// depends on, confirmed reachable for a genuinely fresh project rather
/// than assumed -- this project's own "positive control" discipline,
/// applied at the same level every sibling surface's empty state is
/// checked at (`approval_history_view`/`trust_settings_view` have no
/// deeper test of their own empty-state branches either; `iced`'s
/// `Element` tree is not introspected anywhere in this suite).
#[test]
fn a_freshly_created_project_has_no_agent_runs_to_show() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("agent-run-report-no-runs"))
        .expect("a freshly created directory is a valid project root");
    let state = state_with(app_shell);

    assert!(
        state
            .app_shell
            .state()
            .active_project()
            .unwrap()
            .agent_runs()
            .last()
            .is_none(),
        "a fresh project must have no agent runs -- the no-runs branch must be reachable, not \
         dead code"
    );
}

/// Response 233: the real navigation-to-dispatch path for
/// `ProjectOpenSurface::ApprovalHistory`, the same shape
/// `a_toggle_project_mode_shell_input_dispatches_the_real_app_command`
/// already proves for a different action. `OpenActiveProjectSurface`
/// forces `ProjectMode::Content` unconditionally
/// (`AppState::open_active_project_surface`, unchanged by this
/// response) -- proven here by starting the project in
/// `TerminalImmersion` first, so success is not an accident of already
/// being in the right mode.
#[test]
fn opening_approval_history_from_navigation_sets_the_open_surface_and_forces_content_mode() {
    let mut app_shell = ApplicationShell::new();
    app_shell
        .add_project_from_path(fresh_project_dir("approval-history-navigate"))
        .expect("a freshly created directory is a valid project root");
    app_shell
        .state_mut()
        .open_active_project_terminal_workspace();
    assert_eq!(
        app_shell
            .state()
            .active_project()
            .map(tekstide_core::project::ProjectSession::mode),
        Some(tekstide_core::project::ProjectMode::TerminalImmersion),
        "test precondition: starting in TerminalImmersion, not already the mode this action \
         would leave behind by accident"
    );

    let mut state = state_with(app_shell);
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenApprovalHistory,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );

    let project = state.app_shell.state().active_project().unwrap();
    assert_eq!(
        project.open_surface(),
        tekstide_core::project::ProjectOpenSurface::ApprovalHistory,
        "OpenApprovalHistory must reach the real AppCommand, not be silently swallowed"
    );
    assert_eq!(
        project.mode(),
        tekstide_core::project::ProjectMode::Content,
        "opening a surface must land in Content mode regardless of which mode preceded it"
    );
    assert_eq!(
        state.app_shell.route(),
        tekstide_core::route::AppRoute::ActiveProjectWorkspace
    );
}

/// Response 233's own design answer, proven end to end: manually
/// opening a live entry from the history surface must **not** consult
/// `should_promote_to_modal` -- a `Low`-risk entry (the real reference
/// adapter's own unconfigurable default, which `evaluate_promotion`
/// would never promote) must still open when the user explicitly asks
/// for it. Reuses the real receive pipeline (no mock, no override),
/// the same methodology `launch_real_managed_agent_run`'s own doc
/// comment establishes, then drives the manual-open message through
/// real `update()` routing and confirms a real decision still reaches
/// the real coordinator afterward -- not just that a dialog appeared.
#[test]
fn manually_opening_a_low_risk_live_entry_bypasses_the_promotion_predicate() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-history-manual-open");
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
    assert_eq!(
        request.risk_level,
        tekstide_core::domain::RiskLevel::Low,
        "test precondition: the real adapter's own default proposal, which \
         should_promote_to_modal would never promote"
    );
    assert!(
        state.modal.is_none(),
        "test precondition: a Low-risk proposal must not have auto-promoted"
    );

    let _ = super::update(
        &mut state,
        Message::OpenApprovalHistoryEntry(request.id.clone()),
    );

    let proposal_id = match state.modal {
        Some(ModalContent::Approval(ref dialog)) => {
            assert_eq!(dialog.request.id, request.id);
            assert_eq!(
                dialog.focus,
                ApprovalDialogButton::Reject,
                "focus must still default to Reject, the same as automatic promotion"
            );
            dialog.proposal_id.clone()
        }
        ref other => panic!(
            "manually opening a Low-risk live entry must open the real dialog anyway, got \
             {other:?}"
        ),
    };

    // Deciding it must still reach the real coordinator -- proving this
    // is the same real dialog/decide path, not a second, inline UI.
    // Rebuilt via `ApprovalDialog::for_test` (`ignore_input_until: None`)
    // rather than deciding the dialog `open_approval_history_entry` just
    // constructed directly -- that one carries a real post-open
    // input-ignore window (the same one promotion uses), which this
    // immediate `ModalActivate` would otherwise fall inside of; the
    // dialog's own construction (request/proposal_id/focus, asserted
    // above) is what this test is actually about, the same separation
    // `deciding_the_promoted_dialog_sends_a_real_decision_and_updates_the_stored_request`
    // already establishes for the promoted case.
    state.modal = Some(ModalContent::Approval(Box::new(ApprovalDialog::for_test(
        request.clone(),
        proposal_id,
        ApprovalDialogButton::Reject,
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
        tekstide_core::domain::ApprovalDecision::Rejected,
        "the real decide round trip must reach Decided -- got {stored:?}"
    );
}

/// The one rule response 233 said still applies to a manual open even
/// though promotion's own guards do not: never replace an open modal.
/// Opens a real, promoted `Destructive` dialog first (any modal would
/// do; a real one is used since it is already available), then attempts
/// to manually open a second, different entry -- the first dialog must
/// still be the one showing.
#[test]
fn manually_opening_an_entry_does_not_replace_an_already_open_modal() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-history-manual-open-guard");
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
    let first_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .id
        .clone();

    let _ = super::update(
        &mut state,
        Message::OpenApprovalHistoryEntry(first_id.clone()),
    );
    assert!(
        matches!(state.modal, Some(ModalContent::Approval(_))),
        "test precondition: the first manual open must succeed"
    );

    // A second, distinct request in the same project -- a fabricated
    // one is fine here, since the guard under test (`state.modal.is_some()`)
    // is checked before this id is ever looked up.
    let second_id = tekstide_core::domain::ApprovalId::new_uuid();
    let _ = super::update(&mut state, Message::OpenApprovalHistoryEntry(second_id));

    match state.modal {
        Some(ModalContent::Approval(ref dialog)) => {
            assert_eq!(
                dialog.request.id, first_id,
                "an already-open modal must not be replaced by a second manual-open request"
            );
        }
        ref other => panic!("the first dialog must still be open, got {other:?}"),
    }
}

fn press(key: iced::keyboard::Key) -> crate::input::KeyPress {
    crate::input::KeyPress {
        key,
        modifiers: iced::keyboard::Modifiers::empty(),
    }
}

fn send_main_area_key(state: &mut State, key: iced::keyboard::Key) {
    let routed = crate::input::RoutedInput::Surface(crate::input::surface_input_for_test(
        FocusZone::MainArea,
        press(key),
    ));
    let _ = super::update(state, Message::Input(routed));
}

/// Response 234: the reviewer's own required change -- every other
/// interactive list in this crate (the explorer) is keyboard-navigable,
/// and a mouse-only history list silently re-imposes the "some
/// non-promoted proposals are unanswerable" design the owner rejected,
/// for every keyboard user. Proves Up/Down actually move
/// `state.approval_history_highlight`, the same real-routing shape
/// `a_typed_key_edits_the_real_active_document_through_real_routing`
/// already establishes for the editor's own zone.
#[test]
fn arrow_keys_move_the_approval_history_highlight() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-history-arrow-nav");
    app_shell
        .add_project_from_path(&project_dir)
        .expect("a freshly created directory is a valid project root");
    let mut state = state_with(app_shell);

    // Two real, retained requests -- one launch alone would leave
    // nothing for Down to move *to*.
    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| !project.approval_requests().is_empty())
    });
    launch_real_managed_agent_run(&mut state);
    poll_approval_channels_until(&mut state, |state| {
        state
            .app_shell
            .state()
            .active_project()
            .is_some_and(|project| project.approval_requests().len() == 2)
    });

    // approval-history-binding handoff: opened through the real
    // `Ctrl+Alt+H` route (`shell_input_for_test`, dispatched the same way
    // a real key press reaches `update`), not a directly-dispatched
    // `AppCommand` -- the same lesson response 248 established for
    // `press_trust_settings_action` below.
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenApprovalHistory,
    );
    let _ = super::update(
        &mut state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );
    assert_eq!(state.approval_history_highlight, 0, "test precondition");

    send_main_area_key(
        &mut state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
    );
    assert_eq!(
        state.approval_history_highlight, 1,
        "ArrowDown must move the highlight to the second retained request"
    );

    send_main_area_key(
        &mut state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
    );
    assert_eq!(
        state.approval_history_highlight, 1,
        "ArrowDown must clamp at the last row, not run past it"
    );

    send_main_area_key(
        &mut state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
    );
    assert_eq!(
        state.approval_history_highlight, 0,
        "ArrowUp must move the highlight back up"
    );
}

/// Enter on the highlighted row is the keyboard equivalent of the
/// mouse control `manually_opening_a_low_risk_live_entry_bypasses_the_promotion_predicate`
/// already proves -- same real dialog, same real coordinator, reached
/// by a different input this time.
#[test]
fn enter_on_the_highlighted_live_entry_opens_the_real_dialog() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-history-enter-live");
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
    let request_id = state
        .app_shell
        .state()
        .active_project()
        .unwrap()
        .approval_requests()[0]
        .id
        .clone();

    state.app_shell.dispatch(
        tekstide_core::command::AppCommand::OpenActiveProjectSurface(
            tekstide_core::project::ProjectOpenSurface::ApprovalHistory,
        ),
    );
    send_main_area_key(
        &mut state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
    );

    match state.modal {
        Some(ModalContent::Approval(ref dialog)) => {
            assert_eq!(dialog.request.id, request_id);
        }
        ref other => panic!("Enter on a live, highlighted entry must open the dialog: {other:?}"),
    }
}

/// The keyboard path must respect "nothing left to decide" exactly the
/// way the mouse control does (no button rendered at all for a
/// non-live entry) -- Enter on a decided entry must not open anything.
#[test]
fn enter_on_a_decided_highlighted_entry_does_nothing() {
    let mut app_shell = ApplicationShell::new();
    let project_dir = fresh_project_dir("approval-history-enter-decided");
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
        "test precondition: decided and closed"
    );

    state.app_shell.dispatch(
        tekstide_core::command::AppCommand::OpenActiveProjectSurface(
            tekstide_core::project::ProjectOpenSurface::ApprovalHistory,
        ),
    );
    send_main_area_key(
        &mut state,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
    );

    assert!(
        state.modal.is_none(),
        "Enter on a decided entry must not open anything -- there is nothing left to decide"
    );
}

/// Response 234's own required fix, proven directly: a document left
/// open from an earlier `TextEditor` visit must not keep absorbing
/// keystrokes once the surface has switched to `ApprovalHistory`. `!`
/// is not one of `handle_approval_history_key`'s own keys
/// (Up/Down/Enter only) -- if `handle_editor_key`'s new guard were
/// missing, this exact key would silently reach the hidden document.
#[test]
fn switching_to_approval_history_stops_the_hidden_document_from_absorbing_keystrokes() {
    let (mut state, _dir) = state_with_an_open_document("approval-history-editor-leak", "hello");
    let before = active_document_text(&state);

    state.app_shell.dispatch(
        tekstide_core::command::AppCommand::OpenActiveProjectSurface(
            tekstide_core::project::ProjectOpenSurface::ApprovalHistory,
        ),
    );
    send_main_area_key(&mut state, iced::keyboard::Key::Character("!".into()));

    assert_eq!(
        active_document_text(&state),
        before,
        "a key aimed at the ApprovalHistory surface must not edit a document hidden behind it"
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

/// Response 233 (`ApprovalHistory`): the same escaping property
/// `approval_dialog_body_escapes_a_bidi_override_in_the_cwd` proves for
/// the dialog, proven here for the history surface's own render
/// function -- a second `ApprovalRequest.cwd` consumer, escaped
/// independently rather than assumed safe by association.
#[test]
fn approval_history_entry_body_escapes_a_bidi_override_in_the_cwd() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let request = approval_request_fixture(
        "cat notes.txt",
        "/home/user/proj\u{202E}gpj",
        tekstide_core::domain::RiskLevel::Low,
    );

    let body = super::approval_history_entry_body(&catalog, &request, false);

    assert!(
        body.contains("<U+202E>"),
        "expected the escaped marker in {body:?}"
    );
    assert!(
        !body.contains('\u{202E}'),
        "the raw override character must never reach the history surface, got {body:?}"
    );
}

/// The state distinction this surface exists for: a `Pending` decision
/// alone cannot tell a user whether an entry is still answerable
/// (RFC-022's own "expiry is a connection property, not a decision
/// outcome"). Both readings of the same `Pending` request must render
/// distinguishably.
#[test]
fn approval_history_entry_body_distinguishes_answerable_from_expired_pending() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let request = approval_request_fixture(
        "ls",
        "/home/user/project",
        tekstide_core::domain::RiskLevel::Low,
    );

    let answerable = super::approval_history_entry_body(&catalog, &request, false);
    let expired = super::approval_history_entry_body(&catalog, &request, true);

    assert_ne!(
        answerable, expired,
        "an answerable and an expired Pending request must not render identically"
    );
    assert!(
        answerable.to_lowercase().contains("awaiting"),
        "an answerable entry must say so: {answerable:?}"
    );
    assert!(
        expired.to_lowercase().contains("no longer answerable")
            || expired.to_lowercase().contains("expired"),
        "an expired entry must be visibly unanswerable, not merely fail when acted on: \
         {expired:?}"
    );
}

/// Every decided outcome must render distinguishably from every other
/// decided outcome and from both `Pending` readings -- the same
/// "distinguishable, not just present" property
/// `approval_dialog_body_renders_each_risk_level_distinguishably` proves
/// for risk levels.
#[test]
fn approval_history_entry_body_renders_every_decision_state_distinguishably() {
    let catalog = state_with(ApplicationShell::new()).catalog;
    let mut request = approval_request_fixture(
        "ls",
        "/home/user/project",
        tekstide_core::domain::RiskLevel::Low,
    );

    let mut rendered = Vec::new();
    rendered.push(super::approval_history_entry_body(
        &catalog, &request, false,
    ));
    rendered.push(super::approval_history_entry_body(&catalog, &request, true));
    for decision in [
        tekstide_core::domain::ApprovalDecision::ApprovedOnce,
        tekstide_core::domain::ApprovalDecision::Rejected,
        tekstide_core::domain::ApprovalDecision::EditedAndApproved,
    ] {
        request.decide(decision).unwrap();
        rendered.push(super::approval_history_entry_body(
            &catalog, &request, false,
        ));
        // Reset for the next iteration -- `decide` is one-shot.
        request = approval_request_fixture(
            "ls",
            "/home/user/project",
            tekstide_core::domain::RiskLevel::Low,
        );
    }

    let unique: std::collections::HashSet<_> = rendered.iter().collect();
    assert_eq!(
        unique.len(),
        rendered.len(),
        "all five decision-state readings must render distinguishably from each other: \
         {rendered:?}"
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

// RFC-038 PR-038-A: the Project Board empty state's path field.

/// Drives `text.chars()` through the **real** router
/// (`route_non_modal_input`) and the **real** `update`, one `KeyPress`
/// at a time, exactly the shape `a_real_typed_key_inserts_into_the_
/// active_document` already established for the editor -- "proven from
/// a real key event through production code," per this project's
/// standing evidence rule, not from a `Message` constructed and handed
/// to `update` directly.
fn type_through_the_real_path_field(state: &mut State, text: &str) {
    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    for character in text.chars() {
        let press = crate::input::KeyPress {
            key: iced::keyboard::Key::Character(character.to_string().into()),
            modifiers: iced::keyboard::Modifiers::empty(),
        };
        let proof = crate::input::ModalAbsent::check(&state.modal)
            .expect("test precondition: no modal open");
        let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
        let _ = super::update(state, Message::Input(routed));
    }
}

fn press_enter_in_the_real_path_field(state: &mut State) {
    press_named_key_in_the_real_path_field(state, iced::keyboard::key::Named::Enter);
}

fn press_backspace_in_the_real_path_field(state: &mut State) {
    press_named_key_in_the_real_path_field(state, iced::keyboard::key::Named::Backspace);
}

fn press_named_key_in_the_real_path_field(state: &mut State, key: iced::keyboard::key::Named) {
    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key: iced::keyboard::Key::Named(key),
        modifiers: iced::keyboard::Modifiers::empty(),
    };
    let proof =
        crate::input::ModalAbsent::check(&state.modal).expect("test precondition: no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
    let _ = super::update(state, Message::Input(routed));
}

/// **The acceptance criterion RFC-038 exists for.** A cold, fresh
/// `ApplicationShell` (no projects, no recent projects -- the exact
/// state a first-time user's `tekstide` with no arguments produces),
/// with **every character of a real path typed through the real
/// router**, then a real Enter. No dispatched `Message` constructed by
/// hand anywhere in this test.
#[test]
fn a_real_typed_path_and_enter_opens_a_project_from_a_cold_empty_board() {
    let mut state = state_with(ApplicationShell::new());
    assert!(
        state.app_shell.project_board().empty_state.is_some(),
        "test precondition: a fresh ApplicationShell has an empty board"
    );

    let project_dir = fresh_project_dir("path-field-real-open");
    type_through_the_real_path_field(&mut state, &project_dir.display().to_string());
    press_enter_in_the_real_path_field(&mut state);

    let active = state
        .app_shell
        .state()
        .active_project()
        .expect("a real typed path followed by Enter must open and activate a project");
    assert_eq!(active.root_path(), project_dir.as_path());
    assert_eq!(
        active.trust_state(),
        tekstide_core::project::WorkspaceTrust::Restricted,
        "a project added through the field must arrive Restricted, exactly like one added \
         from the CLI -- what-a-path-field-must-not-trust.md §4"
    );
    assert!(
        state.path_field.is_empty(),
        "the field must clear on a successful open"
    );
    assert!(state.path_field_notice.is_none());
}

/// The other half of §4: `Restricted` is not just a label on the row --
/// a real agent-run launch against a project added *through the field*
/// must be refused exactly the way one added from the CLI already is
/// (`agent_run_launch_shell_input_switches_to_terminal_immersion_and_shows_the_real_trust_refusal`).
/// Proves the field does not take a shortcut around RFC-032's trust
/// gate for the one property that would matter most if it did.
#[test]
fn a_project_opened_through_the_field_refuses_an_agent_run_until_trust_is_granted() {
    let mut state = state_with(ApplicationShell::new());
    let project_dir = fresh_project_dir("path-field-restricted-refusal");
    type_through_the_real_path_field(&mut state, &project_dir.display().to_string());
    press_enter_in_the_real_path_field(&mut state);
    assert!(state.app_shell.state().active_project().is_some());

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
        "a refused agent run launch must still land in Terminal Immersion, matching the CLI \
         path's own refusal shape"
    );
    assert!(
        state.agent_run_launch_notice.is_some(),
        "the launch must have been refused, not silently ignored"
    );
}

/// `what-a-path-field-must-not-trust.md` §5's own audit guard, proven
/// live: a real `ProjectAdded` record exists for a project opened
/// through the field, the same proof
/// `opening_a_real_new_project_from_the_cli_path_writes_exactly_one_real_project_added_record`
/// already gives the CLI path. **Ablated**: temporarily commenting out
/// `attempt_open_project_from_path_field`'s call to
/// `record_path_field_project_added` makes this assertion fail (0
/// records, not 1) -- confirmed by hand while writing this test, then
/// restored; not left as a standing ablation in the tree since this
/// crate's tests do not carry a runtime toggle for it, matching every
/// other single-variable ablation in this codebase's convention of
/// reverting before commit.
#[test]
fn opening_a_project_through_the_real_field_writes_exactly_one_real_project_added_record() {
    let mut state = state_with(ApplicationShell::new());
    let project_dir = fresh_project_dir("path-field-audit-record");
    type_through_the_real_path_field(&mut state, &project_dir.display().to_string());
    press_enter_in_the_real_path_field(&mut state);

    let project_id = state
        .app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("test precondition: the field must have opened a project");

    let audit_store =
        open_real_audit_store(&state.app_shell).expect("the real audit store must open");
    let records: Vec<_> = audit_store
        .query(&tekstide_core::audit::AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| record.project_id.as_ref() == Some(&project_id))
        .filter(|record| record.family == tekstide_core::audit::AuditEventFamily::ProjectAdded)
        .collect();

    assert_eq!(
        records.len(),
        1,
        "exactly one ProjectAdded record must exist for a project opened through the real \
         field: {records:?}"
    );
}

/// Re-submitting the same path a second time must focus the existing
/// session, not add a duplicate or write a second record -- the same
/// `FocusedExisting` distinction `reopening_the_same_project_path_
/// focuses_it_instead_of_writing_a_second_record` already proves for
/// the CLI path, proven here for the field's own, separate call site.
#[test]
fn resubmitting_the_same_path_through_the_field_focuses_it_without_a_second_record() {
    let mut state = state_with(ApplicationShell::new());
    let project_dir = fresh_project_dir("path-field-refocus");
    type_through_the_real_path_field(&mut state, &project_dir.display().to_string());
    press_enter_in_the_real_path_field(&mut state);
    let project_id = state
        .app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("test precondition: the first open must succeed");

    type_through_the_real_path_field(&mut state, &project_dir.display().to_string());
    press_enter_in_the_real_path_field(&mut state);

    assert_eq!(
        state.app_shell.state().active_project_id(),
        Some(&project_id),
        "resubmitting the same path must focus the same project, not create a second one"
    );

    let audit_store =
        open_real_audit_store(&state.app_shell).expect("the real audit store must open");
    let record_count = audit_store
        .query(&tekstide_core::audit::AuditQuery::latest(50))
        .expect("querying the real audit store must succeed")
        .records
        .into_iter()
        .map(|sequenced| sequenced.record)
        .filter(|record| record.project_id.as_ref() == Some(&project_id))
        .filter(|record| record.family == tekstide_core::audit::AuditEventFamily::ProjectAdded)
        .count();
    assert_eq!(
        record_count, 1,
        "re-focusing an already-open project through the field must not write a second record"
    );
}

/// `what-a-path-field-must-not-trust.md` §2: a bad path must render a
/// diagnostic and leave the application running, never exit. There is
/// no `std::process::exit` anywhere reachable from
/// `attempt_open_project_from_path_field` to assert the *absence* of
/// directly, so this proves the property behaviourally instead: the
/// notice appears, the field is preserved (not silently wiped, so the
/// user can correct it), and -- the real proof the process kept
/// running -- a **second**, valid submission through the same still-live
/// `state` afterwards succeeds normally.
#[test]
fn a_bad_path_renders_a_notice_and_the_application_keeps_running() {
    let mut state = state_with(ApplicationShell::new());
    let bad_path = fresh_project_dir("path-field-bad-parent").join("does-not-exist-at-all");
    type_through_the_real_path_field(&mut state, &bad_path.display().to_string());
    press_enter_in_the_real_path_field(&mut state);

    assert_eq!(
        state.path_field_notice,
        Some(PathFieldError::DoesNotExist),
        "a nonexistent path must be refused with the specific reason, not a generic one"
    );
    assert_eq!(
        state.path_field,
        bad_path.display().to_string(),
        "the field must keep exactly what the user typed so they can correct it, not be \
         cleared on failure"
    );
    assert!(
        state.app_shell.state().active_project().is_none(),
        "a refused path must not have added anything"
    );

    // The real proof of "kept running": the same live `state`, still
    // fully functional, opens a real project right after -- cleared with
    // real Backspace presses (not a direct assignment) so this half of
    // the test is exactly as "real key event through production code"
    // as the first half.
    for _ in bad_path.display().to_string().chars() {
        press_backspace_in_the_real_path_field(&mut state);
    }
    assert!(
        state.path_field.is_empty(),
        "test precondition: real Backspace presses must clear the field"
    );
    let project_dir = fresh_project_dir("path-field-after-bad-path");
    type_through_the_real_path_field(&mut state, &project_dir.display().to_string());
    press_enter_in_the_real_path_field(&mut state);
    assert!(
        state.app_shell.state().active_project().is_some(),
        "the application must still be able to open a real project after an earlier refusal"
    );
}

/// `what-a-path-field-must-not-trust.md` §1: the typed path is untrusted
/// even though the user typed it -- a directionality override must
/// render as a visible marker, never obeyed. Tests the pure function
/// directly (`path_field_error_text`), the same "test the rendered
/// string, not the widget tree" shape `row_lines` and `status_bar_
/// summary` already use, since this is exactly what
/// `attempt_open_project_from_path_field` calls to build the notice a
/// user actually sees.
#[test]
fn a_directionality_override_in_the_typed_path_renders_as_a_visible_marker_not_obeyed() {
    let catalog = Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()));
    let hostile_path = "/tmp/\u{202E}gpj.exe";

    let text = path_field_error_text(&catalog, hostile_path, PathFieldError::DoesNotExist);

    assert!(
        text.contains("<U+202E>"),
        "the override character must render as a visible marker: {text:?}"
    );
    assert!(
        !text.contains('\u{202E}'),
        "the real U+202E character must never reach the rendered string unescaped: {text:?}"
    );
}

/// `push_to_path_field` is the one place `state.path_field` grows, from
/// either typing or a resolved paste -- proves its own bound directly,
/// the pure-function level `MAX_PATH_FIELD_CHARS` is documented against,
/// rather than only indirectly through a real (slow) 4096-keystroke
/// sequence through the router.
#[test]
fn the_path_field_stops_growing_at_its_bound_rather_than_unbounded() {
    let mut state = state_with(ApplicationShell::new());
    let oversized = "a".repeat(MAX_PATH_FIELD_CHARS + 500);

    super::push_to_path_field(&mut state, &oversized);

    assert_eq!(
        state.path_field.chars().count(),
        MAX_PATH_FIELD_CHARS,
        "the field must stop growing exactly at the bound, not silently accept an oversized \
         paste"
    );
}

// RFC-038 PR-038-B: `Ctrl+Alt+O`, the second-project case.

fn press_ctrl_alt_o(state: &mut State) {
    let shell_input = crate::input::shell_input_for_test(
        tekstide_core::navigation::NavigationAction::OpenProjectEntryField,
    );
    let _ = super::update(
        state,
        Message::Input(crate::input::RoutedInput::Shell(shell_input)),
    );
}

/// **The acceptance criterion this slice exists for**: a user with a
/// project already open, who cannot use the empty board's own field
/// (there are rows, so `empty_state` is `None`), can still add a second
/// project -- real `Ctrl+Alt+O`, every character of a real path typed
/// through the real router, a real Enter.
///
/// **A real finding while writing this test, disclosed rather than
/// asserted around**: the second project genuinely lands on the board,
/// but does *not* become active. `AppState::add_project_session`'s own
/// `if self.active_project_id.is_none() { ... }` guard is deliberate,
/// pre-existing, already-reviewed core behaviour -- it exists so
/// `boot()`'s multi-path CLI loop does not fight itself over which of
/// several paths given at once ends up active. This test proves the
/// field inherits that behaviour rather than special-casing around it
/// (`switch_active_project` would be the way to bring the second
/// project into focus, and it has no live binding today --
/// `NavigationAction::SwitchActiveProject` is still `Configurable`/
/// `None`, a pre-existing gap `future-work.md` already names, unrelated
/// to this slice).
#[test]
fn ctrl_alt_o_opens_a_second_project_through_real_keys_on_a_populated_board() {
    let (mut state, first_project_id) = state_with_a_real_project("path-field-o-first");
    assert!(
        state.app_shell.project_board().empty_state.is_none(),
        "test precondition: one open project means the board is not empty"
    );

    press_ctrl_alt_o(&mut state);
    assert!(
        state.path_field_requested,
        "Ctrl+Alt+O must reveal the field on a populated board"
    );
    assert_eq!(
        state.app_shell.route(),
        tekstide_core::route::AppRoute::ProjectBoard
    );

    let second_dir = fresh_project_dir("path-field-o-second");
    type_through_the_real_path_field(&mut state, &second_dir.display().to_string());
    press_enter_in_the_real_path_field(&mut state);

    let rows = state.app_shell.project_board().rows;
    assert_eq!(
        rows.len(),
        2,
        "both projects must be on the board: {rows:?}"
    );
    let second_row = rows
        .iter()
        .find(|row| row.project_id != first_project_id)
        .expect("a second, distinct project must be on the board");
    assert_eq!(second_row.root_path_hint, second_dir.display().to_string());
    assert_eq!(
        state.app_shell.state().active_project_id(),
        Some(&first_project_id),
        "adding a second project must not silently switch which one is active -- matches \
         AppState::add_project_session's own pre-existing, deliberate first-project-only \
         auto-activation"
    );
    assert!(
        !state.path_field_requested,
        "the field must hide itself again once it has done its job"
    );
}

/// `Escape` backs out of the on-demand field without submitting or
/// touching the already-open project -- the field has no other dismiss
/// gesture, so this is the one way to recover from `Ctrl+Alt+O` pressed
/// by mistake.
#[test]
fn escape_dismisses_the_on_demand_field_without_submitting_or_touching_the_open_project() {
    let (mut state, project_id) = state_with_a_real_project("path-field-o-escape");
    press_ctrl_alt_o(&mut state);
    type_through_the_real_path_field(&mut state, "/tmp/whatever-was-being-typed");
    assert!(!state.path_field.is_empty());

    press_named_key_in_the_real_path_field(&mut state, iced::keyboard::key::Named::Escape);

    assert!(
        !state.path_field_requested,
        "Escape must hide the on-demand field"
    );
    assert!(
        state.path_field.is_empty(),
        "Escape must discard whatever was typed, not leave it for a later Ctrl+Alt+O to reveal"
    );
    assert_eq!(
        state.app_shell.state().active_project_id(),
        Some(&project_id),
        "Escape must not have touched the already-open project"
    );
    assert_eq!(
        state.app_shell.project_board().rows.len(),
        1,
        "Escape must not have added anything"
    );
}

/// The empty board's own field has nothing else in `MainArea` for
/// `Escape` to usefully reveal by dismissing it -- confirms this is a
/// deliberate no-op there, not an oversight that happens to look the
/// same.
#[test]
fn escape_is_a_no_op_on_the_permanently_shown_empty_board_field() {
    let mut state = state_with(ApplicationShell::new());
    type_through_the_real_path_field(&mut state, "partial");

    press_named_key_in_the_real_path_field(&mut state, iced::keyboard::key::Named::Escape);

    assert_eq!(
        state.path_field, "partial",
        "Escape must not clear the empty board's own permanent field"
    );
    assert!(
        state.app_shell.project_board().empty_state.is_some(),
        "the board must still be empty and still showing its own field"
    );
}

/// `Ctrl+Alt+O` is proven unclaimed mechanically in
/// `navigation::tests::open_project_entry_field_shortcut_is_a_candidate_that_collides_with_no_other_rule`;
/// this proves the GUI-side half -- the field's own hint names the
/// paste gesture the response accepting PR-038-A required
/// (`Ctrl+Shift+V` is what the rest of the product teaches, but does
/// nothing in this field; response 297 required naming `Ctrl+V` here
/// instead of retargeting the binding).
#[test]
fn the_path_field_hint_names_the_paste_gesture_that_actually_works_here() {
    let catalog = Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()));
    let hint = catalog.get("project-board-path-field-label");
    assert!(
        hint.contains("Ctrl+V"),
        "the field must name the paste gesture that actually works in it, not the one the \
         rest of the product teaches (Ctrl+Shift+V): {hint:?}"
    );
}
