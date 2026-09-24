//! **RFC-054 PR-054-B.** What the window looks like, as a user may set it: seven
//! colour roles, a font family, three font sizes.
//!
//! This is the *pure* half. It owns the shipped palette and sizes (so
//! `Theme::default` and the validator can never disagree about what "the
//! default" is), the colour grammar, the contrast arithmetic, and the rule that
//! a configured pair below [`MIN_TEXT_CONTRAST`] does not get used. It knows
//! nothing about `iced` and nothing about fonts on disk: whether a family is
//! *installed* is asked of the renderer's own font database by the shell, and a
//! family here is only ever a **name** (RFC-054 D6, §3 -- never a path).

use std::collections::BTreeMap;
use std::fmt;

use super::model::{FallbackReason, SettingFallback};

/// WCAG AA for body text. RFC-054 D5.
pub const MIN_TEXT_CONTRAST: f32 = 4.5;

/// RFC-054 D6: a font size outside this range falls back to the default.
pub const MIN_FONT_SIZE_PX: f32 = 8.0;
pub const MAX_FONT_SIZE_PX: f32 = 32.0;

/// A family name longer than this is not a family name. Bounds what the shell
/// has to keep for the life of the process (see `UiFont` there) as much as
/// what a diagnostic has to say.
pub const MAX_FONT_FAMILY_CHARS: usize = 64;

/// One colour, straight (non-premultiplied) alpha, components in `0.0..=1.0`.
///
/// `f32` rather than the `u8` a hex spelling carries, because the *shipped*
/// palette is defined in `f32` (`0.08`, `0.90`, ...) and re-expressing it in
/// bytes would move it by up to half a step per channel -- a change to the
/// default look that no one asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Colour {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// `#RRGGBB`, or `#RRGGBBAA` when `allow_alpha`. Nothing else: no names, no
    /// `rgb()`, no shorthand -- one spelling, so there is one thing to learn and
    /// nothing to guess at.
    pub fn parse_hex(spelling: &str, allow_alpha: bool) -> Result<Self, ColourError> {
        let digits = spelling.strip_prefix('#').ok_or(ColourError::Malformed)?;
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ColourError::Malformed);
        }
        let byte = |index: usize| -> f32 {
            let pair = &digits[index * 2..index * 2 + 2];
            f32::from(u8::from_str_radix(pair, 16).unwrap_or(0)) / 255.0
        };
        match digits.len() {
            6 => Ok(Self::rgb(byte(0), byte(1), byte(2))),
            8 if allow_alpha => Ok(Self::rgba(byte(0), byte(1), byte(2), byte(3))),
            8 => Err(ColourError::AlphaNotAllowed),
            _ => Err(ColourError::Malformed),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColourError {
    /// Not `#` and six hex digits (or eight, where alpha is allowed).
    Malformed,
    /// Eight digits, on a role that is opaque by definition.
    AlphaNotAllowed,
}

/// The seven roles `Theme` already names -- no new ones (RFC-054 §7).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ThemeRole {
    Background,
    Foreground,
    Accent,
    BorderDefault,
    BorderFocused,
    SurfaceElevated,
    Scrim,
}

impl ThemeRole {
    pub const ALL: [ThemeRole; 7] = [
        ThemeRole::Background,
        ThemeRole::Foreground,
        ThemeRole::Accent,
        ThemeRole::BorderDefault,
        ThemeRole::BorderFocused,
        ThemeRole::SurfaceElevated,
        ThemeRole::Scrim,
    ];

    /// The key in `[theme]`, and the part of the setting name after `theme.`.
    pub const fn config_name(self) -> &'static str {
        match self {
            ThemeRole::Background => "background",
            ThemeRole::Foreground => "foreground",
            ThemeRole::Accent => "accent",
            ThemeRole::BorderDefault => "border_default",
            ThemeRole::BorderFocused => "border_focused",
            ThemeRole::SurfaceElevated => "surface_elevated",
            ThemeRole::Scrim => "scrim",
        }
    }

    pub fn from_config_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|role| role.config_name() == name)
    }

    /// Only the scrim is translucent by design (RFC-018 PR-018-G).
    pub const fn allows_alpha(self) -> bool {
        matches!(self, ThemeRole::Scrim)
    }
}

