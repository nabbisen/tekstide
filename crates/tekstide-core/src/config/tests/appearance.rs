//! RFC-054 PR-054-B: `[theme]` and `[font]`.

use std::collections::BTreeMap;

use crate::config::{
    CONTRAST_RULES, Colour, ColourError, ContrastFor, ContrastRatio, FallbackReason, FamilyError,
    MAX_FONT_FAMILY_CHARS, MAX_SCRIM_ALPHA, MIN_FOCUS_CONTRAST, MIN_TEXT_CONTRAST, Palette,
    SettingFallback, ThemeRole, contrast_ratio, enforce_contrast, parse_and_validate,
};

fn outcome(source: &str) -> crate::config::ConfigLoadOutcome {
    parse_and_validate(source).expect("a bad appearance value is a fallback, not a refusal")
}

fn reasons(source: &str) -> Vec<(String, FallbackReason)> {
    outcome(source)
        .fallbacks
        .into_iter()
        .map(|fallback| (fallback.setting, fallback.reason))
        .collect()
}

fn ratio_of(source: &str, setting: &str) -> (u32, ThemeRole) {
    let found = reasons(source)
        .into_iter()
        .find(|(name, _)| name == setting)
        .unwrap_or_else(|| panic!("no fallback for {setting}"));
    match found.1 {
        FallbackReason::LowContrast { ratio, against, .. } => (ratio.hundredths(), against),
        other => panic!("{setting} fell back for {other:?}, not contrast"),
    }
}

// --- the shipped palette ----------------------------------------------------

/// The validator's own precondition, and what makes its loop terminate: with
/// nothing configured, every rule already holds.
#[test]
fn the_shipped_palette_meets_the_rules_it_enforces() {
    let palette = Palette::default();
    for (drawn, surface, purpose) in CONTRAST_RULES {
        let ratio = contrast_ratio(palette.get(drawn), palette.get(surface));
        assert!(
            ratio >= purpose.minimum(),
            "{drawn:?} on {surface:?} is {ratio}"
        );
    }
    assert_eq!(ContrastFor::Text.minimum(), MIN_TEXT_CONTRAST);
    assert_eq!(ContrastFor::FocusIndicator.minimum(), MIN_FOCUS_CONTRAST);
}

// --- colours ------------------------------------------------------------------

#[test]
fn a_readable_theme_is_kept_whole() {
    let document = outcome(
        "[theme]\nbackground = \"#000000\"\nforeground = \"#FFFFFF\"\naccent = \"#ff8800\"\n\
         border_default = \"#888888\"\nsurface_elevated = \"#101010\"\nscrim = \"#00000080\"\n",
    );
    assert!(document.fallbacks.is_empty(), "{:?}", document.fallbacks);
    let overrides = &document.document.theme.overrides;
    assert_eq!(overrides.len(), 6);
    assert_eq!(
        overrides[&ThemeRole::Background],
        Colour::rgb(0.0, 0.0, 0.0)
    );
    assert_eq!(
        overrides[&ThemeRole::Foreground],
        Colour::rgb(1.0, 1.0, 1.0)
    );
    // Case-insensitive hex, and the scrim keeps its alpha.
    assert_eq!(overrides[&ThemeRole::Accent].r, 1.0);
    assert!((overrides[&ThemeRole::Scrim].a - 128.0 / 255.0).abs() < 1e-6);
}

/// **D5.** A pair below the minimum falls back, and the diagnostic carries the
/// measured ratio -- a number the user can act on. White against the shipped
/// foreground (`0.90` grey, luminance 0.7873) is (1.05)/(0.8373) = 1.2539.
/// Ablated by dropping the contrast check: this fails, and nothing else does.
#[test]
fn a_low_contrast_pair_falls_back_and_the_diagnostic_names_the_measured_ratio() {
    let source = "[theme]\nbackground = \"#FFFFFF\"\n";
    let (hundredths, against) = ratio_of(source, "theme.background");
    assert_eq!(hundredths, 125, "measured ratio, floored to hundredths");
    assert_eq!(against, ThemeRole::Foreground);
    assert!(
        outcome(source).document.theme.overrides.is_empty(),
        "the low-contrast colour must not be in force"
    );
}

