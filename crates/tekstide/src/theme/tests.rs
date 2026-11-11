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
    /// PR-076-D3: explicit rather than inferred from `threshold == 4.5`,
    /// because WCAG defines no AAA tier for a non-text (UI-component
    /// boundary) pair at all -- `theme_default_text_pairs_meet_wcag_aaa_
    /// not_only_the_enforced_aa_floor` below must skip `border_default`/
    /// `border_focused`/`accent` on that basis, not because their
    /// threshold happens to differ today.
    is_text: bool,
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
        font,
        editor_font,
    } = *theme;
    let _ = (
        scrim,
        font_size_body,
        font_size_heading,
        font_size_status,
        font,
        editor_font,
    );

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
            is_text: true,
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
                is_text: false,
            });
        }
    }

    pairs
}

/// **PR-076-D3, review 669 required-1: the positive form, not a ban-list.**
/// An earlier version of this guard banned one syntactic *spelling* of
/// derivation (`..theme.<role>()` struct-update syntax) and was proven
/// insufficient in review: `..state.theme.foreground()` and a two-step
/// `let base = theme.foreground(); Color { a: .., ..base }` both derive a
/// modified colour from a role and neither matches a ban aimed at how a
/// colour might be *produced* -- that space has no enumerable end
/// (`self.theme`, any local binding, a future `Color::with_alpha`).
///
/// What **is** enumerable is the finite, small set of field *assignments*
/// that exist in shipped source -- not how a value might be derived, but
/// where a value is finally *used* as a text colour. Every such assignment
/// must be a bare, unmodified role accessor (`theme.<role>()`, or behind an
/// arbitrary `.`-separated receiver path ending in `theme`, e.g.
/// `state.theme.<role>()`, optionally wrapped in `Some(..)` for a
/// container-style field) or a declared exception in
/// [`DECLARED_TEXT_COLOUR_EXCEPTIONS`] below. Anything else fails --
/// including a struct literal, a method chained after the accessor call,
/// or a local variable not named in the exceptions list -- with no way to
/// spell around it by choosing a different receiver or a different
/// derivation mechanism, because the check does not look for a way to
/// *derive* a colour; it looks for anything that is not a bare one.
///
/// **Review 669 round 2, required-5: one marker is not the whole surface.**
/// iced sets a text colour through more than the `text_color:` field name --
/// `iced_core::widget::text::Style`'s own field is bare `color`, reachable
/// from a `Text`/`Span` builder via `.color(..)`/`.color_maybe(..)`
/// (`iced_core::widget::text.rs:169,177`, `iced_core::text::Span`, same
/// method names), or, in principle, by constructing that `Style` directly
/// (`text::Style { color: .. }`). Checked directly against the pinned
/// `iced_core 0.14.0`: those are the only three spellings that set a *text*
/// colour in this version -- every other `color:`-shaped field found by
/// grep (`Border::color`, `rule::Style`, `pane_grid::Style`, `svg::Style`)
/// belongs to a non-text type, confirmed by checking each struct's own
/// field list rather than assumed from the field name. `field_assignments_
/// in` below is marker-driven (one call per spelling) rather than
/// field-name-driven, so adding a fourth spelling -- should iced ever gain
/// one -- is one more call with its own marker, not a rewrite.
fn field_assignments_in(code: &str, marker: &str) -> Vec<String> {
    let flat: String = code.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = flat[cursor..].find(marker) {
        let start = cursor + relative + marker.len();
        // Scans forward tracking bracket depth rather than stopping at the
        // first comma or `}`, so a value containing its own nested call
        // (`Some(theme.foreground())`) is captured whole rather than cut
        // off at its first inner `)`.
        let bytes = flat.as_bytes();
        let mut depth: i32 = 0;
        let mut end = start;
        while end < bytes.len() {
            match bytes[end] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' => depth -= 1,
                b'}' if depth == 0 => break,
                b'}' => depth -= 1,
                b',' if depth == 0 => break,
                _ => {}
            }
            end += 1;
        }
        values.push(flat[start..end].trim().to_string());
        cursor = end;
    }
    values
}

