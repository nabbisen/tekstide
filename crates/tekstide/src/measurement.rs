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
//! **Terminal input under flood (C3 / `NFR-PERF-004`)** was originally
//! built (RFC-017 PR-017-G) reusing only the input-to-state-change half
//! of the `Typing`/`ModeSwitch` decomposition -- dispatch plus one pty
//! `write(2)`, deliberately **not** `NFR-PERF-004` as that requirement
//! is actually understood ("terminal input latency" means keystroke-to-
//! echo-visible). At the time, echo visibility depended entirely on a
//! fixed-interval poll tick -- the only place PTY bytes reached the
//! emulator grid -- so a keystroke's echo was uncorrelated with when the
//! key arrived, and no in-app timestamp could honestly attribute a given
//! echo to a given keystroke. The tick contributed an arithmetic expected
//! p95 of ~47.5ms from poll-wait alone, roughly three times the entire
//! 16ms budget, before any other cost was even added -- recorded as
//! `NFR-PERF-004` **not met** on that arithmetic (RFC-017 PR-017-H).
//!
//! **RFC-017 Amendment 1 removed the tick.** PTY output now reaches the
//! grid only when `Message::TerminalWoke` fires -- one real `poll(2)`-
//! driven wake per pane, sent the instant the reader thread sees new
//! bytes, not on a fixed schedule. This makes a keystroke's own
//! echo attributable for the first time: `Message::TerminalWoke` itself
//! deliberately carries no content (response 205's `TerminalId`-only
//! constraint, so terminal bytes never become `Debug`/`Clone`-reachable
//! through `Message`), so this criterion cannot know *from the message*
//! which wake carried its own echo -- it has to look at the grid.
//!
//! **What the figure now covers, in two samples per send from one
//! `sent_at`**: `input` (unchanged since PR-017-G -- dispatch plus the
//! pty `write(2)`, via `record_input`) and `echo` (new, PR-A1-D) -- the
//! full interval from the same `sent_at` to the moment
//! [`Measurement::check_echo_visible`] first observes this send's own
//! fresh marker string (via [`Measurement::next_echo_marker`], not a
//! bare repeated character -- see that method's own doc for why:
//! response 209's required fix, after a real, reproducible PTY
//! canonical-mode redraw behaviour was found to duplicate a repeated
//! character's occurrence count) as a substring of the grid's rendered
//! text, checked once per real wake from `handle_terminal_woke` (never
//! on a timer, and throttled -- see [`Measurement::should_check_echo`]).
//! `echo` **is** `NFR-PERF-004` as actually defined, to the same
//! "app-internal, not end-to-end" precision this whole module already
//! commits to for every other criterion (grid-state-visible, not
//! painted-to-screen -- avoiding the exact `frames()` contamination this
//! module exists to avoid, stated once at the top rather than re-argued
//! per criterion). **It is not, on its own, sufficient to claim
//! `NFR-PERF-004` "met"**: a clean measurement here is only a lower-bound
//! contributor (dispatch-to-grid cost), not an upper bound on the true
//! end-to-end path, which also includes compositor/GPU present -- this
//! module's own `frames()`-avoidance means it structurally cannot
//! measure that part at all, on any machine.
//!
//! It still does **not** use the view-build decomposition
//! (`uses_input_view_decomposition` stays `false` for it): `view` is
//! timed by wrapping `shell::view` itself, and this criterion's own
//! `handle_terminal_woke` call already happens on a message dispatch
//! separate from `MeasuredTerminalInput`'s -- a `view` sample logged
//! against either message would describe an unrelated cycle's rebuild
//! cost, not this criterion's own, the same reasoning PR-017-G
//! established and PR-A1-D changes nothing about.
//!
//! The one real terminal pane this criterion measures against still
//! renders normally in `view()` every cycle (registered with
//! `tekstide-core`, project mode set to `TerminalImmersion`) -- that
//! rendering, and the concurrent flood's own wake-driven poll/VTE cost,
//! are real, uninstrumented contention on the same `iced` executor as
//! this criterion's own message dispatches, and do affect both graded
//! figures even though they are not part of either: a flood busy enough
//! to delay `MeasuredTerminalInput`'s dispatch shows up as a higher
//! `input` sample via queuing, and a flood busy enough to delay this
//! send's own `TerminalWoke` behind others already queued shows up as a
//! higher `echo` sample the same way.
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

