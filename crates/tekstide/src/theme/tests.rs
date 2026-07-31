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
