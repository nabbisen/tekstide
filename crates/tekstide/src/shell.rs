//! RFC-015 PR-015-B/PR-015-C: window, layer composition, chrome, the
//! theme/i18n seams (PR-015-B), and real input routing (PR-015-C).
//! **No surfaces yet** -- PR-015-D adds the Project Board. Input
//! *classification* lives in [`crate::input`]; this module is the one
//! place that turns a classified [`input::RoutedInput`] into an actual
//! state change, via [`update`].
//!
//! **Layer composition** follows RFC-015's layer model:
//!
//! | Layer | Contents | Trust |
//! | --- | --- | --- |
//! | Chrome | top bar, status bar | Trusted |
//! | Content | placeholder (no surface yet) | untrusted content will land here from PR-015-D |
//! | Modal | layer-composition demo | Trusted, exclusive |
//!
//! Composed via `stack`/`opaque`, the mechanism the RFC-014 spike proved
//! (C8). Real dialogs are RFC-022's job; this slice's modal occupant is
//! still the PR-015-B placeholder (`implementation-handoff.md` §8's
//! explicit allowance), but PR-015-C makes it genuinely dismissible via
//! real input, since a placeholder that never closes cannot exercise
//! the focus-trap and modal-exclusivity properties this slice must
//! prove. Opening it remains env-gated (`TEKSTIDE_LAYER_DEMO`, read once
//! at boot) -- there is still no real trigger to open a dialog
//! (RFC-022), only a real way to close one now that input exists.
//!
//! **Modal exclusivity is structural**, via [`input::ModalAbsent`]: see
//! [`subscription`] and `input`'s module doc. While `state.modal` is
//! `Some`, the *only* subscription active is [`modal_subscription`],
//! which has no path to producing `input::SurfaceInput` or
//! `input::TextStream` at all -- not "produced and ignored."
//!
//! **No shell-local state mirrors core state** (`implementation-handoff.md`
//! §2). [`State`] holds exactly one [`ApplicationShell`] -- the sole
//! source of model state -- plus purely presentational fields
//! (`catalog`, `theme`, `focus`, `modal`), none of which duplicate a
//! value already inside it.

use iced::widget::{center, column, container, opaque, row, stack, text};
use iced::{Background, Border, Element, Length, Subscription, Task, keyboard};

use tekstide_core::command::AppCommand;
use tekstide_core::navigation::{KeybindingPolicy, NavigationAction};
use tekstide_core::project::ProjectMode;
use tekstide_core::route::AppRoute;
use tekstide_core::shell::ApplicationShell;

use crate::i18n::{Catalog, CatalogArgs};
use crate::input::{self, FocusZone, RoutedInput, TextStream};
use crate::measurement::{self, Measurement};
use crate::theme::Theme;

/// RFC-015 PR-015-F: the synthetic typing-measurement surface's preloaded
/// content -- a real ~1,500-line source file, not a lorem-ipsum
/// placeholder, so the layout cost the measurement exercises is the same
/// shape a real editor would see. `tekstide-gui-spike` (`publish = false`)
/// used the RFC-014 spike's own precedent of `include_str!`-ing this file
/// directly out of `tekstide-core`; `tekstide` is a published crate, and a
/// package tarball can never contain a sibling crate's source, so this is
/// a static, committed snapshot living inside this crate instead --
/// discovered by `cargo package`'s verification step during 0.4.0 RC prep,
/// not by inspection. Only loaded into `State.typing_doc` when actually
/// measuring `Typing` (see `State::new`); otherwise never referenced.
const TYPING_MEASUREMENT_DOCUMENT: &str = include_str!("../typing-measurement-sample.rs");

/// The two focusable targets of the layer-composition demo modal --
/// still scaffolding (see the module doc), but now real enough for a
/// genuine focus-trap test: while `state.modal` is `Some`, Tab/Shift+Tab
/// must cycle only between these two, never `state.focus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalButton {
    Acknowledge,
    Dismiss,
}

impl ModalButton {
    const ORDER: [ModalButton; 2] = [ModalButton::Acknowledge, ModalButton::Dismiss];

    fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|button| *button == self)
            .unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

