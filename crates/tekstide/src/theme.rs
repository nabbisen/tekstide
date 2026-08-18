//! RFC-015 PR-015-B: the theme seam. RFC-023 will supply these values
//! from configuration; until then [`Theme::default`] is the compiled
//! default this module owns. `NFR-UX-004` requires colours and font
//! sizes to be configurable, which is only true if no widget hardcodes
//! one -- every colour and every font size the shell draws comes from a
//! `Theme` value, never a literal `Color::from_rgb(...)` or `.size(13)`
//! written at a call site.
//!
//! This type is unrelated to `iced::Theme` (the base theme parameter
//! `iced`'s own style closures take, e.g. `container::Style`'s
//! `move |_theme: &iced::Theme| ...`) -- that one selects between
//! `iced`'s built-in palettes and is not used here. This module's
//! `Theme` is referenced by its full path, `crate::theme::Theme`, at
//! every call site specifically to avoid that ambiguity.

use iced::Color;

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
}

impl Theme {
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
    fn default() -> Self {
        Self {
            background: Color::from_rgb(0.08, 0.08, 0.09),
            foreground: Color::from_rgb(0.90, 0.90, 0.90),
            accent: Color::from_rgb(0.30, 0.60, 1.0),
            // theme-contrast-verification handoff, Slice B: raised from
            // 0.35 (2.63:1 on `background`, 2.37:1 on `surface_elevated`
            // -- both fail WCAG 2.1 SC 1.4.11's 3:1 non-text threshold).
            // 0.45 measures 3.85:1 / 3.48:1, real headroom over the
            // minimum (~0.42, 3.44:1 / 3.11:1) so a future adjustment to
            // `surface_elevated` does not re-break this the moment
            // someone touches an unrelated colour.
            border_default: Color::from_rgb(0.45, 0.45, 0.45),
            border_focused: Color::from_rgb(0.30, 0.60, 1.0),
            surface_elevated: Color::from_rgb(0.12, 0.12, 0.12),
            // derived-contrast-pairs handoff, Slice B: raised from 0.55
            // (worst case 2.40:1 against the modal card's own
            // border/fill, at ~0.78 grey terminal content behind it --
            // fails WCAG 2.1 SC 1.4.11's 3:1). 0.75 measures 3.62:1 at
            // its own worst case (still at the bright end, content near
            // white). Keeps the accent-coloured border rather than
            // switching to the alternative grey-border lever, and moves
            // in the same direction as RFC-018's own goal: more chrome
            // dimming is a stronger spoofing tell, not a weaker one.
            // Still visibly translucent -- verified against the real
            // rendered window, not only the arithmetic; see the
            // handoff's own evidence for what was observed.
            scrim: Color::from_rgba(0.0, 0.0, 0.0, 0.75),
            font_size_body: 14.0,
            font_size_heading: 16.0,
            font_size_status: 13.0,
        }
    }
}

#[cfg(test)]
mod tests;
