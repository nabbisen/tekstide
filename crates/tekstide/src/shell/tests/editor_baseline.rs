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
//! | `view` | building the `Element` tree, which copies the whole document into the body `text` widget (`body_text`) | the real `view` |
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

/// The measured document: **100 000 lines**, ~3.3 MiB, ordinary source-shaped
/// text with varying line lengths, generated from the line number alone so it is
/// byte-identical everywhere. Committed as a generator rather than as a 3.5 MiB
/// blob: a reviewer runs the same function and gets the same bytes, and the test
/// below pins its size, its line count and a checksum.
pub(super) fn fixture_text() -> String {
    const WORDS: [&str; 16] = [
        "let", "value", "return", "match", "self", "state", "buffer", "cursor", "render", "layout",
        "widget", "column", "offset", "length", "result", "option",
    ];
    let mut text = String::with_capacity(FIXTURE_LINES * 36);
    for line in 0..FIXTURE_LINES {
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
        if line + 1 < FIXTURE_LINES {
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

struct Stage {
    update: std::time::Duration,
    view: std::time::Duration,
    layout: std::time::Duration,
}

fn open_fixture(label: &str, text: &str) -> State {
    let (state, _dir) = state_with_an_open_document(label, text);
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
/// the same string, font, size, line height and width the widget is given. The
/// height is unbounded, as it is inside the editor's column.
fn lay_out(body: &str, size: f32, width: f32) -> usize {
    let paragraph = iced::advanced::graphics::text::Paragraph::with_text(Text {
        content: body,
        bounds: Size::new(width, f32::INFINITY),
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

fn active_body(state: &State) -> String {
    state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| project.content_workspace().active_document())
        .map(crate::surface::editor::body_text)
        .expect("an active document")
}

fn one_keystroke(state: &mut State, key: iced::keyboard::Key, width: f32) -> Stage {
    // What the `text` widget compared against last frame: an unchanged body is
    // only compared, a changed one is laid out again (`Plain::update`).
    let previous_body = active_body(state);
    let started = std::time::Instant::now();
    press_key(state, key);
    let update = started.elapsed();

    let started = std::time::Instant::now();
    let element = super::super::view(state);
    let view = started.elapsed();
    drop(element);

    let body = active_body(state);
    let started = std::time::Instant::now();
    if body != previous_body {
        let _ = lay_out(&body, state.theme.font_size_body(), width);
    }
    let layout = started.elapsed();

    Stage {
        update,
        view,
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
    let scenarios: [(
        &str,
        usize,
        fn() -> iced::keyboard::Key,
        tekstide_core::content::TextCursor,
    ); 4] = [
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
    println!("{out}");
}
