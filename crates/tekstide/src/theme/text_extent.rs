//! "GUI evidence prerequisites" (PR-076-D3 follow-up), tier-2 harness: the real
//! rendered extent of a piece of text, measured the way `shell/tests.rs`'s
//! `change_review_decision_controls_measured_layout_cost` first did it for
//! wrapped catalog text and `surface::terminal::font_metrics` does it for the
//! monospace grid -- `iced::advanced::graphics::text::Paragraph`, the same
//! shaping primitive `iced`'s own `Text` widget uses internally, no window, no
//! GPU, real measured glyphs. The null renderer (`iced_core::renderer::null`)
//! is the wrong tool for this: its `text::Paragraph` impl returns `Size::ZERO`
//! from `bounds()`/`min_bounds()` unconditionally, so a `Text` widget always
//! measures zero height under it regardless of content -- right for
//! *structural* properties (child count, ordering, widget kind), unable to
//! answer a pixel question at all.
//!
//! **What this module fixes that the first `wrapped_height` closure got
//! wrong**, found by checking the pinned sources rather than assumed:
//! `LineHeight::Relative(1.0)` where production uses `LineHeight::default()`
//! (`Relative(1.3)`) -- every measured height was about 30% short per line;
//! `Shaping::Basic` where production uses `Shaping::default()` (`Auto`,
//! because this workspace's `iced` features, `["tokio", "advanced"]`, enable
//! neither `advanced-shaping` nor `basic-shaping`) -- different shaping than
//! the screen, widest exactly where it matters (diacritics in a non-English
//! locale); and `Font::DEFAULT` where production uses [`super::ui_font`] (the
//! *configured* family). A property built on the old numbers would pass text
//! that clips on the real screen.
//!
//! **Why `line_height`/`shaping` are read from `LineHeight::default()`/
//! `Shaping::default()` rather than written as a literal that happens to
//! match today**: calling the same function production's own default path
//! calls means a future change to either iced's defaults or this workspace's
//! own `iced` feature flags moves both sides together, by construction, with
//! nothing here to update. What calling the *function* cannot catch is
//! `super::text` itself gaining an override (`.line_height(..)`, `.shaping(
//! ..)`, `.wrapping(..)`) that this module does not know to mirror --
//! `tests::theme_text_overrides_nothing_the_harness_assumes_is_default`
//! is the check that keeps that agreement a tested fact rather than a
//! comment, the same discipline PR-076-D3's own guard was built with.

use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;
use iced::advanced::text::{Alignment, LineHeight, Paragraph as _, Shaping, Text, Wrapping};
use iced::{Pixels, Size};

/// The real rendered size of `content`, wrapped word-by-word within `width`,
/// at `font_size` -- for a property about a paragraph that is *expected* to
/// wrap (a notice, a description), where the question is whether it still
/// fits inside a height budget, not whether it stays on one line.
pub(crate) fn wrapped_extent(content: &str, font_size: f32, width: f32) -> Size {
    measured_extent(
        content,
        font_size,
        Size::new(width, f32::INFINITY),
        Wrapping::Word,
    )
}

/// The real rendered size of `content` laid out on a single line, at
/// `font_size`, wrapping disabled -- for a label that is never meant to wrap
/// (a button, a keybinding chord), where the question is whether that one
/// line fits inside a *width* budget, not how it would wrap if it did not.
pub(crate) fn unwrapped_extent(content: &str, font_size: f32) -> Size {
    measured_extent(
        content,
        font_size,
        Size::new(f32::INFINITY, f32::INFINITY),
        Wrapping::None,
    )
}

fn measured_extent(content: &str, font_size: f32, bounds: Size, wrapping: Wrapping) -> Size {
    let text = Text {
        content,
        bounds,
        size: Pixels(font_size),
        line_height: LineHeight::default(),
        font: super::ui_font(),
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::default(),
        wrapping,
    };
    GraphicsParagraph::with_text(text).min_bounds()
}

#[cfg(test)]
mod tests;