#[test]
fn the_measured_ratio_is_floored_so_a_miss_can_never_read_as_a_pass() {
    assert_eq!(ContrastRatio::from_ratio(4.499).to_string(), "4.49");
    assert_eq!(ContrastRatio::from_ratio(4.5).to_string(), "4.50");
    assert_eq!(ContrastRatio::from_ratio(21.0).to_string(), "21.00");
    assert_eq!(ContrastRatio::from_ratio(4.499).digits(), (4, 4, 9));
}

/// Exactly at the minimum passes; the smallest step below it does not. Built
/// from the arithmetic, not from a hand-picked colour, so the edge is the edge.
#[test]
fn the_minimum_is_inclusive_at_exactly_4_5() {
    let background = Colour::rgb(0.0, 0.0, 0.0);
    // Find the grey whose ratio against black is the smallest one >= 4.5.
    let mut passing = None;
    for step in 0..=255u32 {
        let grey = step as f32 / 255.0;
        if contrast_ratio(Colour::rgb(grey, grey, grey), background) >= MIN_TEXT_CONTRAST {
            passing = Some(step);
            break;
        }
    }
    let step = passing.expect("some grey clears 4.5:1 on black");
    let hex = |step: u32| format!("#{step:02X}{step:02X}{step:02X}");

    let mut fallbacks = Vec::new();
    let kept = enforce_contrast(
        BTreeMap::from([
            (ThemeRole::Background, background),
            (
                ThemeRole::Foreground,
                Colour::parse_hex(&hex(step), false).unwrap(),
            ),
            // The shipped surface is dark; lift it out of the way so this
            // exercises only the pair under test.
            (ThemeRole::SurfaceElevated, Colour::rgb(0.0, 0.0, 0.0)),
        ]),
        &mut fallbacks,
    );
    assert!(fallbacks.is_empty(), "{fallbacks:?}");
    assert_eq!(kept.len(), 3);

    let mut fallbacks = Vec::new();
    let kept = enforce_contrast(
        BTreeMap::from([
            (ThemeRole::Background, background),
            (
                ThemeRole::Foreground,
                Colour::parse_hex(&hex(step - 1), false).unwrap(),
            ),
            (ThemeRole::SurfaceElevated, Colour::rgb(0.0, 0.0, 0.0)),
        ]),
        &mut fallbacks,
    );
    assert!(
        kept.is_empty() || !kept.contains_key(&ThemeRole::Foreground),
        "one step below the minimum was kept"
    );
    assert!(!fallbacks.is_empty());
}

/// A light `background`, a dark `foreground` and a dark focus border agree with
/// one another and all fail the *shipped* dark `surface_elevated` behind every
/// dialog. Taking those two back leaves the light `background` against the
/// *shipped* light foreground and blue focus border, which it now fails -- so the
/// check must run to a fixed point, and say so for each colour it took back.
#[test]
fn reverting_colours_can_fail_another_pair_so_the_check_repeats() {
    let source = "[theme]\nbackground = \"#EEEEEE\"\nforeground = \"#111111\"\n\
                  border_focused = \"#0050A0\"\n";
    let reverted: Vec<String> = reasons(source)
        .into_iter()
        .map(|(setting, _)| setting)
        .collect();
    assert_eq!(
        reverted,
        [
            "theme.foreground",
            "theme.border_focused",
            "theme.background"
        ],
        "round one takes the two that fail the shipped surface; round two the background"
    );
    assert!(outcome(source).document.theme.overrides.is_empty());

    // The user's way out, which the diagnostics make findable: give the surface
    // a colour too.
    let fixed = "[theme]\nbackground = \"#EEEEEE\"\nforeground = \"#111111\"\n\
                 border_focused = \"#0050A0\"\nsurface_elevated = \"#DDDDDD\"\n";
    assert!(
        outcome(fixed).fallbacks.is_empty(),
        "{:?}",
        outcome(fixed).fallbacks
    );
}

