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
//! to `self.term`. **RFC-017 Amendment 1, PR-A1-B**: `poll`'s bytes now
//! come from `TerminalReader::drain_available` (a dedicated reader
//! thread over a bounded channel) rather than
//! `runtime.read_available_bounded_for`'s sleep-and-truncate loop --
//! this re-proof is re-run against that new shape, not assumed to still
//! hold because the old test suite still passes; see
//! `only_one_call_site_ever_advances_a_terminal_processor_in_the_crate`.
//!
//! **P2 (no side channels).** `Term::grid_mut()` is not called anywhere
//! in this module; the only non-byte input this pane's own `Term`
//! receives is its fixed construction-time dimensions, not a live
//! resize -- this slice's split decides *how many* panes to show and
//! *whether* to show them, not a live reflow of any one pane's own grid
//! (see [`layout`]'s module doc for why that is a deliberately smaller
//! claim than "real per-pane resize"). **RFC-017 Amendment 1, PR-A1-B**:
//! `TerminalPane.reader` is a `TerminalReader`'s only owner, and
//! `TerminalReader` is not `Clone` -- a second consumer of its channel
//! is unrepresentable by the type, and `poll` is the only place in this
//! crate that calls `drain_available` at all (see
//! `only_this_field_drains_a_terminalreader_in_the_crate`), covering the
//! data channel. The reader's shutdown `eventfd` is unreachable from
//! `crates/tekstide` by construction, unchanged by this note -- it is
//! private to `tekstide-core::runtime::terminal::reader`, reachable only
//! from `TerminalReader`'s own `Drop`. **RFC-017 Amendment 1, PR-A1-C**:
//! the reader's *second* `eventfd` (the wake signal) is different --
//! [`Self::wake_notifier`] is a real, new call site in this crate that
//! reaches it, so it needs its own enumeration rather than inheriting
//! the shutdown fd's "unreachable" claim. `shell.rs`'s
//! `terminal_wake_subscriptions` is the one production caller of
//! `wake_notifier`, and `terminal_wake_stream` is the one production
//! caller of the resulting `WakeNotifier::block_until_woken` --
//! `only_one_call_site_ever_asks_a_terminalpane_for_its_wake_notifier`
//! and `only_one_call_site_ever_blocks_on_a_wake_notifier` in
//! `shell::tests` prove both by occurrence count, the same shape
//! response 203 required for the `Processor::advance`/
//! `TerminalReader::drain_available` enumerations below (deliberately
//! spelled without their own trailing `(` here, so this sentence does
//! not itself become a second match for those two scans).
//!
//! **Modal exclusivity is unchanged by PR-A1-B, and that is the point.**
//! The reader thread changes how *output* reaches this pane; it does not
//! touch [`Self::write_input`] or its one caller
//! (`shell.rs`'s `write_terminal_input`, gated on `state.modal.is_some()`).
//! Output continuing to flow (and being drawn) while a modal is open is
//! not itself wrong -- a terminal rendering behind a dialog is normal.
//! What must not happen, and does not, is *input* reaching the PTY: the
//! reader thread has no write access to anything, and the guard that
//! already existed is untouched by this slice. Re-checked at the state
//! level, unmodified, against this slice's new reader-based `poll`:
//! `shell::tests::modal_open_blocks_pty_write_and_closing_it_resumes_delivery`.
//! Re-checked as a live GUI capture with the Tab positive control the
//! ingress re-proof document requires (keystrokes suppressed and a
//! modal's own focus marker visibly moving, in the same screenshot):
//! `rfcs/handoffs/017-amendment-1-readiness-driven-terminal-io/evidence/pr-a1-b/`,
//! recorded in that handoff's own `qa-evidence.md`.
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
//! panes still get their own wake subscription and are polled whenever
//! it fires, the same as a visible one -- `shell.rs`'s
//! `terminal_wake_subscriptions` builds one per tracked pane regardless
//! of visible slot (RFC-017 Amendment 1, PR-A1-C replaced the old fixed
//! tick that iterated every pane with this, but the "hidden panes are
//! not skipped" property is the same one, re-proven against the new
//! shape rather than assumed) -- proven in `terminal::tests` that a
//! hidden pane's content keeps growing while hidden and is exactly what
//! a caller sees once it becomes visible again, not a reset.
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
    BoundedRuntimeSummary, LinuxTerminalRuntime, TerminalDimensions, TerminalLaunchError,
    TerminalLaunchSpec, TerminalReader, TerminalRuntimeHandle, TerminationOutcome,
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
    /// RFC-017 Amendment 1, PR-A1-B: this pane's single source of PTY
    /// output, replacing `runtime.read_available_bounded_for` as
    /// `poll()`'s data source. `TerminalReader` is not `Clone` and this
    /// is its only owner, so this field is also this crate's half of
    /// P2's "exactly one consumer" -- see
    /// `only_this_field_drains_a_terminalreader_in_the_crate` for the
    /// enumeration proof.
    reader: TerminalReader,
    processor: Processor<StdSyncHandler>,
    term: Term<VoidListener>,
    /// RFC-017 PR-017-G (response 156): cumulative bytes actually read
    /// (accepted, not dropped) across every `poll()` call -- paired with
    /// `Measurement::elapsed` to compute the flood's *observed*,
    /// in-app throughput, the precondition check for whether a flood
    /// run actually reached rate inside the application at all.
    bytes_read_total: u64,
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
        let reader = runtime.spawn_output_reader(&handle).map_err(|error| {
            TerminalLaunchError::ReaderUnavailable {
                summary: BoundedRuntimeSummary::new(format!(
                    "failed to spawn PTY reader thread: {error:?}"
                )),
            }
        })?;

        Ok((
            Self {
                runtime,
                handle,
                reader,
                processor: Processor::new(),
                term: Term::new(pane_config(), &PaneSize, VoidListener),
                bytes_read_total: 0,
            },
            session,
        ))
    }

    /// Drains whatever PTY output the reader thread has buffered so far
    /// (never blocks) and advances the filtered emulator. **The only
    /// place in this crate `Processor::advance` is called, and the only
    /// place `self.term` is mutably borrowed outside construction** --
    /// P1's re-enumeration. Callers must not skip `poll()` for a hidden
    /// pane -- see the module doc's hidden-session decision.
    ///
    /// RFC-017 Amendment 1, PR-A1-C: called from `shell.rs`'s
    /// `handle_terminal_woke`, triggered by this pane's own
    /// `wake_notifier()` firing -- not a fixed-interval tick anymore.
    /// See [`Self::wake_notifier`].
    pub fn poll(&mut self) {
        let drain = self.reader.drain_available();
        self.bytes_read_total += drain.bytes().len() as u64;
        if drain.bytes().is_empty() {
            return;
        }

        let mut filter = SecurityFilter::new(&mut self.term);
        self.processor.advance(&mut filter, drain.bytes());
    }

    /// RFC-017 Amendment 1, PR-A1-C: a duplicated handle onto this
    /// pane's reader thread's wake signal -- what `shell.rs`'s
    /// subscription bridges into an event-driven `poll()` trigger,
    /// replacing the fixed 50ms tick this amendment removes. `Err` only
    /// on `eventfd(2)` resource exhaustion, the same failure mode
    /// `TerminalReader::spawn` can already fail on; the caller decides
    /// what a failure here means for that one pane (`shell.rs`'s
    /// `terminal_wake_subscriptions` currently just excludes it).
    pub fn wake_notifier(&self) -> std::io::Result<tekstide_core::runtime::terminal::WakeNotifier> {
        self.reader.try_clone_wake_notifier()
    }

    /// RFC-017 PR-017-G (response 156): cumulative bytes actually read
    /// across every `poll()` call this pane has made so far -- see the
    /// field's own doc comment. Read by the flood measurement's
    /// evidence-gathering only; no production caller.
    pub fn bytes_read_total(&self) -> u64 {
        self.bytes_read_total
    }

    /// Terminal launch UX handoff: a **non-blocking** check for whether
    /// this pane's shell has already exited -- `Some` the first (and
    /// only) time it has. Built on `wait_for_exit(handle, Duration::ZERO)`
    /// rather than a new runtime method, because tracing its own loop
    /// shows it already degrades to a single, non-blocking `try_wait()`
    /// at a zero timeout: the loop's only blocking `sleep` happens when
    /// it is about to retry, gated on `elapsed() > timeout`, and with
    /// `timeout` zero that comparison is true as soon as any
    /// (non-zero-resolution) time has passed since the loop started --
    /// which real monotonic clocks always show between two sequential
    /// reads. Callers must not call this from a context where blocking
    /// would be acceptable and then rely on that margin; it is a real
    /// property of the existing loop, not a documented contract of it,
    /// and is exactly why `terminal_poll_handler_cost_under_a_real_flood_headless_benchmark`
    /// (response 158) exists as a standing, real-timing check on this
    /// same code path rather than a one-time argument.
    pub fn check_exit(&mut self) -> Option<TerminationOutcome> {
        self.runtime
            .wait_for_exit(&self.handle, Duration::ZERO)
            .ok()
            .flatten()
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
