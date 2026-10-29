//! RFC-054 PR-054-A: the chord grammar, spelled the way the product already
//! prints it.
//!
//! **One grammar, not two (D8′).** The Help modal and `--help` print
//! `Ctrl+Alt+P`; that exact spelling is what a user types in the file, and
//! `crates/tekstide/src/input.rs`'s `format_binding` renders a real key press in
//! the same shape. [`Chord`]'s [`Display`](std::fmt::Display) is that shape --
//! modifiers in the fixed order `Ctrl`, `Alt`, `Shift`, then one key -- and
//! `every_advertised_chord_round_trips` (`navigation/tests.rs`) holds the file
//! and the help to each other over every binding the policy ships.
//!
//! **RFC-028 D9: `Ctrl` is this grammar's own canonical name for the primary
//! modifier slot, held fixed everywhere a chord is matched, stored, parsed
//! or compared -- `Chord::parse`, `Display`, `config.toml`'s own rebind
//! text, and the policy table's stored strings all stay `Ctrl`, on every
//! host.** Only the *keypress that fills that slot* varies by platform
//! (`crates/tekstide/src/input.rs`'s `format_binding` checks
//! `Modifiers::command()`, not `Modifiers::control()`, so a real `⌘` press
//! on macOS is read into the same `Ctrl` slot a real `Ctrl` press fills on
//! Linux) and the *labels a human reads*, via [`Chord::display_spelling`]
//! (Round 2: it respells `Alt` and `Shift` too, not just `Ctrl` -- a
//! macOS reader must never see a hybrid like `⌘+Alt+P`, one modifier
//! translated and two not, since no real Mac keyboard shows `Alt` or
//! `Shift` either) -- used only by presentation call sites (the Help
//! modal, `--help`, the status bar, a handful of `locales/en.ftl`
//! strings), never by anything that matches, stores or parses a chord.
//! **The five `config.toml` parse-error strings naming this grammar's own modifier words
//! (`not-a-string`/`chord-empty`/`chord-unknown-part`/`chord-no-ctrl-or-alt`/
//! `chord-shift-digit`, in `project-board-configuration-fallback`,
//! `locales/en.ftl`) are the one deliberate exception**: they name the
//! *file's* grammar, which stays `Ctrl`-spelled on every host, so they
//! must never be routed through `display_spelling` -- doing so would
//! instruct a macOS user to write `"⌘+Alt+J"` into `config.toml`, which
//! `Chord::parse` rejects outright. Baking a label into stored data was
//! tried and reverted: `Chord::parse` cannot read `⌘` back (it is not in
//! the grammar at all), so a stored chord spelled with the label could
//! never be rebound, matched, or compared against collision-free, only
//! displayed -- the exact bug the swap-tested ablation in RFC-028's own
//! PR-028-E review caught before it shipped, and the same bug the error
//! strings would reintroduce through a different door if ever respelled.
//!
//! **Review 650 round 3: symbols for accelerators, words in prose -- the boundary for
//! everything else in `locales/en.ftl` that mentions a modifier by name.** A string
//! naming a *displayed accelerator* (the Help modal, `--help`, the status bar, a chord
//! quoted back from the policy) goes through `display_spelling`, on this rule's own say-so
//! above. A string that is *prose about a fixed UI convention* -- `Tab`/`Shift+Tab` cycling
//! focus through a dialog, `Shift+Enter` for a newline, neither one a rebindable chord this
//! module's own grammar governs, both named only in the named keys exception two
//! paragraphs below -- is deliberately left spelled out, the same way Apple's own
//! documentation writes "press Shift-Tab" in prose while showing `⌘` in a menu. Nine such
//! strings exist today (grep `locales/en.ftl` for `Shift+Tab` or `Shift+Enter`); none of
//! them is a bug, and nothing here should "complete" that sweep.
//!
//! What a chord may be is narrower than what a keyboard can send, on purpose:
//!
//! * **At least `Ctrl` or `Alt`.** A global binding on a bare letter, or on
//!   `Shift` plus a letter, would take that key away from typing in the
//!   terminal and the editor -- a rebind that makes the product unusable.
//! * **One key, `A`-`Z` or `0`-`9`.** Named keys (`Enter`, `Tab`, arrows, `F1`)
//!   are the surfaces' own and the focus cycle's; none is rebindable.
//! * **No `Shift` with a digit.** `Shift+1` arrives as `!` on one layout and
//!   `1` on another, so the chord would silently never match. Refused rather
//!   than accepted and unreachable (§4).
//!
//! Modifier and key names are matched without regard to case
//! (`ctrl+alt+p`), and rendered canonically, so the spelling a user typed is
//! never echoed back as a second form.

