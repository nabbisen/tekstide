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
    fn linearize(channel: f32) -> f32 {
        // The formula's own linear segment for low values -- this
        // boundary (0.03928, not 0.04045 or some other nearby constant)
        // is the usual transcription error, which is exactly why the
        // anchor tests below check known values rather than trusting
        // this by inspection.
        if channel <= 0.03928 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * linearize(color.r) + 0.7152 * linearize(color.g) + 0.0722 * linearize(color.b)
}

/// WCAG 2.1 contrast ratio between two opaque colours: `(lighter + 0.05)
/// / (darker + 0.05)`, symmetric in its two arguments by construction.
/// Callers passing a translucent colour must composite it first
/// ([`composite_over`]) -- this function does not know about alpha at
/// all.
pub(crate) fn contrast_ratio(first: Color, second: Color) -> f32 {
    let first_luminance = relative_luminance(first);
    let second_luminance = relative_luminance(second);
    let (lighter, darker) = if first_luminance >= second_luminance {
        (first_luminance, second_luminance)
    } else {
        (second_luminance, first_luminance)
    };
    (lighter + 0.05) / (darker + 0.05)
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

#[cfg(test)]
mod tests;
