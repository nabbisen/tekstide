//! RFC-017: the terminal surface. PR-017-B built the interposition
//! filter; PR-017-C gave it a real, fixed-size rendering caller;
//! PR-017-D wired real keyboard input to it. This slice (PR-017-E)
//! gives the pane real chrome: a [`session_bar`], a split decided from
//! real font metrics ([`font_metrics`], [`layout`]) rather than a
//! fraction, and the hidden-session grid-state decision.
//!
//! **Module layout, and why it changed this slice.** Grid rendering
//! (the only code with a legitimate reason to construct a raw `Color`)
//! moved into [`grid_colors`] specifically so the colour-scan exemption
//! (`shell::tests::no_raw_color_construction_anywhere_in_the_crate`)
//! could narrow to that one file instead of staying a claim about this
//! whole file -- reviewed and confirmed correct in response 148, with
//! the expectation that it would move exactly here the moment this file
//! grew chrome. [`session_bar`] is that chrome, and it is **not**
//! exempt: every colour there comes from `crate::theme::Theme`.
//!
//! **Input, RFC-017 PR-017-D.** [`TerminalPane::write_input`] is the one
//! production path that reaches `LinuxTerminalRuntime::write_input`;
//! `shell.rs`'s `update` is its only caller, gated on `state.modal` being
//! absent and on `input::TextStream::to_pty_bytes` -- this module never
//! constructs a `TextStream` itself or reaches into `iced::keyboard::Key`
//! directly, matching `TextStream`'s own privacy boundary.
//!
//! **P1 (single ingress), re-proven against production code, not just a
//! test harness.** [`TerminalPane`] is the *only* place in this crate
//! that constructs a `Term`/`Processor`, and [`TerminalPane::poll`] is
//! the only place that calls `Processor::advance` — always through
//! `filter::SecurityFilter::new(&mut self.term)`, never any other path
//! to `self.term`.
//!
//! **P2 (no side channels).** `Term::grid_mut()` is not called anywhere
//! in this module; the only non-byte input this pane's own `Term`
//! receives is its fixed construction-time dimensions, not a live
//! resize -- this slice's split decides *how many* panes to show and
//! *whether* to show them, not a live reflow of any one pane's own grid
//! (see [`layout`]'s module doc for why that is a deliberately smaller
//! claim than "real per-pane resize").
//!
//! **Bounded scrollback.** `pane_config`'s `scrolling_history` is set
//! explicitly to [`SCROLLBACK_LINES`], not left at `alacritty_terminal`'s
//! own default (10,000). **Hidden sessions are retained in memory, not
//! torn down** -- decided against this same bound: a hidden pane's `Term`
//! costs exactly the same bounded amount of memory a visible one does
//! (this bound does not change based on visibility), and the number of
//! sessions a project can hold at all is itself bounded
//! (`ProjectResourceLimits::terminal_session_limit`). Tearing a hidden
//! session down and rebuilding it from scrollback would lose state and
//! change what "hidden" means to a user checking on it later; retaining
//! it costs a bound already paid for, not a new, unbounded one. Hidden
//! panes are still polled every tick (`shell.rs`'s `TerminalDemoTick`
//! handler iterates all of them, not only the visible ones) -- proven
//! in `terminal::tests` that a hidden pane's content keeps growing while
//! hidden and is exactly what a caller sees once it becomes visible
//! again, not a reset.
//!
//! **The grid renders as data, never as chrome** (RFC-015/RFC-018):
//! [`grid_colors::view`] takes `&TerminalPane` and a font size only --
//! nothing here can reach `shell::State`'s modal or chrome fields, the
//! same "cannot construct/cannot reach" shape `surface::board` already
//! established.

pub mod filter;
mod font_metrics;
mod grid_colors;
mod layout;
pub mod session_bar;

pub use grid_colors::view;
pub(crate) use layout::layout_class_for;

use std::path::PathBuf;
use std::time::Duration;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

use tekstide_core::domain::TerminalSession;
use tekstide_core::project::{ProjectId, ProjectSession};
use tekstide_core::runtime::terminal::{
    LinuxTerminalRuntime, TerminalDimensions, TerminalLaunchError, TerminalLaunchSpec,
    TerminalRuntimeEvent, TerminalRuntimeHandle,
};

use filter::SecurityFilter;