pub(crate) struct ModalContent {
    focus: ModalButton,
}

impl Default for ModalContent {
    fn default() -> Self {
        // Defaulting to the less destructive-sounding target, the same
        // reasoning the RFC-014 spike's `DialogButton::Deny` default
        // used -- this modal has no real consequence either way, but the
        // convention is cheap to keep consistent.
        Self {
            focus: ModalButton::Dismiss,
        }
    }
}

/// Response 134 Required: measurement and the demo modal must be mutually
/// exclusive, or `subscription()`'s measurement branch (checked first,
/// ahead of `SubscriptionMode::for_modal` entirely -- see `subscription`'s
/// doc) would skip modal exclusivity while the modal is still on screen,
/// silently reopening the "produced-then-ignored" gap PR-015-C closed.
/// Measurement wins: it is a bounded, self-terminating diagnostic run
/// PR-015-C's structural property has no reason to apply to in the first
/// place, whereas the demo modal exists only to be screenshotted
/// interactively. A pure function, not inlined into `State::new`, so the
/// exclusivity itself is testable without racing on process-global
/// `TEKSTIDE_LAYER_DEMO`/`TEKSTIDE_MEASURE_CRITERION` env vars against
/// concurrently-running tests that also construct a `State`.
fn modal_for_state(measurement_active: bool, layer_demo_requested: bool) -> Option<ModalContent> {
    if measurement_active {
        None
    } else {
        layer_demo_requested.then(ModalContent::default)
    }
}

pub struct State {
    app_shell: ApplicationShell,
    catalog: Catalog,
    theme: Theme,
    focus: FocusZone,
    modal: Option<ModalContent>,
    /// RFC-015 PR-015-F: `None` unless `TEKSTIDE_MEASURE_CRITERION` names
    /// a recognized criterion -- see `measurement`'s module doc.
    measurement: Option<Measurement>,
    /// RFC-015 PR-015-F: the typing-measurement surface's live content.
    /// Empty unless actually measuring `Typing`.
    typing_doc: String,
}

impl State {
    pub fn new(app_shell: ApplicationShell, catalog: Catalog) -> Self {
        let measurement = Measurement::from_env();
        let typing_doc = if matches!(
            measurement.as_ref().map(Measurement::criterion),
            Some(measurement::Criterion::Typing)
        ) {
            TYPING_MEASUREMENT_DOCUMENT.to_string()
        } else {
            String::new()
        };

        let modal = modal_for_state(
            measurement.is_some(),
            std::env::var("TEKSTIDE_LAYER_DEMO").is_ok(),
        );

        Self {
            app_shell,
            catalog,
            theme: Theme::default(),
            focus: FocusZone::MainArea,
            modal,
            measurement,
            typing_doc,
        }
    }

    pub fn window_title(&self) -> String {
        self.catalog.get("app-title")
    }

    /// Whether `content_area` should substitute the synthetic
    /// typing-measurement view for the real content (RFC-015 PR-015-F).
    /// `false` for `ModeSwitch`: C4 measures the *real* content's view
    /// cost, so nothing is substituted for it.
    pub fn is_measuring_typing(&self) -> bool {
        matches!(
            self.measurement.as_ref().map(Measurement::criterion),
            Some(measurement::Criterion::Typing)
        )
    }

