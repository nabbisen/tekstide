//! RFC-015 PR-015-C: input routing. **The security-critical slice** --
//! read `pr-015-c-input-routing.md` before touching this file.
//!
//! # The property
//!
//! While a modal dialog is open, no keystroke can reach a PTY, a
//! surface, or shell navigation. And at no time can terminal-originated
//! input address trusted state. This module makes that a type-level
//! fact, not a guard condition:
//!
//! - [`ShellInput`] -- global keybindings from `KeybindingPolicy`.
//!   Constructible only inside this module (private field); no surface
//!   can synthesize one.
//! - [`SurfaceInput`] -- keyboard for the currently focused shell zone.
//! - [`TextStream`] -- keystrokes destined for a PTY. Its sole
//!   constructor lives in [`terminal_surface`], `pub(super)` rather than
//!   `pub(crate)`, so nothing outside this module (not `shell.rs`, not a
//!   future surface module) can construct one. See that module's doc.
//!
//! # Modal exclusivity is structural, not a guard
//!
//! [`route_non_modal_input`] cannot be called without a [`ModalAbsent`]
//! proof, and the *only* way to obtain one is [`ModalAbsent::check`],
//! which inspects the real `Option<ModalContent>` at the call site. There
//! is no other constructor for `ModalAbsent` (private unit-tuple field).
//! Deleting the check that gates a call to `route_non_modal_input` is
//! therefore not a runtime behaviour change -- it is a missing-argument
//! compile error, because there is no `ModalAbsent` value left to pass.
//! This is the same "make the invalid state unrepresentable" pattern
//! already used for `DisplayText`, `VerifiedCwd`, `RunCapabilityToken`,
//! and `CatalogArgs` -- applied here to a routing decision instead of a
//! text-safety or capability one.
//!
//! `shell::subscription` is what this buys: when a modal is active, it
//! calls a *different* function (a modal-only key subscription) that has
//! no path to constructing `SurfaceInput` or `TextStream` at all --
//! "not produced," not "produced and discarded."
//!
//! # What this slice does not have a real caller for yet
//!
//! `TextStream` and terminal-focused routing have no real terminal
//! surface to receive input from (RFC-017 has not landed) --
//! `terminal_focus` is always `None` from `shell.rs` today. The type and
//! its routing rules are proven directly via synthetic
//! `TerminalId`/`KeyPress` values in `input::tests`, the same "headless"
//! shape as `i18n::Catalog` before RFC-015 gave it a caller.

mod terminal_surface;

pub use terminal_surface::TextStream;

use tekstide_core::domain::TerminalId;
use tekstide_core::navigation::{KeybindingPolicy, NavigationAction};

/// Raw key input, decoupled from `iced::keyboard::Event`'s many fields
/// so routing logic (and its tests) do not need to construct a full
/// `iced` event just to express "this key, these modifiers."
#[derive(Debug, Clone, PartialEq)]
pub struct KeyPress {
    pub key: iced::keyboard::Key,
    pub modifiers: iced::keyboard::Modifiers,
}

/// The shell's own focus zones. PR-015-B shipped a single real variant;
/// PR-015-E adds `Sidebar`, the scaffolding for RFC-017/019/020's real
/// sidebar content -- `#[non_exhaustive]` was kept specifically so this
/// addition would not need `route_non_modal_input`'s structure to
/// change, the same reason `LocalePreference`'s fields exist ahead of
/// their real callers. It did not; only this enum and its `next`/
/// `previous` grew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FocusZone {
    MainArea,
    Sidebar,
}

impl FocusZone {
    /// Two zones, so cycling is a genuine toggle now, not the no-op it
    /// was with one.
    pub fn next(self) -> Self {
        match self {
            Self::MainArea => Self::Sidebar,
            Self::Sidebar => Self::MainArea,
        }
    }

