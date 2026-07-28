//! PR-014-B: static Content Mode shell with a keyboard-only focus model.
//!
//! Layout proportions follow `tekstide-uiux-wireframes-v0.md` §7.2 and
//! external design §4.4: sidebar ~20% width, remaining width for the main
//! content area, one-line top bar and status bar.
//!
//! This module renders no real project data. It is measurement/rendering
//! scaffolding only.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use iced::widget::text::Span;
use iced::widget::{center, column, container, opaque, rich_text, row, stack, text};
use iced::{Background, Border, Color, Element, Length, Subscription, Task, Theme, keyboard};

use crate::terminal_pane::TerminalPane;

const SIDEBAR_WIDTH_FRACTION: f32 = 0.20;

/// PR-014-E: a large real source file, used verbatim as the "editor
/// surface with a large document loaded" that C2 (typing latency) asks
/// for. Not a fabricated lorem-ipsum blob -- a real ~1,500-line file from
/// this workspace, so the layout cost the measurement exercises is the
/// same shape a user's editor would actually see.
const TYPING_MEASUREMENT_DOCUMENT: &str =
    include_str!("../../tekstide-core/src/project/session.rs");

/// First line of `main()`, captured once, before anything else runs.
/// This is the only honest definition of "process start" available from
/// inside the process itself -- it excludes exec()/dynamic-linker/runtime
/// init time, which is disclosed in qa-evidence.md rather than implied away.
static PROCESS_START: OnceLock<Instant> = OnceLock::new();

pub fn mark_process_start() {
    PROCESS_START.get_or_init(Instant::now);
}

fn process_start() -> Instant {
    *PROCESS_START.get_or_init(Instant::now)
}

/// PR-014-E measurement criteria. `None` (the default, no env var set)
/// preserves the exact PR-014-B/C/D interactive behaviour already
/// reviewed -- this instrumentation is additive and off by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash)]
enum MeasureCriterion {
    /// C2: typing latency, large document loaded, keystrokes appended.
    Typing,
    /// C3: terminal input latency under a background output flood.
    TerminalFlood,
    /// C4: Content <-> Terminal toggle latency.
    ModeSwitch,
    /// C5: process start to first frame painted.
    Startup,
}

/// Receipt-confirming latency sampler: every measured input pushes an
/// `Instant` here, and every `Message::Frame` (from `iced::window::frames()`,
/// which fires once per real `RedrawRequested`) drains it, writing one log
/// line per drained sample. The external harness compares lines-written to
/// keys-sent and aborts on mismatch rather than computing a percentile over
/// a partial sample -- see qa-evidence.md PR-014-E for why (response 107 Q3).
struct Measurement {
    criterion: MeasureCriterion,
    log: std::fs::File,
    pending: VecDeque<Instant>,
    received: u32,
    target: u32,
    startup_recorded: bool,
}

impl Measurement {
    fn from_env() -> Option<Self> {
        let criterion = match std::env::var("TEKSTIDE_MEASURE_CRITERION").ok()?.as_str() {
            "typing" => MeasureCriterion::Typing,
            "terminal-flood" => MeasureCriterion::TerminalFlood,
            "mode-switch" => MeasureCriterion::ModeSwitch,
            "startup" => MeasureCriterion::Startup,
            _ => return None,
        };
        let log_path = std::env::var("TEKSTIDE_MEASURE_LOG").ok()?;
        let target: u32 = std::env::var("TEKSTIDE_MEASURE_TARGET")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1100);
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .ok()?;

        Some(Self {
            criterion,
            log,
            pending: VecDeque::new(),
            received: 0,
            target,
            startup_recorded: false,
        })
    }

    fn on_input(&mut self) {
        self.pending.push_back(Instant::now());
        self.received += 1;
    }

    fn on_frame(&mut self, at: Instant) {
        if self.criterion == MeasureCriterion::Startup {
            if !self.startup_recorded {
                self.startup_recorded = true;
                let elapsed = at.saturating_duration_since(process_start());
                let _ = writeln!(self.log, "{}", elapsed.as_micros());
            }
            return;
        }

        while let Some(input_at) = self.pending.pop_front() {
            let elapsed = at.saturating_duration_since(input_at);
            let _ = writeln!(self.log, "{}", elapsed.as_micros());
        }
    }

    fn is_done(&self) -> bool {
        match self.criterion {
            MeasureCriterion::Startup => self.startup_recorded,
            _ => self.received >= self.target && self.pending.is_empty(),
        }
    }
}

