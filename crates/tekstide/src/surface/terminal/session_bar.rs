//! RFC-017 PR-017-E: the terminal pane's first chrome
//! (`implementation-handoff.md` §1's expected module). This is the file
//! that makes the RFC-016 grid-not-chrome boundary live for the first
//! time -- everything here is trusted UI describing session state, not
//! the grid itself, so it is **not** exempt from
//! `shell::tests::no_raw_color_construction_anywhere_in_the_crate`:
//! every colour below comes from `crate::theme::Theme`, the same
//! contract `shell::zone_style` already holds itself to.
//!
//! **`NFR-UX-002`, satisfied by text alone.** Each entry's label already
//! names its slot ("Primary"/"Secondary"/"Hidden") and its status
//! ("Running", etc.) in words -- state is never conveyed by colour
//! alone here, so there is no second channel to add on top of the text
//! that already carries the information. Hidden sessions get a row the
//! same as visible ones: "remains addressable and its state visible"
//! (RFC-017) means the row exists, not that it renders a grid.

use iced::widget::{container, row, text};
use iced::{Background, Border, Element, Length};

use tekstide_core::domain::{TerminalStatus, VisibleSlot};

use crate::theme::Theme;

/// One session bar entry's data, computed by the caller (`shell.rs`,
/// which has the real `ApplicationShell` state) rather than this module
/// reaching for it itself -- the same "renders, does not own" shape
/// `grid_colors::view` already uses for the pane itself.
pub struct SessionBarEntry {
    pub label: String,
    pub slot: VisibleSlot,
    pub status: TerminalStatus,
}

fn slot_label(slot: VisibleSlot) -> &'static str {
    match slot {
        VisibleSlot::Primary => "Primary",
        VisibleSlot::Secondary => "Secondary",
        VisibleSlot::Hidden => "Hidden",
    }
}

fn status_label(status: TerminalStatus) -> &'static str {
    match status {
        TerminalStatus::Starting => "Starting",
        TerminalStatus::Running => "Running",
        TerminalStatus::Exited => "Exited",
        TerminalStatus::Failed => "Failed",
        TerminalStatus::Terminating => "Terminating",
        TerminalStatus::OrphanedUnknown => "Unknown",
    }
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

pub fn view<'a, Message: 'a>(theme: Theme, entries: &[SessionBarEntry]) -> Element<'a, Message> {
    row(entries
        .iter()
        .map(|entry| {
            container(text(format!(
                "{} ({}) — {}",
                entry.label,
                slot_label(entry.slot),
                status_label(entry.status)
            )))
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
    use super::*;

    #[test]
    fn every_slot_and_status_has_a_distinct_textual_label() {
        let slot_labels: Vec<&str> = [
            VisibleSlot::Primary,
            VisibleSlot::Secondary,
            VisibleSlot::Hidden,
        ]
        .into_iter()
        .map(slot_label)
        .collect();
        let unique: std::collections::HashSet<&str> = slot_labels.iter().copied().collect();
        assert_eq!(
            unique.len(),
            slot_labels.len(),
            "NFR-UX-002: every slot must have its own distinct textual label, not share one \
             with another slot"
        );

        let status_labels: Vec<&str> = [
            TerminalStatus::Starting,
            TerminalStatus::Running,
            TerminalStatus::Exited,
            TerminalStatus::Failed,
            TerminalStatus::Terminating,
            TerminalStatus::OrphanedUnknown,
        ]
        .into_iter()
        .map(status_label)
        .collect();
        let unique_statuses: std::collections::HashSet<&str> =
            status_labels.iter().copied().collect();
        assert_eq!(
            unique_statuses.len(),
            status_labels.len(),
            "NFR-UX-002: every status must have its own distinct textual label"
        );
    }
}