    /// Whether `main.rs`'s view wrapper should time this render as a
    /// view-build-cost sample (RFC-015 PR-015-F/PR-015-E). `Startup`
    /// times its one frame via `frames()` instead and needs no view-cost
    /// log; with no measurement active (the default), this is always
    /// `false`. Broader than [`Self::is_measuring_typing`]: `ModeSwitch`
    /// also needs view-cost timing, but over the real content, not a
    /// substituted one.
    pub fn is_measuring_view_cost(&self) -> bool {
        matches!(
            self.measurement.as_ref().map(Measurement::criterion),
            Some(measurement::Criterion::Typing) | Some(measurement::Criterion::ModeSwitch)
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Input(RoutedInput),
    ModalFocusNext,
    ModalFocusPrevious,
    ModalActivate,
    ModalDismiss,
    /// RFC-015 PR-015-F: a synthetic measurement keystroke arrived; the
    /// `Instant` is when the measurement subscription first saw it, not
    /// when `update` gets around to handling it -- the gap between the
    /// two is exactly the input-to-state-change sample.
    MeasuredKey(std::time::Instant),
    /// RFC-015 PR-015-F: periodic check for whether the measurement run
    /// has reached its sample target and should self-exit.
    MeasurementTick,
    /// RFC-015 PR-015-F: a frame was painted during `Startup` measurement.
    MeasurementFrame(std::time::Instant),
    /// RFC-015 PR-015-E: a synthetic C4 measurement keystroke arrived;
    /// same timing convention as `MeasuredKey`, but its handler dispatches
    /// the real `AppCommand::ToggleActiveProjectMode` instead of
    /// appending to a synthetic document.
    MeasuredModeSwitch(std::time::Instant),
}

pub fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Input(RoutedInput::Shell(shell_input)) => {
            if let Some(command) = app_command_for(shell_input.action()) {
                state.app_shell.dispatch(command);
            }
        }
        Message::Input(RoutedInput::Surface(_surface_input)) => {
            // No surface exists yet to receive this (PR-015-D). The
            // routing that produced it is proven correct in
            // `input::tests`; there is nothing to consume here yet.
        }
        Message::Input(RoutedInput::Terminal(text_stream)) => {
            // No PTY-writing path exists yet (RFC-017) -- only the
            // liveness check this slice owns is exercised here. RFC-017
            // will call the same function before actually writing.
            let _ = terminal_stream_targets_a_live_terminal(&state.app_shell, &text_stream);
        }
        Message::Input(RoutedInput::FocusNext) => state.focus = state.focus.next(),
        Message::Input(RoutedInput::FocusPrevious) => state.focus = state.focus.previous(),
        Message::ModalFocusNext => {
            if let Some(modal) = state.modal.as_mut() {
                modal.focus = modal.focus.next();
            }
        }
        Message::ModalFocusPrevious => {
            if let Some(modal) = state.modal.as_mut() {
                modal.focus = modal.focus.previous();
            }
        }
        // Both dismiss. Real distinct outcomes (e.g. an actual
        // accept/reject decision) belong to RFC-022's real dialogs; this
        // placeholder has no decision to record.
        Message::ModalActivate | Message::ModalDismiss => {
            state.modal = None;
        }
        Message::MeasuredKey(sent_at) => {
            if let Some(measurement) = state.measurement.as_mut() {
                measurement.record_input(sent_at);
            }
            state.typing_doc.push('x');
        }
        Message::MeasurementTick => {
            if state.measurement.as_ref().is_some_and(Measurement::is_done) {
                std::process::exit(0);
            }
        }
        Message::MeasurementFrame(at) => {
            if let Some(measurement) = state.measurement.as_mut() {
                measurement.record_startup_frame(at);
                if measurement.is_done() {
                    std::process::exit(0);
                }
            }
        }
        Message::MeasuredModeSwitch(sent_at) => {
            if let Some(measurement) = state.measurement.as_mut() {
                measurement.record_input(sent_at);
            }
            // The real command a `Ctrl+Alt+M` press would dispatch --
            // measuring the actual state mutation and the view rebuild
            // it causes, not a synthetic stand-in. Requires a real
            // active project (see `pr-015-e-mode-switching.md`'s C4
            // section); a measurement run without one records real,
            // if uninformative, ~0-cost samples rather than panicking.
            state
                .app_shell
                .dispatch(tekstide_core::command::AppCommand::ToggleActiveProjectMode);
        }
    }
    Task::none()
}

