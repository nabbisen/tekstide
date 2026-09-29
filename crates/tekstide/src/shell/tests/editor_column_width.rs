//! RFC-057 PR-057-C review 443, Q3: falsifies `CHAR_WIDTH_FACTOR`'s approximation
//! against real text layout, on the two character classes most likely to break
//! it -- a line of the widest ordinary characters (`W`) and one of the
//! narrowest (`i`), at the **default** face (`Font::DEFAULT`, unconfigured --
//! the same font capture `01-` in `evidence/pr-057-c/` shows is proportional).
//!
//! `columns_that_fit` decides the horizontal window's capacity from
//! `CHAR_WIDTH_FACTOR * font_size`, an approximation disclosed as such
//! (`row_pitch`'s own doc makes the same kind of approximation for height, and
//! review 443 found nothing wrong with reusing that shape -- the requirement
//! is to measure whether it holds, not to remove it). This measures the real,
//! rendered pixel width of the text the horizontal window's own arithmetic
//! says is inside it, with iced's own text-layout engine -- the same technique
//! `editor_baseline`'s harness already uses for the vertical axis -- and
//! reports whether the caret is still inside the region that was actually
//! measured, in pixels and in columns, rather than only inside the character
//! count `viewport_following` computed.
//!
//! **The owner's font ruling (2026-09-29) added a second test, held, not
//! merely reported**: the editor body's own default becomes `Font::MONOSPACE`
//! (`theme.editor_font()`), and `the_default_editor_font_is_monospace_and_
//! the_width_approximation_holds_for_it` measures that face with this same
//! harness -- `Font::MONOSPACE` gives every character the identical real
//! advance width, so the approximation cannot single out one character the
//! way it did `W` in the proportional default above. Measured, not assumed:
//! `8.400` px/char measured against `8.400` approximated, `0.0%` off, zero
//! overflow. `Font::DEFAULT`'s own failure above is unaffected -- it is still
//! what a user gets if they configure a proportional family.

use iced::advanced::text::{Alignment, LineHeight, Paragraph as _, Shaping, Text, Wrapping};
use iced::{Pixels, Size};

/// The caret element's own fixed width (`surface::editor::caret_element`'s
/// `CARET_WIDTH`, private by design -- D2: the caret is an implementation
/// detail of `view`, not a value other code should read). **Restated, not
/// verified**: if the real constant ever changes, this measurement is off by
/// the difference until someone notices and updates it here too. Small
/// relative to the overflow this file measures (a few pixels against
/// hundreds), so it does not change what the numbers say.
const CARET_WIDTH: f32 = 2.0;

fn real_width(text_for_measurement: &str, font_size: f32, font: iced::Font) -> f32 {
    let paragraph = iced::advanced::graphics::text::Paragraph::with_text(Text {
        content: text_for_measurement,
        bounds: Size::new(f32::INFINITY, font_size * 2.0),
        size: Pixels(font_size),
        line_height: LineHeight::default(),
        font,
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::default(),
        wrapping: Wrapping::default(),
    });
    paragraph.min_bounds().width
}

/// A body region this wide, at this font size -- the same figures
/// `editor_baseline`'s own harness measures against, so this test's numbers
/// are comparable to the ones already published.
const REGION_WIDTH: f32 = 880.0;
const FONT_SIZE: f32 = 14.0;

struct Falsification {
    class: &'static str,
    /// Real pixels per character, measured, against the approximation.
    measured_char_width: f32,
    approximated_char_width: f32,
    /// The largest amount, in pixels, the caret's own real position overflowed
    /// the region -- zero if it never did, across every column of the line.
    max_overflow_px: f32,
    /// The same overflow, in whole columns of the approximated width -- the
    /// number a person reads as "how far off screen."
    max_overflow_columns: f32,
}