/// The property, over input nobody chose: **whatever a file says, the palette
/// that comes out meets D5**, every colour kept is one the user wrote, and every
/// colour dropped was reported.
#[test]
fn the_palette_that_comes_out_always_meets_every_rule() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..3000 {
        let mut wanted = BTreeMap::new();
        for role in ThemeRole::ALL {
            if next() % 2 == 0 {
                let bits = next();
                wanted.insert(
                    role,
                    Colour::rgb(
                        (bits & 0xFF) as f32 / 255.0,
                        (bits >> 8 & 0xFF) as f32 / 255.0,
                        (bits >> 16 & 0xFF) as f32 / 255.0,
                    ),
                );
            }
        }
        let mut fallbacks: Vec<SettingFallback> = Vec::new();
        let kept = enforce_contrast(wanted.clone(), &mut fallbacks);

        let palette = Palette::with_overrides(&kept);
        for (drawn, surface, purpose) in CONTRAST_RULES {
            assert!(
                contrast_ratio(palette.get(drawn), palette.get(surface)) >= purpose.minimum(),
                "{wanted:?} produced an unreadable {drawn:?} on {surface:?}"
            );
        }
        for (role, colour) in &kept {
            assert_eq!(wanted.get(role), Some(colour), "kept a colour nobody wrote");
        }
        for role in wanted.keys().filter(|role| !kept.contains_key(role)) {
            assert!(
                fallbacks
                    .iter()
                    .any(|f| f.setting == format!("theme.{}", role.config_name())),
                "{role:?} was dropped without a word"
            );
        }
    }
}

#[test]
fn a_bad_colour_falls_back_alone_and_the_rest_of_the_file_applies() {
    let source = "[theme]\nbackground = \"red\"\nforeground = \"#12345\"\naccent = \"#GGGGGG\"\n\
                  border_default = \"888888\"\nborder_focused = 7\nsurface_elevated = \"#10101099\"\n\
                  scrim = \"#00000080\"\n[font]\nbody_size = 15\n";
    let found = reasons(source);
    assert_eq!(
        found,
        vec![
            (
                "theme.accent".to_owned(),
                FallbackReason::BadColour(ColourError::Malformed)
            ),
            (
                "theme.background".to_owned(),
                FallbackReason::BadColour(ColourError::Malformed)
            ),
            (
                "theme.border_default".to_owned(),
                FallbackReason::BadColour(ColourError::Malformed)
            ),
            (
                "theme.border_focused".to_owned(),
                FallbackReason::NotAString
            ),
            (
                "theme.foreground".to_owned(),
                FallbackReason::BadColour(ColourError::Malformed)
            ),
            (
                "theme.surface_elevated".to_owned(),
                FallbackReason::BadColour(ColourError::AlphaNotAllowed)
            ),
        ]
    );
    let document = outcome(source).document;
    assert_eq!(document.theme.overrides.len(), 1, "the valid scrim applies");
    assert_eq!(document.font.body_size, Some(15.0), "and so does [font]");
}

// --- sizes --------------------------------------------------------------------

#[test]
fn each_size_bound_is_inclusive_and_refuses_just_outside() {
    for key in ["body_size", "heading_size", "status_size"] {
        let setting = format!("font.{key}");
        for accepted in ["8", "8.0", "14", "31.9", "32", "32.0"] {
            let source = format!("[font]\n{key} = {accepted}\n");
            assert!(
                outcome(&source).fallbacks.is_empty(),
                "{key} = {accepted} refused"
            );
        }
        for refused in ["7.99", "7", "0", "-14", "32.01", "33", "1000000"] {
            let source = format!("[font]\n{key} = {refused}\n");
            assert_eq!(
                reasons(&source),
                vec![(setting.clone(), FallbackReason::SizeOutOfRange)],
                "{key} = {refused}"
            );
        }
        for not_a_number in ["\"big\"", "true", "[14]"] {
            let source = format!("[font]\n{key} = {not_a_number}\n");
            assert_eq!(
                reasons(&source),
                vec![(setting.clone(), FallbackReason::NotANumber)],
                "{key} = {not_a_number}"
            );
        }
    }
    // TOML has `nan` and `inf`, and `contains` on a range is false for both.
    for weird in ["nan", "inf", "-inf"] {
        let source = format!("[font]\nbody_size = {weird}\n");
        assert_eq!(
            reasons(&source),
            vec![("font.body_size".to_owned(), FallbackReason::SizeOutOfRange)],
            "{weird}"
        );
    }
}

#[test]
fn one_bad_size_does_not_take_the_others_with_it() {
    let document = outcome("[font]\nbody_size = 3\nheading_size = 20\nstatus_size = 11\n").document;
    assert_eq!(document.font.body_size, None);
    assert_eq!(document.font.sizes().body, 14.0, "the default stands");
    assert_eq!(document.font.sizes().heading, 20.0);
    assert_eq!(document.font.sizes().status, 11.0);
}

// --- family -------------------------------------------------------------------