use std::collections::VecDeque;
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
    /// C3 (`NFR-PERF-004`, RFC-017 PR-017-G, redefined RFC-017
    /// Amendment 1 PR-A1-D): two samples per send -- `input`
    /// (dispatch plus the pty `write(2)`, PR-017-G's original figure)
    /// and `echo` (dispatch to grid-visible, the full interval the
    /// budget actually names, first measurable once PR-A1-C's wake made
    /// a keystroke's own echo attributable) -- against a real, live
    /// terminal pane under a bounded background output flood. See the
    /// module doc for why this one still does not use the view-build
    /// half of the decomposition.
    TerminalFlood,
}

impl Criterion {
    fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "typing" => Some(Self::Typing),
            "startup" => Some(Self::Startup),
            "mode_switch" => Some(Self::ModeSwitch),
            "terminal_flood" => Some(Self::TerminalFlood),
            _ => None,
        }
    }

    /// Both criteria needing the input-to-state-change/view-build
    /// decomposition, as opposed to `Startup`'s single `frames()` sample
    /// and `TerminalFlood`'s input-only figure (see the module doc).
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
    started_at: Instant,
    /// RFC-017 Amendment 1, PR-A1-D: one entry per `MeasuredTerminalInput`
    /// send still waiting for its own echo to land in the grid -- see
    /// [`Self::note_measured_send`]/[`Self::check_echo_visible`]. Empty
    /// for every criterion but `TerminalFlood`, which never pushes to it.
    ///
    /// **A distinctive per-send marker string, not a repeated character's
    /// occurrence count** -- response 209's required fix. An earlier
    /// version tracked "grid occurrence count of a single repeated
    /// character reaches N," which a real, reproducible property of PTY
    /// canonical-mode echo defeats: past roughly 20 accumulated
    /// characters on one unterminated line, this environment's terminal
    /// occasionally re-echoes the **entire current line** in one wake
    /// (traced directly: occurrence count jumped from 20 to 41 in a
    /// single step -- 20 already present plus a full 21-character
    /// re-echo). A repeated character's Nth occurrence is indistinguishable
    /// from its (N-1)th reappearing in a redraw; a *new, distinct* marker's
    /// first appearance cannot be confused with an *old* marker's
    /// reappearance, so substring presence of a fresh marker per send is
    /// immune to the same failure. See [`Self::next_echo_marker`].
    pending_echo: VecDeque<(Instant, String)>,
    /// RFC-017 Amendment 1, PR-A1-D: when [`Self::check_echo_visible`]
    /// was last actually run -- see [`Self::should_check_echo`]'s own
    /// doc for why a real wall-clock throttle, not just "is anything
    /// pending," is required here.
    last_echo_check: Option<Instant>,
    /// RFC-017 Amendment 1, PR-A1-D: counter behind [`Self::next_echo_marker`].
    next_marker_index: u64,
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
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok()?;

        init_view_log(criterion, &log_path);
        write_environment_snapshot(&mut log);

        Some(Self {
            criterion,
            log,
            received: 0,
            target,
            startup_recorded: false,
            started_at: Instant::now(),
            pending_echo: VecDeque::new(),
            last_echo_check: None,
            next_marker_index: 0,
        })
    }

    pub fn criterion(&self) -> Criterion {
        self.criterion
    }

    /// Wall-clock time since this `Measurement` was constructed --
    /// RFC-017 PR-017-G response 156: paired with
    /// [`crate::surface::terminal::TerminalPane::bytes_read_total`] at
    /// report time to compute the flood's *observed* (in-app) throughput,
    /// as opposed to the flood script's own standalone throughput --
    /// the precondition check the reviewer asked for: if observed
    /// throughput is far below what the script produces alone, the flood
    /// never reached rate inside the application and the run is void,
    /// detectable from the run's own output rather than argued after the
    /// fact.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Records one terminal-poll tick-handler sample (`poll()`'s own
    /// wall time -- the PTY read plus `Processor::advance`) -- RFC-017
    /// PR-017-G response 156's discriminator between "the environment
    /// was loaded" and "the update loop cannot keep up with a real
    /// flood": if this stays in single-digit milliseconds while
    /// `record_input`'s figure is orders of magnitude higher, the
    /// bottleneck is not this handler. Meaningful relative to the tick
    /// period regardless of machine load, so -- unlike `record_input`'s
    /// figure -- it does not itself need a quiet machine to be
    /// informative. Prefixed `tick` in the same log file, same
    /// separate-streams convention `input`/`view` already use.
    ///
    /// **`bytes_read_total`, response 156 Required 2**: the pane's
    /// cumulative accepted-bytes count *at this tick*, not just at exit.
    /// `elapsed()` alone cannot safely denominate observed throughput,
    /// because `FLOOD_SCRIPT` self-terminates after 30s: a run that
    /// takes *longer* than that (the update loop genuinely saturating,
    /// exactly the scenario this instrumentation exists to catch) would
    /// see `bytes_read_total` plateau while `elapsed_secs` keeps
    /// climbing, collapsing observed throughput and misreading a
    /// saturating run as "flood never reached rate." Logging the running
    /// total per tick lets throughput be computed over the flood's
    /// actual active window instead, derived rather than assumed.
    pub fn record_tick_handler(&mut self, elapsed: Duration, bytes_read_total: u64) {
        let _ = writeln!(self.log, "tick {} {bytes_read_total}", elapsed.as_micros());
    }

    /// Records one input-to-state-change sample: the elapsed time from
    /// when the measurement subscription first saw the keystroke
    /// (`sent_at`) to this call, which happens at the end of
    /// `shell::update`'s handling of it. One line per sample, prefixed
    /// `input` so the driver can separate this stream from the `view`/
    /// `tick`/`env`/`echo` lines that can share the same log file
    /// (RFC-017 PR-017-G response 156: **the confirmed sample count for
    /// R9's stopping/reporting rule is `grep -c '^input ' <log>`, never
    /// `wc -l`** -- once `tick`/`env` lines exist alongside `input`
    /// ones, a bare line count silently stops meaning "samples received."
    /// Do not substitute `Measurement::received` for this either: it
    /// counts arrivals at this handler, the dispatched-side quantity R9
    /// exists to distrust, not the on-disk, survives-a-crash ground
    /// truth the grep gives. The same rule applies to `echo` lines,
    /// PR-A1-D's own addition: **`grep -c '^echo ' <log>`**, checked
    /// against `grep -c '^input ' <log>` -- `Measurement::is_done`
    /// already requires `pending_echo` to be empty before a `TerminalFlood`
    /// run exits, but the on-disk count is the one that survives a crash
    /// or a `kill -9`, which `is_done`'s in-memory check does not.
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

    /// RFC-017 Amendment 1, PR-A1-D, response 209's required fix: a fresh,
    /// never-reused marker string per send, written to the pty instead of
    /// a bare repeated character. `"j{index}"` -- keeps `j` as a visible
    /// prefix (this stays what a real routed `j` keystroke would look
    /// like echoed) while the numeric suffix makes every send's marker
    /// distinct from every other send's, which is the property that
    /// matters: substring presence of a *new* marker cannot be confused
    /// with an *old* marker's reappearance the way two occurrences of a
    /// single repeated character can. See the `pending_echo` field's own
    /// doc for the redraw-duplication finding this replaces.
    pub fn next_echo_marker(&mut self) -> String {
        let marker = format!("j{}", self.next_marker_index);
        self.next_marker_index += 1;
        marker
    }

    /// Records that `MeasuredTerminalInput` has just written `marker` to
    /// the pty, and that this send's own echo has not yet been observed.
    /// Paired with [`Self::record_input`] (called right after this, from
    /// the same handler) so every send produces two samples from one
    /// `sent_at`: `input` (dispatch plus the pty `write(2)`, unchanged
    /// since PR-017-G) and, later, `echo` (the full keystroke-to-grid-
    /// visible interval `NFR-PERF-004` actually names).
    pub fn note_measured_send(&mut self, sent_at: Instant, marker: String) {
        self.pending_echo.push_back((sent_at, marker));
    }

    /// A 1ms floor between real grid checks -- see [`Self::should_check_echo`].
    const ECHO_CHECK_INTERVAL: Duration = Duration::from_millis(1);

    /// The caller's guard against computing the (real, `O(grid)`)
    /// rendered text on every single wake. **Not just "is anything
    /// pending"** -- an earlier version of this method was exactly that,
    /// and it was not enough: under a genuine flood the reader can wake
    /// hundreds of thousands of times per second (measured: 504,712 in
    /// 2s headlessly, with a bare `poll()` keeping up cleanly at that
    /// rate), and while *any* send is outstanding, "pending, so check"
    /// alone still means computing `rendered_text()` on every one of
    /// those wakes for as long as the send stays outstanding -- which
    /// starved the reader of CPU time itself, extended how long sends
    /// stayed outstanding, and compounded into a headless benchmark
    /// completing only 2 of 200 samples in 25 real seconds. A 1ms
    /// wall-clock floor between checks caps this at ~1,000 checks/sec
    /// regardless of wake volume -- negligible bias (at most ~1ms of
    /// detection-delay jitter on figures this slice otherwise measures
    /// in milliseconds-to-tens-of-milliseconds) against a large,
    /// necessary reduction in redundant work.
    pub fn should_check_echo(&self) -> bool {
        if self.pending_echo.is_empty() {
            return false;
        }
        match self.last_echo_check {
            None => true,
            Some(at) => at.elapsed() >= Self::ECHO_CHECK_INTERVAL,
        }
    }

    /// Called from `handle_terminal_woke`, guarded by
    /// [`Self::should_check_echo`], with the grid's current rendered
    /// text. Checks **every** pending send's own marker independently
    /// (not just the oldest) -- markers are not ordered the way occurrence
    /// counts were, so an out-of-order arrival must not head-of-line-block
    /// a later marker that already landed. Logs one `echo`-prefixed sample
    /// per marker found, oldest-`sent_at`-first for log readability, and
    /// removes it from the pending queue.
    ///
    /// **The accumulated input line is never cleared, deliberately --
    /// response 209's second caution ("decide what happens as the line
    /// grows... pick one and state it") is answered by letting it grow
    /// for the run's own bounded lifetime.** A `Ctrl+U` (`VKILL`) clear
    /// was built and reverted: it would have added a fourth production
    /// `TerminalPane::write_input` call site, and
    /// `write_terminal_input_has_exactly_the_three_named_production_call_sites`
    /// (`shell::tests`, RFC-018's own "one PTY ingress" enumeration)
    /// exists specifically to catch that -- exactly the trap its own doc
    /// names ("a second inline `.write_input(` call in a new message
    /// arm"). A diagnostic-only line-length concern is not worth
    /// expanding a security-reviewed enumeration for without review; a
    /// bounded run (`target`, default 1,100 sends) times a short marker
    /// is a bounded total line length, which is the accepted tradeoff
    /// this method makes instead.
    pub fn check_echo_visible(&mut self, rendered_text: &str) {
        self.last_echo_check = Some(Instant::now());
        let mut still_pending = VecDeque::with_capacity(self.pending_echo.len());
        while let Some((sent_at, marker)) = self.pending_echo.pop_front() {
            if rendered_text.contains(&marker) {
                let elapsed = Instant::now().saturating_duration_since(sent_at);
                let _ = writeln!(self.log, "echo {}", elapsed.as_micros());
            } else {
                still_pending.push_back((sent_at, marker));
            }
        }
        self.pending_echo = still_pending;
    }

    pub fn is_done(&self) -> bool {
        match self.criterion {
            Criterion::Startup => self.startup_recorded,
            Criterion::Typing | Criterion::ModeSwitch => self.received >= self.target,
            // RFC-017 Amendment 1, PR-A1-D: also requires every dispatched
            // send's echo to have actually been observed and logged --
            // exiting the instant `received` hits target (as the other
            // criteria do) would race the last few in-flight echoes and
            // silently under-report the `echo` sample count relative to
            // the `input` one, the exact kind of quiet loss response 157
            // required a confirmed-on-disk count to catch.
            Criterion::TerminalFlood => {
                self.received >= self.target && self.pending_echo.is_empty()
            }
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
            started_at: Instant::now(),
            pending_echo: VecDeque::new(),
            last_echo_check: None,
            next_marker_index: 0,
        }
    }
}