/// `OpenProjectBoard` and, since PR-015-E, `ToggleProjectMode` map to
/// existing `AppCommand`s. `OpenCommandPalette` has a real, reserved
/// binding (`KeybindingPolicy::linux_mvp()`) but no command palette
/// feature exists yet to dispatch to; every other `NavigationAction` has
/// no default binding at all until RFC-023 supplies one. Not a
/// placeholder -- an honest reflection of what is real right now.
fn app_command_for(action: NavigationAction) -> Option<AppCommand> {
    match action {
        NavigationAction::OpenProjectBoard => Some(AppCommand::OpenProjectBoard),
        NavigationAction::ToggleProjectMode => Some(AppCommand::ToggleActiveProjectMode),
        NavigationAction::OpenCommandPalette
        | NavigationAction::SwitchActiveProject
        | NavigationAction::CycleVisibleTerminalSession
        | NavigationAction::OpenCurrentAgentRunDetail
        | NavigationAction::OpenPendingApproval
        | NavigationAction::OpenDiffReview
        | NavigationAction::OpenSafeCloseDialog => None,
    }
}

/// The check `pr-015-c-input-routing.md` requires before a `TextStream`
/// may be delivered: "a stale or cross-project id is dropped, not
/// best-effort delivered." No PTY-writing code exists yet for this to
/// gate in practice (RFC-017); proven directly against `ApplicationShell`
/// fixtures in `shell::tests` so the property is real the moment RFC-017
/// calls it, not discovered wrong then.
pub(crate) fn terminal_stream_targets_a_live_terminal(
    app_shell: &ApplicationShell,
    stream: &TextStream,
) -> bool {
    app_shell
        .state()
        .active_project()
        .and_then(|project| project.terminal_session(stream.target()))
        .is_some()
}

pub fn view(state: &State) -> Element<'_, Message> {
    let base: Element<'_, Message> =
        column![top_bar(state), content_area(state), status_bar(state)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

    if let Some(modal) = &state.modal {
        stack![
            base,
            opaque(center(layer_composition_demo_modal(state, modal)))
        ]
        .into()
    } else {
        base
    }
}

/// Structural proof this module cannot bypass modal exclusivity for the
/// *call*: `route_non_modal_input` needs an `input::ModalAbsent`,
/// obtainable only by checking `state.modal` itself -- there is no other
/// way to reach [`non_modal_subscription`]. See `input`'s module doc for
/// why deleting this `match` is a compile error, not a behaviour change.
///
/// **What that alone does not prove** (response 130 Required 1): actual
/// exclusivity -- that `SurfaceInput`/`TextStream` are never produced
/// while a modal is shown -- also depends on `iced` tearing down the
/// non-modal subscription (and the `ModalAbsent` it captured, which is
/// `Copy` and therefore can outlive the instant it was checked) the
/// moment this function starts returning [`input::SubscriptionMode::Modal`]
/// instead. That is a real dependency on `iced`'s subscription-rebuild
/// lifecycle, not a second type-level guarantee -- named here rather
/// than left implicit, and `input::SubscriptionMode::for_modal` is
/// tested directly (`shell::tests`) so at least the branch this function
/// picks is asserted, even though the framework half is not.
pub fn subscription(state: &State) -> Subscription<Message> {
    // Checked first, ahead of modal/non-modal routing entirely: a
    // measurement run is a special, bounded, self-terminating mode (see
    // `measurement`'s module doc), never part of ordinary interactive
    // use -- `state.measurement` is `None` unless the env var is set, so
    // this branch changes nothing about PR-015-C's reviewed structure
    // for any normal run.
    if let Some(measurement) = &state.measurement {
        return measurement_subscription(measurement.criterion());
    }

    match input::SubscriptionMode::for_modal(&state.modal) {
        input::SubscriptionMode::NonModal(proof) => {
            non_modal_subscription(proof, state.focus).map(Message::Input)
        }
        input::SubscriptionMode::Modal => modal_subscription(),
    }
}

/// `Startup` reuses the RFC-014 spike's exact mechanism (subscribe
/// `frames()`, record the first one, exit) -- safe here specifically
/// because the process exits immediately after, so there is no
/// *sustained* redraw-forcing during any real interactive session for
/// it to contaminate. `Typing` never subscribes to `frames()` at all --
/// see `measurement`'s module doc for why -- and instead pairs a
/// measurement-only keyboard listener with a periodic tick used solely
/// to detect "target sample count reached, time to self-exit."
fn measurement_subscription(criterion: measurement::Criterion) -> Subscription<Message> {
    match criterion {
        measurement::Criterion::Startup => iced::window::frames().map(Message::MeasurementFrame),
        measurement::Criterion::Typing => {
            measured_key_subscription(Message::MeasuredKey as fn(std::time::Instant) -> Message)
        }
        // RFC-015 PR-015-E: identical shape to `Typing`'s arm -- the same
        // measurement key, the same self-terminating tick -- differing
        // only in which `Message` variant (and therefore which `update`
        // handler) the keystroke produces. See `measured_key_subscription`.
        measurement::Criterion::ModeSwitch => measured_key_subscription(
            Message::MeasuredModeSwitch as fn(std::time::Instant) -> Message,
        ),
    }
}

