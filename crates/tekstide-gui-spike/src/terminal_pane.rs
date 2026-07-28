//! PR-014-C: a PTY-backed terminal pane driven by
//! `tekstide_core::runtime::terminal::LinuxTerminalRuntime`, rendered
//! through `alacritty_terminal` with the RFC-009 policy interposed via
//! [`crate::filter::SecurityFilter`].
//!
//! This is the only path PTY bytes take in this spike: every byte read
//! from the PTY goes through `Processor::advance(&mut SecurityFilter::new(
//! &mut self.term), bytes)` and nothing else in this module holds a
//! `&mut Term` outside that call. See `filter.rs`'s module doc for why that
//! matters for P1/P2.
//!
//! Uses a temp directory as the project root and cwd, per the handoff:
//! this spike must not touch any real Tekstide state.

use std::path::PathBuf;
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Color as CellColor, NamedColor, Processor, StdSyncHandler};

use tekstide_core::project::{ProjectId, ProjectSession};
use tekstide_core::runtime::terminal::{
    LinuxTerminalRuntime, TerminalDimensions, TerminalLaunchSpec, TerminalRuntimeHandle,
};

use crate::filter::{BlockedCall, SecurityFilter};

const ROWS: usize = 24;
const COLS: usize = 80;

#[derive(Clone, Copy)]
struct PaneSize;

impl alacritty_terminal::grid::Dimensions for PaneSize {
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

/// Discards alacritty event-loop-facing events (title/clipboard/etc. --
/// this spike does not act on them; blocking is proven by the filter's own
/// `blocked` log, not by what this listener does with events that do
/// arrive from *accepted* operations).
struct NullListener;

impl EventListener for NullListener {
    fn send_event(&self, _event: Event) {}
}

pub struct TerminalPane {
    runtime: LinuxTerminalRuntime,
    handle: TerminalRuntimeHandle,
    processor: Processor<StdSyncHandler>,
    term: Term<NullListener>,
    pub blocked_log: Vec<BlockedCall>,
    demo_sent: bool,
}

#[derive(Debug)]
pub struct TerminalPaneError {
    pub message: String,
}

impl TerminalPaneError {
    fn temp_dir_unavailable(error: std::io::Error) -> Self {
        Self {
            message: format!("temp directory unavailable for spike terminal pane: {error}"),
        }
    }

    fn launch(error: impl std::fmt::Debug) -> Self {
        Self {
            message: format!("terminal pane launch failed: {error:?}"),
        }
    }
}

impl TerminalPane {
    /// Launches `/bin/sh` in a fresh temp directory (never a real project
    /// root) and prepares the filtered emulator side.
    pub fn launch() -> Result<Self, TerminalPaneError> {
        let temp_root =
            std::env::temp_dir().join(format!("tekstide-gui-spike-pr014c-{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).map_err(TerminalPaneError::temp_dir_unavailable)?;
        let canonical_root = temp_root
            .canonicalize()
            .map_err(TerminalPaneError::temp_dir_unavailable)?;

        let project_id = ProjectId::new_uuid();
        let project = ProjectSession::new(
            project_id.clone(),
            "rfc-014-spike-scratch",
            canonical_root.clone(),
            canonical_root.clone(),
        );

        let mut spec = TerminalLaunchSpec::plain_shell(
            project_id,
            "RFC-014 spike terminal",
            canonical_root,
            PathBuf::from("/bin/sh"),
        );
        spec.dimensions = TerminalDimensions {
            rows: ROWS as u16,
            cols: COLS as u16,
        };

        let mut runtime = LinuxTerminalRuntime::new();
        let (session, _events) = runtime
            .launch_project_shell(&project, spec)
            .map_err(TerminalPaneError::launch)?;

        let handle = TerminalRuntimeHandle::new(session.id.clone(), project.id().clone());

        Ok(Self {
            runtime,
            handle,
            processor: Processor::new(),
            term: Term::new(Config::default(), &PaneSize, NullListener),
            blocked_log: Vec::new(),
            demo_sent: false,
        })
    }

    /// Sends a fixed demonstration script once, covering: styled-span
    /// rendering (multiple SGR colors in one line), and at least three
    /// RFC-009 inert families (OSC 52 clipboard, OSC title, OSC 8
    /// hyperlink) so PR-014-D's screenshot evidence has something concrete
    /// to show. This is demonstration input, not product behavior: the
    /// spike does not implement a command palette or arbitrary user input
    /// path here.
    pub fn send_demo_script_once(&mut self) {
        if self.demo_sent {
            return;
        }
        self.demo_sent = true;

        let script = concat!(
            "printf '\\033[31mred \\033[32mgreen \\033[1;34mbold-blue\\033[0m plain\\n'\n",
            "printf '\\033]52;c;U0VDUkVUCg==\\007'\n",
            "printf '\\033]0;PWNED-TITLE\\007'\n",
            "printf '\\033]8;;https://evil.invalid/\\007link-text\\033]8;;\\007\\n'\n",
            "printf 'after-inert-sequences\\n'\n",
        );
        let _ = self.runtime.write_input(&self.handle, script.as_bytes());
    }

    /// Reads whatever PTY output is currently available (bounded, short
    /// poll -- this is called from a GUI tick subscription, so it must not
    /// block the render loop) and advances the filtered emulator.
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
        self.blocked_log.append(&mut filter.blocked);
    }

    /// Styled-span evidence for C1: groups each row's cells into runs of
    /// consecutive identical resolved foreground color, using the same
    /// `renderable_content()`/`Colors` API a real terminal renderer would
    /// use (not a bespoke shortcut). Proves multiple colors/attributes can
    /// render within one text block, which is what RFC-014 asks the spike
    /// to confirm on behalf of the deferred syntax-highlighting work.
    pub fn styled_rows(&self) -> Vec<Vec<(String, [f32; 3])>> {
        let content = self.term.renderable_content();
        let colors = content.colors;

        let mut rows: Vec<Vec<(char, [f32; 3])>> = vec![Vec::new(); ROWS];
        for indexed in content.display_iter {
            let point = indexed.point;
            if point.line.0 < 0 || point.line.0 as usize >= ROWS {
                continue;
            }
            let rgb = resolve_color(indexed.fg, colors);
            rows[point.line.0 as usize].push((indexed.c, rgb));
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
}

/// Standard ANSI 16-color fallback palette (used when `Term`'s `Colors`
/// table has no override for a given named color, which is the case for
/// every color in this spike, since nothing seeds a palette). Indexed
/// colors beyond 0-15 fall back to the default foreground: a full 256-color
/// resolution table is out of scope for a rendering-strategy spike and is
/// recorded as a known simplification, not silently implied to be complete.
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