#[test]
fn a_family_is_a_name_and_only_a_name() {
    let kept = |source: &str| outcome(source).document.font.family;
    assert_eq!(
        kept("[font]\nfamily = \"  Inter Tight \"\n").as_deref(),
        Some("Inter Tight"),
        "trimmed"
    );
    let longest = "a".repeat(MAX_FONT_FAMILY_CHARS);
    assert_eq!(
        kept(&format!("[font]\nfamily = \"{longest}\"\n")).as_deref(),
        Some(longest.as_str())
    );

    let refused = |value: &str| {
        let source = format!("[font]\nfamily = {value}\n");
        let found = reasons(&source);
        assert!(outcome(&source).document.font.family.is_none(), "{value}");
        found
    };
    let family = |error| vec![("font.family".to_owned(), FallbackReason::BadFamily(error))];
    assert_eq!(refused("\"\""), family(FamilyError::Empty));
    assert_eq!(refused("\"   \""), family(FamilyError::Empty));
    assert_eq!(
        refused(&format!("\"{}\"", "a".repeat(MAX_FONT_FAMILY_CHARS + 1))),
        family(FamilyError::TooLong)
    );
    // §3: a path is not a name. Each of these would be a file open if anything
    // downstream ever opened what it was given.
    for path in [
        "\"/usr/share/fonts/x.ttf\"",
        "\"fonts/x.ttf\"",
        "\"..\\\\x.ttf\"",
        "'C:\\Windows\\Fonts\\arial.ttf'",
        "\"a\\u0007b\"",
        "\"a\\nb\"",
    ] {
        assert_eq!(
            refused(path),
            family(FamilyError::ForbiddenCharacter),
            "{path}"
        );
    }
    assert_eq!(
        refused("14"),
        vec![("font.family".to_owned(), FallbackReason::NotAString)]
    );
}

// --- §6: no configured string is echoed ---------------------------------------

/// A fallback is a setting name this crate defines and a closed reason, so there
/// is nowhere for a configured string to be. This holds the *type* to it by
/// feeding every kind of hostile value through the parser and demanding that
/// nothing the file said appears in anything that comes back -- and that an
/// unknown key, the one place file text does reach a diagnostic, is escaped.
#[test]
fn no_diagnostic_echoes_a_configured_string() {
    let hostile = "\u{1b}[31mEVIL\u{202e}/etc/passwd\u{0}\n";
    let toml_escaped = hostile
        .replace('\u{1b}', "\\u001b")
        .replace('\u{202e}', "\\u202e")
        .replace('\u{0}', "\\u0000")
        .replace('\n', "\\n");
    let source = format!(
        "[theme]\nbackground = \"{toml_escaped}\"\nforeground = \"{toml_escaped}\"\n\
         accent = 5\n\"{toml_escaped}\" = \"#000000\"\n\
         [font]\nfamily = \"{toml_escaped}\"\nbody_size = \"{toml_escaped}\"\n\
         \"{toml_escaped}\" = 1\n"
    );
    let outcome = outcome(&source);
    assert!(!outcome.fallbacks.is_empty());
    let known_settings: Vec<String> = ThemeRole::ALL
        .iter()
        .map(|role| format!("theme.{}", role.config_name()))
        .chain(
            ["family", "body_size", "heading_size", "status_size"]
                .iter()
                .map(|key| format!("font.{key}")),
        )
        .collect();
    for fallback in &outcome.fallbacks {
        assert!(
            known_settings.contains(&fallback.setting),
            "a fallback named {:?}, which this crate does not define",
            fallback.setting
        );
        let rendered = format!("{fallback:?}");
        assert!(
            !rendered.contains("EVIL") && !rendered.contains("passwd"),
            "{rendered}"
        );
    }
    // The two unknown keys warn -- once each, bounded and escaped.
    assert_eq!(outcome.warnings.len(), 2, "{:?}", outcome.warnings);
    for warning in &outcome.warnings {
        assert!(
            !warning
                .key
                .chars()
                .any(|c| c.is_control() || c == '\u{202e}'),
            "{:?}",
            warning.key
        );
    }
}

// --- the section itself -------------------------------------------------------

#[test]
fn a_theme_or_font_section_that_is_not_a_table_refuses_the_file() {
    for source in ["theme = \"dark\"\n", "font = 14\n"] {
        let error = parse_and_validate(source).unwrap_err();
        assert_eq!(error.message, "expected a table", "{source}");
    }
}