/// Shared by `Typing` and `ModeSwitch`: a measurement-only keyboard
/// listener for [`measurement::MEASURED_KEY_CHARACTER`], paired with the
/// periodic self-exit tick. Parameterized on which `Message` variant to
/// produce so the two criteria's `update` handlers stay distinct (one
/// appends to a synthetic document, the other dispatches a real
/// `AppCommand`) without duplicating this subscription's shape twice.
fn measured_key_subscription(
    to_message: fn(std::time::Instant) -> Message,
) -> Subscription<Message> {
    Subscription::batch([
        keyboard::listen()
            .with(to_message)
            .filter_map(|(to_message, event)| match event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(ref character),
                    ..
                } if character.as_str() == measurement::MEASURED_KEY_CHARACTER => {
                    Some(to_message(std::time::Instant::now()))
                }
                _ => None,
            }),
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::MeasurementTick),
    ])
}

fn non_modal_subscription(
    proof: input::ModalAbsent,
    focus: FocusZone,
) -> Subscription<RoutedInput> {
    // No terminal surface exists yet (RFC-017), so nothing can ever set
    // this to `Some` today -- the parameter exists so `route_non_modal_input`
    // does not need to change shape when RFC-017 lands, the same reason
    // `LocalePreference`'s fields exist ahead of their real callers.
    let terminal_focus: Option<tekstide_core::domain::TerminalId> = None;
    // `.filter_map`'s closure must be non-capturing (`iced` panics
    // otherwise: "cannot capture external variables"). `.with(...)`
    // threads `proof`/`focus`/`terminal_focus` in through the closure's
    // own parameter instead of a capture, which is why `ModalAbsent` and
    // `FocusZone` both derive `Hash` -- `.with` requires it to detect
    // whether the subscription's identity changed across rebuilds.
    keyboard::listen()
        .with((proof, focus, terminal_focus))
        .filter_map(|((proof, focus, terminal_focus), event)| {
            let press = key_press_from_event(event)?;
            let policy = KeybindingPolicy::linux_mvp();
            Some(input::route_non_modal_input(
                proof,
                &policy,
                focus,
                terminal_focus.as_ref(),
                press,
            ))
        })
}

fn modal_subscription() -> Subscription<Message> {
    keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            modifiers,
            ..
        } if modifiers.shift() => Some(Message::ModalFocusPrevious),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            ..
        } => Some(Message::ModalFocusNext),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Enter),
            ..
        } => Some(Message::ModalActivate),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        } => Some(Message::ModalDismiss),
        _ => None,
    })
}

fn key_press_from_event(event: keyboard::Event) -> Option<input::KeyPress> {
    match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } => {
            Some(input::KeyPress { key, modifiers })
        }
        _ => None,
    }
}

/// Owned, `Copy` colour values rather than a borrowed `&Theme`, so this
/// helper's return type needs no lifetime capture at all -- simpler than
/// reasoning about RPIT capture rules for a borrow that would otherwise
/// need to outlive the returned closure.
fn chrome_style(
    background: iced::Color,
    foreground: iced::Color,
    border: iced::Color,
) -> impl Fn(&iced::Theme) -> container::Style {
    move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(background)),
        text_color: Some(foreground),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn top_bar(state: &State) -> Element<'_, Message> {
    container(text(state.window_title()).size(state.theme.font_size_heading()))
        .width(Length::Fill)
        .padding(8)
        .style(chrome_style(
            state.theme.surface_elevated(),
            state.theme.foreground(),
            state.theme.border_default(),
        ))
        .into()
}

