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
}

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