/// The text pairs D5 holds to [`MIN_TEXT_CONTRAST`]: what is drawn *on* the two
/// surfaces text sits on. `accent`, the borders and the scrim are never text --
/// they are restyled, and validated only as far as being colours (see the
/// judgment call recorded in `qa-evidence.md`).
pub const TEXT_PAIRS: [(ThemeRole, ThemeRole); 2] = [
    (ThemeRole::Foreground, ThemeRole::Background),
    (ThemeRole::Foreground, ThemeRole::SurfaceElevated),
];

/// Every role's colour. `Default` is the **shipped** palette; `Theme::default`
/// is built from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: Colour,
    pub foreground: Colour,
    pub accent: Colour,
    pub border_default: Colour,
    pub border_focused: Colour,
    pub surface_elevated: Colour,
    pub scrim: Colour,
}

impl Palette {
    pub fn get(&self, role: ThemeRole) -> Colour {
        match role {
            ThemeRole::Background => self.background,
            ThemeRole::Foreground => self.foreground,
            ThemeRole::Accent => self.accent,
            ThemeRole::BorderDefault => self.border_default,
            ThemeRole::BorderFocused => self.border_focused,
            ThemeRole::SurfaceElevated => self.surface_elevated,
            ThemeRole::Scrim => self.scrim,
        }
    }

    fn set(&mut self, role: ThemeRole, colour: Colour) {
        match role {
            ThemeRole::Background => self.background = colour,
            ThemeRole::Foreground => self.foreground = colour,
            ThemeRole::Accent => self.accent = colour,
            ThemeRole::BorderDefault => self.border_default = colour,
            ThemeRole::BorderFocused => self.border_focused = colour,
            ThemeRole::SurfaceElevated => self.surface_elevated = colour,
            ThemeRole::Scrim => self.scrim = colour,
        }
    }

    /// The shipped palette with `overrides` laid over it.
    pub fn with_overrides(overrides: &BTreeMap<ThemeRole, Colour>) -> Self {
        let mut palette = Self::default();
        for (role, colour) in overrides {
            palette.set(*role, *colour);
        }
        palette
    }
}

impl Default for Palette {
    /// The values behind `Theme::default`, moved here so that the palette the
    /// validator measures a configured pair against **is** the palette the
    /// window draws when nothing is configured. Their rationale (the contrast
    /// each border and the scrim were raised to) stays with `Theme`'s tests,
    /// which still hold these numbers to it.
    fn default() -> Self {
        Self {
            background: Colour::rgb(0.08, 0.08, 0.09),
            foreground: Colour::rgb(0.90, 0.90, 0.90),
            accent: Colour::rgb(0.30, 0.60, 1.0),
            border_default: Colour::rgb(0.45, 0.45, 0.45),
            border_focused: Colour::rgb(0.30, 0.60, 1.0),
            surface_elevated: Colour::rgb(0.12, 0.12, 0.12),
            scrim: Colour::rgba(0.0, 0.0, 0.0, 0.75),
        }
    }
}

/// The three sizes `Theme` names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontSizes {
    pub body: f32,
    pub heading: f32,
    pub status: f32,
}

impl Default for FontSizes {
    fn default() -> Self {
        Self {
            body: 14.0,
            heading: 16.0,
            status: 13.0,
        }
    }
}

/// `[theme]`: the colour overrides that **survived** validation -- a refused one
/// is a [`SettingFallback`], not an entry here, so the palette
/// [`Palette::with_overrides`] builds from these always satisfies
/// [`TEXT_PAIRS`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThemeSettings {
    pub overrides: BTreeMap<ThemeRole, Colour>,
}

/// `[font]`: what survived. `family` is syntactically a name; whether it is
/// *installed* is the shell's question (it holds the renderer's font database),
/// answered after this and reported as [`FallbackReason::FamilyUnavailable`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontSettings {
    pub family: Option<String>,
    pub body_size: Option<f32>,
    pub heading_size: Option<f32>,
    pub status_size: Option<f32>,
}