/// The route symbol `status_bar_summary` selects on -- a compile-time
/// literal per `AppRoute` variant, not runtime-derived text, so it is
/// exactly what `CatalogArgs::trusted_symbol` is for.
fn route_symbol(route: AppRoute) -> &'static str {
    match route {
        AppRoute::ProjectBoard => "project-board",
        AppRoute::ActiveProjectWorkspace => "active-project-workspace",
    }
}

/// The status bar's text, factored out from [`status_bar`] so it is
/// directly testable without going through `iced`'s `Element` tree.
/// Response 132 Required: this count must agree with the number of rows
/// the Project Board actually renders, or the first thing a user sees
/// is chrome disagreeing with the surface directly below it. Counting
/// `state.app_shell.state().projects().len()` (open sessions only) was
/// correct in PR-015-B, when no board existed to disagree with it --
/// PR-015-D's board deliberately also lists recent-but-not-open
/// projects (RFC-005's model), so the two collections are different
/// sizes in general. Using `project_board().rows.len()` here is the
/// same computation `surface::board::view` renders from, not a second,
/// independently-arrived-at count that could drift again.
pub(crate) fn status_bar_summary(state: &State) -> String {
    let project_count = state.app_shell.project_board().rows.len();
    state.catalog.get_with_args(
        "status-bar-summary",
        &CatalogArgs::new()
            .trusted_symbol("route", route_symbol(state.app_shell.route()))
            .number("count", project_count as u32),
    )
}

fn status_bar(state: &State) -> Element<'_, Message> {
    container(text(status_bar_summary(state)).size(state.theme.font_size_status()))
        .width(Length::Fill)
        .padding(6)
        .style(chrome_style(
            state.theme.surface_elevated(),
            state.theme.foreground(),
            state.theme.border_default(),
        ))
        .into()
}

fn content_area(state: &State) -> Element<'_, Message> {
    let content: Element<'_, Message> = if state.is_measuring_typing() {
        typing_measurement_view(state)
    } else {
        match state.app_shell.route() {
            AppRoute::ProjectBoard => crate::surface::board::view(
                &state.app_shell.project_board(),
                &state.catalog,
                &state.theme,
            ),
            AppRoute::ActiveProjectWorkspace => active_project_workspace_view(state),
        }
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_base_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(state.theme.background())),
            text_color: Some(state.theme.foreground()),
            ..container::Style::default()
        })
        .into()
}

/// RFC-015 PR-015-E: the sidebar/main-area scaffolding RFC-017 (terminal),
/// RFC-019 (editor/explorer), and RFC-020 (diff/review) plug real content
/// into. Both zones are catalog-driven placeholders today -- `surface.rs`'s
/// contract stays concrete methods, not a `trait Surface`, because a
/// second implementor still gives nothing to generalise from (unchanged
/// from PR-015-D's reasoning; this slice adds a second *zone*, not a
/// second surface implementation).
fn active_project_workspace_view(state: &State) -> Element<'_, Message> {
    let mode = state
        .app_shell
        .state()
        .active_project()
        .map(tekstide_core::project::ProjectSession::mode);

    row![sidebar_view(state), main_area_view(state, mode)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Style for a focusable content-area zone (sidebar, main area):
/// `NFR-UX-002` forbids a colour-only status, so `focused` changes two
/// channels at once -- border colour (`Theme::border_focused`) and
/// border width -- never colour alone. Callers additionally prefix their
/// own text with the same `"> "`/`"  "` marker `shell.rs`'s modal
/// buttons already use, a third, purely textual channel.
fn zone_style(theme: Theme, focused: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(theme.background())),
        text_color: Some(theme.foreground()),
        border: Border {
            color: if focused {
                theme.border_focused()
            } else {
                theme.border_default()
            },
            width: if focused { 2.0 } else { 1.0 },
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Focus marker matching the modal's own convention
/// (`layer_composition_demo_modal`'s `button_line`) -- a textual channel
/// independent of colour or border width, so the indicator survives even
/// if a future theme makes the two border widths visually similar.
fn focus_marker(focused: bool) -> &'static str {
    if focused { "> " } else { "  " }
}

/// Factored out from [`sidebar_view`] so the focus-marker/catalog-text
/// combination is directly testable, the same shape as
/// `status_bar_summary`/`surface::board::row_lines`.
pub(crate) fn sidebar_label(state: &State) -> String {
    let focused = state.focus == FocusZone::Sidebar;
    format!(
        "{}{}",
        focus_marker(focused),
        state.catalog.get("sidebar-placeholder-title")
    )
}

/// The catalog key [`main_area_label`]/[`main_area_view`] select on for a
/// given `mode` -- `None` (no active project) should not be reachable
/// while routed to `ActiveProjectWorkspace` (core guards every
/// transition into this route on an active project existing), but the
/// fallback is Content Mode's placeholder rather than a panic, matching
/// this crate's "fail visible" convention.
fn main_area_key(mode: Option<ProjectMode>) -> &'static str {
    match mode {
        None | Some(ProjectMode::Content) => "main-area-content-mode-placeholder",
        Some(ProjectMode::TerminalImmersion) => "main-area-terminal-mode-placeholder",
    }
}

