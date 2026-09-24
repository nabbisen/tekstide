//! theme-contrast-verification handoff, Slice A: WCAG 2.1 contrast math,
//! ~80 lines, no dependency (`snora-design` was evaluated and declined
//! as a dependency for this -- see `future-work.md`). Three pure
//! functions only; the *claims* about `Theme::default`'s own palette
//! belong in `theme/tests.rs`, not here, so this module can be trusted
//! (validated against known anchors below) before any such claim is
//! made.

use iced::Color;

/// WCAG 2.1 relative luminance of an opaque sRGB colour
/// (<https://www.w3.org/TR/WCAG21/#dfn-relative-luminance>). Alpha is
/// ignored -- a translucent colour has no luminance of its own until
/// composited over a backdrop; see [`composite_over`].
pub(crate) fn relative_luminance(color: Color) -> f32 {
    // RFC-054 PR-054-B: the arithmetic moved into `tekstide_core::config`, where
    // the configuration validator needs it, and this module now calls it -- so
    // there is one implementation, and the anchor tests below (known values,
    // the 0.03928 boundary) hold the *shipped* math to account rather than a
    // copy of it.
    tekstide_core::config::relative_luminance(to_colour(color))
}

fn to_colour(color: Color) -> tekstide_core::config::Colour {
    tekstide_core::config::Colour::rgba(color.r, color.g, color.b, color.a)
}

/// WCAG 2.1 contrast ratio between two opaque colours: `(lighter + 0.05)
/// / (darker + 0.05)`, symmetric in its two arguments by construction.
/// Callers passing a translucent colour must composite it first
/// ([`composite_over`]) -- this function does not know about alpha at
/// all.
pub(crate) fn contrast_ratio(first: Color, second: Color) -> f32 {
    tekstide_core::config::contrast_ratio(to_colour(first), to_colour(second))
}

/// Composites a possibly-translucent foreground colour over an opaque
/// backdrop (source-over, straight alpha), returning the opaque colour
/// that actually appears on screen. **Required, not optional**: a
/// translucent colour (this theme's `scrim`, `rgba(0, 0, 0, 0.55)`) has
/// no contrast ratio of its own, and any assertion that skips this step
/// measures a number that never appears on screen.
pub(crate) fn composite_over(foreground: Color, backdrop: Color) -> Color {
    let alpha = foreground.a;
    Color {
        r: foreground.r * alpha + backdrop.r * (1.0 - alpha),
        g: foreground.g * alpha + backdrop.g * (1.0 - alpha),
        b: foreground.b * alpha + backdrop.b * (1.0 - alpha),
        a: 1.0,
    }
}

/// derived-contrast-pairs handoff: finds the minimum of a **unimodal**
/// function over `[low, high]` -- one with a single interior minimum
/// (curve descends, then ascends) or, as a degenerate case, one that is
/// simply monotonic across the whole interval (the minimum then sits at
/// an endpoint, which this still finds correctly). Ternary search halves
/// the search triple every iteration rather than sampling a fixed grid,
/// so it cannot straddle-and-miss a true minimum the way a coarse grid
/// can: the handoff's own finding was that a 2,000-step grid reported
/// `2.4016` where the true minimum is `2.401129`, harmless at that
/// margin but not in general -- a grid straddling a true minimum of
/// `2.9995` would happily report `3.0002` and pass a failing palette.
///
/// `max` of two functions that are each monotonic over the interval is
/// itself unimodal, which is the property the modal-over-scrim sweep
/// below relies on (`max(contrast_ratio(border, ..), contrast_ratio(fill,
/// ..))`) -- **not proven here**, argued in the handoff and re-derived
/// independently before being trusted (see that test's own doc comment).
///
/// Returns `(t, value)` for the `t` the minimum was found at, not only
/// the value -- the handoff's own instruction: "report the content
/// value it occurs at, not only the ratio."
pub(crate) fn minimize_unimodal(
    mut low: f32,
    mut high: f32,
    iterations: u32,
    f: impl Fn(f32) -> f32,
) -> (f32, f32) {
    for _ in 0..iterations {
        let third = (high - low) / 3.0;
        let m1 = low + third;
        let m2 = high - third;
        if f(m1) < f(m2) {
            high = m2;
        } else {
            low = m1;
        }
    }
    let t = (low + high) / 2.0;
    (t, f(t))
}

#[cfg(test)]
mod tests;
