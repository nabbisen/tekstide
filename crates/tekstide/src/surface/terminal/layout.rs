//! RFC-017 PR-017-E: the split decision RFC-017's own text names
//! explicitly -- "a split that produces panes too narrow for a minimum
//! column count is not a split; it is a rendering bug that shows up on
//! someone else's display scaling." This module is where that decision
//! is made, from real measured width and real glyph metrics
//! ([`super::font_metrics`]), not from a fixed fraction of the window.
//!
//! **Why the minimum is [`super::COLS`], not some smaller practical
//! number.** [`super::COLS`] is the launch-time default every pane
//! starts at; the terminal-resize handoff gives each pane its own real,
//! independently-resized grid after launch (`TerminalPane::resize`), but
//! that is a *per-pane* size, not this module's concern -- this module
//! only decides *whether to split at all* ("the existing split policy"
//! the resize handoff's own scope explicitly leaves alone). Using
//! [`super::COLS`] as the refusal threshold means a split is only
//! offered when each pane would start out able to render a full,
//! unclipped grid -- so that is the refusal threshold, not a smaller
//! number that would just move the clipping bug from "obvious" to
//! "occasionally, on narrower displays."

use tekstide_core::navigation::TerminalLayoutClass;

/// Gap between two side-by-side panes, logical pixels. `pub(crate)`
/// (terminal resize handoff): `shell.rs`'s geometry function needs this
/// to compute each pane's own share of the available width, the same
/// number [`layout_class_from_glyph_advance`] already subtracts here.
pub(crate) const PANE_GAP_PX: f32 = 8.0;
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

/// Terminal resize handoff: the one function that turns a pane's real
/// available content area into a real grid size -- the row-count/
/// column-count analogue of [`layout_class_for`], using the same real
/// font metrics rather than a guessed ratio. Called from both
/// `shell.rs`'s `update()` (to drive [`super::TerminalPane::resize`])
/// and, indirectly, from rendering (a pane's stored `rows`/`cols`, which
/// this function is what last set them to) -- one formula, not two that
/// could drift apart, matching response 242's requirement.
///
/// Clamps to [`super::MIN_COLS`]/[`super::MIN_ROWS`] rather than
/// returning a zero or negative grid -- a too-small pane shows a small
/// terminal, not an error.
pub(crate) fn pane_dimensions_for_area(
    available_width_px: f32,
    available_height_px: f32,
    font_size: f32,
) -> (u16, u16) {
    let glyph_advance_px = super::font_metrics::monospace_glyph_advance_px(font_size);
    let line_height_px = super::font_metrics::line_height_px(font_size);

    let cols = super::font_metrics::columns_for_width(
        available_width_px,
        glyph_advance_px,
        PANE_PADDING_PX,
    );
    let rows =
        super::font_metrics::rows_for_height(available_height_px, line_height_px, PANE_PADDING_PX);

    let cols = u16::try_from(cols).unwrap_or(u16::MAX).max(super::MIN_COLS);
    let rows = u16::try_from(rows).unwrap_or(u16::MAX).max(super::MIN_ROWS);

    (cols, rows)
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

    /// One pane's width, at the exact real per-pane padding
    /// (`PANE_PADDING_PX`, both sides) this module's own formula
    /// subtracts, plus `extra_margin_px` slack -- referencing the real
    /// constant here, not a re-derived literal, is what response 150
    /// flagged: a test asserting against its own hand-copied number
    /// keeps passing if `PANE_PADDING_PX` ever changes.
    fn one_pane_width(glyph_advance: f32, extra_margin_px: f32) -> f32 {
        super::super::COLS as f32 * glyph_advance + 2.0 * PANE_PADDING_PX + extra_margin_px
    }

    #[test]
    fn a_generously_wide_window_is_classified_wide() {
        let glyph_advance = real_glyph_advance();
        // Comfortably wider than 2x(COLS columns + padding) + gap at
        // this glyph width -- not a boundary case, a sanity check that
        // the obviously-splittable case actually splits.
        let comfortable_width = 2.0 * one_pane_width(glyph_advance, 16.0) + PANE_GAP_PX;
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
        let narrow_width = one_pane_width(glyph_advance, 16.0);
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
        let exact_columns_width = 2.0 * one_pane_width(glyph_advance, 0.0) + PANE_GAP_PX;
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

    /// **Not a directional proof that `font_size` is wired in** --
    /// `comfortable_width` is derived from the same `glyph_advance` this
    /// test passes to `layout_class_for`, so a `layout_class_for` that
    /// ignored `font_size` and hardcoded a *narrower* advance would
    /// still classify `Wide` here (response 150's finding). What this
    /// test actually proves: `layout_class_for`'s font-size-taking
    /// signature composes correctly end to end against a real theme
    /// font size. The directional proof that a different `font_size`
    /// genuinely changes the measurement lives in
    /// `font_metrics::tests::a_larger_font_size_measures_a_wider_glyph_advance`.
    #[test]
    fn layout_class_for_composes_correctly_with_a_real_font_size() {
        let font_size = crate::theme::Theme::default().font_size_body();
        let glyph_advance = super::super::font_metrics::monospace_glyph_advance_px(font_size);
        let comfortable_width = 2.0 * one_pane_width(glyph_advance, 16.0) + PANE_GAP_PX;
        assert_eq!(
            layout_class_for(comfortable_width, font_size),
            TerminalLayoutClass::Wide
        );
    }
}