/// Walks every column of a 300-character line built from one repeated
/// character, following the horizontal window the way a real keystroke would
/// (via [`viewport_following`]), and measures -- with real text layout -- how
/// far past `REGION_WIDTH` the caret's own real pixel position falls, if at
/// all.
fn falsify(class: &'static str, character: char, font: iced::Font) -> Falsification {
    let line: String = std::iter::repeat_n(character, 300).collect();
    let digits = 1;
    let column_capacity =
        crate::surface::editor::columns_that_fit(Some(REGION_WIDTH), FONT_SIZE, digits);
    let mut viewport = tekstide_core::content::TextViewport::default();
    let mut max_overflow_px: f32 = 0.0;
    for column in 0..=line.chars().count() {
        let cursor = tekstide_core::content::TextCursor { line: 0, column };
        viewport = crate::surface::editor::viewport_following(
            &line,
            cursor,
            viewport,
            10,
            column_capacity,
        );
        let windowed = crate::surface::editor::windowed_line(
            &line,
            viewport.first_visible_column,
            column_capacity,
        );
        let (row, offset) =
            crate::surface::editor::caret_row_position(cursor, viewport, 10, column_capacity)
                .expect("the vertical/logical window always contains the cursor (PR-057-B/C)");
        assert_eq!(row, 0);
        let (before, _after) = crate::surface::editor::caret_split(windowed, offset);
        // The gutter's own real width is not part of this measurement -- Q3
        // is about the *text* region the horizontal window sizes, which is
        // already net of the gutter reservation in `columns_that_fit`.
        let caret_right_edge_px = real_width(before, FONT_SIZE, font) + CARET_WIDTH;
        let overflow = (caret_right_edge_px - REGION_WIDTH).max(0.0);
        max_overflow_px = max_overflow_px.max(overflow);
    }
    let measured_char_width = real_width(&line, FONT_SIZE, font) / line.chars().count() as f32;
    let approximated_char_width = crate::surface::editor::column_pitch(FONT_SIZE);
    Falsification {
        class,
        measured_char_width,
        approximated_char_width,
        max_overflow_px,
        max_overflow_columns: max_overflow_px / approximated_char_width,
    }
}

/// **Q3.** The answer is not close: `W` overflows by **~51 columns** (measured
/// below), not a rounding error. Asserting a tolerance that passes today would
/// either be too loose to mean anything or would encode the defect as
/// tolerated; asserting the real, honest boundary would fail every ordinary
/// gate on a real, disclosed, structural limitation, pending the owner's
/// decision (review 443: join the fixed-width side, or measure real text) --
/// neither of which PR-057-C is the one to make. **`#[ignore]`d for the same
/// reason `editor_typing_latency_baseline_100_000_lines` is: a measurement,
/// not a pass/fail check** -- run it and read the numbers, which are also
/// copied into `qa-evidence.md` and the book, verbatim:
///
/// `cargo test -p tekstide editor_column_width -- --ignored --nocapture`
#[test]
#[ignore = "a measurement, not a check: see the module doc and qa-evidence.md § PR-057-C, Q3"]
fn the_width_approximation_is_falsified_against_real_text_layout_and_the_result_is_reported() {
    let wide = falsify("wide (W)", 'W', iced::Font::DEFAULT);
    let narrow = falsify("narrow (i)", 'i', iced::Font::DEFAULT);

    let report = |f: &Falsification| {
        format!(
            "{}: measured {:.3} px/char vs approximated {:.3} px/char ({:+.1}%); max caret overflow {:.1} px ({:.2} columns)",
            f.class,
            f.measured_char_width,
            f.approximated_char_width,
            (f.measured_char_width / f.approximated_char_width - 1.0) * 100.0,
            f.max_overflow_px,
            f.max_overflow_columns,
        )
    };
    println!("Q3 (review 443): {}", report(&wide));
    println!("Q3 (review 443): {}", report(&narrow));

    // Sanity only: the measurement itself produced real, positive numbers --
    // not an assertion on their size, which is the finding, not a check.
    assert!(wide.measured_char_width > 0.0 && narrow.measured_char_width > 0.0);
}

/// **The owner's font ruling (2026-09-29): the editor body's own default
/// becomes `Font::MONOSPACE`, not `Font::DEFAULT`.** Unlike Q3's finding
/// above, this is a real, held assertion, not a measurement report --
/// a monospace face has one advance width for every character by
/// definition, so `CHAR_WIDTH_FACTOR`'s approximation cannot single out one
/// character the way it did `W` in the proportional default (the ~51-column
/// loss above). Measured against real `iced`/`cosmic-text` layout, the same
/// harness Q3 used, on both the widest and narrowest ASCII classes -- if a
/// real monospace face ever answered them differently, that would itself be
/// the interesting finding.
#[test]
fn the_default_editor_font_is_monospace_and_the_width_approximation_holds_for_it() {
    let wide = falsify("wide (W), Font::MONOSPACE", 'W', iced::Font::MONOSPACE);
    let narrow = falsify("narrow (i), Font::MONOSPACE", 'i', iced::Font::MONOSPACE);

    println!(
        "Font::MONOSPACE: measured {:.3} px/char vs approximated {:.3} px/char ({:+.1}%)",
        wide.measured_char_width,
        wide.approximated_char_width,
        (wide.measured_char_width / wide.approximated_char_width - 1.0) * 100.0,
    );

    assert_eq!(
        wide.measured_char_width, narrow.measured_char_width,
        "Font::MONOSPACE must give every character the same real advance width"
    );
    assert_eq!(
        wide.max_overflow_columns, 0.0,
        "the shipped default must never overflow the horizontal window: {:.3} px/char measured \
         vs {:.3} approximated",
        wide.measured_char_width, wide.approximated_char_width
    );
    assert_eq!(narrow.max_overflow_columns, 0.0);
}