    pub fn previous(self) -> Self {
        // Two zones: reverse cycling is identical to forward cycling.
        // Kept as its own function (not aliased to `next`) so a third
        // zone later does not need callers of `previous` to notice a
        // silent behaviour change -- the same reasoning `ModalButton`'s
        // `next`/`previous` in `shell.rs` already applies to its own
        // two-item cycle.
        self.next()
    }
}

/// Global navigation input, produced only by matching a live
/// `KeybindingPolicy` rule inside this module. Private field: no code
/// outside `input` (a surface, `shell.rs` directly) can construct one by
/// struct literal, only receive one already produced by
/// [`route_non_modal_input`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellInput(NavigationAction);

impl ShellInput {
    pub fn action(self) -> NavigationAction {
        self.0
    }
}

/// Keyboard for the currently focused shell zone. No surface exists yet
/// to receive one (PR-015-D); this slice proves the *routing* is
/// correct, not that anything consumes the payload.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceInput {
    target: FocusZone,
    key: KeyPress,
}

impl SurfaceInput {
    pub fn target(&self) -> FocusZone {
        self.target
    }
}

/// What [`route_non_modal_input`] decided a key press means. Includes
/// the shell's own focus-cycle command (Tab/Shift+Tab) alongside the
/// three RFC-015 input classes -- focus-cycling is shell-local
/// presentational orchestration (`implementation-handoff.md` §2's "which
/// zone has focus"), not one of the three classes, but it must be gated
/// by the exact same `ModalAbsent` proof: RFC-015's property statement is
/// explicit that a modal excludes "shell navigation" too, not only
/// surface and terminal input.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutedInput {
    Shell(ShellInput),
    Surface(SurfaceInput),
    Terminal(TextStream),
    FocusNext,
    FocusPrevious,
}

/// Proof that no modal is active *at the moment it was checked*. The
/// only constructor is [`Self::check`]; the private unit field means no
/// other code, anywhere, can construct one by struct literal -- so
/// nothing can call [`route_non_modal_input`] without having genuinely
/// asked "is a modal active?" immediately beforehand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModalAbsent(());

impl ModalAbsent {
    pub fn check<T>(modal: &Option<T>) -> Option<Self> {
        modal.is_none().then_some(Self(()))
    }
}

/// Response 130 Required 2: the branch `shell::subscription` takes,
/// extracted so a test can assert it directly instead of the choice
/// living only inside an opaque `Subscription` value. Naming this
/// separately from `ModalAbsent` matters for a reason beyond style:
/// `ModalAbsent` is `Copy` (required by `.with()`'s `Hash` bound for
/// threading it into a subscription closure), so a proof obtained once
/// can be held past the instant it was true. The actual exclusivity
/// guarantee is therefore `ModalAbsent`'s call-time gate *plus* `iced`
/// tearing down the non-modal subscription -- and its captured proof --
/// the moment `subscription()` starts returning [`Self::Modal`] instead.
/// That second half is a framework-lifecycle dependency, not a type-level
/// one; naming it here, and testing that this function alone picks the
/// right branch, is what keeps that dependency visible rather than an
/// implicit assumption RFC-017 could inherit unknowingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionMode {
    NonModal(ModalAbsent),
    Modal,
}

impl SubscriptionMode {
    pub fn for_modal<T>(modal: &Option<T>) -> Self {
        match ModalAbsent::check(modal) {
            Some(proof) => Self::NonModal(proof),
            None => Self::Modal,
        }
    }
}

