//! RFC-017: the terminal surface. PR-017-B built the interposition
//! filter; this slice (PR-017-C) gives it its first real caller: a
//! PTY-backed pane rendering the emulator grid under RFC-015's surface
//! contract.
//!
//! **Input, RFC-017 PR-017-D.** [`TerminalPane::write_input`] is the one
//! production path that reaches `LinuxTerminalRuntime::write_input`;
//! `shell.rs`'s `update` is its only caller, gated on `state.modal` being
//! absent (re-proven live in `shell::tests`, not only headless as RFC-015
//! left it) and on `input::TextStream::to_pty_bytes` -- this module never
//! constructs a `TextStream` itself or reaches into `iced::keyboard::Key`
//! directly, matching `TextStream`'s own privacy boundary.
//!
//! **P1 (single ingress), re-proven against production code, not just a
//! test harness.** [`TerminalPane`] is the *only* place in this crate
//! that constructs a `Term`/`Processor`, and [`TerminalPane::poll`] is
//! the only place that calls `Processor::advance` — always through
//! `filter::SecurityFilter::new(&mut self.term)`, never any other path
//! to `self.term`. PR-017-B's own enumeration was honest that it covered
//! a crate with no production caller; this is that re-enumeration,
//! confirmed by the same `grep` in `qa-evidence.md`.
//!
//! **P2 (no side channels).** `Term::grid_mut()` is not called anywhere
//! in this module (confirmed by inspection, `qa-evidence.md`); the only
//! non-byte input this pane's own `Term` receives is its fixed
//! construction-time dimensions, not a live resize -- split/resize
//! handling is PR-017-E's job.
//!
//! **Bounded scrollback.** `PANE_CONFIG`'s `scrolling_history` is set
//! explicitly to [`SCROLLBACK_LINES`], not left at `alacritty_terminal`'s
//! own default (10,000) -- an unbounded (or merely large-by-default)
//! buffer is a memory-exhaustion path driven entirely by untrusted PTY
//! output. Tested under sustained output in `terminal::tests`.
//!
//! **The grid renders as data, never as chrome** (RFC-015/RFC-018): the
//! pane exposes only [`view`], taking `&TerminalPane` and `&Theme` --
//! nothing here can reach `shell::State`'s modal or chrome fields, the
//! same "cannot construct/cannot reach" shape `surface::board` already
//! established. There is no session title, pane header, or tooltip in
//! this slice (PR-017-E's `session_bar.rs`) for the RFC-016 chrome
//! exception to apply to yet.

pub mod filter;

use std::path::PathBuf;
use std::time::Duration;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Color as CellColor, NamedColor, Processor, StdSyncHandler};

use iced::widget::text::Span;
use iced::widget::{column, rich_text};
use iced::{Color, Element};

use tekstide_core::project::{ProjectId, ProjectSession};
use tekstide_core::runtime::terminal::{
    LinuxTerminalRuntime, TerminalDimensions, TerminalLaunchError, TerminalLaunchSpec,
    TerminalRuntimeHandle,
};

use filter::SecurityFilter;

/// Fixed for this slice -- real font-metrics-driven sizing is PR-017-E's
/// job (`implementation-handoff.md` §6). 80x24 matches the spike's own
/// choice and every existing `TerminalPanePolicy` default.
const ROWS: usize = 24;
const COLS: usize = 80;

/// Chosen well below `alacritty_terminal`'s own 10,000-line default:
/// bounded specifically so sustained adversarial output cannot grow the
/// pane's memory use without limit, while still keeping enough history
/// to be useful. Tested under sustained output, not merely asserted.
const SCROLLBACK_LINES: usize = 2_000;

#[derive(Clone, Copy)]
struct PaneSize;

impl Dimensions for PaneSize {
    fn total_lines(&self) -> usize {
        ROWS
    }

    fn screen_lines(&self) -> usize {
        ROWS
    }

    fn columns(&self) -> usize {
        COLS
    }
}

fn pane_config() -> Config {
    Config {
        scrolling_history: SCROLLBACK_LINES,
        ..Config::default()
    }
}

/// A PTY-backed terminal pane: the emulator grid, filtered, rendered.
/// Every field here is either this pane's own rendering state (`term`,
/// `processor`) or a handle back to `tekstide-core`'s runtime
/// (`runtime`, `handle`) -- nothing duplicates `tekstide-core`'s own
/// session/project state (RFC-015's "no shell-local shadow copy" rule).
pub struct TerminalPane {
    runtime: LinuxTerminalRuntime,
    handle: TerminalRuntimeHandle,
    processor: Processor<StdSyncHandler>,
    term: Term<VoidListener>,
}

