use tekstide_core::domain::TerminalId;
use tekstide_core::navigation::{KeybindingPolicy, NavigationAction};

use super::{FocusZone, KeyPress, ModalAbsent, RoutedInput, route_non_modal_input};

fn character_press(c: &str, ctrl: bool, alt: bool, shift: bool) -> KeyPress {
    let mut modifiers = iced::keyboard::Modifiers::empty();
    if ctrl {
        modifiers |= iced::keyboard::Modifiers::CTRL;
    }
    if alt {
        modifiers |= iced::keyboard::Modifiers::ALT;
    }
    if shift {
        modifiers |= iced::keyboard::Modifiers::SHIFT;
    }
    KeyPress {
        key: iced::keyboard::Key::Character(c.into()),
        modifiers,
    }
}

fn tab_press(shift: bool) -> KeyPress {
    let modifiers = if shift {
        iced::keyboard::Modifiers::SHIFT
    } else {
        iced::keyboard::Modifiers::empty()
    };
    KeyPress {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab),
        modifiers,
    }
}

fn proof() -> ModalAbsent {
    ModalAbsent::check(&None::<()>).expect("no modal is active in this fixture")
}

/// The proof-token gate itself: a modal being active must make it
/// impossible to *obtain* a proof at all -- there is no other way to
/// reach `route_non_modal_input`.
#[test]
fn modal_absent_check_reflects_the_real_option() {
    assert!(ModalAbsent::check(&None::<()>).is_some());
    assert!(ModalAbsent::check(&Some(())).is_none());
}

/// Global keybindings win even while a terminal nominally holds focus --
/// `Ctrl+Alt+P` must reach the shell, never the PTY, exactly the
/// property RFC-015 names explicitly ("so Ctrl+Esc mode switching and
/// Project Board access cannot be captured by a surface -- including a
/// terminal").
#[test]
fn a_global_keybinding_wins_over_a_focused_terminal() {
    let policy = KeybindingPolicy::linux_mvp();
    let terminal = TerminalId::new_uuid();
    let routed = route_non_modal_input(
        proof(),
        &policy,
        FocusZone::MainArea,
        Some(&terminal),
        character_press("p", true, true, false),
    );
    assert_eq!(
        routed,
        RoutedInput::Shell(super::ShellInput(NavigationAction::OpenProjectBoard))
    );
}

/// The other reserved binding, proven the same way, with no terminal
/// focus in play at all -- both real `default_binding`s this policy
/// currently ships must actually route, not just exist as data.
#[test]
fn the_command_palette_binding_routes_to_the_shell() {
    let policy = KeybindingPolicy::linux_mvp();
    let routed = route_non_modal_input(
        proof(),
        &policy,
        FocusZone::MainArea,
        None,
        character_press("p", true, false, true),
    );
    assert_eq!(
        routed,
        RoutedInput::Shell(super::ShellInput(NavigationAction::OpenCommandPalette))
    );
}

/// A key with no matching binding, no terminal focus: falls through to
/// the currently focused shell zone as a `SurfaceInput` -- proven
/// non-numeric-shaped test-of-fall-through rather than assuming it.
#[test]
fn an_unbound_key_with_no_terminal_focus_becomes_surface_input() {
    let policy = KeybindingPolicy::linux_mvp();
    let routed = route_non_modal_input(
        proof(),
        &policy,
        FocusZone::MainArea,
        None,
        character_press("x", false, false, false),
    );
    match routed {
        RoutedInput::Surface(surface) => {
            assert_eq!(surface.target(), FocusZone::MainArea);
        }
        other => panic!("expected SurfaceInput, got {other:?}"),
    }
}

/// A key with no matching binding, a terminal focused: becomes a
/// `TextStream` addressed to that exact terminal.
#[test]
fn an_unbound_key_with_a_focused_terminal_becomes_a_text_stream() {
    let policy = KeybindingPolicy::linux_mvp();
    let terminal = TerminalId::new_uuid();
    let routed = route_non_modal_input(
        proof(),
        &policy,
        FocusZone::MainArea,
        Some(&terminal),
        character_press("x", false, false, false),
    );
    match routed {
        RoutedInput::Terminal(stream) => assert_eq!(stream.target(), &terminal),
        other => panic!("expected Terminal, got {other:?}"),
    }
}

/// Tab/Shift+Tab are the shell's own focus-cycle command, ahead of
/// terminal-focus routing -- proven with a terminal focused, so the
/// precedence is real rather than untested.
#[test]
fn tab_cycles_focus_even_with_a_terminal_focused() {
    let policy = KeybindingPolicy::linux_mvp();
    let terminal = TerminalId::new_uuid();
    assert_eq!(
        route_non_modal_input(
            proof(),
            &policy,
            FocusZone::MainArea,
            Some(&terminal),
            tab_press(false)
        ),
        RoutedInput::FocusNext
    );
    assert_eq!(
        route_non_modal_input(
            proof(),
            &policy,
            FocusZone::MainArea,
            Some(&terminal),
            tab_press(true)
        ),
        RoutedInput::FocusPrevious
    );
}

/// Response 130 Recommended: `format_binding` only handles
/// `Key::Character` -- a `KeybindingPolicy` rule bound to a named key
/// (`F1`, `Escape`, `Delete`) would make `matching_global_action` return
/// `None` for it, silently falling through to `SurfaceInput` and
/// quietly defeating "global keybindings are not capturable by a
/// surface" for that one binding. No live gap today (both real
/// `default_binding`s are single-character), but this test fails the
/// day that changes, which is the day it matters, rather than staying
/// silent until someone notices a keybinding "doesn't work."
#[test]
fn every_default_binding_in_linux_mvp_round_trips_through_format_binding() {
    let policy = KeybindingPolicy::linux_mvp();
    for rule in &policy.rules {
        let Some(binding) = rule.default_binding else {
            continue;
        };
        let press = key_press_for_binding(binding);
        assert_eq!(
            super::format_binding(&press),
            Some(binding.to_string()),
            "binding {binding:?} (action {:?}) does not round-trip through format_binding -- \
             it will silently fall through to SurfaceInput instead of reaching the shell",
            rule.action
        );
    }
}

/// Reconstructs a plausible `KeyPress` for a binding string like
/// `"Ctrl+Alt+P"` -- the inverse of `format_binding`, built only for
/// this test. Panics on a key segment it does not recognize rather than
/// silently skipping it, so a future named-key binding is a loud
/// failure here, not a silent gap.
fn key_press_for_binding(binding: &str) -> KeyPress {
    let mut modifiers = iced::keyboard::Modifiers::empty();
    let mut parts = binding.split('+').peekable();
    let mut key_segment = "";
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            key_segment = part;
            break;
        }
        match part {
            "Ctrl" => modifiers |= iced::keyboard::Modifiers::CTRL,
            "Alt" => modifiers |= iced::keyboard::Modifiers::ALT,
            "Shift" => modifiers |= iced::keyboard::Modifiers::SHIFT,
            other => {
                panic!("test does not know how to parse modifier {other:?} in binding {binding:?}")
            }
        }
    }

    assert_eq!(
        key_segment.chars().count(),
        1,
        "test does not know how to construct a KeyPress for named key segment {key_segment:?} \
         in binding {binding:?} -- extend this helper (and check format_binding still needs to \
         change too) rather than skip it"
    );
    KeyPress {
        key: iced::keyboard::Key::Character(key_segment.to_lowercase().into()),
        modifiers,
    }
}