/// RFC-017 PR-017-G response 156: "a one-line environment capture (free
/// RAM, swap in use) at run start, so a future reader can tell at a
/// glance whether a recorded figure was taken on a sane machine." Read
/// directly from `/proc/meminfo` (Linux-only, matching this crate's
/// existing Linux-specific runtime); silently a no-op if unreadable --
/// an environment snapshot failing to write must never block the
/// measurement it exists to contextualize.
fn write_environment_snapshot(log: &mut std::fs::File) {
    let Ok(contents) = std::fs::read_to_string("/proc/meminfo") else {
        return;
    };
    let Some((mem_available_kib, swap_used_kib)) = parse_meminfo_snapshot(&contents) else {
        return;
    };
    let _ = writeln!(
        log,
        "env mem_available_kib={mem_available_kib} swap_used_kib={swap_used_kib}"
    );
}

/// Pure parsing half of [`write_environment_snapshot`], factored out so
/// it is testable against a fixed string rather than the real,
/// machine-dependent `/proc/meminfo`. `None` if any of the three fields
/// this needs is missing or unparsable.
fn parse_meminfo_snapshot(contents: &str) -> Option<(u64, u64)> {
    let field = |name: &str| -> Option<u64> {
        contents.lines().find_map(|line| {
            line.strip_prefix(name)
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse().ok())
        })
    };
    let mem_available_kib = field("MemAvailable:")?;
    let swap_total_kib = field("SwapTotal:")?;
    let swap_free_kib = field("SwapFree:")?;
    Some((
        mem_available_kib,
        swap_total_kib.saturating_sub(swap_free_kib),
    ))
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
