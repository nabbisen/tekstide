//! Headless layout tests -- `iced`'s null renderer (`()`), no GPU and no
//! font backend, the technique `assemble_change_review_layout`'s test
//! already uses. What they prove is a property of the layout *algorithm*
//! (order of layout, limits handed down), so it holds for real text metrics
//! too; the live captures in `rfcs/handoffs/053-window-truth/evidence/`
//! cover the metrics.

use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::widget::Tree;
use iced::advanced::{Shell, clipboard};
use iced::widget::{column, container, scrollable, text};
use iced::{Element, Event, Length, Rectangle, Size, mouse, window};

use super::{MeasureSize, PinnedFooter};

type El<'a> = Element<'a, (), iced::Theme, ()>;

/// `count` stand-in text lines, each a fixed 20px tall. The null renderer
/// measures text as zero-sized, so a line's height is stated rather than
/// read from a font -- which is fine: what is under test is how the layout
/// algorithm distributes a height, not what a font measures.
fn lines<'a>(_prefix: &str, count: usize) -> El<'a> {
    column(
        (0..count)
            .map(|_| {
                container(text::<iced::Theme, ()>(""))
                    .width(Length::Fixed(100.0))
                    .height(Length::Fixed(20.0))
                    .into()
            })
            .collect::<Vec<El<'a>>>(),
    )
    .into()
}

fn lay_out(mut element: El<'_>, width: f32, height: f32) -> Node {
    let mut tree = Tree::new(element.as_widget());
    element.as_widget_mut().layout(
        &mut tree,
        &(),
        &Limits::new(Size::ZERO, Size::new(width, height)),
    )
}

fn pinned(body_lines: usize, viewport_height: f32) -> Node {
    let element: El<'_> = PinnedFooter::new(
        scrollable(lines("BODY", body_lines)),
        lines("FOOTER", 3),
        10.0,
    )
    .into();
    lay_out(element, 760.0, viewport_height)
}

fn footer_bottom(node: &Node) -> f32 {
    let footer = &node.children()[1];
    footer.bounds().y + footer.bounds().height
}

/// D2's property, at the two window heights the RFC names and one absurdly
/// tall body: however long the body, the footer's bottom edge is inside
/// the height the widget was given -- `Close` and the dismiss hint never
/// leave the window.
#[test]
fn the_footer_stays_inside_the_window_however_long_the_body_is() {
    for viewport_height in [560.0, 400.0] {
        for body_lines in [1, 12, 400, 20_000] {
            let node = pinned(body_lines, viewport_height);
            let footer = &node.children()[1];
            assert!(
                footer.bounds().height > 0.0,
                "the footer must get real height at {body_lines} body lines in \
                 {viewport_height}px: {:?}",
                footer.bounds()
            );
            assert!(
                footer_bottom(&node) <= viewport_height + 0.5,
                "footer bottom {} is below the {viewport_height}px window at \
                 {body_lines} body lines",
                footer_bottom(&node)
            );
            assert!(
                node.size().height <= viewport_height + 0.5,
                "the whole dialog must fit the window"
            );
        }
    }
}

/// The reason the widget exists, pinned so nobody "simplifies" it back:
/// the ordinary `column![scrollable(body), footer]` gives the scrollable
/// the whole height and the footer none. If this test ever fails, `iced`'s
/// `Column` has learned to reserve a footer and `PinnedFooter` can go.
#[test]
fn a_plain_column_clips_the_footer_which_is_why_the_widget_exists() {
    let element: El<'_> = column![scrollable(lines("BODY", 400)), lines("FOOTER", 3)]
        .spacing(10)
        .into();
    let node = lay_out(element, 760.0, 560.0);
    let footer = &node.children()[1];
    assert!(
        footer.bounds().height < 1.0 || footer.bounds().y + footer.bounds().height > 560.5,
        "a plain Column was expected to lose the footer: {:?}",
        footer.bounds()
    );
}

/// A short body is not stretched to fill the window: a dialog with one line
/// of content is as tall as one line of content plus its footer, never the
/// window's height.
#[test]
fn a_short_body_is_not_stretched_to_the_window() {
    let short = pinned(1, 560.0);
    let long = pinned(400, 560.0);
    assert!(
        short.size().height < 200.0,
        "a one-line body must not fill a 560px window: {}",
        short.size().height
    );
    assert!(long.size().height > short.size().height);
}

/// Body above footer, in that order, in every case.
#[test]
fn the_body_is_above_the_footer() {
    for body_lines in [1, 400] {
        let node = pinned(body_lines, 560.0);
        let body = node.children()[0].bounds();
        let footer = node.children()[1].bounds();
        assert!(body.y + body.height <= footer.y + 0.5);
    }
}

/// One redraw of `element` against `node`, returning whatever it published.
fn redraw(
    element: &mut Element<'_, Size, iced::Theme, ()>,
    tree: &mut Tree,
    node: &Node,
) -> Vec<Size> {
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    element.as_widget_mut().update(
        tree,
        &Event::Window(window::Event::RedrawRequested(std::time::Instant::now())),
        Layout::new(node),
        mouse::Cursor::Unavailable,
        &(),
        &mut clipboard::Null,
        &mut shell,
        &Rectangle::with_size(Size::new(1000.0, 1000.0)),
    );
    messages
}

/// `MeasureSize` reports the size the layout engine produced, once, and
/// again only when it changes -- so a steady window costs no messages and a
/// resize costs exactly one.
#[test]
fn measure_size_publishes_the_laid_out_size_once_and_again_only_on_change() {
    let mut element: Element<'_, Size, iced::Theme, ()> = MeasureSize::new(
        container(text::<iced::Theme, ()>("region"))
            .width(Length::Fill)
            .height(Length::Fill),
        |size| size,
    )
    .into();
    let mut tree = Tree::new(element.as_widget());
    let limits = |height| Limits::new(Size::ZERO, Size::new(500.0, height));

    let first = element
        .as_widget_mut()
        .layout(&mut tree, &(), &limits(300.0));
    assert_eq!(
        redraw(&mut element, &mut tree, &first),
        vec![Size::new(500.0, 300.0)],
        "the first layout reports the size the engine produced"
    );
    assert!(
        redraw(&mut element, &mut tree, &first).is_empty(),
        "an unchanged size must not publish again"
    );

    let second = element
        .as_widget_mut()
        .layout(&mut tree, &(), &limits(180.0));
    assert_eq!(
        redraw(&mut element, &mut tree, &second),
        vec![Size::new(500.0, 180.0)],
        "a smaller region reports its new size"
    );
}

/// It only speaks on a redraw: other events publish nothing, so the widget
/// cannot become a source of message storms.
#[test]
fn measure_size_ignores_events_other_than_a_redraw() {
    let mut element: Element<'_, Size, iced::Theme, ()> =
        MeasureSize::new(text::<iced::Theme, ()>("region"), |size| size).into();
    let mut tree = Tree::new(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        &(),
        &Limits::new(Size::ZERO, Size::new(500.0, 300.0)),
    );

    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    element.as_widget_mut().update(
        &mut tree,
        &Event::Mouse(mouse::Event::CursorMoved {
            position: iced::Point::ORIGIN,
        }),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &(),
        &mut clipboard::Null,
        &mut shell,
        &Rectangle::with_size(Size::new(1000.0, 1000.0)),
    );
    assert!(messages.is_empty());
}
