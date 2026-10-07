//! RFC-057 PR-057-A: `NFR-PERF-003`, measured for the first time, against the
//! editor **as it stands** — before any rendering change (D11).
//!
//! **No product code changed to make this possible.** Everything here drives
//! the real `update`, the real `view` and iced's own text-layout engine.
//!
//! # What is measured, and what is not
//!
//! One keystroke in the editor costs, today, three things, and they are
//! reported **separately and summed** (the decomposition `measurement.rs`
//! established, for the reason it gives: one degenerate combined figure hides
//! where the time is):
//!
//! | Stage | What it is | How |
//! | --- | --- | --- |
//! | `update` | input arriving to the edit applied: `handle_editor_key` copies the document out, `apply_edit_key` splits it into a `Vec<String>` of every line and joins it again, `replace_text` installs the result | the real `update`, via the real router |
//! | `view` | building the `Element` tree | the real `view` |
//! | `layout` | **the `text` widget re-shaping and laying out the whole changed string**, which iced does whenever a `text` widget's content changes | iced's own `Paragraph::with_text` (`iced_graphics`, the engine the widget calls) on the same string, font, size and width |
//!
//! **Not measured: painting.** Rasterising or GPU-drawing the laid-out text and
//! presenting the frame are outside any headless measurement, the same
//! disclosed limit `measurement.rs` carries. The figures below are therefore
//! **a lower bound on what a user waits**: if the lower bound already misses
//! the budget, the real figure does too; if it met it, painting would still
//! have to be added.
//!
//! The fixture is [`fixture_text`]: 100 000 lines, deterministic, under the 4 MiB
//! editable cap, and asserted to be all of that by an ordinary test so it
//! cannot drift.

use super::*;

use iced::advanced::text::{Alignment, LineHeight, Paragraph as _, Shaping, Text, Wrapping};
use iced::{Pixels, Size};

pub(super) const FIXTURE_LINES: usize = 100_000;

/// The height the editor's text widget is given: the content area of the
/// 1250 x 1378 window the captures use, below the header and the chrome lines.
const VIEWPORT_HEIGHT: f32 = 1000.0;

/// The measured document: **100 000 lines**, ~3.3 MiB, ordinary source-shaped
/// text with varying line lengths, generated from the line number alone so it is
/// byte-identical everywhere. Committed as a generator rather than as a 3.5 MiB
/// blob: a reviewer runs the same function and gets the same bytes, and the test
/// below pins its size, its line count and a checksum.
pub(super) fn fixture_text() -> String {
    fixture_prefix(FIXTURE_LINES)
}

/// The first `lines` lines of the fixture — the same bytes, cut short — for the
/// scaling table.
pub(super) fn fixture_prefix(lines: usize) -> String {
    const WORDS: [&str; 16] = [
        "let", "value", "return", "match", "self", "state", "buffer", "cursor", "render", "layout",
        "widget", "column", "offset", "length", "result", "option",
    ];
    let mut text = String::with_capacity(lines * 36);
    for line in 0..lines {
        // Indentation and word count vary with the line, so lines are not the
        // same width — a measurement over identical lines would flatter a cache.
        let indent = (line % 4) * 2;
        let count = 1 + (line * 7) % 4;
        text.push_str(&" ".repeat(indent));
        for word in 0..count {
            if word > 0 {
                text.push(' ');
            }
            text.push_str(WORDS[(line * 3 + word * 5) % WORDS.len()]);
        }
        text.push_str(&format!(" // line {line}"));
        if line + 1 < lines {
            text.push('\n');
        }
    }
    text
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The fixture is what the evidence says it is: 100 000 lines, under the cap
/// the editor enforces, and byte-identical everywhere.
#[test]
fn the_baseline_fixture_is_100_000_lines_under_the_editable_cap_and_deterministic() {
    let text = fixture_text();

    assert_eq!(text.split('\n').count(), FIXTURE_LINES);
    assert!(
        (text.len() as u64) < tekstide_core::content::DEFAULT_MAX_EDITABLE_BYTES,
        "the fixture must be openable: {} bytes against the cap",
        text.len()
    );
    assert_eq!(text, fixture_text(), "generated from the line number alone");
    assert_eq!(
        (text.len(), fnv1a(text.as_bytes())),
        FIXTURE_IDENTITY,
        "the fixture changed: every baseline number taken against the old one is void"
    );
}

/// Bytes and FNV-1a of [`fixture_text`], pinned.
const FIXTURE_IDENTITY: (usize, u64) = (3_307_639, 12_832_468_849_609_611_213);

/// A named keystroke scenario: its label, how many keystrokes, the key, and where
/// the cursor starts.
type Scenario = (
    &'static str,
    usize,
    fn() -> iced::keyboard::Key,
    tekstide_core::content::TextCursor,
);

struct Stage {
    update: std::time::Duration,
    view: std::time::Duration,
    layout: std::time::Duration,
}

fn open_fixture(label: &str, text: &str) -> State {
    let (mut state, _dir) = state_with_an_open_document(label, text);
    // The body region the 1250 x 1378 window gives the editor, as the layout
    // engine would report it.
    state.editor_viewport = Some(Size::new(880.0, VIEWPORT_HEIGHT));
    state
}

fn press_key(state: &mut State, key: iced::keyboard::Key) {
    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key,
        modifiers: iced::keyboard::Modifiers::empty(),
    };
    let proof = crate::input::ModalAbsent::check(&state.modal).expect("no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
    let _ = super::super::update(state, Message::Input(routed));
}

/// The text widget's own layout of the body string: iced's `Paragraph`, given
/// the same string, font, size, line height and width the widget is given, **and
/// the height the widget is given**: the editor's column sits in a container of
/// the window's height, so the text widget's limits are finite, and cosmic-text
/// shapes only what that height shows. A first version of this harness passed
/// an unbounded height and measured 0.75 s a keystroke; the live app answered
/// in under 100 ms, which is how the mistake was found.
fn lay_out(body: &str, size: f32, width: f32, height: f32) -> usize {
    let paragraph = iced::advanced::graphics::text::Paragraph::with_text(Text {
        content: body,
        bounds: Size::new(width, height),
        size: Pixels(size),
        line_height: LineHeight::default(),
        font: crate::theme::ui_font(),
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::default(),
        wrapping: Wrapping::default(),
    });
    paragraph.min_bounds().height as usize
}

/// The whole document as one string -- what PR-057-A's baseline handed to one
/// `text` widget, and what the editor **no longer does** (RFC-057 Q1). Kept only
/// for the labelled reference scenario.
fn whole_document(state: &State) -> String {
    state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| project.content_workspace().active_document())
        .map(|document| document.text().to_string())
        .expect("an active document")
}

