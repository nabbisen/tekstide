//! RFC-017 PR-017-E: the terminal pane's first chrome
//! (`implementation-handoff.md` §1's expected module). This is the file
//! that makes the RFC-016 grid-not-chrome boundary live for the first
//! time -- everything here is trusted UI describing session state, not
//! the grid itself, so it is **not** exempt from
//! `shell::tests::no_raw_color_construction_anywhere_in_the_crate`:
//! every colour below comes from `crate::theme::Theme`, the same
//! contract `shell::zone_style` already holds itself to.
//!
//! **Every user-facing word goes through `Catalog`** (response 150
//! Required -- the first version of this file hardcoded English at the
//! render layer, the exact shape `CountDisplay::label()`/
//! `AttentionState::label()` are banned from this crate for, response
//! 130). `slot`/`status` select Fluent branches inside one message
//! (`session-bar-entry`, `en.ftl`) via compile-time literal symbols
//! (`slot_symbol`/`status_symbol`) -- the same `route_symbol`/
//! `status-bar-summary` shape `shell.rs` already uses, not a string
//! built by concatenating three separately-resolved lookups.
//!
//! **`NFR-UX-002`, satisfied by text alone.** Each entry's resolved text
//! already names its slot and its status in words -- state is never
//! conveyed by colour alone here, so there is no second channel to add
//! on top of the text that already carries the information. Hidden
//! sessions get a row the same as visible ones: "remains addressable
//! and its state visible" (RFC-017) means the row exists, not that it
//! renders a grid.

use iced::widget::{container, row, text};
use iced::{Background, Border, Element, Length};

use tekstide_core::domain::{TerminalStatus, VisibleSlot};

use crate::i18n::{Catalog, CatalogArgs};
use crate::theme::Theme;

/// One session bar entry's data, computed by the caller (`shell.rs`,
/// which has the real `ApplicationShell` state) rather than this module
/// reaching for it itself -- the same "renders, does not own" shape
/// `grid_colors::view` already uses for the pane itself. `number` is a
/// genuine 1-based count (interpolated via `CatalogArgs::number`, real
/// plural-category selection if a locale ever needs it), not part of a
/// pre-formatted label.
pub struct SessionBarEntry {
    pub number: u32,
    pub slot: VisibleSlot,
    pub status: TerminalStatus,
}

/// A compile-time literal symbol, not the displayed word -- the word
/// lives in `en.ftl`'s `session-bar-entry` select expression, the same
/// division of labour `shell::route_symbol` already uses.
fn slot_symbol(slot: VisibleSlot) -> &'static str {
    match slot {
        VisibleSlot::Primary => "primary",
        VisibleSlot::Secondary => "secondary",
        VisibleSlot::Hidden => "hidden",
    }
}

fn status_symbol(status: TerminalStatus) -> &'static str {
    match status {
        TerminalStatus::Starting => "starting",
        TerminalStatus::Running => "running",
        TerminalStatus::Exited => "exited",
        TerminalStatus::Failed => "failed",
        TerminalStatus::Terminating => "terminating",
        TerminalStatus::OrphanedUnknown => "unknown",
    }
}

/// The one catalog lookup an entry's full text takes -- factored out so
/// tests can assert distinctness over what actually renders, not over
/// the symbol names `slot_symbol`/`status_symbol` return.
fn entry_text(catalog: &Catalog, entry: &SessionBarEntry) -> String {
    catalog.get_with_args(
        "session-bar-entry",
        &CatalogArgs::new()
            .number("number", entry.number)
            .trusted_symbol("slot", slot_symbol(entry.slot))
            .trusted_symbol("status", status_symbol(entry.status)),
    )
}