use std::fmt;

/// A parsed, canonical chord.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Chord {
    ctrl: bool,
    alt: bool,
    shift: bool,
    key: char,
}

/// Why a chord was refused. **Carries no text from the file**: each variant is
/// a fixed reason, so a diagnostic built from it cannot echo what the user
/// wrote (§6).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ChordError {
    Empty,
    /// A part that is not `Ctrl`, `Alt`, `Shift` or a single key.
    UnknownPart,
    /// The same modifier twice.
    RepeatedModifier,
    /// No key after the modifiers, or more than one.
    NoSingleKey,
    /// The key is not `A`-`Z` or `0`-`9`.
    KeyNotRebindable,
    /// Neither `Ctrl` nor `Alt`: it would take a key from typing.
    NoCtrlOrAlt,
    /// `Shift` with a digit: layout-dependent, so it would not reliably match.
    ShiftWithDigit,
}

impl Chord {
    /// Parses a chord. See the module doc for what is accepted.
    pub fn parse(spelling: &str) -> Result<Self, ChordError> {
        if spelling.trim().is_empty() {
            return Err(ChordError::Empty);
        }
        let (mut ctrl, mut alt, mut shift) = (false, false, false);
        let mut key: Option<char> = None;
        let parts: Vec<&str> = spelling.split('+').collect();
        for (index, part) in parts.iter().enumerate() {
            let part = part.trim();
            let is_last = index + 1 == parts.len();
            let modifier = match part.to_ascii_lowercase().as_str() {
                "ctrl" => Some(&mut ctrl),
                "alt" => Some(&mut alt),
                "shift" => Some(&mut shift),
                _ => None,
            };
            match modifier {
                Some(flag) => {
                    if *flag {
                        return Err(ChordError::RepeatedModifier);
                    }
                    *flag = true;
                    if is_last {
                        return Err(ChordError::NoSingleKey);
                    }
                }
                None => {
                    let mut characters = part.chars();
                    let (Some(character), None) = (characters.next(), characters.next()) else {
                        return Err(if part.is_empty() {
                            ChordError::NoSingleKey
                        } else {
                            ChordError::UnknownPart
                        });
                    };
                    if !is_last || key.is_some() {
                        return Err(ChordError::NoSingleKey);
                    }
                    key = Some(character.to_ascii_uppercase());
                }
            }
        }
        let key = key.ok_or(ChordError::NoSingleKey)?;
        if !key.is_ascii_alphanumeric() {
            return Err(ChordError::KeyNotRebindable);
        }
        if !ctrl && !alt {
            return Err(ChordError::NoCtrlOrAlt);
        }
        if shift && key.is_ascii_digit() {
            return Err(ChordError::ShiftWithDigit);
        }
        Ok(Self {
            ctrl,
            alt,
            shift,
            key,
        })
    }