#[test]
fn an_unknown_key_in_theme_or_font_warns_and_does_nothing() {
    let outcome = outcome("[theme]\nchrome = \"#000000\"\n[font]\nweight = 700\n");
    let keys: Vec<&str> = outcome.warnings.iter().map(|w| w.key.as_str()).collect();
    assert_eq!(keys, ["theme.chrome", "font.weight"]);
    assert!(outcome.document.theme.overrides.is_empty());
}

// --- review 428 R1: the focus border ------------------------------------------

/// A focus indicator you cannot see is a property the product claims and does
/// not have. 3:1 against each surface it is drawn on; the fallback names the
/// purpose so the board can say which minimum it missed.
#[test]
fn a_focus_border_below_3_to_1_falls_back_and_names_the_measured_ratio() {
    let source = "[theme]\nborder_focused = \"#101014\"\n";
    let found = reasons(source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, "theme.border_focused");
    match found[0].1 {
        FallbackReason::LowContrast {
            ratio,
            against,
            purpose,
        } => {
            assert_eq!(purpose, ContrastFor::FocusIndicator);
            assert_eq!(against, ThemeRole::Background);
            assert!(
                ratio.hundredths() < 300 && ratio.hundredths() >= 100,
                "{ratio}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(outcome(source).document.theme.overrides.is_empty());

    // Exactly-at and just-under, from the arithmetic: the smallest grey that
    // clears 3:1 on the shipped background is kept; one step darker is not.
    let background = Palette::default().background;
    let hex = |step: u32| format!("#{step:02X}{step:02X}{step:02X}");
    let step = (0..=255u32)
        .find(|step| {
            let grey = *step as f32 / 255.0;
            contrast_ratio(Colour::rgb(grey, grey, grey), background) >= MIN_FOCUS_CONTRAST
                && contrast_ratio(
                    Colour::rgb(grey, grey, grey),
                    Palette::default().surface_elevated,
                ) >= MIN_FOCUS_CONTRAST
        })
        .expect("some grey clears 3:1 on both surfaces");
    assert!(
        outcome(&format!("[theme]\nborder_focused = \"{}\"\n", hex(step)))
            .fallbacks
            .is_empty()
    );
    assert!(
        !outcome(&format!(
            "[theme]\nborder_focused = \"{}\"\n",
            hex(step - 1)
        ))
        .fallbacks
        .is_empty()
    );
}

/// `accent` and the non-focus border are colours the user chose; they decorate,
/// and the word or second channel is always there. Deliberately unmeasured, and
/// said so in the book -- a decision, not an oversight.
#[test]
fn accent_and_the_ordinary_border_are_not_measured() {
    let source = "[theme]\naccent = \"#0B0B0F\"\nborder_default = \"#0B0B0F\"\n";
    let document = outcome(source);
    assert!(document.fallbacks.is_empty(), "{:?}", document.fallbacks);
    assert_eq!(document.document.theme.overrides.len(), 2);
}

// --- review 428 R2: the scrim stays a dimming layer ---------------------------

#[test]
fn a_scrim_more_opaque_than_the_cap_is_reduced_to_it_with_a_diagnostic() {
    // `#000000` has no alpha digits: it is fully opaque, and is exactly the
    // rectangle the cap exists to prevent.
    let source = "[theme]\nscrim = \"#000000\"\n";
    let found = outcome(source);
    assert_eq!(
        found.fallbacks,
        vec![SettingFallback {
            setting: "theme.scrim".to_owned(),
            reason: FallbackReason::ScrimTooOpaque,
        }]
    );
    let scrim = found.document.theme.overrides[&ThemeRole::Scrim];
    assert_eq!(
        scrim.a, MAX_SCRIM_ALPHA,
        "in force at the limit, not dropped"
    );
    assert_eq!((scrim.r, scrim.g, scrim.b), (0.0, 0.0, 0.0));

    // The edge, in bytes: 0xE5 = 229/255 = 0.898 is under 0.90 and kept as
    // written; 0xE6 = 230/255 = 0.902 is over it.
    let kept = outcome("[theme]\nscrim = \"#000000E5\"\n");
    assert!(kept.fallbacks.is_empty());
    assert!((kept.document.theme.overrides[&ThemeRole::Scrim].a - 229.0 / 255.0).abs() < 1e-6);
    let over = outcome("[theme]\nscrim = \"#000000E6\"\n");
    assert_eq!(over.fallbacks.len(), 1);
    assert_eq!(
        over.document.theme.overrides[&ThemeRole::Scrim].a,
        MAX_SCRIM_ALPHA
    );
}