/// The rows the editor draws right now: its window, one string per row.
pub(super) fn drawn_rows_for_test(state: &State) -> Vec<String> {
    drawn_rows(state)
}

fn drawn_rows(state: &State) -> Vec<String> {
    let capacity = super::super::editor_window_capacity(state);
    let document = state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| project.content_workspace().active_document())
        .expect("an active document");
    crate::surface::editor::drawn_rows(document, capacity)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// One keystroke against the editor **as built now**: rows bounded by the
/// viewport. The `layout` stage lays out each drawn row whose string differs from
/// the same row last frame, which is what a `text` widget does
/// (`Plain::update`: an unchanged string is only compared).
fn one_keystroke(state: &mut State, key: iced::keyboard::Key, width: f32) -> Stage {
    let previous_rows = drawn_rows(state);
    let started = std::time::Instant::now();
    press_key(state, key);
    let update = started.elapsed();

    let started = std::time::Instant::now();
    let element = super::super::view(state);
    let view = started.elapsed();
    drop(element);

    let rows = drawn_rows(state);
    let size = state.theme.font_size_body();
    let pitch = crate::surface::editor::row_pitch(size);
    let started = std::time::Instant::now();
    for (index, row) in rows.iter().enumerate() {
        if previous_rows.get(index) != Some(row) {
            let _ = lay_out(row, size, width, pitch);
        }
    }
    let layout = started.elapsed();

    Stage {
        update,
        view,
        layout,
    }
}

/// **The reference the editor must never return to** (RFC-057 Q1): the same
/// keystroke with the whole document handed to one `text` widget laid out with no
/// height bound -- what a scrollable body, or anything letting the widget see
/// past the visible height, would pay. ~745 ms a keystroke at 100 000 lines.
fn one_keystroke_whole_body_unbounded(
    state: &mut State,
    key: iced::keyboard::Key,
    width: f32,
) -> Stage {
    let previous_body = whole_document(state);
    let started = std::time::Instant::now();
    press_key(state, key);
    let update = started.elapsed();
    let body = whole_document(state);
    let started = std::time::Instant::now();
    if body != previous_body {
        let _ = lay_out(&body, state.theme.font_size_body(), width, f32::INFINITY);
    }
    let layout = started.elapsed();
    Stage {
        update,
        view: std::time::Duration::ZERO,
        layout,
    }
}

fn character(text: &str) -> iced::keyboard::Key {
    iced::keyboard::Key::Character(text.into())
}

fn named(key: iced::keyboard::key::Named) -> iced::keyboard::Key {
    iced::keyboard::Key::Named(key)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let index = ((sorted.len() as f64) * p).ceil() as usize;
    sorted[index.saturating_sub(1).min(sorted.len() - 1)]
}

