//! RFC-015 PR-015-F: discharges residual risk R1 (RFC-014's `iced`
//! substrate decision was approved with input latency **unverified**,
//! explicitly conditional on this RFC discharging it).
//!
//! # Why this does not reuse RFC-014 PR-014-E's measurement shape as-is
//!
//! The spike proved `iced::window::frames() -> Subscription<Instant>`
//! is the *only* application-level "a frame was painted" signal, and
//! that subscribing to it forces continuous compositor-driven redraw
//! (~57 Hz measured, ~2.7% of one core, with nothing animating) even
//! when idle. Every one of the spike's C2/C3/C4 samples read exactly
//! `0µs` as a direct consequence: once `frames()` is subscribed, a
//! frame is always <1ms away, so "time from input to next frame"
//! mostly measures redraw cadence, not input cost. RFC-014's own
//! qa-evidence recorded this as a degenerate result, not a pass.
//!
//! RFC-015 anticipated this exact failure mode and requires a specific
//! fallback: **"measure input-to-state-change and frame cost
//! separately, and report the decomposition rather than a degenerate
//! combined figure... another all-zero figure is not an acceptable
//! outcome."** This module implements that fallback structurally,
//! rather than reusing `frames()` for typing latency at all:
//!
//! - **Input-to-state-change**: wall-clock time from a measurement
//!   keystroke's arrival (timestamped the instant the subscription
//!   receives it) to [`crate::shell::update`] returning. Pure Rust
//!   function-call timing -- no `frames()` involved, so there is
//!   nothing for this figure to contaminate.
//! - **View-build cost**: wall-clock time for [`crate::shell::view`]
//!   to construct its `Element` tree for the current state, timed by
//!   wrapping the view function passed to `iced::application` (see
//!   `main.rs`), not by anything inside `shell::view` itself. Also
//!   `frames()`-free.
//!
//! Neither figure is "full paint-to-screen time" -- that would still
//! need `frames()` and would reintroduce the exact contamination this
//! module exists to avoid. This is disclosed precisely in
//! `qa-evidence.md`, the same "app-internal, not end-to-end" framing
//! RFC-014 already established, carried one level further: even
//! "app-internal" here means "this app's own `update`/`view`
//! functions," not `iced`'s internal render pipeline.
//!
//! **Startup (C5) is the one criterion that still uses `frames()`**,
//! exactly as the spike did -- safely, because the process exits
//! immediately after the first frame, so there is no *sustained*
//! redraw-forcing during any real interactive session for it to
//! contaminate.
//!
//! **Mode switch (C4 / `NFR-PERF-002`) reuses this exact mechanism**
//! (RFC-015 PR-015-E) rather than a new one: `Criterion::ModeSwitch`'s
//! measurement key dispatches the real `AppCommand::ToggleActiveProjectMode`
//! (bypassing only the `KeybindingPolicy` lookup step a real `Ctrl+Alt+M`
//! press would go through first -- irrelevant to what this measures, the
//! cost of the state mutation and view rebuild, not input classification),
//! and the same input-to-state-change/view-build decomposition applies.
//! Deferred here in PR-015-F (response 133) because M8 had no real mode
//! to switch into that wasn't the Project Board against an empty
//! placeholder; PR-015-E is what makes it a real target.
//!
//! # Non-contamination when inactive
//!
//! `Measurement::from_env()` returns `None` unless
//! `TEKSTIDE_MEASURE_CRITERION` is set to a recognized value -- the
//! same convention RFC-014's spike and this crate's own
//! `TEKSTIDE_LAYER_DEMO` (PR-015-B) already use. With it unset (the
//! default for every normal interactive run), `shell::State.measurement`
//! is always `None`, no measurement subscription is ever added, and the
//! view-timing wrapper in `main.rs` is a single cheap boolean check with
//! no timer started and nothing written -- verified by idle-CPU
//! comparison in `qa-evidence.md`, not merely asserted here.

use std::io::Write;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static PROCESS_START: OnceLock<Instant> = OnceLock::new();

/// Must be called as the first statement of `main()`, before
/// `iced::application(...)` does anything -- the only honest definition
/// of "process start" available from inside the process itself (it
/// excludes exec()/dynamic-linker/runtime init time before `main` is
/// reached). Same approach as the RFC-014 spike's `mark_process_start`.
pub fn mark_process_start() {
    PROCESS_START.get_or_init(Instant::now);
}

fn process_start() -> Instant {
    *PROCESS_START.get_or_init(Instant::now)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Criterion {
    /// C2 (`NFR-PERF-003`): input-to-state-change and view-build cost,
    /// measured separately -- see the module doc.
    Typing,
    /// C5 (`NFR-PERF-001`): process start to first frame painted.
    Startup,
    /// C4 (`NFR-PERF-002`, RFC-015 PR-015-E): input-to-state-change and
    /// view-build cost for a real mode switch, the same decomposition
    /// `Typing` uses -- see the module doc.
    ModeSwitch,
}

impl Criterion {
    fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "typing" => Some(Self::Typing),
            "startup" => Some(Self::Startup),
            "mode_switch" => Some(Self::ModeSwitch),
            _ => None,
        }
    }

    /// Both criteria needing the input-to-state-change/view-build
    /// decomposition, as opposed to `Startup`'s single `frames()` sample.
    fn uses_input_view_decomposition(self) -> bool {
        matches!(self, Self::Typing | Self::ModeSwitch)
    }
}