/// The router. Requires [`ModalAbsent`] -- see the module doc for why
/// deleting the check that produces one is a compile error here, not a
/// behaviour change.
///
/// Precedence, matching RFC-015's stated order minus the modal check
/// (already discharged by requiring `_proof`):
/// 1. A key matching a live `KeybindingPolicy` rule -- global keybindings
///    always win, so they cannot be captured by a surface, including a
///    terminal holding focus.
/// 2. Tab / Shift+Tab -- the shell's own focus cycle. Deliberately
///    checked ahead of terminal-focus: whether Tab should instead reach
///    a terminal's text input (shell completion, etc.) is a real
///    question with no terminal surface yet to answer it against;
///    recorded for RFC-017 to decide, not resolved here.
/// 3. `terminal_focus`, if the (currently always-`None`) target names a
///    focused terminal.
/// 4. Otherwise, the currently focused shell zone.
pub fn route_non_modal_input(
    _proof: ModalAbsent,
    policy: &KeybindingPolicy,
    focus: FocusZone,
    terminal_focus: Option<&TerminalId>,
    press: KeyPress,
) -> RoutedInput {
    if let Some(action) = matching_global_action(policy, &press) {
        return RoutedInput::Shell(ShellInput(action));
    }

    if is_focus_cycle_key(&press, false) {
        return RoutedInput::FocusNext;
    }
    if is_focus_cycle_key(&press, true) {
        return RoutedInput::FocusPrevious;
    }

    if let Some(terminal_id) = terminal_focus {
        return RoutedInput::Terminal(TextStream::from_terminal_key(terminal_id.clone(), press));
    }

    RoutedInput::Surface(SurfaceInput {
        target: focus,
        key: press,
    })
}

fn matching_global_action(policy: &KeybindingPolicy, press: &KeyPress) -> Option<NavigationAction> {
    let binding = format_binding(press)?;
    policy
        .rules
        .iter()
        .find(|rule| rule.default_binding == Some(binding.as_str()))
        .map(|rule| rule.action)
}

/// Renders a [`KeyPress`] the same way `KeybindingPolicy::linux_mvp()`'s
/// own `default_binding` strings are written (`"Ctrl+Shift+P"`), so a
/// real key press can be compared against them directly. Only the
/// modifiers this policy actually uses today (Ctrl, Alt, Shift) are
/// handled; Logo/Super is not part of any current binding.
fn format_binding(press: &KeyPress) -> Option<String> {
    let iced::keyboard::Key::Character(ref character) = press.key else {
        return None;
    };
    // Capitalised unconditionally, matching the display convention
    // `KeybindingPolicy::linux_mvp()`'s strings use (`"Ctrl+Alt+P"` has
    // no Shift held at all, yet still shows a capital `P`) -- this is a
    // binding *name*, not a literal transcript of which modifiers were
    // physically held for the letter's case.
    let mut parts: Vec<String> = Vec::new();
    if press.modifiers.control() {
        parts.push("Ctrl".to_string());
    }
    if press.modifiers.alt() {
        parts.push("Alt".to_string());
    }
    if press.modifiers.shift() {
        parts.push("Shift".to_string());
    }
    parts.push(character.to_uppercase());
    Some(parts.join("+"))
}

fn is_focus_cycle_key(press: &KeyPress, reverse: bool) -> bool {
    press.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab)
        && press.modifiers.shift() == reverse
}

/// Test-only constructors, `#[cfg(test)]`-gated so they do not exist in
/// the shipped binary at all (compiled out entirely, not merely hidden)
/// -- the same shape as `TerminalId::for_test` in `tekstide-core`, kept
/// crate-internal here since `input`, `shell`, and their test modules
/// all live in the one `tekstide` crate. These do not weaken the real
/// privacy boundary: `shell::tests` (a *different* module than `input`)
/// still cannot construct a `ShellInput` or `TextStream` any other way,
/// and none of this exists in a release build for anything to bypass.
#[cfg(test)]
pub(crate) fn shell_input_for_test(action: NavigationAction) -> ShellInput {
    ShellInput(action)
}

#[cfg(test)]
pub(crate) fn terminal_stream_for_test(
    target: TerminalId,
    modifiers: iced::keyboard::Modifiers,
) -> TextStream {
    TextStream::from_terminal_key(
        target,
        KeyPress {
            key: iced::keyboard::Key::Character("x".into()),
            modifiers,
        },
    )
}

#[cfg(test)]
mod tests;
