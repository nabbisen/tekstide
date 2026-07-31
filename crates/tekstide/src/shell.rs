//! RFC-015 PR-015-B: window, layer composition, chrome, and the theme
//! and i18n seams. **No surfaces yet** -- PR-015-D adds the Project
//! Board; PR-015-C adds real input routing (`ShellInput`/`SurfaceInput`/
//! `TextStream`). This slice is deliberately non-interactive: it renders
//! a static chrome over real `ApplicationShell` state and has no message
//! that changes anything, which is why `Message` below is uninhabited.
//!
//! **Layer composition** follows RFC-015's layer model:
//!
//! | Layer | Contents | Trust |
//! | --- | --- | --- |
//! | Chrome | top bar, status bar | Trusted |
//! | Content | placeholder (no surface yet) | untrusted content will land here from PR-015-D |
//! | Modal | layer-composition demo only in this slice | Trusted, exclusive |
//!
//! Composed via `stack`/`opaque`, the mechanism the RFC-014 spike proved
//! (C8). Real dialogs are RFC-022's job; this slice's modal occupant is
//! a placeholder that exists solely to prove the layer renders above
//! content rather than inside it -- exactly the exception
//! `implementation-handoff.md` §8 allows ("a placeholder dialog for
//! testing the layer is fine and should be clearly marked as such").
//!
//! **The demo modal is env-gated, not keyboard-gated.** `TEKSTIDE_LAYER_DEMO`
//! is read once at boot, the same convention the RFC-014 spike used for
//! its own demo/measurement flags (`TEKSTIDE_MEASURE_CRITERION`,
//! `TEKSTIDE_I18N_DEMO`). Deliberately not a keyboard toggle: PR-015-C
//! is the slice that introduces any input at all, and giving this slice
//! its own ad hoc key handler would be exactly the kind of pre-empting
//! of PR-015-C's job that `pr-015-c-input-routing.md` warns against.
//!
//! **No shell-local state mirrors core state** (`implementation-handoff.md`
//! §2). [`State`] holds exactly one [`ApplicationShell`] -- the sole
//! source of model state -- plus purely presentational fields
//! (`catalog`, `theme`, `layer_composition_demo_modal_open`), none of
//! which duplicate a value already inside it.

use iced::widget::{center, column, container, opaque, stack, text};
use iced::{Background, Border, Element, Length, Task};

use tekstide_core::route::AppRoute;
use tekstide_core::shell::ApplicationShell;

use crate::i18n::{Catalog, CatalogArgs};
use crate::theme::Theme;

pub struct State {
    app_shell: ApplicationShell,
    catalog: Catalog,
    theme: Theme,
    /// Layer-composition demo scaffolding only -- see the module doc.
    /// Set once at boot from `TEKSTIDE_LAYER_DEMO`; nothing in this
    /// slice can change it at runtime, since there is no input yet.
    layer_composition_demo_modal_open: bool,
}

impl State {
    pub fn new(app_shell: ApplicationShell, catalog: Catalog) -> Self {
        Self {
            app_shell,
            catalog,
            theme: Theme::default(),
            layer_composition_demo_modal_open: std::env::var("TEKSTIDE_LAYER_DEMO").is_ok(),
        }
    }

    pub fn window_title(&self) -> String {
        self.catalog.get("app-title")
    }
}

/// Uninhabited: this slice has no interactivity. `update` therefore can
/// never actually be called -- `match message {}` typechecks because
/// `Message` has no variants to match, not because a case was left
/// unhandled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {}

pub fn update(_state: &mut State, message: Message) -> Task<Message> {
    match message {}
}

pub fn view(state: &State) -> Element<'_, Message> {
    let base: Element<'_, Message> =
        column![top_bar(state), content_area(state), status_bar(state)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

    if state.layer_composition_demo_modal_open {
        stack![base, opaque(center(layer_composition_demo_modal(state)))].into()
    } else {
        base
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
pub(crate) fn status_bar_summary(state: &State) -> String {
    let project_count = state.app_shell.state().projects().len();
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
    .style(move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(state.theme.background())),
        text_color: Some(state.theme.foreground()),
        ..container::Style::default()
    })
    .into()
}

fn layer_composition_demo_modal(state: &State) -> Element<'_, Message> {
    container(
        column![
            text(state.catalog.get("layer-demo-modal-title")).size(state.theme.font_size_heading()),
            text(state.catalog.get("layer-demo-modal-body")).size(state.theme.font_size_body()),
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
