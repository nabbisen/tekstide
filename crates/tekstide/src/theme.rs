//! RFC-015 PR-015-B: the theme seam; RFC-054 supplies its values from the user's
//! configuration, and [`Theme::default`] is the shipped look when it supplies
//! none. `NFR-UX-004` requires colours and font sizes to be configurable, which
//! is only true if no widget hardcodes one -- every colour and every font size
//! the shell draws comes from a `Theme` value, never a literal
//! `Color::from_rgb(...)` or `.size(13)` written at a call site.
//!
//! **What "every" covers, exactly** (review 428 found it had stopped being true):
//! ordinary text takes its face from [`text`], and every button takes its fill,
//! text and border from [`button`] -- iced's own of each draws from
//! `iced::Theme`, which no configuration reaches, and
//! `no_shipped_view_builds_text_or_buttons_outside_the_theme` fails if a view
//! imports either. **Two things are deliberately not themed:** the colours a
//! terminal program asks for (`grid_colors` draws that data, not chrome), and
//! `Color::TRANSPARENT` where a widget must draw *no* box.
//!
//! This type is unrelated to `iced::Theme` (the base theme parameter
//! `iced`'s own style closures take, e.g. `container::Style`'s
//! `move |_theme: &iced::Theme| ...`) -- that one selects between
//! `iced`'s built-in palettes and is not used here. This module's
//! `Theme` is referenced by its full path, `crate::theme::Theme`, at
//! every call site specifically to avoid that ambiguity.

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use iced::{Color, Font};
use tekstide_core::config::{Colour, FontSettings, Palette, ThemeSettings};

// theme-contrast-verification handoff: exists to verify `Theme::default`'s
// palette against real WCAG thresholds, not to be drawn with -- no
// production render path needs a contrast ratio, so this is test-only
// rather than a dead-code-suppressed always-compiled module.
#[cfg(test)]
mod contrast;

/// A colour role. Naming roles instead of exposing raw RGB fields keeps
/// call sites like `theme.accent()` self-describing, and keeps this
/// struct's shape stable if RFC-023 later needs to add a role without
/// touching every existing call site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    background: Color,
    foreground: Color,
    accent: Color,
    border_default: Color,
    border_focused: Color,
    surface_elevated: Color,
    scrim: Color,
    font_size_body: f32,
    font_size_heading: f32,
    font_size_status: f32,
    /// **RFC-054 PR-054-B.** The family the interface's own text is set in.
    /// `Font::DEFAULT` unless the user named an installed family. The
    /// terminal and the file tree keep their monospace face either way
    /// (`explorer::TREE_FONT` and `MONOSPACE` in the terminal set it explicitly): a family
    /// chosen for reading prose must not undo what makes a column of code line
    /// up.
    font: Font,
}

fn colour(colour: Colour) -> Color {
    Color::from_rgba(colour.r, colour.g, colour.b, colour.a)
}

impl Theme {
    /// **RFC-054.** The theme a configuration puts in force. `family` is what
    /// [`installed_family`] answered for the configured name -- already
    /// resolved against the renderer's own font database, so a name that got
    /// here is one the renderer will find. Nothing in `settings` is validated
    /// again: `parse_and_validate` only hands over overrides that met D5 and D6.
    pub fn from_settings(
        theme: &ThemeSettings,
        font: &FontSettings,
        family: Option<&'static str>,
    ) -> Self {
        let palette = Palette::with_overrides(&theme.overrides);
        let sizes = font.sizes();
        Self {
            background: colour(palette.background),
            foreground: colour(palette.foreground),
            accent: colour(palette.accent),
            border_default: colour(palette.border_default),
            border_focused: colour(palette.border_focused),
            surface_elevated: colour(palette.surface_elevated),
            scrim: colour(palette.scrim),
            font_size_body: sizes.body,
            font_size_heading: sizes.heading,
            font_size_status: sizes.status,
            font: family.map_or(Font::DEFAULT, Font::with_name),
        }
    }

    pub fn font(&self) -> Font {
        self.font
    }

    pub fn background(&self) -> Color {
        self.background
    }

    pub fn foreground(&self) -> Color {
        self.foreground
    }

    pub fn accent(&self) -> Color {
        self.accent
    }

    pub fn border_default(&self) -> Color {
        self.border_default
    }

    /// RFC-015 PR-015-E: reinstated -- cut in PR-015-B for having no
    /// caller (there was no focus concept to render with one
    /// `FocusZone` variant), correctly. `FocusZone::Sidebar` gives Tab
    /// somewhere to go, and `NFR-UX-002` requires the indicator not rely
    /// on colour alone, so callers pair this with a second channel
    /// (border width, a marker glyph) rather than colour by itself.
    pub fn border_focused(&self) -> Color {
        self.border_focused
    }

    pub fn surface_elevated(&self) -> Color {
        self.surface_elevated
    }

    /// RFC-018 PR-018-G: the full-window dimming layer behind a modal.
    /// Translucent (`a < 1.0`), not opaque black -- the argument for
    /// building this at all is that it dims chrome no terminal pane can
    /// draw into, which only reads as a change if what was already there
    /// remains faintly visible underneath. An opaque scrim would look
    /// identical to any solid full-window rectangle a spoofing attempt
    /// could also draw, undermining the property this exists to add.
    pub fn scrim(&self) -> Color {
        self.scrim
    }