fn millis(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn report(label: &str, samples: &[Stage]) -> String {
    let column = |select: fn(&Stage) -> std::time::Duration| {
        let mut values: Vec<f64> = samples.iter().map(|s| millis(select(s))).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (
            percentile(&values, 0.50),
            percentile(&values, 0.95),
            percentile(&values, 0.99),
        )
    };
    let mut totals: Vec<f64> = samples
        .iter()
        .map(|s| millis(s.update + s.view + s.layout))
        .collect();
    totals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (u, v, l) = (
        column(|s| s.update),
        column(|s| s.view),
        column(|s| s.layout),
    );
    format!(
        "{label} ({n} keystrokes)\n  stage           p50 ms     p95 ms     p99 ms\n  update      {:>10.3} {:>10.3} {:>10.3}\n  view        {:>10.3} {:>10.3} {:>10.3}\n  layout      {:>10.3} {:>10.3} {:>10.3}\n  SUM         {:>10.3} {:>10.3} {:>10.3}   (budget: p95 <= 16, p99 <= 33)\n",
        u.0,
        u.1,
        u.2,
        v.0,
        v.1,
        v.2,
        l.0,
        l.1,
        l.2,
        percentile(&totals, 0.50),
        percentile(&totals, 0.95),
        percentile(&totals, 0.99),
        n = samples.len(),
    )
}

/// **RFC-057 Q1, with PR-057-A's reference number beside it.** The editor draws a
/// window of a 100 000-line file, not the file: what reaches the text widgets per
/// frame is a few dozen rows, a tiny fraction of the 3.3 MB document. Laid out
/// with the whole file in one widget and no height bound, the same keystroke cost
/// **~745 ms** (the labelled reference in the measurement below, against a 16 ms
/// budget). Asserted on bytes, not on a clock, so it cannot depend on the machine.
#[test]
fn the_editor_draws_a_window_of_a_100_000_line_file_and_never_the_file() {
    let text = fixture_text();
    let state = open_fixture("editor-baseline-q1", &text);

    let rows = drawn_rows(&state);
    let drawn: usize = rows.iter().map(String::len).sum();

    assert_eq!(rows.len(), super::super::editor_window_capacity(&state));
    assert!(
        rows.len() < 100,
        "a window, not 100 000 rows: {}",
        rows.len()
    );
    assert!(
        drawn * 500 < text.len(),
        "{drawn} bytes reach the widgets against a {}-byte document: the whole file must never be handed to a widget (~745 ms a keystroke, PR-057-A)",
        text.len()
    );
    // And the view really builds from them: it does not panic on the huge document.
    drop(super::super::view(&state));
}

/// The harness runs end to end on a small document and its figures are sane:
/// every stage is measured, and the edit actually landed. Runs in the ordinary
/// suite so the measurement cannot rot; **asserts nothing about speed**.
#[test]
fn the_baseline_harness_measures_every_stage_of_a_real_keystroke() {
    let mut state = open_fixture("editor-baseline-smoke", "one\ntwo\nthree");
    let samples: Vec<Stage> = (0..5)
        .map(|_| one_keystroke(&mut state, character("x"), 800.0))
        .collect();

    assert_eq!(samples.len(), 5);
    assert_eq!(
        active_document_text(&state),
        "xxxxxone\ntwo\nthree",
        "the keys edited the document"
    );
    assert!(samples.iter().all(|s| s.update > std::time::Duration::ZERO));
    assert!(samples.iter().all(|s| s.layout > std::time::Duration::ZERO));
}

/// **The measurement.** Ignored in the ordinary suite because it is slow and
/// meaningless in a debug build; run it exactly as the evidence says:
///
/// `cargo test --release -p tekstide editor_typing_latency_baseline -- --ignored --nocapture`
#[test]
#[ignore = "a measurement, not a check: release build, see the module doc"]
fn editor_typing_latency_baseline_100_000_lines() {
    let text = fixture_text();
    let width = 880.0;
    let mut out = String::new();
    out.push_str(&format!(
        "fixture: {} lines, {} bytes; body font {} px; layout width {} px\n",
        FIXTURE_LINES,
        text.len(),
        crate::theme::Theme::default().font_size_body(),
        width
    ));
    use iced::keyboard::key::Named;
    let last_line = FIXTURE_LINES - 1;
    let scenarios: [Scenario; 4] = [
        (
            "typing a character at the start of the file",
            30,
            || character("x"),
            tekstide_core::content::TextCursor { line: 0, column: 0 },
        ),
        (
            "typing a character at the end of the file",
            30,
            || character("x"),
            tekstide_core::content::TextCursor {
                line: last_line,
                column: 0,
            },
        ),
        (
            "Enter in the middle of the file",
            30,
            || named(Named::Enter),
            tekstide_core::content::TextCursor {
                line: FIXTURE_LINES / 2,
                column: 4,
            },
        ),
        (
            "an arrow key (the cursor moves; the text does not)",
            60,
            || named(Named::ArrowRight),
            tekstide_core::content::TextCursor {
                line: FIXTURE_LINES / 2,
                column: 0,
            },
        ),
    ];
    for (label, keystrokes, key, cursor) in scenarios {
        let mut state = open_fixture("editor-baseline", &text);
        let _ = state.app_shell.set_active_project_cursor(cursor);
        for _ in 0..3 {
            let _ = one_keystroke(&mut state, key(), width); // warm-up, discarded
        }
        let samples: Vec<Stage> = (0..keystrokes)
            .map(|_| one_keystroke(&mut state, key(), width))
            .collect();
        out.push_str(&report(label, &samples));
    }
    // For reference only: the same keystroke if the text widget were laid out
    // with **no** height bound — what a scrollable body, or any design that lets
    // the widget see the whole file, would pay. The shipped editor does not.
    {
        let mut state = open_fixture("editor-baseline-unbounded", &text);
        for _ in 0..2 {
            let _ = one_keystroke_whole_body_unbounded(&mut state, character("w"), width);
        }
        let samples: Vec<Stage> = (0..12)
            .map(|_| one_keystroke_whole_body_unbounded(&mut state, character("x"), width))
            .collect();
        out.push_str(&report(
            "REFERENCE, which the editor must never return to: the whole file in one widget, unbounded layout height, typing at the start",
            &samples,
        ));
    }
    out.push_str("\nscaling: typing a character at the end, by file length\n");
    for lines in [1_000usize, 10_000, 100_000] {
        let text = fixture_prefix(lines);
        let mut state = open_fixture("editor-baseline-scaling", &text);
        let _ = state
            .app_shell
            .set_active_project_cursor(tekstide_core::content::TextCursor {
                line: lines - 1,
                column: 0,
            });
        for _ in 0..3 {
            let _ = one_keystroke(&mut state, character("w"), width);
        }
        let samples: Vec<Stage> = (0..30)
            .map(|_| one_keystroke(&mut state, character("x"), width))
            .collect();
        out.push_str(&report(
            &format!("{lines} lines, {} bytes", text.len()),
            &samples,
        ));
    }
    println!("{out}");
}

/// Writes the fixture to the path in `TEKSTIDE_BASELINE_FIXTURE_OUT`, so the
/// live capture opens **the same bytes** the measurement used.
#[test]
#[ignore = "writes a file where the environment says; used by the live capture"]
fn write_the_baseline_fixture() {
    let path =
        std::env::var("TEKSTIDE_BASELINE_FIXTURE_OUT").expect("set TEKSTIDE_BASELINE_FIXTURE_OUT");
    std::fs::write(path, fixture_text()).unwrap();
}

/// What the runtime delivered between keystrokes, and what it cost.
struct BurstRun {
    stages: Vec<Stage>,
    delivery: std::time::Duration,
    notices: usize,
    scans_run: usize,
    scans_applied: usize,
    files_written: usize,
    /// Wall time of the whole loop, and of the part after the burst thread had finished.
    elapsed: std::time::Duration,
    after_burst: std::time::Duration,
    notices_after_burst: usize,
}

/// RFC-026 slice C, `REQ-FILE-004`: keystrokes measured in the same process as a watched burst,
/// with the same stages as the RFC-057 baseline. The burst writes `files` files into the project
/// root (always watched) at one file per `pace`, from a thread of its own. Between keystrokes this
/// delivers what the runtime's subscriptions would, in the order they would arrive: each watch
/// notice, a drain tick when a window is due, and each explorer scan result, all through `update`,
/// and it runs each pending explorer scan on a worker thread. Delivery is timed apart from the
/// keystroke, so the keystroke figures are the editor's own. Runs until the burst has finished and
/// settled, and at least `min_keystrokes` have been measured.
fn keystrokes_under_a_watched_burst(
    state: &mut State,
    files: usize,
    pace: std::time::Duration,
    min_keystrokes: usize,
    width: f32,
    watched: bool,
    // RFC-065 D13: generalized from `Option<(PathBuf, String)>` to a `Vec`, the minimal
    // change the harness's own doc requires ("add a condition... do not rebuild it") to let
    // N open documents be rewritten during the burst instead of only one.
    touch_documents: Vec<(std::path::PathBuf, String)>,
) -> BurstRun {
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};
    let project_id = state
        .app_shell
        .state()
        .active_project_id()
        .cloned()
        .expect("the fixture has an active project");
    let root = state
        .app_shell
        .state()
        .project(&project_id)
        .expect("the project is open")
        .canonical_root_path()
        .clone();
    // The unwatched control removes the project's owner first: no watch, no notices, no drains,
    // no scans. The same keystrokes, the same stages, in the same process.
    if !watched {
        state.project_watches.remove(&project_id);
    }
    let generation = state
        .project_watches
        .get(&project_id)
        .map_or(0, |watch| watch.generation);
    let (notice_tx, notice_rx) = channel();
    if let Some(events) = state
        .project_watches
        .get(&project_id)
        .and_then(|watch| watch.watcher.events())
        .and_then(|slot| slot.take())
    {
        std::thread::spawn(move || {
            let mut events = events;
            while let Some(notice) = events.wait_for_notice() {
                if notice_tx.send(notice).is_err() {
                    break;
                }
            }
        });
    }

    let burst = (files > 0).then(|| {
        let root = root.clone();
        std::thread::spawn(move || {
            let mut written = 0;
            // RFC-026 D8, generalized by RFC-065 D13: every touched document (one, before
            // this slice; N, with it) is rewritten every hundred files, each alternating
            // between two versions, so each rewrite is a real change each one's own refresh
            // must read.
            let mut versions: Vec<(std::path::PathBuf, [String; 2], usize)> = touch_documents
                .into_iter()
                .map(|(path, text)| {
                    let other = format!("external edit\n{text}");
                    (path, [text, other], 0usize)
                })
                .collect();
            for index in 0..files {
                if std::fs::write(root.join(format!("burst-{index:05}.txt")), b"x").is_ok() {
                    written += 1;
                }
                if index % 100 == 99 {
                    for (path, versions, turn) in versions.iter_mut() {
                        let _ = std::fs::write(&*path, versions[*turn % 2].as_bytes());
                        *turn += 1;
                    }
                }
                std::thread::sleep(pace);
            }
            written
        })
    });

    let (scan_tx, scan_rx) = channel::<Message>();
    let mut launched: std::collections::HashSet<(std::path::PathBuf, u64)> = Default::default();
    let mut run = BurstRun {
        stages: Vec::new(),
        delivery: Duration::ZERO,
        notices: 0,
        scans_run: 0,
        scans_applied: 0,
        files_written: 0,
        elapsed: Duration::ZERO,
        after_burst: Duration::ZERO,
        notices_after_burst: 0,
    };
    let mut burst_finished_at: Option<Instant> = None;
    let mut notices_at_burst_end = 0usize;
    let mut last_tick = Instant::now();
    let mut burst_done = burst.is_none();
    let mut quiet_since: Option<Instant> = None;
    let mut keystrokes = 0usize;

    // One pass of what the runtime delivers between two keystrokes.
    let deliver = |state: &mut State,
                   run: &mut BurstRun,
                   last_tick: &mut Instant,
                   launched: &mut std::collections::HashSet<(std::path::PathBuf, u64)>|
     -> bool {
        let started = Instant::now();
        let mut progressed = false;
        while let Ok(notice) = notice_rx.try_recv() {
            run.notices += 1;
            progressed = true;
            let _ = super::super::update(
                state,
                Message::ProjectWatchNotice {
                    project_id: project_id.clone(),
                    generation,
                    notice,
                },
            );
        }
        if state
            .project_watches
            .get(&project_id)
            .is_some_and(|watch| watch.watcher.has_pending_scans())
            && last_tick.elapsed() >= tekstide_core::project::SCAN_WINDOW
        {
            *last_tick = Instant::now();
            progressed = true;
            let _ = super::super::update(state, Message::WatchDrainTick);
        }
        for request in state
            .app_shell
            .state()
            .project(&project_id)
            .map(|project| project.explorer_scan_requests())
            .unwrap_or_default()
        {
            if launched.insert((request.path().to_path_buf(), request.generation())) {
                run.scans_run += 1;
                progressed = true;
                let scan_tx = scan_tx.clone();
                let project_id = project_id.clone();
                std::thread::spawn(move || {
                    let completed = request.run();
                    let _ = scan_tx.send(Message::ExplorerScanFinished {
                        project_id,
                        completed,
                    });
                });
            }
        }
        while let Ok(message) = scan_rx.try_recv() {
            run.scans_applied += 1;
            progressed = true;
            let _ = super::super::update(state, message);
        }
        run.delivery += started.elapsed();
        progressed
    };

    let started_at = Instant::now();
    loop {
        let progressed = deliver(state, &mut run, &mut last_tick, &mut launched);
        if !burst_done {
            burst_done = burst.as_ref().is_some_and(|handle| handle.is_finished());
            if burst_done {
                burst_finished_at = Some(Instant::now());
                notices_at_burst_end = run.notices;
            }
        }
        if started_at.elapsed() > Duration::from_secs(120) {
            break;
        }
        if keystrokes >= min_keystrokes && burst_done {
            // The burst has finished and enough keystrokes are measured: stop typing, and
            // deliver until nothing is left, then stop.
            let idle = !progressed
                && !state
                    .project_watches
                    .get(&project_id)
                    .is_some_and(|watch| watch.watcher.has_pending_scans());
            if idle {
                let since = *quiet_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= Duration::from_millis(300) {
                    break;
                }
            } else {
                quiet_since = None;
            }
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }
        run.stages.push(one_keystroke(state, character("x"), width));
        keystrokes += 1;
    }

    run.elapsed = started_at.elapsed();
    run.notices_after_burst = run.notices - notices_at_burst_end;
    if let Some(finished) = burst_finished_at {
        run.after_burst = finished.elapsed();
    }
    if let Some(handle) = burst {
        run.files_written = handle.join().unwrap_or(0);
    }
    run
}

