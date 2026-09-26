//! RFC-057 PR-057-B: the editor draws a window of rows, and the window follows
//! the cursor. Driven through the real router and the real `update`, on a
//! document with a measured body region, the way a person's keys reach it.

use super::*;

use iced::Size;
use tekstide_core::content::TextCursor;

fn hundred_lines() -> String {
    (0..100)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A document open with a body region exactly `rows` rows tall.
fn open_with_window(label: &str, text: &str, rows: usize) -> State {
    let (mut state, _dir) = state_with_an_open_document(label, text);
    let pitch = crate::surface::editor::row_pitch(state.theme.font_size_body());
    state.editor_viewport = Some(Size::new(800.0, pitch * rows as f32 + 0.5));
    assert_eq!(super::super::editor_window_capacity(&state), rows);
    state
}

fn press(state: &mut State, key: iced::keyboard::key::Named) {
    let policy = tekstide_core::navigation::KeybindingPolicy::linux_mvp();
    let press = crate::input::KeyPress {
        key: iced::keyboard::Key::Named(key),
        modifiers: iced::keyboard::Modifiers::empty(),
    };
    let proof = crate::input::ModalAbsent::check(&state.modal).expect("no modal open");
    let routed = crate::input::route_non_modal_input(proof, &policy, state.focus, None, press);
    let _ = super::super::update(state, Message::Input(routed));
}

fn first_visible_line(state: &State) -> usize {
    state
        .app_shell
        .state()
        .active_project()
        .and_then(|project| project.content_workspace().active_document())
        .expect("an active document")
        .viewport()
        .first_visible_line
}

fn cursor_line(state: &State) -> usize {
    active_document_cursor(state).line
}

/// D9, through real keys: the window does not move while the cursor stays in it,
/// moves by one when the cursor steps past its edge, and comes back the same way.
#[test]
fn the_window_follows_the_cursor_through_real_arrow_keys() {
    use iced::keyboard::key::Named::{ArrowDown, ArrowUp};
    let mut state = open_with_window("editor-rows-follow", &hundred_lines(), 10);

    for _ in 0..9 {
        press(&mut state, ArrowDown);
    }
    assert_eq!(
        (cursor_line(&state), first_visible_line(&state)),
        (9, 0),
        "still inside the window"
    );

    press(&mut state, ArrowDown);
    assert_eq!(
        (cursor_line(&state), first_visible_line(&state)),
        (10, 1),
        "one past the edge scrolls by one"
    );

    for _ in 0..30 {
        press(&mut state, ArrowDown);
    }
    assert_eq!((cursor_line(&state), first_visible_line(&state)), (40, 31));

    for _ in 0..5 {
        press(&mut state, ArrowUp);
    }
    assert_eq!(
        (cursor_line(&state), first_visible_line(&state)),
        (35, 31),
        "moving up inside the window does not scroll it"
    );
    for _ in 0..5 {
        press(&mut state, ArrowUp);
    }
    assert_eq!(
        (cursor_line(&state), first_visible_line(&state)),
        (30, 30),
        "above the edge scrolls by one"
    );
}

/// The rows drawn are the window's lines and only those, and the cursor's line is
/// among them wherever the cursor is.
#[test]
fn the_drawn_rows_always_contain_the_cursors_line() {
    use iced::keyboard::key::Named::ArrowDown;
    let mut state = open_with_window("editor-rows-cursor-visible", &hundred_lines(), 10);
    for _ in 0..57 {
        press(&mut state, ArrowDown);
    }

    let rows = super::editor_baseline::drawn_rows_for_test(&state);

    assert_eq!(rows.len(), 10);
    assert!(
        rows.contains(&format!("line {}", cursor_line(&state))),
        "the cursor's own line is drawn: {rows:?}"
    );
    assert_eq!(
        rows.first().map(String::as_str),
        Some(format!("line {}", first_visible_line(&state)).as_str())
    );
}

/// An edit that moves the cursor off the window brings the window with it.
#[test]
fn typing_enter_at_the_bottom_of_the_window_scrolls_it() {
    use iced::keyboard::key::Named::{ArrowDown, Enter};
    let mut state = open_with_window("editor-rows-enter", &hundred_lines(), 5);
    for _ in 0..4 {
        press(&mut state, ArrowDown);
    }
    assert_eq!(first_visible_line(&state), 0);

    press(&mut state, Enter);

    assert_eq!((cursor_line(&state), first_visible_line(&state)), (5, 1));
}

/// A re-measure that leaves the cursor off the new, smaller window moves the
/// window: the size is measured, so it can change under a cursor that has not.
#[test]
fn a_smaller_measured_region_brings_the_window_to_the_cursor() {
    let mut state = open_with_window("editor-rows-remeasure", &hundred_lines(), 30);
    let _ = state.app_shell.set_active_project_cursor(TextCursor {
        line: 20,
        column: 0,
    });
    assert_eq!(first_visible_line(&state), 0);

    let pitch = crate::surface::editor::row_pitch(state.theme.font_size_body());
    let _ = super::super::update(
        &mut state,
        Message::EditorViewportMeasured(Size::new(800.0, pitch * 5.0 + 0.5)),
    );

    assert_eq!(
        first_visible_line(&state),
        16,
        "the cursor is the last of five rows"
    );
}

#[test]
fn a_measurement_with_no_document_open_changes_nothing() {
    let mut state = state_with(ApplicationShell::new());

    let _ = super::super::update(
        &mut state,
        Message::EditorViewportMeasured(Size::new(800.0, 400.0)),
    );

    assert!(state.editor_viewport.is_some());
}