impl TerminalPane {
    /// Launches a plain shell (`TerminalLaunchSpec::plain_shell`, never
    /// an AI CLI profile -- this pane has no input path for a launch
    /// contract to authorize against yet) in `root`, already validated
    /// to exist, and prepares the filtered emulator side. `root` is the
    /// project root the caller resolved; this function does no path
    /// resolution of its own.
    pub fn launch(
        project_id: ProjectId,
        title: impl Into<String>,
        root: PathBuf,
        shell: PathBuf,
    ) -> Result<Self, TerminalLaunchError> {
        let project = ProjectSession::new(project_id.clone(), "terminal-pane", &root, &root);

        let mut spec = TerminalLaunchSpec::plain_shell(project_id, title, &root, shell);
        spec.dimensions = TerminalDimensions {
            rows: ROWS as u16,
            cols: COLS as u16,
        };

        let mut runtime = LinuxTerminalRuntime::new();
        let (session, _events) = runtime.launch_project_shell(&project, spec)?;
        let handle = TerminalRuntimeHandle::new(session.id.clone(), project.id().clone());

        Ok(Self {
            runtime,
            handle,
            processor: Processor::new(),
            term: Term::new(pane_config(), &PaneSize, VoidListener),
        })
    }

    /// Reads whatever PTY output is currently available (bounded, short
    /// poll -- called from a GUI tick subscription, so it must not block
    /// the render loop) and advances the filtered emulator. **The only
    /// place in this crate `Processor::advance` is called, and the only
    /// place `self.term` is mutably borrowed outside construction** --
    /// P1's re-enumeration for this slice.
    pub fn poll(&mut self) {
        let Ok((bytes, _event)) = self.runtime.read_available_bounded_for(
            &self.handle,
            Duration::from_millis(5),
            64 * 1024,
        ) else {
            return;
        };
        if bytes.is_empty() {
            return;
        }

        let mut filter = SecurityFilter::new(&mut self.term);
        self.processor.advance(&mut filter, &bytes);
    }

    /// This pane's real, live `TerminalId` -- what a caller compares a
    /// `TextStream`'s target against before calling [`Self::write_input`].
    pub fn terminal_id(&self) -> &tekstide_core::domain::TerminalId {
        &self.handle.terminal_id
    }

    /// Writes bytes to this pane's PTY. The caller (`shell::update`) is
    /// responsible for the two things this method does not itself
    /// check: that `bytes` came from `TextStream::to_pty_bytes` (never a
    /// raw `iced::keyboard::Key`), and that no modal is currently open --
    /// this method has no access to `shell::State` to verify either.
    pub fn write_input(&mut self, bytes: &[u8]) {
        let _ = self.runtime.write_input(&self.handle, bytes);
    }

    /// Plain-text rendering, for `shell::tests`'s live-input assertions
    /// (RFC-017 PR-017-D) -- `pub(crate)` rather than a bare
    /// `#[cfg(test)]` fn, since it must be reachable from `shell`'s own
    /// test module, a different module tree than this one's.
    #[cfg(test)]
    pub(crate) fn rendered_text(&self) -> String {
        styled_rows(&self.term)
            .into_iter()
            .flat_map(|runs| runs.into_iter().map(|(text, _)| text))
            .collect()
    }
}

/// Groups each visible row's cells into runs of consecutive identical
/// resolved foreground colour, using the same `renderable_content()`/
/// `Colors` API a real renderer uses -- ported from the RFC-014 spike's
/// `styled_rows`, unchanged in shape.
fn styled_rows(term: &Term<VoidListener>) -> Vec<Vec<(String, [f32; 3])>> {
    let content = term.renderable_content();
    let colors = content.colors;

    let mut rows: Vec<Vec<(char, [f32; 3])>> = vec![Vec::new(); ROWS];
    for indexed in content.display_iter {
        let point = indexed.point;
        if point.line.0 < 0 || point.line.0 as usize >= ROWS {
            continue;
        }
        let rgb = resolve_color(indexed.cell.fg, colors);
        rows[point.line.0 as usize].push((indexed.cell.c, rgb));
    }

    rows.into_iter()
        .map(|cells| {
            let mut runs: Vec<(String, [f32; 3])> = Vec::new();
            for (c, rgb) in cells {
                match runs.last_mut() {
                    Some((text, last_rgb)) if *last_rgb == rgb => text.push(c),
                    _ => runs.push((c.to_string(), rgb)),
                }
            }
            runs
        })
        .collect()
}

