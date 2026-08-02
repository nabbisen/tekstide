//! RFC-017 PR-017-E: split out of `terminal.rs` specifically so the
//! colour-scan exemption (`shell::tests::no_raw_color_construction_anywhere_in_the_crate`)
//! can narrow to exactly this file rather than staying a claim about
//! `terminal.rs` as a whole. That file now has chrome
//! ([`super::session_bar`]) with real theme colours; this one still has
//! exactly one kind of colour construction -- the grid cell's own,
//! PTY-determined colour, which cannot come from `state.theme` because
//! it is not a chrome role at all (RFC-016's grid exception, applied to
//! colour rather than text). See `is_scan_exempt`'s own comment for the
//! full reasoning, confirmed correct by review 148 and expected, by that
//! same response, to move exactly here once chrome existed.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{Color as CellColor, NamedColor};

use iced::widget::text::Span;
use iced::widget::{column, rich_text};
use iced::{Color, Element};

use super::{ROWS, TerminalPane};

/// Groups each visible row's cells into runs of consecutive identical
/// resolved foreground colour, using the same `renderable_content()`/
/// `Colors` API a real renderer uses -- ported from the RFC-014 spike's
/// `styled_rows`, unchanged in shape.
pub(super) fn styled_rows(term: &Term<VoidListener>) -> Vec<Vec<(String, [f32; 3])>> {
    let content = term.renderable_content();
    let colors = content.colors;

    let mut rows: Vec<Vec<(char, [f32; 3])>> = vec![Vec::new(); ROWS];
    for indexed in content.display_iter {
        let point = indexed.point;
        if point.line.0 < 0 || point.line.0 as usize >= ROWS {
            continue;
        }
        let rgb = resolve_color(indexed.cell.fg, colors);
        rows[point.line.0 as usize].push((indexed.cell.c, rgb));
    }

    rows.into_iter()
        .map(|cells| {
            let mut runs: Vec<(String, [f32; 3])> = Vec::new();
            for (c, rgb) in cells {
                match runs.last_mut() {
                    Some((text, last_rgb)) if *last_rgb == rgb => text.push(c),
                    _ => runs.push((c.to_string(), rgb)),
                }
            }
            runs
        })
        .collect()
}

/// The pane's rendered content only -- takes no `&shell::State`, no
/// theme role beyond the monospace font size, and cannot reach
/// `state.modal` or chrome, the same shape `surface::board::view` uses.
/// Untrusted PTY bytes render as data (RFC-016's grid exception): no
/// `text_safety::quote_untrusted` call here, deliberately -- escaping
/// would corrupt the grid the way it must not for `surface::board`'s
/// trusted-chrome fields.
pub fn view<'a, Message: 'a>(pane: &TerminalPane, font_size: f32) -> Element<'a, Message> {
    let rows = styled_rows(&pane.term);
    column(
        rows.into_iter()
            .map(|runs| {
                let spans: Vec<Span<'static, ()>> = runs
                    .into_iter()
                    .map(|(text, rgb)| {
                        Span::new(text).color(Color::from_rgb(rgb[0], rgb[1], rgb[2]))
                    })
                    .collect();
                rich_text(spans)
                    .size(font_size)
                    .font(iced::Font::MONOSPACE)
                    .into()
            })
            .collect::<Vec<Element<'a, Message>>>(),
    )
    .into()
}

/// Standard ANSI 16-colour fallback palette, used when `Term`'s `Colors`
/// table has no override for a given named colour -- true for every
/// colour here, since nothing seeds a palette in this slice (theme
/// integration for terminal colours is unscoped, same simplification
/// the spike recorded). Indexed colours beyond 0-15 fall back to the
/// default foreground; full 256-colour resolution is out of scope here,
/// recorded as a known limitation rather than silently implied complete.
fn resolve_color(color: CellColor, colors: &alacritty_terminal::term::color::Colors) -> [f32; 3] {
    const DEFAULT_FOREGROUND: [f32; 3] = [0.85, 0.85, 0.85];

    let named_fallback = |named: NamedColor| -> [f32; 3] {
        match named {
            NamedColor::Black => [0.0, 0.0, 0.0],
            NamedColor::Red => [0.80, 0.0, 0.0],
            NamedColor::Green => [0.0, 0.75, 0.0],
            NamedColor::Yellow => [0.80, 0.80, 0.0],
            NamedColor::Blue => [0.30, 0.55, 1.0],
            NamedColor::Magenta => [0.75, 0.0, 0.75],
            NamedColor::Cyan => [0.0, 0.75, 0.75],
            NamedColor::White => [0.85, 0.85, 0.85],
            NamedColor::BrightBlack => [0.4, 0.4, 0.4],
            NamedColor::BrightRed => [1.0, 0.3, 0.3],
            NamedColor::BrightGreen => [0.3, 1.0, 0.3],
            NamedColor::BrightYellow => [1.0, 1.0, 0.3],
            NamedColor::BrightBlue => [0.5, 0.7, 1.0],
            NamedColor::BrightMagenta => [1.0, 0.4, 1.0],
            NamedColor::BrightCyan => [0.4, 1.0, 1.0],
            NamedColor::BrightWhite => [1.0, 1.0, 1.0],
            NamedColor::Foreground => DEFAULT_FOREGROUND,
            _ => DEFAULT_FOREGROUND,
        }
    };

    match color {
        CellColor::Spec(rgb) => [
            f32::from(rgb.r) / 255.0,
            f32::from(rgb.g) / 255.0,
            f32::from(rgb.b) / 255.0,
        ],
        CellColor::Named(named) => colors[named]
            .map(|rgb| {
                [
                    f32::from(rgb.r) / 255.0,
                    f32::from(rgb.g) / 255.0,
                    f32::from(rgb.b) / 255.0,
                ]
            })
            .unwrap_or_else(|| named_fallback(named)),
        CellColor::Indexed(index) if index < 16 => named_fallback(named_color_from_index(index)),
        CellColor::Indexed(_) => DEFAULT_FOREGROUND,
    }
}

fn named_color_from_index(index: u8) -> NamedColor {
    match index {
        0 => NamedColor::Black,
        1 => NamedColor::Red,
        2 => NamedColor::Green,
        3 => NamedColor::Yellow,
        4 => NamedColor::Blue,
        5 => NamedColor::Magenta,
        6 => NamedColor::Cyan,
        7 => NamedColor::White,
        8 => NamedColor::BrightBlack,
        9 => NamedColor::BrightRed,
        10 => NamedColor::BrightGreen,
        11 => NamedColor::BrightYellow,
        12 => NamedColor::BrightBlue,
        13 => NamedColor::BrightMagenta,
        14 => NamedColor::BrightCyan,
        _ => NamedColor::BrightWhite,
    }
}