impl FontSettings {
    pub fn sizes(&self) -> FontSizes {
        let defaults = FontSizes::default();
        FontSizes {
            body: self.body_size.unwrap_or(defaults.body),
            heading: self.heading_size.unwrap_or(defaults.heading),
            status: self.status_size.unwrap_or(defaults.status),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FamilyError {
    Empty,
    TooLong,
    /// A control character, or a path separator. **The second is the point:** a
    /// font is a name, and the one thing that must never happen is a string a
    /// user wrote reaching a file open (§3). The name is only ever *compared*
    /// with family names the font database already holds, so it could not; this
    /// makes a path-shaped one a mistake the user is told about instead of an
    /// "unavailable" they might chase.
    ForbiddenCharacter,
}

/// A family name, or why it is not one. Trimmed; nothing else is normalised.
pub fn parse_family(spelling: &str) -> Result<String, FamilyError> {
    let name = spelling.trim();
    if name.is_empty() {
        return Err(FamilyError::Empty);
    }
    if name.chars().count() > MAX_FONT_FAMILY_CHARS {
        return Err(FamilyError::TooLong);
    }
    if name
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(FamilyError::ForbiddenCharacter);
    }
    Ok(name.to_owned())
}

/// A contrast ratio as the diagnostic states it: **floored** to hundredths, so a
/// pair that misses the minimum can never be printed as meeting it (`4.499`
/// reads `4.49`, never `4.50`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContrastRatio {
    hundredths: u32,
}

impl ContrastRatio {
    pub fn from_ratio(ratio: f32) -> Self {
        Self {
            hundredths: (ratio * 100.0).floor().max(0.0) as u32,
        }
    }

    pub fn hundredths(self) -> u32 {
        self.hundredths
    }

    /// The three digits of `W.TH`, for a catalog that must not let a locale
    /// reformat a number it is only *quoting* (`4.49` is a measurement, not a
    /// quantity to be grouped or localised).
    pub fn digits(self) -> (u32, u32, u32) {
        (
            self.hundredths / 100,
            self.hundredths / 10 % 10,
            self.hundredths % 10,
        )
    }
}

impl fmt::Display for ContrastRatio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.hundredths / 100, self.hundredths % 100)
    }
}