fn entry_style(theme: Theme) -> impl Fn(&iced::Theme) -> container::Style {
    move |_base_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(theme.background())),
        text_color: Some(theme.foreground()),
        border: Border {
            color: theme.border_default(),
            width: 1.0,
            radius: 2.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn view<'a, Message: 'a>(
    theme: Theme,
    catalog: &Catalog,
    entries: &[SessionBarEntry],
) -> Element<'a, Message> {
    row(entries
        .iter()
        .map(|entry| {
            container(text(entry_text(catalog, entry)))
                .padding(4)
                .style(entry_style(theme))
                .into()
        })
        .collect::<Vec<Element<'a, Message>>>())
    .spacing(6)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::i18n::LocalePreference;

    use super::*;

    fn real_locales_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
    }

    fn real_catalog() -> Catalog {
        Catalog::resolve(LocalePreference::default(), Some(&real_locales_dir()))
    }

    /// **Response 150 Required, the fix's own regression test.** Must
    /// assert distinctness over resolved catalog values -- text a user
    /// actually sees -- or this only proves the symbol names
    /// (`slot_symbol`/`status_symbol`) are distinct from each other,
    /// which says nothing about `NFR-UX-002` once those symbols are
    /// routed through a Fluent select expression that could (by a typo)
    /// map two of them to the same branch.
    #[test]
    fn every_slot_and_status_combination_resolves_to_distinct_text() {
        let catalog = real_catalog();

        let slot_texts: Vec<String> = [
            VisibleSlot::Primary,
            VisibleSlot::Secondary,
            VisibleSlot::Hidden,
        ]
        .into_iter()
        .map(|slot| {
            entry_text(
                &catalog,
                &SessionBarEntry {
                    number: 1,
                    slot,
                    status: TerminalStatus::Running,
                },
            )
        })
        .collect();
        let unique_slots: std::collections::HashSet<&String> = slot_texts.iter().collect();
        assert_eq!(
            unique_slots.len(),
            slot_texts.len(),
            "NFR-UX-002: every slot must resolve to its own distinct text, not share one with \
             another slot: {slot_texts:?}"
        );

        let status_texts: Vec<String> = [
            TerminalStatus::Starting,
            TerminalStatus::Running,
            TerminalStatus::Exited,
            TerminalStatus::Failed,
            TerminalStatus::Terminating,
            TerminalStatus::OrphanedUnknown,
        ]
        .into_iter()
        .map(|status| {
            entry_text(
                &catalog,
                &SessionBarEntry {
                    number: 1,
                    slot: VisibleSlot::Primary,
                    status,
                },
            )
        })
        .collect();
        let unique_statuses: std::collections::HashSet<&String> = status_texts.iter().collect();
        assert_eq!(
            unique_statuses.len(),
            status_texts.len(),
            "NFR-UX-002: every status must resolve to its own distinct text: {status_texts:?}"
        );
    }

    /// Fluent's automatic bidi isolation (`use_isolating: true`, already
    /// documented and asserted in `i18n.rs`'s own tests) wraps each
    /// interpolated placeable in First Strong Isolate/Pop Directional
    /// Isolate -- expected literally here, matching `shell/tests.rs`'s
    /// own `ISOLATE_START`/`ISOLATE_END` convention, rather than
    /// asserting a plain substring that would never match.
    const ISOLATE_START: &str = "\u{2068}";
    const ISOLATE_END: &str = "\u{2069}";

    /// The catalog lookup must actually be reached -- a regression to
    /// the pre-fix hardcoded strings would still pass the distinctness
    /// test above (hardcoded strings are distinct from each other too),
    /// so this asserts the resolved text contains real, catalog-sourced
    /// words rather than a symbol name or an empty/placeholder string.
    #[test]
    fn resolved_text_contains_the_real_words_not_symbol_names() {
        let catalog = real_catalog();
        let text = entry_text(
            &catalog,
            &SessionBarEntry {
                number: 2,
                slot: VisibleSlot::Secondary,
                status: TerminalStatus::Running,
            },
        );
        assert_eq!(
            text,
            format!(
                "Terminal {ISOLATE_START}2{ISOLATE_END} ({ISOLATE_START}Secondary{ISOLATE_END}) — \
                 {ISOLATE_START}Running{ISOLATE_END}"
            ),
            "must resolve to the catalog's real words (not a symbol name, not an empty string), \
             with the number/slot/status placeables isolated the same way every other \
             multi-placeable key in this crate already is"
        );
    }
}