    /// RFC-028 D9: this crate's one modifier abstraction, and the *only*
    /// thing this slice makes platform-aware -- **never** used by
    /// [`Self::parse`], [`Display`](std::fmt::Display), or anything that
    /// stores or matches a chord (see the module doc for why: `⌘` is not
    /// in the grammar, so a value spelled with it could not be read back).
    /// A label for a human to read, nothing else: `"Ctrl"` on Linux, the
    /// physical key a Linux user presses for every binding this policy
    /// ships; `"⌘"` on macOS, because `winit`/`iced` surface Command as
    /// `Modifiers::LOGO`, a different bit than `Modifiers::CTRL`, so a
    /// Help modal that kept *printing* `Ctrl` on macOS would show a key no
    /// Mac user presses for that binding, even though the stored chord
    /// (and the real key that fills its `ctrl` slot -- see
    /// `crates/tekstide/src/input.rs`'s `format_binding`) is unaffected.
    /// Same convention as
    /// [`crate::config::ConfigPathProvider::default_for_host`]: a
    /// whole-function `#[cfg(target_os = "...")]` split, never a blanket
    /// `#[cfg(unix)]`, and a hard compile error for a target this split
    /// does not name.
    #[cfg(target_os = "linux")]
    pub const fn primary_modifier_label() -> &'static str {
        "Ctrl"
    }

    /// See [`Self::primary_modifier_label`] above for the convention.
    #[cfg(target_os = "macos")]
    pub const fn primary_modifier_label() -> &'static str {
        "⌘"
    }

    /// Companion to [`Self::primary_modifier_label`], for the same reason:
    /// a Mac user never sees `Alt` on a real keyboard, only `⌥`. Identity
    /// on Linux. `pub`, not `pub(crate)` (widened review 650, after a real
    /// macOS capture caught `shell.rs`'s own `status-bar-key-hint` printing
    /// the hybrid `⌘+Alt+P` -- that call site builds its Fluent args from
    /// individual `&'static str` labels, the same way
    /// [`Self::primary_modifier_label`] already does, and `display_spelling`
    /// returns an owned `String` the catalog's own `trusted_symbol` cannot
    /// take (it requires `&'static str` by construction, so a caller is
    /// never tempted to carry file-derived text into trusted catalog
    /// substitution) -- so a second, cross-crate-visible source of the real
    /// value was the fix, not a change to that API).
    #[cfg(target_os = "linux")]
    pub const fn alt_modifier_label() -> &'static str {
        "Alt"
    }

    /// See [`Self::alt_modifier_label`] above for the convention.
    #[cfg(target_os = "macos")]
    pub const fn alt_modifier_label() -> &'static str {
        "⌥"
    }

    /// Companion to [`Self::primary_modifier_label`], for the same reason:
    /// a Mac user never sees `Shift` on a real keyboard, only `⇧`.
    /// Identity on Linux. `pub` for the same reason as
    /// [`Self::alt_modifier_label`] -- no production call site needs it
    /// yet, but the same constraint (`trusted_symbol` wants `&'static str`)
    /// would apply the moment one does, and a visibility split between two
    /// otherwise-identical sibling functions would itself be a trap.
    #[cfg(target_os = "linux")]
    pub const fn shift_modifier_label() -> &'static str {
        "Shift"
    }

    /// See [`Self::shift_modifier_label`] above for the convention.
    #[cfg(target_os = "macos")]
    pub const fn shift_modifier_label() -> &'static str {
        "⇧"
    }

    /// Respells a chord **already canonically formatted** (`Chord`'s own
    /// [`Display`](std::fmt::Display), or a policy table's stored
    /// `"Ctrl+..."` string) for a human to read on this host: `Ctrl`, `Alt`
    /// and `Shift` each become their own host label
    /// ([`Self::primary_modifier_label`] for `Ctrl`; `alt_modifier_label`
    /// and `shift_modifier_label`, `pub(crate)` companions of the same
    /// shape, for the other two) -- all three, not just the leading
    /// token, so a macOS reader never sees a hybrid like `⌘+Alt+P` (one
    /// modifier respelled, two not) next to a Help modal that otherwise
    /// labels every key the way the real keyboard does. None of `Ctrl`,
    /// `Alt`, `Shift` is a substring of another, so the replacement order
    /// below does not matter. On Linux all three labels are the identity,
    /// so this is a no-op there. For presentation only -- the Help modal,
    /// `--help`, the status bar -- never for anything matched, parsed or
    /// stored back; see the module doc.
    pub fn display_spelling(canonical: &str) -> String {
        canonical
            .replacen("Ctrl", Self::primary_modifier_label(), 1)
            .replacen("Alt", Self::alt_modifier_label(), 1)
            .replacen("Shift", Self::shift_modifier_label(), 1)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!(
    "Chord::primary_modifier_label() has no arm for this target_os -- RFC-028 D9's own \
     convention (RFC-028 D3) requires a dispatch arm added deliberately for a new platform, not \
     a fall-through to a Unix-shaped default that may not hold."
);

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            formatter.write_str("Ctrl+")?;
        }
        if self.alt {
            formatter.write_str("Alt+")?;
        }
        if self.shift {
            formatter.write_str("Shift+")?;
        }
        write!(formatter, "{}", self.key)
    }
}
