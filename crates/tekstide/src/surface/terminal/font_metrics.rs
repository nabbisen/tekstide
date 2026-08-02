//! RFC-017 PR-017-E: ported from the RFC-014 spike's own
//! `font_metrics.rs` (PR-014-E, C7), which measured real glyph width via
//! the exact text-layout primitive `iced`'s own `Text` widget uses
//! internally (`iced::advanced::graphics::text::Paragraph`, backed by
//! `cosmic-text`), rather than guessing a glyph width from a nominal
//! font size. No window is created -- `Paragraph::with_text` reads from
//! a process-global font system, so this is real measured output.
//!
//! **Parameterized on `font_size`, unlike the spike's hardcoded
//! constant.** The spike only ever printed measurements for its own
//! fixed size; this module's measurement must match whatever size
//! [`super::grid_colors::view`] actually renders at
//! (`state.theme.font_size_body()`), or the column math it feeds
//! [`super::layout`] would be answering a different question than the
//! one the pane actually renders -- exactly the two-sources-of-truth
//! class of bug this project has already found and fixed once
//! (RFC-017 PR-017-B's private-mode classifier duplication).
//!
//! Scale-invariant: `iced` always lays text out in logical pixels, and
//! the compositor/backend applies the display scale factor afterwards.

use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;
use iced::advanced::text::{Alignment, LineHeight, Paragraph, Shaping, Text, Wrapping};
use iced::{Font, Pixels, Size};

const SAMPLE_GLYPH_COUNT: usize = 200;

/// Average monospace glyph advance width, in logical pixels, at
/// `font_size`. Measured over `SAMPLE_GLYPH_COUNT` repeated glyphs and
/// divided down, rather than measuring a single glyph, so that any
/// fixed layout overhead in `min_bounds()` does not skew the estimate.
pub(super) fn monospace_glyph_advance_px(font_size: f32) -> f32 {
    let content: String = "M".repeat(SAMPLE_GLYPH_COUNT);

    let text = Text {
        content: content.as_str(),
        bounds: Size::new(f32::INFINITY, f32::INFINITY),
        size: Pixels(font_size),
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
/// pane body, after subtracting `pane_padding_px` (the padding a caller
/// applies around the pane's own content, per side, doubled here since
/// it applies to both sides of the available width).
pub(super) fn columns_for_width(
    available_width_px: f32,
    glyph_advance_px: f32,
    pane_padding_px: f32,
) -> u32 {
    let usable = (available_width_px - 2.0 * pane_padding_px).max(0.0);
    (usable / glyph_advance_px).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_advance_is_positive_and_plausible_for_a_monospace_font() {
        let advance = monospace_glyph_advance_px(14.0);
        assert!(
            advance > 0.0 && advance < 14.0,
            "a monospace glyph advance at 14px should be positive and narrower than the \
             font size itself, got {advance}"
        );
    }

    #[test]
    fn a_larger_font_size_measures_a_wider_glyph_advance() {
        let small = monospace_glyph_advance_px(10.0);
        let large = monospace_glyph_advance_px(20.0);
        assert!(
            large > small,
            "measuring at a larger font size must yield a wider glyph advance -- proves this \
             function actually uses its font_size argument, not a hardcoded value (small: \
             {small}, large: {large})"
        );
    }

    #[test]
    fn columns_for_width_floors_and_never_goes_negative() {
        assert_eq!(columns_for_width(100.0, 10.0, 8.0), 8);
        assert_eq!(
            columns_for_width(4.0, 10.0, 8.0),
            0,
            "a width narrower than the padding must yield zero columns, not underflow"
        );
    }
}