fn tail_lines(doc: &str, count: usize) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].join("\n")
}

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

/// PR-014-D: the two focusable elements of the genuine trusted dialog.
/// Kept entirely separate from `FocusZone` -- while the dialog is shown,
/// Tab/Shift+Tab must cycle *only* between these two, never the shell
/// zones behind it and never anything the terminal pane could influence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButton {
    Approve,
    Deny,
}

impl DialogButton {
    const ORDER: [DialogButton; 2] = [DialogButton::Approve, DialogButton::Deny];

    fn next(self) -> Self {
        let index = Self::ORDER.iter().position(|b| *b == self).unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ORDER.iter().position(|b| *b == self).unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }

    fn label(self) -> &'static str {
        match self {
            DialogButton::Approve => "Approve",
            DialogButton::Deny => "Deny",
        }
    }
}

pub struct State {
    focus: FocusZone,
    terminal_mode: bool,
    pane: Option<TerminalPane>,
    pane_launch_error: Option<String>,
    dialog_shown: bool,
    dialog_focus: DialogButton,
    dialog_decision: Option<DialogButton>,
    measure: Option<Measurement>,
    typing_doc: String,
    /// PR-014-E (C10): when set (via `TEKSTIDE_I18N_DEMO`), the editor
    /// surface shows a CJK + RTL sample instead of the static PR-014-B
    /// placeholder, and the terminal pane is pre-launched with the same
    /// sample printed into it, for one-frame-per-surface screenshots.
    i18n_demo: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            focus: FocusZone::Sidebar,
            terminal_mode: false,
            pane: None,
            pane_launch_error: None,
            dialog_shown: false,
            dialog_focus: DialogButton::Deny,
            dialog_decision: None,
            measure: None,
            typing_doc: String::new(),
            i18n_demo: false,
        }
    }
}

/// PR-014-E (C10): CJK (Simplified Chinese, Japanese) and one RTL script
/// (Arabic), each on its own line, mixed with ASCII so a reader can see
/// alignment/shaping behaviour against plain text in the same block.
const I18N_SAMPLE: &str = "\
中文: 你好，世界。这是一个非 Latin 脚本的例子。
日本語: こんにちは、世界。これは非ラテン文字の例です。
العربية: مرحبا بالعالم. هذا مثال على نص من اليمين إلى اليسار.
plain ascii control line for comparison";

/// Boot function for PR-014-E measurement runs. With no
/// `TEKSTIDE_MEASURE_CRITERION` env var set this is behaviourally
/// identical to `State::default()` -- the exact shell already reviewed in
/// PR-014-B/C/D. Each criterion pre-launches whatever it needs (a pane, a
/// preloaded document) so the measured loop never has to pay one-time
/// launch cost.
fn boot() -> State {
    let mut state = State {
        measure: Measurement::from_env(),
        ..State::default()
    };

    match state
        .measure
        .as_ref()
        .map(|measurement| measurement.criterion)
    {
        Some(MeasureCriterion::Typing) => {
            state.typing_doc = TYPING_MEASUREMENT_DOCUMENT.to_string();
        }
        Some(MeasureCriterion::TerminalFlood) => {
            state.terminal_mode = true;
            match TerminalPane::launch() {
                Ok(mut pane) => {
                    pane.send_flood_script_once();
                    state.pane = Some(pane);
                }
                Err(error) => state.pane_launch_error = Some(error.message),
            }
        }
        Some(MeasureCriterion::ModeSwitch) => match TerminalPane::launch() {
            Ok(pane) => state.pane = Some(pane),
            Err(error) => state.pane_launch_error = Some(error.message),
        },
        Some(MeasureCriterion::Startup) | None => {}
    }

    state.i18n_demo = std::env::var("TEKSTIDE_I18N_DEMO").is_ok();
    if state.i18n_demo {
        state.typing_doc = I18N_SAMPLE.to_string();
        if state.pane.is_none() {
            match TerminalPane::launch() {
                Ok(mut pane) => {
                    pane.send_i18n_demo_once(I18N_SAMPLE);
                    state.pane = Some(pane);
                }
                Err(error) => state.pane_launch_error = Some(error.message),
            }
        }
    }

    state
}