/// Fixed for this slice -- see the module doc's P2 note for why a live
/// per-pane reflow is a deliberately separate, larger claim this slice
/// does not make. 80x24 matches the spike's own choice and every
/// existing `TerminalPanePolicy` default.
pub(crate) const ROWS: usize = 24;
pub(crate) const COLS: usize = 80;

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
/// Which slot (`Primary`/`Secondary`/`Hidden`) this pane's session
/// occupies is *not* a field here -- it lives in the real, registered
/// `TerminalSession` this pane's [`Self::terminal_id`] names, asked of
/// `tekstide-core` fresh each time (`shell.rs`), not cached here.
pub struct TerminalPane {
    runtime: LinuxTerminalRuntime,
    handle: TerminalRuntimeHandle,
    processor: Processor<StdSyncHandler>,
    term: Term<VoidListener>,
    /// RFC-017 PR-017-G (response 155 item 3): cumulative bytes
    /// discarded across every `poll()` call because a single bounded
    /// read exceeded its 64KiB cap -- surfaced for the flood
    /// measurement's own evidence ("dropped bytes are a result, not a
    /// footnote"), not consumed by any production decision.
    dropped_bytes_total: u64,
}

impl TerminalPane {
    /// Launches a plain shell (`TerminalLaunchSpec::plain_shell`, never
    /// an AI CLI profile -- this pane has no input path for a launch
    /// contract to authorize against yet) in `root`, already validated
    /// to exist, and prepares the filtered emulator side. `root` is the
    /// project root the caller resolved; this function does no path
    /// resolution of its own.
    ///
    /// Returns the real `TerminalSession` `launch_project_shell`
    /// produced alongside the pane -- RFC-017 PR-017-E's caller
    /// registers it on the real active project
    /// (`AppState::attach_terminal_session`) so the session-bar/hidden
    /// state this slice adds has something real to describe. `project_id`
    /// should be that real project's id, not a throwaway one, or
    /// registration will fail closed (`OwnershipError::CrossProject`).
    pub fn launch(
        project_id: ProjectId,
        title: impl Into<String>,
        root: PathBuf,
        shell: PathBuf,
    ) -> Result<(Self, TerminalSession), TerminalLaunchError> {
        let project = ProjectSession::new(project_id.clone(), "terminal-pane", &root, &root);

        let mut spec = TerminalLaunchSpec::plain_shell(project_id, title, &root, shell);
        spec.dimensions = TerminalDimensions {
            rows: ROWS as u16,
            cols: COLS as u16,
        };

        let mut runtime = LinuxTerminalRuntime::new();
        let (session, _events) = runtime.launch_project_shell(&project, spec)?;
        let handle = TerminalRuntimeHandle::new(session.id.clone(), project.id().clone());

        Ok((
            Self {
                runtime,
                handle,
                processor: Processor::new(),
                term: Term::new(pane_config(), &PaneSize, VoidListener),
                dropped_bytes_total: 0,
            },
            session,
        ))
    }

    /// Reads whatever PTY output is currently available (bounded, short
    /// poll -- called from a GUI tick subscription, so it must not block
    /// the render loop) and advances the filtered emulator. **The only
    /// place in this crate `Processor::advance` is called, and the only
    /// place `self.term` is mutably borrowed outside construction** --
    /// P1's re-enumeration. Called every tick regardless of this pane's
    /// visible slot (see the module doc's hidden-session decision) --
    /// callers must not skip `poll()` for a hidden pane.
    pub fn poll(&mut self) {
        let Ok((bytes, event)) = self.runtime.read_available_bounded_for(
            &self.handle,
            Duration::from_millis(5),
            64 * 1024,
        ) else {
            return;
        };
        // RFC-017 PR-017-G (response 155 item 3): the event this call
        // produces alongside `bytes` names how many bytes this read
        // discarded, if the pty had more available than the 64KiB cap --
        // previously discarded here unread (`let Ok((bytes, _event))`).
        // Accumulated, not acted on: nothing downstream changes because
        // of this count today, it exists only to be reported.
        if let TerminalRuntimeEvent::OutputBuffered { summary, .. } = &event {
            self.dropped_bytes_total += summary.dropped_bytes as u64;
        }
        if bytes.is_empty() {
            return;
        }

        let mut filter = SecurityFilter::new(&mut self.term);
        self.processor.advance(&mut filter, &bytes);
    }

    /// RFC-017 PR-017-G: cumulative bytes discarded across every
    /// `poll()` call this pane has made so far -- see the field's own
    /// doc comment. Read by the flood measurement's evidence-gathering
    /// only; no production caller.
    pub fn dropped_bytes_total(&self) -> u64 {
        self.dropped_bytes_total
    }

    /// This pane's real, live `TerminalId` -- what a caller compares a
    /// `TextStream`'s target against before calling [`Self::write_input`],
    /// and what a caller looks up in the real, registered
    /// `TerminalSession` list to find this pane's slot/status.
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
        grid_colors::styled_rows(&self.term)
            .into_iter()
            .flat_map(|runs| runs.into_iter().map(|(text, _)| text))
            .collect()
    }
}

#[cfg(test)]
mod tests;
