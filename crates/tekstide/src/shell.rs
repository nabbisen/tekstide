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

use iced::widget::{center, column, container, opaque, stack, text};
use iced::{Background, Border, Element, Length, Subscription, Task, keyboard};

use tekstide_core::command::AppCommand;
use tekstide_core::navigation::{KeybindingPolicy, NavigationAction};
use tekstide_core::route::AppRoute;
use tekstide_core::shell::ApplicationShell;

use crate::i18n::{Catalog, CatalogArgs};
use crate::input::{self, FocusZone, RoutedInput, TextStream};
use crate::theme::Theme;

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

pub struct State {
    app_shell: ApplicationShell,
    catalog: Catalog,
    theme: Theme,
    focus: FocusZone,
    modal: Option<ModalContent>,
}

impl State {
    pub fn new(app_shell: ApplicationShell, catalog: Catalog) -> Self {
        Self {
            app_shell,
            catalog,
            theme: Theme::default(),
            focus: FocusZone::MainArea,
            modal: std::env::var("TEKSTIDE_LAYER_DEMO")
                .is_ok()
                .then(ModalContent::default),
        }
    }

    pub fn window_title(&self) -> String {
        self.catalog.get("app-title")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Input(RoutedInput),
    ModalFocusNext,
    ModalFocusPrevious,
    ModalActivate,
    ModalDismiss,
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
    }
    Task::none()
}

/// Only `OpenProjectBoard` maps to an existing `AppCommand` today.
/// `OpenCommandPalette` has a real, reserved binding
/// (`KeybindingPolicy::linux_mvp()`) but no command palette feature
/// exists yet to dispatch to; every other `NavigationAction` has no
/// default binding at all until RFC-023 supplies one. Not a placeholder
/// -- an honest reflection of what is real right now.
fn app_command_for(action: NavigationAction) -> Option<AppCommand> {
    match action {
        NavigationAction::OpenProjectBoard => Some(AppCommand::OpenProjectBoard),
        NavigationAction::OpenCommandPalette
        | NavigationAction::SwitchActiveProject
        | NavigationAction::ToggleProjectMode
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
    match input::SubscriptionMode::for_modal(&state.modal) {
        input::SubscriptionMode::NonModal(proof) => {
            non_modal_subscription(proof, state.focus).map(Message::Input)
        }
        input::SubscriptionMode::Modal => modal_subscription(),
    }
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
    let content: Element<'_, Message> = match state.app_shell.route() {
        AppRoute::ProjectBoard => crate::surface::board::view(
            &state.app_shell.project_board(),
            &state.catalog,
            &state.theme,
        ),
        AppRoute::ActiveProjectWorkspace => no_surface_placeholder(state),
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

/// `AppRoute::ActiveProjectWorkspace` has no real surface yet -- the
/// editor/explorer/terminal surfaces RFC-019/RFC-017 add. Kept as its
/// own function (not a shared default `content_area` fallback) so the
/// day a real workspace surface lands, this becomes the one line that
/// changes.
fn no_surface_placeholder(state: &State) -> Element<'_, Message> {
    container(
        column![
            text(state.catalog.get("content-area-placeholder-title"))
                .size(state.theme.font_size_body()),
            text(state.catalog.get("content-area-placeholder-body"))
                .size(state.theme.font_size_body()),
        ]
        .spacing(6),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(16)
    .into()
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
