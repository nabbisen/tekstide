use super::Theme;

/// A colour channel is a Fluent-independent, but still real, thing to
/// get wrong (e.g. transposing red and blue). Cheap sanity check that
/// the compiled default is a real dark theme, not zeroed-out or
/// out-of-range values.
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
/// defeat the point of having a separate role at all.
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
/// should read as *dimmed*, not replaced).
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