/// The harness runs end to end on a small document: a real burst reaches the watch, becomes
/// notices, drains into explorer scans, and the scan results are applied, while keystrokes
/// still edit the document. Runs in the ordinary suite; asserts nothing about speed.
#[test]
fn the_watched_burst_harness_delivers_the_burst_end_to_end() {
    let (mut state, _dir) = state_with_an_open_document("editor-watched-smoke", "one\ntwo");
    state.editor_viewport = Some(Size::new(880.0, VIEWPORT_HEIGHT));

    let run = keystrokes_under_a_watched_burst(
        &mut state,
        100,
        std::time::Duration::from_micros(500),
        5,
        880.0,
        true,
        Vec::new(),
    );

    assert_eq!(run.files_written, 100, "the burst wrote every file");
    assert!(run.notices > 0, "the watch reported the burst");
    assert!(
        run.scans_applied >= 1,
        "a drained window reached the explorer and its result was applied"
    );
    assert!(
        active_document_text(&state).starts_with("x"),
        "keystrokes still edited the document during the burst"
    );
}

/// `N`, RFC-065 D7/D13's own open-document count for [`WatchCondition::BurstWithNDocuments`]
/// -- comfortably below the real `open_document_limit` default of 20 (D4), high enough to
/// show real multiplicative scaling without the round itself taking unreasonably long.
const BURST_N_DOCUMENTS: usize = 10;