/// Companion to [`field_assignments_in`] for the call-style spellings
/// (`.color(..)`, `.color_maybe(..)`): the marker's own trailing `(` has
/// already opened one level of depth, so this starts at depth `1` and
/// returns the argument text up to (not including) that call's own
/// matching `)`, rather than the field-style terminator (a top-level comma
/// or `}`) `field_assignments_in` looks for.
fn call_argument_assignments_in(code: &str, marker: &str) -> Vec<String> {
    let flat: String = code.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut values = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = flat[cursor..].find(marker) {
        let start = cursor + relative + marker.len();
        let bytes = flat.as_bytes();
        let mut depth: i32 = 1;
        let mut end = start;
        while end < bytes.len() && depth > 0 {
            match bytes[end] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        let value_end = end.saturating_sub(1);
        values.push(flat[start..value_end].trim().to_string());
        cursor = end;
    }
    values
}

/// Every production text-colour assignment this crate and `tekstide-core`
/// ship, across all three spellings `field_assignments_in`'s own doc
/// comment enumerates, tagged with which spelling produced it (purely for
/// a readable violation message).
fn all_text_colour_assignments(code: &str) -> Vec<(&'static str, String)> {
    let mut found = Vec::new();
    found.extend(
        field_assignments_in(code, "text_color:")
            .into_iter()
            .map(|value| ("text_color:", value)),
    );
    found.extend(
        field_assignments_in(code, "Style { color:")
            .into_iter()
            .map(|value| ("text::Style { color: .. }", value)),
    );
    found.extend(
        call_argument_assignments_in(code, ".color(")
            .into_iter()
            .map(|value| (".color(..)", value)),
    );
    found.extend(
        call_argument_assignments_in(code, ".color_maybe(")
            .into_iter()
            .map(|value| (".color_maybe(..)", value)),
    );
    found
}

/// Whether `value` is exactly a bare theme role accessor: optionally
/// `Some(..)`-wrapped, optionally behind a `.`-separated receiver path
/// (any identifiers, any depth) ending in `theme`, calling one of
/// `Theme`'s own colour accessors with no arguments and nothing chained
/// after it. Anything else -- a struct literal, an extra method call, an
/// arithmetic expression -- fails this check regardless of what it
/// contains, which is what makes it a positive enumeration rather than a
/// ban list: there is exactly one shape that passes, not an open set of
/// shapes that fail.
fn is_bare_theme_role_accessor(value: &str) -> bool {
    const ROLES: [&str; 7] = [
        "background",
        "foreground",
        "accent",
        "border_default",
        "border_focused",
        "surface_elevated",
        "scrim",
    ];
    let value = value
        .strip_prefix("Some(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or(value);
    ROLES.iter().any(|role| {
        value
            .strip_suffix(&format!(".{role}()"))
            .is_some_and(|receiver| {
                !receiver.is_empty()
                    && receiver.ends_with("theme")
                    && receiver
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            })
    })
}

/// **Declared, verified exceptions to the bare-accessor rule above, each
/// keyed to the file it was verified in -- review 669 round 2, required-6.**
/// A bare value string (`"Some(foreground)"` alone) would have matched
/// anywhere either crate happens to spell a local the same way, including a
/// future site where that local is genuinely derived; keying each entry to
/// a path suffix makes an exception a statement about one verified site,
/// not a string that travels.
///
/// - `("shell.rs", "Some(foreground)")`: `chrome_style`'s own `foreground:
///   iced::Color` parameter is a local binding, not a receiver path, so
///   `Some(foreground)` inside its body cannot match the bare-accessor shape
///   by construction. It is as safe as the bare accessor it passes through:
///   both of its call sites (the project-board chrome and the tab-strip
///   chrome) feed it a bare `state.theme.foreground()` and nothing else --
///   verified by reading at the time this list was written. A future call
///   site that feeds it anything else would not be caught by this test (it
///   cannot see across a function boundary); that is the one gap this
///   entry carries forward rather than closes.
/// - `("grid_colors.rs", "Color::from_rgb(rgb[0], rgb[1], rgb[2])")`:
///   `Span::new(text).color(..)` painting a terminal cell -- ANSI colours
///   the running program chose, not a theme role at all, out of scope by
///   the start file's own §6 ("Terminal cell colours are out of scope").
const DECLARED_TEXT_COLOUR_EXCEPTIONS: &[(&str, &str)] = &[
    ("shell.rs", "Some(foreground)"),
    ("grid_colors.rs", "Color::from_rgb(rgb[0], rgb[1], rgb[2])"),
];

/// **PR-076-D3, review 669 required-1, round 2 required-5/-6.** Enumerates
/// every text-colour assignment this crate and `tekstide-core` ship (the
/// same `tests::production_sources` scan `no_shipped_view_builds_text_or_
/// buttons_outside_the_theme` already trusts), across all three spellings
/// [`all_text_colour_assignments`] covers, and requires each one to be
/// [`is_bare_theme_role_accessor`] or a path-keyed
/// [`DECLARED_TEXT_COLOUR_EXCEPTIONS`] entry.
///
/// This enumeration of *surfaces* is what the completeness claim actually
/// rests on, not the shape of the per-value check: `is_bare_theme_role_
/// accessor` alone only ever covers whichever field names this function
/// hands it. Adding a fourth text-colour spelling to a future iced requires
/// adding it here explicitly; this test cannot discover one on its own, and
/// does not claim to.
///
/// A `0` violation count today is not a vacuous test: it is checked against
/// a non-empty assignment count below, so an empty result from
/// `production_sources` itself (a moved directory, a broken walk) fails
/// loudly rather than passing by having nothing to check.
#[test]
fn every_production_text_colour_is_a_bare_theme_role_or_a_declared_exception() {
    let mut assignments_seen = 0;
    let mut violations = Vec::new();
    for (path, code) in crate::shell::tests::production_sources() {
        for (spelling, value) in all_text_colour_assignments(&code) {
            assignments_seen += 1;
            if is_bare_theme_role_accessor(&value) {
                continue;
            }
            let declared = DECLARED_TEXT_COLOUR_EXCEPTIONS
                .iter()
                .any(|(file, exception)| path.ends_with(file) && value == *exception);
            if declared {
                continue;
            }
            violations.push(format!("{path}: {spelling} {value}"));
        }
    }
    assert!(
        assignments_seen > 0,
        "found no text-colour assignment in production at all, across any of the spellings \
         this test scans for -- this test would otherwise pass vacuously"
    );
    assert!(
        violations.is_empty(),
        "production assigns a text colour a value that is neither a bare `theme.<role>()` \
         accessor nor a declared, path-keyed exception in DECLARED_TEXT_COLOUR_EXCEPTIONS -- \
         this is exactly the shape the disabled-button-label defect took (a modified, \
         unmeasured colour on a text field): declare it with a measured entry in \
         derived_contrast_pairs, add it to the exceptions list keyed to its own file with the \
         reading that makes it safe, or make it a bare accessor. Violations: {violations:?}"
    );
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

/// **RFC-076 D3, 2026-11-11.** A separate standard from the test above, not a
/// stricter version of it. `theme_default_meets_wcag_contrast_thresholds`
/// holds `MIN_TEXT_CONTRAST` (4.5:1, WCAG AA) as the *floor* every configured
/// palette -- including a user's -- must clear; raising it would retroactively
/// refuse a theme that is valid under `NFR-UX-003`, so that constant stays
/// untouched here and everywhere else in this slice. This test instead holds
/// only `Theme::default` itself, the one palette nobody configured, to WCAG
/// AAA (7:1) for every *text* pair -- D3's "default theme moves toward AAA
/// while the floor stays AA."
///
/// **"Toward" is a floor to hold, not a target to move to.** `foreground` on
/// `background` measures 14.62:1 today, more than double the 7:1 this test
/// requires -- a literal reading of D3 that moved it *down* to 7 would satisfy
/// this assertion and make the product worse for the population RFC-076
/// exists for. Nothing in this test, or in D3, lowers a ratio; it only stops
/// the default from silently regressing toward the AA floor it currently
/// clears with room to spare.
///
/// Only `is_text` pairs are checked: WCAG defines no AAA tier for a non-text,
/// UI-component-boundary pair (`border_default`/`border_focused`/`accent`),
/// so holding one to a 7:1 "AAA" bar would be asserting a standard that does
/// not exist for it, not a stronger safety property.
#[test]
fn theme_default_text_pairs_meet_wcag_aaa_not_only_the_enforced_aa_floor() {
    const AAA_TEXT_CONTRAST: f32 = 7.0;
    let theme = Theme::default();

    let mut text_pairs_checked = 0;
    for pair in derived_contrast_pairs(&theme) {
        if !pair.is_text {
            continue;
        }
        text_pairs_checked += 1;
        let ratio = contrast::contrast_ratio(pair.foreground, pair.backdrop);
        assert!(
            ratio >= AAA_TEXT_CONTRAST,
            "{} is the *default* theme's own standard (WCAG AAA, {AAA_TEXT_CONTRAST}:1), not \
             the enforced minimum every configured theme -- including a user's -- must clear \
             (MIN_TEXT_CONTRAST, 4.5:1, which stays untouched: a user theme at that floor is \
             still valid under NFR-UX-003). Measured {ratio:.2}:1.",
            pair.label
        );
    }
    assert!(
        text_pairs_checked > 0,
        "derived_contrast_pairs declared no text pair at all -- this test would otherwise pass \
         vacuously"
    );
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

/// **Review 428.** A button is drawn from the theme's own roles, so a configured
/// `surface_elevated`, `foreground` and border reach it: its text sits on a
/// surface the contrast rules already measure (`foreground` on
/// `surface_elevated`), and it raises -- a heavier border in the focus colour --
/// when hovered or pressed. Ablated by returning iced's default style.
#[test]
fn a_button_is_drawn_from_the_configured_roles() {
    use iced::widget::button::Status;
    use tekstide_core::config::{Colour, FontSettings, ThemeRole, ThemeSettings};

    let mut settings = ThemeSettings::default();
    settings
        .overrides
        .insert(ThemeRole::SurfaceElevated, Colour::rgb(0.10, 0.20, 0.30));
    settings
        .overrides
        .insert(ThemeRole::Foreground, Colour::rgb(1.0, 1.0, 1.0));
    settings
        .overrides
        .insert(ThemeRole::BorderDefault, Colour::rgb(0.5, 0.6, 0.7));
    settings
        .overrides
        .insert(ThemeRole::BorderFocused, Colour::rgb(1.0, 0.7, 0.0));
    let theme = Theme::from_settings(&settings, &FontSettings::default(), None);
    let style = super::button_style(theme);
    let base = iced::Theme::Dark;

    let active = style(&base, Status::Active);
    assert_eq!(
        active.background,
        Some(iced::Background::Color(theme.surface_elevated()))
    );
    assert_eq!(active.text_color, theme.foreground());
    assert_eq!(active.border.color, theme.border_default());
    assert_eq!(active.border.width, 1.0);

    for raised in [Status::Hovered, Status::Pressed] {
        let style = style(&base, raised);
        assert_eq!(style.border.color, theme.border_focused());
        assert!(style.border.width > active.border.width);
        assert_eq!(style.text_color, theme.foreground());
    }
    // PR-076-D3, 2026-11-11: `Status::Disabled` used to dim `text_color` to
    // `a: 0.6` here -- removed as dead code (`Status::Disabled` cannot occur
    // in the shipped product; see `button_style`'s own doc comment), not
    // tuned, because the dimmed pair measured 5.59:1, below the AAA floor
    // `derived-contrast-pairs.md`'s D3 section holds the default theme to.
    // Asserting equality with `active`'s own text colour, rather than
    // dropping the case, is deliberate: a future reintroduction of dimming
    // here must fail this assertion before it ships unmeasured again.
    assert_eq!(
        style(&base, Status::Disabled).text_color,
        theme.foreground()
    );
}

/// `iced::Element` gives a test no way to ask a button what style it carries, so
/// the wiring is held at the source: the one function every button is built with
/// applies [`super::button_style`]. (`a_button_is_drawn_from_the_configured_roles`
/// holds the style itself; the scan in the shell holds that no view builds a
/// button any other way.) Ablated by removing the `.style(..)`.
///
/// **Review 670 required-1.** Originally asserted `flat.contains(..)`, which
/// a chained call after the expected expression survives (it would still
/// *contain* the substring) -- the same hole review 670 found in
/// `text_extent::tests::theme_text_overrides_nothing_the_harness_assumes_
/// is_default` and asked to be fixed "in the same pass or filed with the
/// reason." Fixed here, in the same pass: asserts the function body
/// **equals** the expected expression exactly, via [`function_body`]'s
/// bracket-depth extraction, so `.padding(..)` or anything else appended
/// after `.style(button_style(theme))` fails this test rather than passing
/// it.
#[test]
fn the_theme_button_applies_the_theme_style() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/theme.rs"),
    )
    .expect("theme.rs");
    let shipped = source.split("#[cfg(test)]\nmod tests").next().unwrap();
    let body = function_body(shipped, "pub(crate) fn button<");
    let flat_body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        flat_body, "iced::widget::button(content).style(button_style(theme))",
        "`theme::button`'s body must be exactly `iced::widget::button(content).style(\
         button_style(theme))` -- nothing chained after it"
    );
}

/// Extracts the `{ ... }` body immediately following the first occurrence of
/// `signature_prefix` in `source`, by bracket depth -- not by looking for
/// the next `pub(crate) fn` (which a multi-line signature or a nested
/// closure could shift) and not by a fixed line count. `text_extent::tests`
/// carries its own copy of this same small shape rather than importing it:
/// this module is private (`mod tests;`) and widening it for three lines of
/// reuse across two sibling test modules was not worth it.
fn function_body(source: &str, signature_prefix: &str) -> String {
    let after_signature = source
        .split(signature_prefix)
        .nth(1)
        .unwrap_or_else(|| panic!("{signature_prefix:?} not found in source"));
    let bytes = after_signature.as_bytes();
    let open = after_signature
        .find('{')
        .unwrap_or_else(|| panic!("no function body opens after {signature_prefix:?}"));
    let mut depth: i32 = 0;
    let mut i = open;
    loop {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    after_signature[open + 1..i].to_owned()
}
