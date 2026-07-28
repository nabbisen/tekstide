//! PR-014-B: static Content Mode shell with a keyboard-only focus model.
//!
//! Layout proportions follow `tekstide-uiux-wireframes-v0.md` §7.2 and
//! external design §4.4: sidebar ~20% width, remaining width for the main
//! content area, one-line top bar and status bar.
//!
//! This module renders no real project data. It is measurement/rendering
//! scaffolding only.

use std::time::Duration;

use iced::widget::text::Span;
use iced::widget::{column, container, rich_text, row, text};
use iced::{Background, Border, Color, Element, Length, Subscription, Task, Theme, keyboard};

use crate::terminal_pane::TerminalPane;

const SIDEBAR_WIDTH_FRACTION: f32 = 0.20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusZone {
    Sidebar,
    MainArea,
}

impl FocusZone {
    const ORDER: [FocusZone; 2] = [FocusZone::Sidebar, FocusZone::MainArea];

    fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|zone| *zone == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|zone| *zone == self)
            .unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }

    fn label(self) -> &'static str {
        match self {
            FocusZone::Sidebar => "Explorer",
            FocusZone::MainArea => "Content",
        }
    }
}

pub struct State {
    focus: FocusZone,
    terminal_mode: bool,
    pane: Option<TerminalPane>,
    pane_launch_error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            focus: FocusZone::Sidebar,
            terminal_mode: false,
            pane: None,
            pane_launch_error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    FocusNext,
    FocusPrevious,
    ToggleTerminalMode,
    Tick,
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::FocusNext => state.focus = state.focus.next(),
        Message::FocusPrevious => state.focus = state.focus.previous(),
        Message::ToggleTerminalMode => {
            state.terminal_mode = !state.terminal_mode;
            if state.terminal_mode && state.pane.is_none() && state.pane_launch_error.is_none() {
                match TerminalPane::launch() {
                    Ok(mut pane) => {
                        pane.send_demo_script_once();
                        state.pane = Some(pane);
                    }
                    Err(error) => state.pane_launch_error = Some(error.message),
                }
            }
        }
        Message::Tick => {
            if let Some(pane) = state.pane.as_mut() {
                pane.poll();
            }
        }
    }
    Task::none()
}

fn focus_border(is_focused: bool) -> Border {
    if is_focused {
        Border {
            color: Color::from_rgb(0.30, 0.60, 1.0),
            width: 2.0,
            radius: 0.0.into(),
        }
    } else {
        Border {
            color: Color::from_rgb(0.35, 0.35, 0.35),
            width: 1.0,
            radius: 0.0.into(),
        }
    }
}

fn zone_container<'a>(
    zone: FocusZone,
    focused: FocusZone,
    width: Length,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let is_focused = zone == focused;
    let focus_marker = if is_focused { "[focused] " } else { "" };

    container(column![
        text(format!("{focus_marker}{}", zone.label())).size(14),
        content,
    ])
    .width(width)
    .height(Length::Fill)
    .padding(8)
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.12, 0.12, 0.12))),
        border: focus_border(is_focused),
        ..container::Style::default()
    })
    .into()
}

fn terminal_pane_view(state: &State) -> Element<'_, Message> {
    let top_bar = container(
        text("tekstide-gui-spike | RFC-014 PR-014-C terminal pane | Esc/F2: Content").size(14),
    )
    .width(Length::Fill)
    .padding(6);

    let body: Element<'_, Message> = if let Some(pane) = state.pane.as_ref() {
        let rows = pane.styled_rows();
        let mut lines: Vec<Element<'_, Message>> = Vec::with_capacity(rows.len());
        for row_spans in &rows {
            if row_spans.is_empty() {
                lines.push(text(" ").size(13).into());
                continue;
            }
            let spans: Vec<Span<'_>> = row_spans
                .iter()
                .map(|(run, rgb)| {
                    Span::new(run.clone()).color(Color::from_rgb(rgb[0], rgb[1], rgb[2]))
                })
                .collect();
            lines.push(rich_text(spans).size(13).font(iced::Font::MONOSPACE).into());
        }
        column(lines).into()
    } else if let Some(error) = state.pane_launch_error.as_ref() {
        text(format!("terminal pane launch failed: {error}"))
            .size(13)
            .into()
    } else {
        text("launching...").size(13).into()
    };

    let blocked_count = state
        .pane
        .as_ref()
        .map(|pane| pane.blocked_log.len())
        .unwrap_or(0);

    let status_bar = container(
        text(format!(
            "RFC-009 filter blocked {blocked_count} calls this session | \
             demo script sends OSC 52/title/hyperlink -- none should visibly take effect"
        ))
        .size(13),
    )
    .width(Length::Fill)
    .padding(6);

    column![
        top_bar,
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8),
        status_bar,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view(state: &State) -> Element<'_, Message> {
    if state.terminal_mode {
        return terminal_pane_view(state);
    }

    let top_bar =
        container(text("tekstide-gui-spike | Project: (none) | Trust: Restricted").size(14))
            .width(Length::Fill)
            .padding(6);

    let sidebar_content: Element<'_, Message> = column![
        text("src/").size(13),
        text("  lib.rs").size(13),
        text("  main.rs").size(13),
        text("tests/").size(13),
    ]
    .spacing(4)
    .into();

    let main_content: Element<'_, Message> = column![
        text("1 | pub mod array;").size(13),
        text("2 | pub mod error;").size(13),
        text("3 |").size(13),
        text("4 | // static Content Mode shell (PR-014-B)").size(13),
    ]
    .spacing(4)
    .into();

    let sidebar_width = Length::FillPortion((SIDEBAR_WIDTH_FRACTION * 100.0) as u16);
    let main_width = Length::FillPortion(((1.0 - SIDEBAR_WIDTH_FRACTION) * 100.0) as u16);

    let sidebar = zone_container(
        FocusZone::Sidebar,
        state.focus,
        sidebar_width,
        sidebar_content,
    );
    let main_area = zone_container(FocusZone::MainArea, state.focus, main_width, main_content);

    let body = row![sidebar, main_area]
        .width(Length::Fill)
        .height(Length::Fill);

    let status_bar = container(
        text(format!(
            "main* | Ln 4, Col 1 | Rust | Focus: {} | Tab/Shift+Tab to move focus",
            state.focus.label()
        ))
        .size(13),
    )
    .width(Length::Fill)
    .padding(6);

    column![top_bar, body, status_bar]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn subscription(state: &State) -> Subscription<Message> {
    let keys = keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            modifiers,
            ..
        } if modifiers.shift() => Some(Message::FocusPrevious),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            ..
        } => Some(Message::FocusNext),
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::F2),
            ..
        } => Some(Message::ToggleTerminalMode),
        _ => None,
    });

    if state.terminal_mode {
        Subscription::batch([
            keys,
            iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick),
        ])
    } else {
        keys
    }
}

pub fn run() -> iced::Result {
    iced::application(State::default, update, view)
        .title("tekstide-gui-spike (RFC-014)")
        .subscription(subscription)
        .run()
}
