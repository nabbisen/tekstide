//! RFC-017 PR-017-E: the split decision RFC-017's own text names
//! explicitly -- "a split that produces panes too narrow for a minimum
//! column count is not a split; it is a rendering bug that shows up on
//! someone else's display scaling." This module is where that decision
//! is made, from real measured width and real glyph metrics
//! ([`super::font_metrics`]), not from a fixed fraction of the window.
//!
//! **Why the minimum is [`super::COLS`], not some smaller practical
//! number.** Each pane's own emulator grid is a fixed 80 columns
//! ([`super::COLS`]) -- this slice does not reflow a live `Term` to an
//! arbitrary width (a materially larger feature: live PTY resize plus
//! emulator reflow, not asked for by this slice's review gate). Given
//! that, the only way a two-pane split can render its full grid content
//! without clipping is if each pane's measured column capacity is at
//! least that fixed width -- so that is the refusal threshold, not a
//! smaller number that would just move the clipping bug from "obvious"
//! to "occasionally, on narrower displays."

use tekstide_core::navigation::TerminalLayoutClass;

/// Gap between two side-by-side panes, logical pixels.
const PANE_GAP_PX: f32 = 8.0;
/// Padding `grid_colors::view`'s caller applies around a single pane's
/// own content -- matches the RFC-014 spike's own `terminal_pane_view`
/// convention, kept in sync here rather than duplicated as a second
/// magic number.
const PANE_PADDING_PX: f32 = 8.0;

/// `Wide` only when splitting `available_width_px` in two, each half
/// losing [`PANE_GAP_PX`]/[`PANE_PADDING_PX`], still measures at least
/// [`super::COLS`] real columns per pane. Otherwise `Narrow` -- the
/// split is refused, not rendered clipped. `font_size` must be the same
/// size the pane actually renders at (`state.theme.font_size_body()`),
/// or this measures a different question than the one being rendered.
pub(crate) fn layout_class_for(available_width_px: f32, font_size: f32) -> TerminalLayoutClass {
    let glyph_advance_px = super::font_metrics::monospace_glyph_advance_px(font_size);
    layout_class_from_glyph_advance(available_width_px, glyph_advance_px)
}

fn layout_class_from_glyph_advance(
    available_width_px: f32,
    glyph_advance_px: f32,
) -> TerminalLayoutClass {
    let per_pane_width = (available_width_px - PANE_GAP_PX) / 2.0;
    let columns_if_split =
        super::font_metrics::columns_for_width(per_pane_width, glyph_advance_px, PANE_PADDING_PX);

    if columns_if_split >= super::COLS as u32 {
        TerminalLayoutClass::Wide
    } else {
        TerminalLayoutClass::Narrow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real measurement at the theme's real font size, so this isn't
    /// testing against a made-up glyph width -- if `font_size_body`
    /// ever changes, this test's own inputs change with it rather than
    /// silently drifting from what the pane actually renders.
    fn real_glyph_advance() -> f32 {
        super::super::font_metrics::monospace_glyph_advance_px(
            crate::theme::Theme::default().font_size_body(),
        )
    }

    #[test]
    fn a_generously_wide_window_is_classified_wide() {
        let glyph_advance = real_glyph_advance();
        // Comfortably wider than 2x(COLS columns + padding + gap) at
        // this glyph width -- not a boundary case, a sanity check that
        // the obviously-splittable case actually splits.
        let comfortable_width = 2.0 * (super::super::COLS as f32 * glyph_advance + 32.0) + 16.0;
        assert_eq!(
            layout_class_from_glyph_advance(comfortable_width, glyph_advance),
            TerminalLayoutClass::Wide
        );
    }

    #[test]
    fn a_narrow_window_that_cannot_fit_two_full_panes_is_classified_narrow() {
        let glyph_advance = real_glyph_advance();
        // Enough for exactly one pane's real columns, nowhere near
        // enough for two -- the case RFC-017 explicitly says must not
        // render as a clipped two-pane split.
        let narrow_width = super::super::COLS as f32 * glyph_advance + 32.0;
        assert_eq!(
            layout_class_from_glyph_advance(narrow_width, glyph_advance),
            TerminalLayoutClass::Narrow
        );
    }

    #[test]
    fn the_boundary_is_the_real_column_count_not_an_arbitrary_pixel_threshold() {
        let glyph_advance = real_glyph_advance();
        // Exactly enough per-pane width for super::COLS columns, both
        // panes: must classify Wide. One glyph-width less: must not.
        let exact_columns_width =
            2.0 * (super::super::COLS as f32 * glyph_advance + 2.0 * 8.0) + PANE_GAP_PX;
        assert_eq!(
            layout_class_from_glyph_advance(exact_columns_width, glyph_advance),
            TerminalLayoutClass::Wide,
            "exactly enough real columns per pane must classify Wide"
        );
        assert_eq!(
            layout_class_from_glyph_advance(
                exact_columns_width - glyph_advance * 2.0,
                glyph_advance
            ),
            TerminalLayoutClass::Narrow,
            "one glyph-width less per pane must drop below the real column threshold"
        );
    }

    /// The real public entry point, `layout_class_for`, taking a font
    /// size rather than a pre-measured glyph advance -- proves it
    /// actually wires `font_metrics::monospace_glyph_advance_px` in,
    /// not just that the glyph-advance-parameterized helper works.
    #[test]
    fn layout_class_for_measures_from_a_real_font_size() {
        let font_size = crate::theme::Theme::default().font_size_body();
        let glyph_advance = super::super::font_metrics::monospace_glyph_advance_px(font_size);
        let comfortable_width = 2.0 * (super::super::COLS as f32 * glyph_advance + 32.0) + 16.0;
        assert_eq!(
            layout_class_for(comfortable_width, font_size),
            TerminalLayoutClass::Wide
        );
    }
}
