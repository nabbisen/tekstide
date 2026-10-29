//! RFC-028 D2, review 650 (round 2): whether the macOS primary/alt/shift modifier labels
//! (`⌘`, `⌥`, `⇧`) draw as real glyphs on the real macOS runner, or as `.notdef` ("tofu")
//! because the shipped font stack has no matching coverage -- the exact risk the scope note's
//! own rendered-capture proposal existed to answer with a picture.
//!
//! **Round 1 of this test compared real advance widths against a Private Use Area reference
//! codepoint (`U+E000`) and was never pushed**: ablating that assumption before shipping it
//! (the same check the reviewer asked of the D2 pinning test) found it false on this machine --
//! `U+E000` and `U+E001` measured *different*, non-round widths (`14.7109375` and `7.2460938`
//! at 14px `Font::DEFAULT`), meaning some installed font assigns real, distinct icon glyphs to
//! arbitrary PUA codepoints. No PUA reference chosen in advance is safe against that.
//!
//! **Round 2 needs no reference at all.** `cosmic_text::shape`'s own shaper already treats
//! `glyph_id == 0` as the one true definition of "no glyph" (`shape.rs:209`, `if info.glyph_id
//! == 0`): a missing glyph is `.notdef`, glyph id zero, by construction of the font format
//! itself, not inferred from how wide it happens to render. `iced_graphics::text::Paragraph`
//! exposes exactly that: `buffer()` returns the real `&cosmic_text::Buffer` the same real-layout
//! machinery `editor_column_width.rs`'s own `real_width` already builds, and `layout_runs()`
//! gives every laid-out `LayoutGlyph`, each carrying its own real `glyph_id: u16` from the
//! shaped font.
//!
//! Uses [`Chord::display_spelling`], not the host-dispatched label functions directly: it is the
//! one function the Help modal, `--help` and the status bar actually call, and on a bare
//! modifier name it returns exactly that modifier's own label -- `"Ctrl"`, `"Alt"` and `"Shift"`
//! are no substring of one another, so `replacen` only ever touches the matching token.
//! `Font::DEFAULT`, not a configured face: `help_modal_view` (`shell.rs`) sets no `.font(...)` on
//! any of its `text(...)` calls, and `main.rs`'s own `iced::application(...)` chain sets no
//! `.default_font(...)` either, so `Font::DEFAULT` is the face every one of these labels is
//! actually drawn with.

use iced::advanced::text::{Alignment, LineHeight, Paragraph as _, Shaping, Text, Wrapping};
use iced::{Font, Pixels, Size};

use tekstide_core::navigation::Chord;

const FONT_SIZE: f32 = 14.0;

/// Lays out `label` the same way [`super::editor_column_width::real_width`] does, and returns
/// every real, shaped glyph id `cosmic_text` produced for it -- `0` is `.notdef` by the shaper's
/// own definition (`cosmic-text`'s `shape.rs`), not a width this test has to interpret.
fn shaped_glyph_ids(label: &str, font: Font) -> Vec<u16> {
    let paragraph = iced::advanced::graphics::text::Paragraph::with_text(Text {
        content: label,
        bounds: Size::new(f32::INFINITY, FONT_SIZE * 2.0),
        size: Pixels(FONT_SIZE),
        line_height: LineHeight::default(),
        font,
        align_x: Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: Shaping::default(),
        wrapping: Wrapping::default(),
    });
    paragraph
        .buffer()
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.glyph_id))
        .collect()
}

/// The three modifier labels the Help modal, `--help` and the status bar actually render.
#[test]
fn the_modifier_labels_the_help_modal_prints_resolve_to_real_glyphs_not_tofu() {
    for canonical in ["Ctrl", "Alt", "Shift"] {
        let label = Chord::display_spelling(canonical);
        let glyph_ids = shaped_glyph_ids(&label, Font::DEFAULT);
        assert!(
            !glyph_ids.is_empty(),
            "{canonical}'s own label, {label:?}, produced no shaped glyphs at all"
        );
        assert!(
            glyph_ids.iter().all(|&id| id != 0),
            "{canonical}'s own label, {label:?}, shaped to glyph id 0 (.notdef) for at least \
             one character: {glyph_ids:?}"
        );
    }
}