/// The conditions of the watched-burst measurement, in one process under one load.
#[derive(Clone, Copy, Debug)]
enum WatchCondition {
    /// The control: the project's owner removed, so no watch exists at all.
    Unwatched,
    /// The project watched, nothing happening.
    WatchedIdle,
    /// Watched while 1,000 files are written into the project root. The open document is not
    /// touched, so this is the pipeline as it was before D8.
    BurstBeforeD8,
    /// The same burst, and the open document is rewritten every hundred files. This is the
    /// pipeline with D8: each drain tick asks the document about its file.
    BurstWithD8,
    /// RFC-065 D7/D13: the same burst, with [`BURST_N_DOCUMENTS`] documents open (not one)
    /// and every one of them rewritten every hundred files -- the open set's own
    /// multiplication of the per-document refresh (measurement 9), on the harness D13 says
    /// to reuse rather than rebuild. The paired difference against `BurstBeforeD8`, divided
    /// by the `BurstWithD8` vs `BurstBeforeD8` difference, is the real scaling factor: close
    /// to `N` if the cost is what measurement 9 assumed (one whole-file read per open
    /// document per touched drain window), not some other shape.
    BurstWithNDocuments,
}

impl WatchCondition {
    fn description(self) -> &'static str {
        match self {
            Self::Unwatched => "unwatched (control)",
            Self::WatchedIdle => "watched, nothing happening",
            Self::BurstBeforeD8 => "burst into the root (before D8)",
            Self::BurstWithD8 => "burst, open document rewritten (with D8)",
            Self::BurstWithNDocuments => "burst, N open documents rewritten (RFC-065 D7/D13)",
        }
    }
}

