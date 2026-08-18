use super::Theme;
use super::contrast;

/// theme-contrast-verification handoff: real WCAG 2.1 AA thresholds,
/// not the "in range" type checks below -- those cannot distinguish a
/// readable palette from an unreadable one, which is how
/// `border_default`'s contrast defect passed from RFC-015 through
/// `0.10.0`. SC 1.4.3 (text, >= 4.5:1) and SC 1.4.11 (non-text UI
/// component boundaries, >= 3:1), both measured against the real
/// composited colour, never a translucent one directly.
#[test]
fn theme_default_meets_wcag_contrast_thresholds() {
    let theme = Theme::default();

    let text_pairs = [
        (
            "foreground on background",
            theme.foreground(),
            theme.background(),
        ),
        (
            "foreground on surface_elevated",
            theme.foreground(),
            theme.surface_elevated(),
        ),
    ];
    for (label, foreground, backdrop) in text_pairs {
        let ratio = contrast::contrast_ratio(foreground, backdrop);
        assert!(
            ratio >= 4.5,
            "{label} must be >= 4.5:1 (WCAG 2.1 SC 1.4.3), measured {ratio:.2}:1"
        );
    }

    let non_text_pairs = [
        (
            "border_default on background",
            theme.border_default(),
            theme.background(),
        ),
        (
            "border_default on surface_elevated",
            theme.border_default(),
            theme.surface_elevated(),
        ),
        (
            "border_focused on background",
            theme.border_focused(),
            theme.background(),
        ),
        (
            "border_focused on surface_elevated",
            theme.border_focused(),
            theme.surface_elevated(),
        ),
        ("accent on background", theme.accent(), theme.background()),
        (
            "accent on surface_elevated",
            theme.accent(),
            theme.surface_elevated(),
        ),
    ];
    for (label, foreground, backdrop) in non_text_pairs {
        let ratio = contrast::contrast_ratio(foreground, backdrop);
        assert!(
            ratio >= 3.0,
            "{label} must be >= 3:1 (WCAG 2.1 SC 1.4.11), measured {ratio:.2}:1"
        );
    }
}

/// A real assertion about the dimming layer, not the "alpha < 1.0"
/// check below -- a translucent colour has no contrast or brightness of
/// its own until composited over what it sits above, and this proves
/// the composited result is genuinely darker than the backdrop, in
/// every channel, for both real backdrops the scrim is drawn over.
#[test]
fn scrim_composited_over_the_theme_genuinely_darkens_it() {
    let theme = Theme::default();
    let scrim = theme.scrim();

    for (label, backdrop) in [
        ("background", theme.background()),
        ("foreground", theme.foreground()),
    ] {
        let composited = contrast::composite_over(scrim, backdrop);
        assert!(
            composited.r < backdrop.r && composited.g < backdrop.g && composited.b < backdrop.b,
            "{label} dimmed by the scrim must be darker in every channel: backdrop={backdrop:?} \
             composited={composited:?}"
        );
    }
}

/// A colour channel is a Fluent-independent, but still real, thing to
/// get wrong (e.g. transposing red and blue). Cheap sanity check that
/// the compiled default is a real dark theme, not zeroed-out or
/// out-of-range values. **Does not check contrast or readability** --
/// see `theme_default_meets_wcag_contrast_thresholds` above for that;
/// no colour a human would plausibly type can fail this.
#[test]
fn the_compiled_default_is_a_dark_theme_with_in_range_channels() {
    let theme = Theme::default();

    for channel in [
        theme.background().r,
        theme.background().g,
        theme.background().b,
        theme.foreground().r,
        theme.accent().b,
    ] {
        assert!(
            (0.0..=1.0).contains(&channel),
            "colour channel out of range: {channel}"
        );
    }

    assert!(
        theme.background().r < theme.foreground().r,
        "the compiled default should be a dark theme: background darker than foreground"
    );
}

/// RFC-015 PR-015-E: `border_focused` must be in range and genuinely
/// distinct from `border_default` -- the colour channel is not the sole
/// indicator (`NFR-UX-002`; callers also change border width), but a
/// focus ring identical in colour to the unfocused border would still
/// defeat the point of having a separate role at all. **Does not check
/// contrast** -- two colours can be unequal and still both be
/// unreadable; see `theme_default_meets_wcag_contrast_thresholds`.
#[test]
fn border_focused_is_in_range_and_distinct_from_border_default() {
    let theme = Theme::default();

    for channel in [
        theme.border_focused().r,
        theme.border_focused().g,
        theme.border_focused().b,
    ] {
        assert!(
            (0.0..=1.0).contains(&channel),
            "colour channel out of range: {channel}"
        );
    }
    assert_ne!(
        theme.border_focused(),
        theme.border_default(),
        "a focus ring identical to the unfocused border renders no differently"
    );
}

/// RFC-018 PR-018-G: `scrim` must be a real, in-range colour, and
/// genuinely translucent -- neither fully transparent (`a == 0.0`, which
/// would render nothing and defeat the whole slice) nor fully opaque
/// (`a == 1.0`, which would hide rather than dim whatever is underneath,
/// undermining the argument that motivated building this at all: chrome
/// should read as *dimmed*, not replaced). **Does not check how dark the
/// composited result actually is** -- see
/// `scrim_composited_over_the_theme_genuinely_darkens_it` for the
/// assertion that measures what appears on screen.
#[test]
fn scrim_is_in_range_and_genuinely_translucent() {
    let theme = Theme::default();
    let scrim = theme.scrim();

    for channel in [scrim.r, scrim.g, scrim.b, scrim.a] {
        assert!(
            (0.0..=1.0).contains(&channel),
            "colour channel out of range: {channel}"
        );
    }
    assert!(
        scrim.a > 0.0 && scrim.a < 1.0,
        "scrim must be translucent, not fully transparent or fully opaque: a = {}",
        scrim.a
    );
}

/// Font sizes must be positive and heading text must be visually larger
/// than body/status text -- the shape `NFR-UX-004` implies even before
/// RFC-023 makes these configurable.
#[test]
fn font_sizes_are_positive_and_heading_is_the_largest() {
    let theme = Theme::default();

    assert!(theme.font_size_body() > 0.0);
    assert!(theme.font_size_heading() > 0.0);
    assert!(theme.font_size_status() > 0.0);
    assert!(theme.font_size_heading() > theme.font_size_body());
}
