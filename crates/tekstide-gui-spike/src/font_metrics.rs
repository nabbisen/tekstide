//! PR-014-E (C7): headless font-metrics measurement.
//!
//! This calls the exact same text-layout primitive iced's own `Text`
//! widget uses internally (`iced::advanced::graphics::text::Paragraph`,
//! backed by `cosmic-text`) rather than guessing a glyph width from a
//! nominal font size. No window is created -- `Paragraph::with_text` reads
//! from a process-global font system, so this is real measured output,
//! not an inference from documentation.
//!
//! "1x and a fractional scaling factor" (implementation-handoff.md §4) is
//! satisfied by this being scale-invariant: iced always lays text out in
//! logical pixels, and the compositor/backend applies the scale factor
//! afterwards. The cross-check that this holds in the real running app --
//! not just in this headless calculation -- is the screenshot comparison
//! recorded in qa-evidence.md.

use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;
use iced::advanced::text::{Alignment, LineHeight, Paragraph, Shaping, Text, Wrapping};
use iced::{Font, Pixels, Size};

const FONT_SIZE: f32 = 13.0;
const SAMPLE_GLYPH_COUNT: usize = 200;

/// Average monospace glyph advance width, in logical pixels, at
/// `FONT_SIZE`. Measured over `SAMPLE_GLYPH_COUNT` repeated glyphs and
/// divided down, rather than measuring a single glyph, so that
/// any fixed layout overhead in `min_bounds()` does not skew the estimate.
pub fn monospace_glyph_advance_px() -> f32 {
    let content: String = "M".repeat(SAMPLE_GLYPH_COUNT);

    let text = Text {
        content: content.as_str(),
        bounds: Size::new(f32::INFINITY, f32::INFINITY),
        size: Pixels(FONT_SIZE),
        line_height: LineHeight::Relative(1.0),
        font: Font::MONOSPACE,
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::Basic,
        wrapping: Wrapping::None,
    };

    let paragraph = GraphicsParagraph::with_text(text);
    paragraph.min_bounds().width / SAMPLE_GLYPH_COUNT as f32
}

/// Column count that fits in `available_width_px` (logical pixels) of
/// pane body, after subtracting the same 8px-per-side padding
/// `terminal_pane_view` applies in `shell.rs`.
pub fn columns_for_width(available_width_px: f32, glyph_advance_px: f32) -> u32 {
    let usable = (available_width_px - 16.0).max(0.0);
    (usable / glyph_advance_px).floor() as u32
}

pub fn run() {
    let glyph_advance = monospace_glyph_advance_px();
    println!("monospace glyph advance (logical px) at {FONT_SIZE}px: {glyph_advance:.4}");

    for width in [1024.0_f32, 1280.0, 1706.4] {
        let columns = columns_for_width(width, glyph_advance);
        println!("pane body width {width:.1} logical px -> {columns} columns");
    }
}