/// The keystroke totals of one run, sorted, in milliseconds.
fn sorted_totals_ms(stages: &[Stage]) -> Vec<f64> {
    let mut totals: Vec<f64> = stages
        .iter()
        .map(|s| millis(s.update + s.view + s.layout))
        .collect();
    totals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    totals
}

/// **REQ-FILE-004, the pre-D8 control and the D8 before-and-after, in one frame** (review 462, and
/// the D8 plan), **plus RFC-065 D7/D13's own N-open-documents condition**, reusing this harness
/// unchanged rather than rebuilding it. The RFC-057 100,000-line fixture, the character-at-the-start
/// keystroke, and five conditions in one process. The **D8 cost is the paired difference between
/// `BurstBeforeD8` and `BurstWithD8`**: the same burst into the root, with and without the open
/// document being rewritten, in the same round, so everything else is held constant. **D13's own
/// figure is that same D8 cost against `BurstWithNDocuments`'s**: if N documents cost about N times
/// one document's own refresh, the ratio of the two paired differences should land near N. The
/// control measures the watcher against no watcher at all.
///
/// The conditions run in a 5 by 5 Latin square over five rounds, so each condition takes each
/// position in the run once. Every run is a fresh project. The figure that matters is the paired
/// difference within a round, not any absolute number.
///
/// `cargo test --release -p tekstide editor_typing_latency_under_a_watched_burst -- --ignored --nocapture`
#[test]
#[ignore = "a measurement, not a check: release build, see the module doc"]
fn editor_typing_latency_under_a_watched_burst() {
    let text = fixture_text();
    let width = 880.0;
    use WatchCondition::{
        BurstBeforeD8 as R, BurstWithD8 as D, BurstWithNDocuments as N, Unwatched as U,
        WatchedIdle as I,
    };
    let orders = [
        [U, R, D, N, I],
        [R, D, N, I, U],
        [D, N, I, U, R],
        [N, I, U, R, D],
        [I, U, R, D, N],
    ];
    println!(
        "fixture: {} lines, {} bytes; body font {} px; layout width {} px; keystroke: character at the start; budget NFR-PERF-003: p95 <= 16 ms, p99 <= 33 ms",
        FIXTURE_LINES,
        text.len(),
        crate::theme::Theme::default().font_size_body(),
        width
    );
    // p95 of the keystroke total, per round, per condition (indexed by the enum's order).
    let index = |condition: WatchCondition| match condition {
        WatchCondition::Unwatched => 0,
        WatchCondition::WatchedIdle => 1,
        WatchCondition::BurstBeforeD8 => 2,
        WatchCondition::BurstWithD8 => 3,
        WatchCondition::BurstWithNDocuments => 4,
    };
    let mut p95 = [[0.0f64; 5]; 5];
    // RFC-065 review 469's required fix: the delivery work itself (`run.delivery`, what the
    // runtime did *between* keystrokes -- notices, drains, scans, refreshes), per round per
    // condition, the same shape as `p95` above. D7's own question is whether the refresh
    // cost scales with N; p95 keystroke latency cannot see it (the refresh runs between
    // keystrokes, not during one), but this can.
    let mut delivery_ms = [[0.0f64; 5]; 5];
    let mut burst_notes = Vec::new();
    for (round, order) in orders.iter().enumerate() {
        for &condition in order {
            let (mut state, dir) = state_with_an_open_document("editor-watched-round", &text);
            state.editor_viewport = Some(Size::new(880.0, VIEWPORT_HEIGHT));
            for _ in 0..3 {
                let _ = one_keystroke(&mut state, character("w"), width);
            }
            let run = match condition {
                WatchCondition::Unwatched => keystrokes_under_a_watched_burst(
                    &mut state,
                    0,
                    std::time::Duration::ZERO,
                    200,
                    width,
                    false,
                    Vec::new(),
                ),
                WatchCondition::WatchedIdle => keystrokes_under_a_watched_burst(
                    &mut state,
                    0,
                    std::time::Duration::ZERO,
                    200,
                    width,
                    true,
                    Vec::new(),
                ),
                WatchCondition::BurstBeforeD8 => keystrokes_under_a_watched_burst(
                    &mut state,
                    1_000,
                    std::time::Duration::from_millis(1),
                    30,
                    width,
                    true,
                    Vec::new(),
                ),
                WatchCondition::BurstWithD8 => {
                    let document = std::fs::canonicalize(dir.join("file.txt"))
                        .expect("the fixture document exists");
                    keystrokes_under_a_watched_burst(
                        &mut state,
                        1_000,
                        std::time::Duration::from_millis(1),
                        30,
                        width,
                        true,
                        vec![(document, text.clone())],
                    )
                }
                WatchCondition::BurstWithNDocuments => {
                    // RFC-065 D7/D13: `BURST_N_DOCUMENTS` total, including the primary one
                    // `state_with_an_open_document` already opened -- each with real, distinct
                    // content, each opened for real through the same path the explorer uses, so
                    // the watcher's own scope (`watch_inputs`) genuinely grows to match.
                    let mut touch_documents = vec![(
                        std::fs::canonicalize(dir.join("file.txt"))
                            .expect("the fixture document exists"),
                        text.clone(),
                    )];
                    for extra in 1..BURST_N_DOCUMENTS {
                        let name = format!("extra-{extra:02}.txt");
                        let extra_text = format!("{text}\nextra document {extra}");
                        std::fs::write(dir.join(&name), &extra_text)
                            .expect("an extra fixture document should be writable");
                        state
                            .app_shell
                            .open_active_project_text_document(&name)
                            .expect("an extra fixture document should open");
                        touch_documents.push((
                            std::fs::canonicalize(dir.join(&name))
                                .expect("the extra fixture document exists"),
                            extra_text,
                        ));
                    }
                    // The keystroke measurement is still against the primary document, the
                    // same one every other condition measures, so the paired comparison holds
                    // only the open-document count different.
                    state
                        .app_shell
                        .open_active_project_text_document("file.txt")
                        .expect("switching back to the primary document must succeed");
                    let project_id = state
                        .app_shell
                        .state()
                        .active_project_id()
                        .cloned()
                        .expect("the fixture has an active project");
                    crate::shell::reconcile_project_watch(&mut state, &project_id);
                    keystrokes_under_a_watched_burst(
                        &mut state,
                        1_000,
                        std::time::Duration::from_millis(1),
                        30,
                        width,
                        true,
                        touch_documents,
                    )
                }
            };
            let totals = sorted_totals_ms(&run.stages);
            let at = index(condition);
            p95[round][at] = percentile(&totals, 0.95);
            delivery_ms[round][at] = millis(run.delivery);
            println!(
                "round {} {:<46} keystrokes {:>4}  p50 {:>6.3}  p95 {:>6.3}  p99 {:>6.3} ms",
                round + 1,
                condition.description(),
                run.stages.len(),
                percentile(&totals, 0.50),
                p95[round][at],
                percentile(&totals, 0.99),
            );
            // RFC-065 review 469: `BurstWithNDocuments` joins the two conditions this note
            // already covered -- D7's own figure needs its delivery work reported the same
            // way, not only the two conditions the D8 measurement originally cared about.
            if matches!(
                condition,
                WatchCondition::BurstBeforeD8
                    | WatchCondition::BurstWithD8
                    | WatchCondition::BurstWithNDocuments
            ) {
                let label = match condition {
                    WatchCondition::BurstWithD8 => "with D8",
                    WatchCondition::BurstWithNDocuments => "with N documents",
                    _ => "before D8",
                };
                burst_notes.push(format!(
                    "round {} {}: {} files written, {} notices, {} explorer scans applied, {:.1} ms of delivery between keystrokes (with D8/N documents this includes the document refreshes)",
                    round + 1,
                    label,
                    run.files_written,
                    run.notices,
                    run.scans_applied,
                    millis(run.delivery),
                ));
            }
        }
    }
    println!("\npaired differences from the control, per round (p95 of the keystroke total):");
    let mut d8_cost = Vec::new();
    let mut n_cost = Vec::new();
    for (round, row) in p95.iter().enumerate() {
        println!(
            "round {}: control {:.3} ms; watched-idle {:+.3}; burst before D8 {:+.3}; burst with D8 {:+.3}; burst with N docs {:+.3}; D8 cost (with minus before) {:+.3} ms; N-doc cost (N minus before) {:+.3} ms",
            round + 1,
            row[0],
            row[1] - row[0],
            row[2] - row[0],
            row[3] - row[0],
            row[4] - row[0],
            row[3] - row[2],
            row[4] - row[2],
        );
        d8_cost.push(row[3] - row[2]);
        n_cost.push(row[4] - row[2]);
    }
    let median = |values: &[f64]| {
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        }
    };
    let median_d8_cost = median(&d8_cost);
    let median_n_cost = median(&n_cost);
    println!(
        "median D8 cost over the five rounds (p95 keystroke latency, with minus before): {median_d8_cost:+.3} ms"
    );
    println!(
        "median {BURST_N_DOCUMENTS}-document cost over the five rounds (p95 keystroke latency, N minus before): {median_n_cost:+.3} ms"
    );
    // RFC-065 review 469's own finding: p95 keystroke latency is the wrong quantity for this
    // ratio -- the refresh runs *between* keystrokes, exactly where that figure cannot see
    // it, which is why this came back 1.11x against measurement 9's own assumption of
    // something near BURST_N_DOCUMENTS. Reported anyway, because "does holding N documents
    // slow typing" is a true and useful question in its own right -- just not D7's.
    if median_d8_cost.abs() > 0.05 {
        println!(
            "p95 keystroke-latency ratio ({BURST_N_DOCUMENTS}-document cost / one-document cost): {:.2}x -- answers \"does holding N documents slow typing\", not D7's own question",
            median_n_cost / median_d8_cost
        );
    } else {
        println!(
            "p95 keystroke-latency ratio not reported: the one-document (D8) cost is too close to zero to divide by meaningfully"
        );
    }
    // RFC-065 D7/D13's own figure, required at review 469: the delivery work itself (what
    // the runtime does *between* keystrokes), not p95 keystroke latency. If one document's
    // own refresh costs the D8 paired difference in delivery work, N documents multiplying
    // that cost by about N is what measurement 9 assumed -- this is the ratio that actually
    // tells whether it does.
    let mut d8_delivery_cost = Vec::new();
    let mut n_delivery_cost = Vec::new();
    for row in delivery_ms.iter() {
        d8_delivery_cost.push(row[3] - row[2]);
        n_delivery_cost.push(row[4] - row[2]);
    }
    let median_d8_delivery_cost = median(&d8_delivery_cost);
    let median_n_delivery_cost = median(&n_delivery_cost);
    println!(
        "median D8 cost over the five rounds (delivery work, with minus before): {median_d8_delivery_cost:+.3} ms"
    );
    println!(
        "median {BURST_N_DOCUMENTS}-document cost over the five rounds (delivery work, N minus before): {median_n_delivery_cost:+.3} ms"
    );
    if median_d8_delivery_cost.abs() > 0.05 {
        println!(
            "delivery-work ratio ({BURST_N_DOCUMENTS}-document cost / one-document cost): {:.2}x (near {BURST_N_DOCUMENTS}x would match measurement 9's assumption of one whole-file read per open document; this is D7's own figure)",
            median_n_delivery_cost / median_d8_delivery_cost
        );
    } else {
        println!(
            "delivery-work ratio not reported: the one-document (D8) delivery cost is too close to zero to divide by meaningfully"
        );
    }
    for note in burst_notes {
        println!("watch, {note}");
    }
}