/// The character a synthetic measurement run sends -- matches the RFC-014
/// spike's own choice (`"j"`), an ordinary lowercase letter unlikely to
/// collide with any real binding.
pub const MEASURED_KEY_CHARACTER: &str = "j";

pub struct Measurement {
    criterion: Criterion,
    log: std::fs::File,
    received: u32,
    target: u32,
    startup_recorded: bool,
}

impl Measurement {
    /// `None` unless `TEKSTIDE_MEASURE_CRITERION` names a recognized
    /// criterion and `TEKSTIDE_MEASURE_LOG` names a writable path --
    /// measurement is off unless both are explicitly set, matching the
    /// spike's convention exactly. Also wires up the view-cost thread-local
    /// log (see [`init_view_log`]) as one atomic setup step, so a caller
    /// only has to call this once for both halves of the decomposition.
    pub fn from_env() -> Option<Self> {
        let criterion =
            Criterion::from_env_value(&std::env::var("TEKSTIDE_MEASURE_CRITERION").ok()?)?;
        let log_path = std::env::var("TEKSTIDE_MEASURE_LOG").ok()?;
        let target: u32 = std::env::var("TEKSTIDE_MEASURE_TARGET")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1100);
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok()?;

        init_view_log(criterion, &log_path);

        Some(Self {
            criterion,
            log,
            received: 0,
            target,
            startup_recorded: false,
        })
    }

    pub fn criterion(&self) -> Criterion {
        self.criterion
    }

    /// Records one input-to-state-change sample: the elapsed time from
    /// when the measurement subscription first saw the keystroke
    /// (`sent_at`) to this call, which happens at the end of
    /// `shell::update`'s handling of it. One line per sample, prefixed
    /// `input` so the driver can separate this stream from `view`
    /// samples in the same log file.
    pub fn record_input(&mut self, sent_at: Instant) {
        self.received += 1;
        let elapsed = Instant::now().saturating_duration_since(sent_at);
        let _ = writeln!(self.log, "input {}", elapsed.as_micros());
    }

    /// Records the first (and only) frame for the `Startup` criterion --
    /// elapsed time since process start. Ignored for `Typing` (which
    /// never subscribes to `frames()` at all -- see the module doc).
    pub fn record_startup_frame(&mut self, at: Instant) {
        if self.criterion == Criterion::Startup && !self.startup_recorded {
            self.startup_recorded = true;
            let elapsed = at.saturating_duration_since(process_start());
            let _ = writeln!(self.log, "{}", elapsed.as_micros());
        }
    }

    pub fn is_done(&self) -> bool {
        match self.criterion {
            Criterion::Startup => self.startup_recorded,
            Criterion::Typing | Criterion::ModeSwitch => self.received >= self.target,
        }
    }

    /// Test-only: builds a `Measurement` writing to a caller-chosen path,
    /// bypassing `TEKSTIDE_MEASURE_*` env vars entirely so tests never
    /// race on process-global environment state.
    #[cfg(test)]
    pub(crate) fn for_test(criterion: Criterion, log_path: &std::path::Path, target: u32) -> Self {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .expect("test log path must be writable");
        Self {
            criterion,
            log,
            received: 0,
            target,
            startup_recorded: false,
        }
    }
}

/// Records one view-build-cost sample -- called from `main.rs`'s
/// wrapper around [`crate::shell::view`], never from inside `view`
/// itself (`view(&State) -> Element` has no mutable access to a
/// `Measurement` living in `State`). A thread-local file handle, set
/// once at boot by [`init_view_log`], is the deliberately narrow
/// exception to "no global mutable state" here: it exists only to let
/// an `&State`-shaped function record a side channel of evidence, never
/// to influence what gets rendered.
pub fn init_view_log(criterion: Criterion, log_path: &str) {
    if !criterion.uses_input_view_decomposition() {
        // Startup's frame is timed via `Measurement::record_startup_frame`
        // instead; no view-cost log is meaningful for a process that
        // exits after its first frame.
        return;
    }
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        VIEW_LOG.with(|cell| *cell.borrow_mut() = Some(file));
    }
}

pub fn record_view_cost(elapsed: Duration) {
    VIEW_LOG.with(|cell| {
        if let Some(file) = cell.borrow_mut().as_mut() {
            let _ = writeln!(file, "view {}", elapsed.as_micros());
        }
    });
}

thread_local! {
    static VIEW_LOG: std::cell::RefCell<Option<std::fs::File>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod tests;