#[derive(Debug, Clone)]
pub enum Message {
    FocusNext,
    FocusPrevious,
    ToggleTerminalMode,
    Tick,
    DialogActivate,
    /// PR-014-E: a measured input for whichever criterion is active
    /// (typed character for Typing/TerminalFlood).
    MeasuredKey,
    /// PR-014-E: fires once per real `RedrawRequested`, timestamped by
    /// iced itself. This is the "frame submitted for presentation" side
    /// of the app-internal latency definition.
    Frame(Instant),
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::FocusNext if state.terminal_mode && state.dialog_shown => {
            state.dialog_focus = state.dialog_focus.next();
        }
        Message::FocusPrevious if state.terminal_mode && state.dialog_shown => {
            state.dialog_focus = state.dialog_focus.previous();
        }
        Message::FocusNext => state.focus = state.focus.next(),
        Message::FocusPrevious => state.focus = state.focus.previous(),
        Message::ToggleTerminalMode => {
            if let Some(measurement) = state.measure.as_mut()
                && measurement.criterion == MeasureCriterion::ModeSwitch
            {
                measurement.on_input();
            }
            state.terminal_mode = !state.terminal_mode;
            if state.terminal_mode && state.pane.is_none() && state.pane_launch_error.is_none() {
                match TerminalPane::launch() {
                    Ok(mut pane) => {
                        pane.send_demo_script_once();
                        pane.send_adversarial_dialog_script();
                        state.pane = Some(pane);
                        state.dialog_shown = true;
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
        Message::DialogActivate if state.dialog_shown => {
            state.dialog_decision = Some(state.dialog_focus);
            state.dialog_shown = false;
        }
        Message::DialogActivate => {}
        Message::MeasuredKey => {
            let criterion = state
                .measure
                .as_ref()
                .map(|measurement| measurement.criterion);
            if let Some(measurement) = state.measure.as_mut() {
                measurement.on_input();
            }
            match criterion {
                Some(MeasureCriterion::Typing) => state.typing_doc.push('x'),
                Some(MeasureCriterion::TerminalFlood) => {
                    if let Some(pane) = state.pane.as_mut() {
                        pane.send_input(b"x");
                    }
                }
                _ => {}
            }
        }
        Message::Frame(at) => {
            if let Some(measurement) = state.measure.as_mut() {
                measurement.on_frame(at);
                if measurement.is_done() {
                    let _ = std::io::stdout().flush();
                    std::process::exit(0);
                }
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

    let decision_label = match state.dialog_decision {
        Some(DialogButton::Approve) => "Approve",
        Some(DialogButton::Deny) => "Deny",
        None => "none yet",
    };

    let status_bar = container(
        text(format!(
            "RFC-009 filter blocked {blocked_count} calls this session | \
             last real-dialog decision: {decision_label} | \
             demo script sends OSC 52/title/hyperlink -- none should visibly take effect"
        ))
        .size(13),
    )
    .width(Length::Fill)
    .padding(6);

    let base: Element<'_, Message> = column![
        top_bar,
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8),
        status_bar,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    if state.dialog_shown {
        stack![base, opaque(center(trusted_dialog_view(state)))].into()
    } else {
        base
    }
}

/// PR-014-D: the genuine Tekstide modal dialog, rendered via `iced::widget::stack`
/// -- a real GUI layer entirely outside the terminal surface, not characters
/// drawn into the terminal grid the way the adversarial imitation is. This is
/// the C8 evidence RFC-009 deferred: "screenshot-backed spoofing evidence."
fn trusted_dialog_view(state: &State) -> Element<'_, Message> {
    let button = |target: DialogButton| {
        let is_focused = state.dialog_focus == target;
        let marker = if is_focused { "> " } else { "  " };
        container(text(format!("{marker}[ {} ]", target.label())).size(14))
            .padding(6)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(if is_focused {
                    Color::from_rgb(0.20, 0.35, 0.60)
                } else {
                    Color::from_rgb(0.15, 0.15, 0.15)
                })),
                border: focus_border(is_focused),
                ..container::Style::default()
            })
    };

    container(
        column![
            text("Command Approval Required").size(16),
            text("Project: rfc-014-spike").size(13),
            text("Command: rm -rf /").size(13),
            row![button(DialogButton::Approve), button(DialogButton::Deny)].spacing(16),
            text("Tab/Shift+Tab moves focus between Approve/Deny; Enter activates.").size(11),
        ]
        .spacing(10),
    )
    .padding(20)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgb(0.05, 0.05, 0.08))),
        border: Border {
            color: Color::from_rgb(0.9, 0.7, 0.1),
            width: 3.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    })
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

    let is_typing_measurement = matches!(
        state
            .measure
            .as_ref()
            .map(|measurement| measurement.criterion),
        Some(MeasureCriterion::Typing)
    ) || state.i18n_demo;

    let main_content: Element<'_, Message> = if is_typing_measurement {
        let visible = tail_lines(&state.typing_doc, 50);
        column(
            visible
                .lines()
                .map(|line| {
                    text(line.to_string())
                        .size(13)
                        .font(iced::Font::MONOSPACE)
                        .into()
                })
                .collect::<Vec<Element<'_, Message>>>(),
        )
        .spacing(2)
        .into()
    } else {
        column![
            text("1 | pub mod array;").size(13),
            text("2 | pub mod error;").size(13),
            text("3 |").size(13),
            text("4 | // static Content Mode shell (PR-014-B)").size(13),
        ]
        .spacing(4)
        .into()
    };

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
    let measure_criterion = state
        .measure
        .as_ref()
        .map(|measurement| measurement.criterion);

    let keys = keyboard::listen()
        .with(measure_criterion)
        .filter_map(|(criterion, event)| match event {
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
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Enter),
                ..
            } => Some(Message::DialogActivate),
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Character(ref c),
                ..
            } if c.as_str() == "j" => match criterion {
                Some(MeasureCriterion::Typing) | Some(MeasureCriterion::TerminalFlood) => {
                    Some(Message::MeasuredKey)
                }
                Some(MeasureCriterion::ModeSwitch) => Some(Message::ToggleTerminalMode),
                _ => None,
            },
            _ => None,
        });

    // `window::frames()` forces continuous compositor-driven redraw for as
    // long as it is subscribed (confirmed empirically: 0 CPU ticks/3s idle
    // without it vs ~8 ticks/3s with it -- see qa-evidence.md PR-014-E).
    // It is therefore only ever included during a measurement run, so it
    // never contaminates the PR-014-B/C/D interactive behaviour already
    // reviewed, or the C6 idle-RSS baseline (which runs with no
    // `TEKSTIDE_MEASURE_CRITERION` set at all).
    let frames: Subscription<Message> = if state.measure.is_some() {
        iced::window::frames().map(Message::Frame)
    } else {
        Subscription::none()
    };

    if state.terminal_mode || state.pane.is_some() {
        Subscription::batch([
            keys,
            frames,
            iced::time::every(Duration::from_millis(50)).map(|_| Message::Tick),
        ])
    } else {
        Subscription::batch([keys, frames])
    }
}

pub fn run() -> iced::Result {
    iced::application(boot, update, view)
        .title("tekstide-gui-spike (RFC-014)")
        .subscription(subscription)
        .run()
}