/// Factored out from [`main_area_view`] for the same reason as
/// [`sidebar_label`].
pub(crate) fn main_area_label(state: &State, mode: Option<ProjectMode>) -> String {
    let focused = state.focus == FocusZone::MainArea;
    format!(
        "{}{}",
        focus_marker(focused),
        state.catalog.get(main_area_key(mode))
    )
}

fn sidebar_view(state: &State) -> Element<'_, Message> {
    let focused = state.focus == FocusZone::Sidebar;
    container(text(sidebar_label(state)))
        .width(Length::Fixed(220.0))
        .height(Length::Fill)
        .padding(16)
        .style(zone_style(state.theme, focused))
        .into()
}

fn main_area_view(state: &State, mode: Option<ProjectMode>) -> Element<'_, Message> {
    let focused = state.focus == FocusZone::MainArea;
    container(
        column![text(main_area_label(state, mode)).size(state.theme.font_size_body()),].spacing(6),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(16)
    .style(zone_style(state.theme, focused))
    .into()
}

/// RFC-015 PR-015-F: renders the tail of `state.typing_doc` in a
/// monospace font, matching the RFC-014 spike's own typing-measurement
/// view -- deliberately not a real editor (RFC-019's job), only enough
/// rendering cost to make view-build-cost measurement meaningful against
/// a realistically-sized document. Only ever reached when
/// `state.is_measuring_typing()` is true, i.e. never during normal use.
fn typing_measurement_view(state: &State) -> Element<'_, Message> {
    let visible = tail_lines(&state.typing_doc, 50);
    container(
        column(
            visible
                .lines()
                .map(|line| {
                    text(line.to_string())
                        .size(state.theme.font_size_body())
                        .font(iced::Font::MONOSPACE)
                        .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(2),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(16)
    .into()
}

pub(crate) fn tail_lines(doc: &str, count: usize) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].join("\n")
}

fn layer_composition_demo_modal<'a>(
    state: &'a State,
    modal: &ModalContent,
) -> Element<'a, Message> {
    let button_line = |target: ModalButton, label_key: &str| {
        let marker = if modal.focus == target { "> " } else { "  " };
        text(format!("{marker}{}", state.catalog.get(label_key))).size(state.theme.font_size_body())
    };

    container(
        column![
            text(state.catalog.get("layer-demo-modal-title")).size(state.theme.font_size_heading()),
            text(state.catalog.get("layer-demo-modal-body")).size(state.theme.font_size_body()),
            button_line(ModalButton::Acknowledge, "layer-demo-modal-acknowledge"),
            button_line(ModalButton::Dismiss, "layer-demo-modal-dismiss"),
            text(state.catalog.get("layer-demo-modal-dismiss-hint"))
                .size(state.theme.font_size_status()),
        ]
        .spacing(10),
    )
    .padding(20)
    .style(move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(state.theme.surface_elevated())),
        text_color: Some(state.theme.foreground()),
        border: Border {
            color: state.theme.accent(),
            width: 2.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests;
