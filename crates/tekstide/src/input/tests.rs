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