    pub fn font_size_body(&self) -> f32 {
        self.font_size_body
    }

    pub fn font_size_heading(&self) -> f32 {
        self.font_size_heading
    }

    pub fn font_size_status(&self) -> f32 {
        self.font_size_status
    }
}

impl Default for Theme {
    /// The shipped look. Its values live in `tekstide_core::config::Palette` and
    /// `FontSizes`, so the palette a configured pair is measured against is the
    /// palette drawn when nothing is configured. The rationale for each border
    /// and the scrim (the contrast each was raised to, and why) is held to its
    /// numbers by `theme/tests.rs`, which still measures this.
    fn default() -> Self {
        Self::from_settings(&ThemeSettings::default(), &FontSettings::default(), None)
    }
}

/// The face the interface's own text is set in. Process-global because iced
/// fixes the *default* font when the application is built and offers no way to
/// change it later -- and D8 says the family applies live. So every ordinary
/// text widget takes its font from here ([`text`]), and a reload sets it.
static UI_FONT: RwLock<Font> = RwLock::new(Font::DEFAULT);

pub(crate) fn ui_font() -> Font {
    UI_FONT.read().map_or(Font::DEFAULT, |font| *font)
}

pub(crate) fn set_ui_font(font: Font) {
    if let Ok(mut current) = UI_FONT.write() {
        *current = font;
    }
}

/// `iced::widget::text`, set in the configured family. Every ordinary text
/// widget in the shell is built with this, not with iced's own, which is what
/// makes [`set_ui_font`] reach the screen; a widget that wants a specific face
/// (the terminal, the tree) sets `.font(..)` after it, as before.
pub(crate) fn text<'a, Theme, Renderer>(
    content: impl iced::widget::text::IntoFragment<'a>,
) -> iced::widget::Text<'a, Theme, Renderer>
where
    Theme: iced::widget::text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = Font>,
{
    iced::widget::text(content).font(ui_font())
}

/// **Every button's look**, from the theme's own roles -- `surface_elevated`
/// fill, `foreground` text (the pair D5 already measures at 4.5:1), a
/// `border_default` outline that becomes the `border_focused` colour and a
/// heavier width when the pointer is over it or it is pressed. iced's default
/// button draws its own blue from `iced::Theme`, which no configuration reaches;
/// this is what makes a configured `background` not sit beside two fixed-colour
/// buttons (RFC-054 review 428). A button is still a box with its words inside
/// it: the shape and the label carry the meaning, the colour only styles it.
pub(crate) fn button_style(
    theme: Theme,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |_base_theme: &iced::Theme, status: iced::widget::button::Status| {
        use iced::widget::button::Status;
        let raised = matches!(status, Status::Hovered | Status::Pressed);
        let disabled = matches!(status, Status::Disabled);
        iced::widget::button::Style {
            background: Some(iced::Background::Color(theme.surface_elevated())),
            text_color: Color {
                a: if disabled { 0.6 } else { 1.0 },
                ..theme.foreground()
            },
            border: iced::Border {
                color: if raised {
                    theme.border_focused()
                } else {
                    theme.border_default()
                },
                width: if raised { 2.0 } else { 1.0 },
                radius: 4.0.into(),
            },
            ..iced::widget::button::Style::default()
        }
    }
}

/// `iced::widget::button`, drawn from the theme ([`button_style`]). Every button
/// in the shell is built with this, not with iced's own; one that wants another
/// look (the tab strip's) sets `.style(..)` after it, as before.
pub(crate) fn button<'a, Message>(
    theme: Theme,
    content: impl Into<iced::Element<'a, Message>>,
) -> iced::widget::Button<'a, Message>
where
    Message: 'a,
{
    iced::widget::button(content).style(button_style(theme))
}

/// The installed font family whose name is `wanted` (compared without regard to
/// case, as the font database itself does), spelled as the database spells it --
/// or `None` if nothing installed answers to it.
///
/// **This is the whole of how a configured family reaches the renderer, and it
/// is a lookup, not a load** (RFC-054 D6, §3): `wanted` is *compared* with names
/// the database already holds, never opened, and what comes back is one of those
/// names, not the user's string. Nothing a user wrote is parsed as a font.
///
/// The renderer's own database is asked (`iced_graphics`' global font system),
/// so a `Some` here is a family the renderer will find; a second scan of the
/// system's fonts would answer a different question. `Font::with_name` needs a
/// `&'static str`, so the database's spelling is interned -- bounded by the
/// number of installed families, **not** by what anyone types.
pub(crate) fn installed_family(wanted: &str) -> Option<&'static str> {
    static INTERNED: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);

    let canonical: String = {
        let system = iced::advanced::graphics::text::font_system();
        let mut system = system.write().ok()?;
        system
            .raw()
            .db()
            .faces()
            .flat_map(|face| face.families.iter())
            .map(|(name, _language)| name)
            .find(|name| name.eq_ignore_ascii_case(wanted))?
            .clone()
    };
    let mut interned = INTERNED.lock().ok()?;
    let interned = interned.get_or_insert_with(HashMap::new);
    let name: &'static str = interned
        .entry(canonical)
        .or_insert_with_key(|name| Box::leak(name.clone().into_boxed_str()));
    Some(name)
}

#[cfg(test)]
mod tests;
