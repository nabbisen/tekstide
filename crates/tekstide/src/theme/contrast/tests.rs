use super::{composite_over, contrast_ratio, relative_luminance};
use iced::Color;

const BLACK: Color = Color::from_rgb(0.0, 0.0, 0.0);
const WHITE: Color = Color::from_rgb(1.0, 1.0, 1.0);

/// The handoff's own required check, run before any claim is made about
/// our palette: a contrast function with a transcription error (the
/// usual one being the `0.03928` linear-segment boundary) will happily
/// pass everything, so the arithmetic itself is verified against known
/// anchors first.
#[test]
fn black_on_white_is_exactly_21_to_1() {
    let ratio = contrast_ratio(BLACK, WHITE);
    assert!(
        (ratio - 21.0).abs() < 0.01,
        "black on white must be 21:1 per WCAG 2.1's own worked example, got {ratio}"
    );
}

#[test]
fn identical_colours_are_exactly_1_to_1() {
    let mid_grey = Color::from_rgb(0.5, 0.5, 0.5);
    assert_eq!(contrast_ratio(mid_grey, mid_grey), 1.0);
    assert_eq!(contrast_ratio(BLACK, BLACK), 1.0);
    assert_eq!(contrast_ratio(WHITE, WHITE), 1.0);
}

#[test]
fn contrast_ratio_is_symmetric() {
    let accent = Color::from_rgb(0.30, 0.60, 1.0);
    let background = Color::from_rgb(0.08, 0.08, 0.09);
    assert_eq!(
        contrast_ratio(accent, background),
        contrast_ratio(background, accent)
    );
}

/// The linear-segment boundary itself (`c <= 0.03928`), the usual
/// transcription error this handoff calls out by name: a channel just
/// below it must use the `/12.92` branch, not the exponential one, or
/// `relative_luminance` silently drifts off the WCAG formula for every
/// dark colour a real theme is made of.
#[test]
fn low_channel_values_use_the_linear_segment() {
    let just_below_boundary = 0.03927_f32;
    let manually_linear = just_below_boundary / 12.92;
    let luminance = relative_luminance(Color::from_rgb(
        just_below_boundary,
        just_below_boundary,
        just_below_boundary,
    ));
    let expected = 0.2126 * manually_linear + 0.7152 * manually_linear + 0.0722 * manually_linear;
    assert!(
        (luminance - expected).abs() < 0.0001,
        "a channel at {just_below_boundary} must take the linear segment: expected {expected}, \
         got {luminance}"
    );
}

/// `composite_over`'s own anchors: a fully opaque foreground must
/// dominate completely (the backdrop cannot show through), and a fully
/// transparent one must leave the backdrop completely unchanged --
/// composited to `a = 1.0` either way, since a translucent colour has no
/// contrast ratio of its own once composited.
#[test]
fn composite_over_opaque_foreground_ignores_the_backdrop() {
    let opaque_red = Color::from_rgba(1.0, 0.0, 0.0, 1.0);
    let composited = composite_over(opaque_red, WHITE);
    assert_eq!(composited, Color::from_rgba(1.0, 0.0, 0.0, 1.0));
}

#[test]
fn composite_over_fully_transparent_foreground_is_just_the_backdrop() {
    let invisible = Color::from_rgba(1.0, 0.0, 0.0, 0.0);
    let composited = composite_over(invisible, WHITE);
    assert_eq!(composited, Color::from_rgba(1.0, 1.0, 1.0, 1.0));
}

/// This theme's real scrim, composited over a real backdrop -- exercises
/// the exact shape the theme-level assertion depends on, at the
/// contrast-module level where the anchor tests live.
#[test]
fn a_real_translucent_scrim_composites_partway_between_its_colour_and_the_backdrop() {
    let scrim = Color::from_rgba(0.0, 0.0, 0.0, 0.55);
    let composited = composite_over(scrim, WHITE);
    assert_eq!(composited.a, 1.0, "a composited colour must be opaque");
    assert!(
        (composited.r - 0.45).abs() < 0.001,
        "55% black over white must land at 45% grey, got {}",
        composited.r
    );
}