/// WCAG 2.1 relative luminance of an opaque colour. Alpha is ignored.
pub fn relative_luminance(colour: Colour) -> f32 {
    fn linearize(channel: f32) -> f32 {
        // The formula's own boundary: 0.03928, not 0.04045 -- the usual
        // transcription error, which is why `theme/contrast/tests.rs` checks
        // known anchor values through this function.
        if channel <= 0.03928 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(colour.r) + 0.7152 * linearize(colour.g) + 0.0722 * linearize(colour.b)
}

/// `(lighter + 0.05) / (darker + 0.05)`, symmetric.
pub fn contrast_ratio(first: Colour, second: Colour) -> f32 {
    let first = relative_luminance(first);
    let second = relative_luminance(second);
    let (lighter, darker) = if first >= second {
        (first, second)
    } else {
        (second, first)
    };
    (lighter + 0.05) / (darker + 0.05)
}

/// `[theme]`. A bad or unreadable colour falls back **on its own**, and the rest
/// of the file still applies (D4).
///
/// The contrast rule is evaluated over the palette the user's colours *make*,
/// against the shipped colours they left alone -- so setting only `background`
/// to white is measured against the default foreground, and fails. When a text
/// pair is below [`MIN_TEXT_CONTRAST`], **every configured member of it** goes
/// back to its default and says the measured ratio; then the palette is
/// measured again, because reverting one member can fail a pair it had passed
/// (a light `background` and a dark `foreground` that agree with each other can
/// both fail the *default* `surface_elevated`). Each round removes at least one
/// configured colour and the all-default palette passes, so it terminates -- and
/// the palette that comes out **always** meets D5, which the tests state over
/// arbitrary input rather than for the cases anyone thought of.
pub(super) fn extract_theme(
    root: &mut toml::Table,
    warnings: &mut Vec<super::load::ConfigWarning>,
    fallbacks: &mut Vec<SettingFallback>,
) -> Result<ThemeSettings, super::load::ConfigDiagnostic> {
    let Some(table) = super::load::section_table(root, "theme")? else {
        return Ok(ThemeSettings::default());
    };

    let mut wanted: BTreeMap<ThemeRole, Colour> = BTreeMap::new();
    for (name, value) in table {
        let Some(role) = ThemeRole::from_config_name(&name) else {
            warnings.push(super::load::ConfigWarning {
                key: format!("theme.{}", super::load::bound_key_segment(&name)),
            });
            continue;
        };
        let setting = format!("theme.{}", role.config_name());
        let toml::Value::String(spelling) = value else {
            fallbacks.push(SettingFallback {
                setting,
                reason: FallbackReason::NotAString,
            });
            continue;
        };
        match Colour::parse_hex(&spelling, role.allows_alpha()) {
            Ok(colour) => {
                wanted.insert(role, colour);
            }
            Err(error) => fallbacks.push(SettingFallback {
                setting,
                reason: FallbackReason::BadColour(error),
            }),
        }
    }

    let overrides = enforce_text_contrast(wanted, fallbacks);
    Ok(ThemeSettings { overrides })
}

/// The contrast half of [`extract_theme`], on its own so it can be held to its
/// invariant over generated palettes.
pub fn enforce_text_contrast(
    mut kept: BTreeMap<ThemeRole, Colour>,
    fallbacks: &mut Vec<SettingFallback>,
) -> BTreeMap<ThemeRole, Colour> {
    loop {
        let palette = Palette::with_overrides(&kept);
        let failing: Vec<(ThemeRole, ThemeRole, f32)> = TEXT_PAIRS
            .iter()
            .map(|&(text, surface)| {
                (
                    text,
                    surface,
                    contrast_ratio(palette.get(text), palette.get(surface)),
                )
            })
            .filter(|&(_, _, ratio)| ratio < MIN_TEXT_CONTRAST)
            .collect();
        if failing.is_empty() {
            return kept;
        }
        let mut reverted = false;
        for (text, surface, ratio) in failing {
            for (member, against) in [(text, surface), (surface, text)] {
                if kept.remove(&member).is_some() {
                    reverted = true;
                    fallbacks.push(SettingFallback {
                        setting: format!("theme.{}", member.config_name()),
                        reason: FallbackReason::LowContrast {
                            ratio: ContrastRatio::from_ratio(ratio),
                            against,
                        },
                    });
                }
            }
        }
        if !reverted {
            // Only reachable if the *shipped* palette failed its own rule, which
            // `the_shipped_palette_meets_the_rule_it_enforces` forbids. Returning
            // keeps a broken build from looping forever.
            return kept;
        }
    }
}

/// `[font]`: `family` (a name) and three sizes, each of which falls back alone.
pub(super) fn extract_font(
    root: &mut toml::Table,
    warnings: &mut Vec<super::load::ConfigWarning>,
    fallbacks: &mut Vec<SettingFallback>,
) -> Result<FontSettings, super::load::ConfigDiagnostic> {
    let Some(mut table) = super::load::section_table(root, "font")? else {
        return Ok(FontSettings::default());
    };
    let mut settings = FontSettings::default();

    if let Some(value) = table.remove("family") {
        let setting = "font.family".to_owned();
        match value {
            toml::Value::String(spelling) => match parse_family(&spelling) {
                Ok(name) => settings.family = Some(name),
                Err(error) => fallbacks.push(SettingFallback {
                    setting,
                    reason: FallbackReason::BadFamily(error),
                }),
            },
            _ => fallbacks.push(SettingFallback {
                setting,
                reason: FallbackReason::NotAString,
            }),
        }
    }

    for (key, slot) in [
        ("body_size", &mut settings.body_size),
        ("heading_size", &mut settings.heading_size),
        ("status_size", &mut settings.status_size),
    ] {
        let Some(value) = table.remove(key) else {
            continue;
        };
        let setting = format!("font.{key}");
        let number = match value {
            toml::Value::Integer(integer) => Some(integer as f32),
            toml::Value::Float(float) => Some(float as f32),
            _ => None,
        };
        match number {
            None => fallbacks.push(SettingFallback {
                setting,
                reason: FallbackReason::NotANumber,
            }),
            // `contains` is false for NaN and both infinities.
            Some(size) if (MIN_FONT_SIZE_PX..=MAX_FONT_SIZE_PX).contains(&size) => {
                *slot = Some(size);
            }
            Some(_) => fallbacks.push(SettingFallback {
                setting,
                reason: FallbackReason::SizeOutOfRange,
            }),
        }
    }

    super::load::warn_unconsumed(table, "font", warnings);
    Ok(settings)
}