/// The pane's rendered content only -- takes no `&shell::State`, no
/// theme role beyond the monospace font size, and cannot reach
/// `state.modal` or chrome, the same shape `surface::board::view` uses.
/// Untrusted PTY bytes render as data (RFC-016's grid exception): no
/// `text_safety::quote_untrusted` call here, deliberately -- escaping
/// would corrupt the grid the way it must not for `surface::board`'s
/// trusted-chrome fields.
pub fn view<'a, Message: 'a>(pane: &TerminalPane, font_size: f32) -> Element<'a, Message> {
    let rows = styled_rows(&pane.term);
    column(
        rows.into_iter()
            .map(|runs| {
                let spans: Vec<Span<'static, ()>> = runs
                    .into_iter()
                    .map(|(text, rgb)| {
                        Span::new(text).color(Color::from_rgb(rgb[0], rgb[1], rgb[2]))
                    })
                    .collect();
                rich_text(spans)
                    .size(font_size)
                    .font(iced::Font::MONOSPACE)
                    .into()
            })
            .collect::<Vec<Element<'a, Message>>>(),
    )
    .into()
}

/// Standard ANSI 16-colour fallback palette, used when `Term`'s `Colors`
/// table has no override for a given named colour -- true for every
/// colour here, since nothing seeds a palette in this slice (theme
/// integration for terminal colours is unscoped, same simplification
/// the spike recorded). Indexed colours beyond 0-15 fall back to the
/// default foreground; full 256-colour resolution is out of scope here,
/// recorded as a known limitation rather than silently implied complete.
fn resolve_color(color: CellColor, colors: &alacritty_terminal::term::color::Colors) -> [f32; 3] {
    const DEFAULT_FOREGROUND: [f32; 3] = [0.85, 0.85, 0.85];

    let named_fallback = |named: NamedColor| -> [f32; 3] {
        match named {
            NamedColor::Black => [0.0, 0.0, 0.0],
            NamedColor::Red => [0.80, 0.0, 0.0],
            NamedColor::Green => [0.0, 0.75, 0.0],
            NamedColor::Yellow => [0.80, 0.80, 0.0],
            NamedColor::Blue => [0.30, 0.55, 1.0],
            NamedColor::Magenta => [0.75, 0.0, 0.75],
            NamedColor::Cyan => [0.0, 0.75, 0.75],
            NamedColor::White => [0.85, 0.85, 0.85],
            NamedColor::BrightBlack => [0.4, 0.4, 0.4],
            NamedColor::BrightRed => [1.0, 0.3, 0.3],
            NamedColor::BrightGreen => [0.3, 1.0, 0.3],
            NamedColor::BrightYellow => [1.0, 1.0, 0.3],
            NamedColor::BrightBlue => [0.5, 0.7, 1.0],
            NamedColor::BrightMagenta => [1.0, 0.4, 1.0],
            NamedColor::BrightCyan => [0.4, 1.0, 1.0],
            NamedColor::BrightWhite => [1.0, 1.0, 1.0],
            NamedColor::Foreground => DEFAULT_FOREGROUND,
            _ => DEFAULT_FOREGROUND,
        }
    };

    match color {
        CellColor::Spec(rgb) => [
            f32::from(rgb.r) / 255.0,
            f32::from(rgb.g) / 255.0,
            f32::from(rgb.b) / 255.0,
        ],
        CellColor::Named(named) => colors[named]
            .map(|rgb| {
                [
                    f32::from(rgb.r) / 255.0,
                    f32::from(rgb.g) / 255.0,
                    f32::from(rgb.b) / 255.0,
                ]
            })
            .unwrap_or_else(|| named_fallback(named)),
        CellColor::Indexed(index) if index < 16 => named_fallback(named_color_from_index(index)),
        CellColor::Indexed(_) => DEFAULT_FOREGROUND,
    }
}

fn named_color_from_index(index: u8) -> NamedColor {
    match index {
        0 => NamedColor::Black,
        1 => NamedColor::Red,
        2 => NamedColor::Green,
        3 => NamedColor::Yellow,
        4 => NamedColor::Blue,
        5 => NamedColor::Magenta,
        6 => NamedColor::Cyan,
        7 => NamedColor::White,
        8 => NamedColor::BrightBlack,
        9 => NamedColor::BrightRed,
        10 => NamedColor::BrightGreen,
        11 => NamedColor::BrightYellow,
        12 => NamedColor::BrightBlue,
        13 => NamedColor::BrightMagenta,
        14 => NamedColor::BrightCyan,
        _ => NamedColor::BrightWhite,
    }
}

#[cfg(test)]
mod tests;
