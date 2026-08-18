use super::Theme;
use super::contrast;
use iced::Color;

/// derived-contrast-pairs handoff: one declared pair per `(role,
/// backdrop)` this crate actually renders, not a hand-written literal
/// list -- see [`derived_contrast_pairs`] for why a hand-written list
/// cannot stay complete on its own.
struct DerivedContrastPair {
    label: String,
    foreground: Color,
    backdrop: Color,
    threshold: f32,
}

/// derived-contrast-pairs handoff: the pair list
/// `theme_default_meets_wcag_contrast_thresholds` checks, **derived**
/// from an exhaustive destructure of `Theme` rather than hand-written.
/// `theme-contrast-verification`'s own list was already complete for
/// the two backdrops that existed when it was written -- this is not a
/// fix to that list's *contents*. It is a fix to how a *future* field
/// gets classified: the old list could silently stay the same size
/// forever; this one cannot. Two independent enforcements:
///
/// 1. The destructure below has no `..` -- adding a field to `Theme`
///    without adding it here fails to compile (`E0027`, "pattern does
///    not mention field"). Ablated in the handoff's own review: add a
///    throwaway field, observe the error naming it, remove it.
/// 2. Every destructured binding must be used or explicitly discarded
///    -- an unused one fails this crate's own `-D warnings` gate. A
///    field cannot be silently forgotten between the destructure and
///    the pairs it produces.
///
/// **Declares intended usage, not the cross-product** -- a pair only
/// belongs here if the role is genuinely rendered on that backdrop
/// (`zone_style`, `modal_dialog_box`). `background`/`surface_elevated`
/// are the two real backdrops; nothing is declared "background on
/// foreground," since neither backdrop role is ever drawn as a
/// foreground itself. `scrim` produces no fixed pair at all -- it is
/// translucent, has no contrast ratio until composited, and its one
/// real backdrop (the modal card drawn over it, over arbitrary terminal
/// content) is checked separately, as a sweep, by
/// `modal_over_scrim_backdrop_clears_3_to_1_at_every_content_value`
/// below; see that test's own doc for why a fixed pair cannot express
/// it. The three font sizes are not colours and carry no contrast
/// obligation.
fn derived_contrast_pairs(theme: &Theme) -> Vec<DerivedContrastPair> {
    let Theme {
        background,
        foreground,
        accent,
        border_default,
        border_focused,
        surface_elevated,
        scrim,
        font_size_body,
        font_size_heading,
        font_size_status,
    } = *theme;
    let _ = (scrim, font_size_body, font_size_heading, font_size_status);

    let backdrops = [
        ("background", background),
        ("surface_elevated", surface_elevated),
    ];
    let mut pairs = Vec::new();

    for (backdrop_label, backdrop_color) in backdrops {
        pairs.push(DerivedContrastPair {
            label: format!("foreground on {backdrop_label}"),
            foreground,
            backdrop: backdrop_color,
            threshold: 4.5,
        });
    }
    for (role_label, role_color) in [
        ("border_default", border_default),
        ("border_focused", border_focused),
        ("accent", accent),
    ] {
        for (backdrop_label, backdrop_color) in backdrops {
            pairs.push(DerivedContrastPair {
                label: format!("{role_label} on {backdrop_label}"),
                foreground: role_color,
                backdrop: backdrop_color,
                threshold: 3.0,
            });
        }
    }

    pairs
}

/// theme-contrast-verification handoff: real WCAG 2.1 AA thresholds,
/// not the "in range" type checks below -- those cannot distinguish a
/// readable palette from an unreadable one, which is how
/// `border_default`'s contrast defect passed from RFC-015 through
/// `0.10.0`. SC 1.4.3 (text, >= 4.5:1) and SC 1.4.11 (non-text UI
/// component boundaries, >= 3:1), both measured against the real
/// composited colour, never a translucent one directly. **derived-contrast-pairs
/// handoff**: the pair list itself is now derived, not hand-written --
/// see [`derived_contrast_pairs`].
#[test]
fn theme_default_meets_wcag_contrast_thresholds() {
    let theme = Theme::default();

    for pair in derived_contrast_pairs(&theme) {
        let ratio = contrast::contrast_ratio(pair.foreground, pair.backdrop);
        assert!(
            ratio >= pair.threshold,
            "{} must be >= {:.1}:1, measured {ratio:.2}:1",
            pair.label,
            pair.threshold
        );
    }
}

/// derived-contrast-pairs handoff: the real failure sampling the
/// endpoints hides. `modal_dialog_box` draws its border (`accent`) and
/// fill (`surface_elevated`) over the scrim, which is composited over
/// whatever the modal opened above -- including real terminal content,
/// which is arbitrary and attacker-influenceable. "Scrim over
/// `background`" and "scrim over white" both pass; the failure lives
/// strictly between them, at content around 0.78 grey, where the
/// border-identifies-the-card and fill-identifies-the-card curves
/// cross and neither alone clears 3:1.
///
/// So this cannot be a fixed pair, and is not sampled as one: swept
/// continuously over greyscale content via [`contrast::minimize_unimodal`],
/// taking `max(border, fill)` -- the best channel available to identify
/// the card -- as the value being minimised, and requiring the true
/// minimum to clear 3:1.
///
/// **Greyscale is sufficient, checked two independent ways before being
/// trusted, not merely asserted from the handoff's own prose.** The
/// scrim composites channelwise and relative luminance is monotonic
/// increasing in each channel, so a composited backdrop's luminance is
/// bounded by its black/white extremes, and greyscale spans that
/// interval continuously -- a coloured backdrop can only reach a
/// luminance a grey one already reaches. Confirmed independently while
/// implementing this test: a fine-grained direct scan (200,001 grey
/// steps, no ternary search) found the same minimum
/// (`2.4011` at `t=0.7844`) `minimize_unimodal` finds below.
///
/// **Review response 261: with the Slice B alpha (`0.75`), the true
/// minimum has moved to the endpoint (`content = 1.0`, ~3.62:1) --
/// sampling would now happen to give the right answer.** Do not read
/// that as a reason to simplify this back to a fixed pair. The minimum
/// sits at an interior point (`~0.78` grey) with the pre-fix `0.55`
/// alpha, and any future change to `accent`, `surface_elevated`, or the
/// scrim itself (RFC-023 supplying any of them from configuration) can
/// move it back into the interior -- where a sampled test would pass a
/// failing palette silently, exactly as it did before this handoff. The
/// sweep is not redundant because it currently resolves at an endpoint;
/// it is what stays correct when it does not.
#[test]
fn modal_over_scrim_backdrop_clears_3_to_1_at_every_content_value() {
    let theme = Theme::default();
    let scrim = theme.scrim();
    let accent = theme.accent();
    let surface_elevated = theme.surface_elevated();

    let worst_case = |content: f32| -> f32 {
        let backdrop = contrast::composite_over(scrim, Color::from_rgb(content, content, content));
        f32::max(
            contrast::contrast_ratio(accent, backdrop),
            contrast::contrast_ratio(surface_elevated, backdrop),
        )
    };

    let (content_at_minimum, minimum_ratio) =
        contrast::minimize_unimodal(0.0, 1.0, 100, worst_case);

    assert!(
        minimum_ratio >= 3.0,
        "the modal card's border/fill must clear 3:1 (WCAG 2.1 SC 1.4.11) against every possible \
         terminal-content backdrop behind the scrim -- minimum {minimum_ratio:.2}:1 at content \
         value {content_at_minimum:.4} grey"
    );
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