/// Diagnostic for the D8 measurement: one run of the burst with the open document rewritten, printing
/// where the time goes. Not a figure; run by hand to explain one.
///
/// `cargo test --release -p tekstide d8_burst_diagnostic -- --ignored --nocapture`
#[test]
#[ignore = "a diagnostic, run by hand to explain a D8 figure"]
fn d8_burst_diagnostic() {
    let text = fixture_text();
    let width = 880.0;
    let (mut state, dir) = state_with_an_open_document("editor-d8-diagnostic", &text);
    state.editor_viewport = Some(Size::new(880.0, VIEWPORT_HEIGHT));
    let document = std::fs::canonicalize(dir.join("file.txt")).unwrap();
    let run = keystrokes_under_a_watched_burst(
        &mut state,
        1_000,
        std::time::Duration::from_millis(1),
        30,
        width,
        true,
        vec![(document, text.clone())],
    );
    println!(
        "loop {:.1} ms; after the burst finished {:.1} ms; keystrokes {}; notices {} ({} after the burst); scans run {}, applied {}; delivery {:.1} ms",
        millis(run.elapsed),
        millis(run.after_burst),
        run.stages.len(),
        run.notices,
        run.notices_after_burst,
        run.scans_run,
        run.scans_applied,
        millis(run.delivery),
    );
}
