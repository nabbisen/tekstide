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
            border_default: Color::from_rgb(0.35, 0.35, 0.35),
            border_focused: Color::from_rgb(0.30, 0.60, 1.0),
            surface_elevated: Color::from_rgb(0.12, 0.12, 0.12),
            font_size_body: 14.0,
            font_size_heading: 16.0,
            font_size_status: 13.0,
        }
    }
}

#[cfg(test)]
mod tests;
