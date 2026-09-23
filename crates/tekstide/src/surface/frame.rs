//! RFC-053 PR-053-B: the two layout primitives `iced` does not have, and
//! the two defects that needed them.
//!
//! **[`PinnedFooter`] -- a body that scrolls above a footer that cannot
//! leave the window (D2).** `iced`'s `Column` lays children out *in order*,
//! each taking whatever height is left, so `column![scrollable(body),
//! footer]` gives the scrollable the whole height and the footer none:
//! the footer is clipped exactly when it is needed. This widget lays the
//! **footer out first**, then gives the body what remains. Whatever the
//! body's length and whatever the window size, the footer's bottom edge is
//! never below the height it was given -- a property of the layout
//! algorithm, provable headlessly with `iced`'s null renderer, not of a
//! font measurement someone estimated.
//!
//! **[`MeasureSize`] -- report the size a region was really laid out at
//! (D3).** `content_area_height` used to *reconstruct* the space available
//! to a terminal by subtracting named constants for chrome heights from the
//! window's. Every constant was a claim about how tall something renders,
//! and nothing enforced any of them: the status bar wraps at a narrow
//! window, and the top bar had grown from one row to three (RFC-039/040)
//! without the constant following. This wrapper publishes the region's
//! laid-out size, so the number that sizes a PTY is the one the layout
//! engine produced.
//!
//! Both are generic over `Renderer` so a test can drive the real layout
//! algorithm with `iced_core::renderer::null` -- no GPU, no font backend --
//! the same technique `assemble_change_review_layout`'s test already uses.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::{Element, Event, Length, Rectangle, Size, Vector, mouse, window};

/// A body above a footer, the footer laid out first. See the module doc.
pub(crate) struct PinnedFooter<'a, Message, Theme, Renderer> {
    body: Element<'a, Message, Theme, Renderer>,
    footer: Element<'a, Message, Theme, Renderer>,
    spacing: f32,
}

impl<'a, Message, Theme, Renderer> PinnedFooter<'a, Message, Theme, Renderer> {
    pub(crate) fn new(
        body: impl Into<Element<'a, Message, Theme, Renderer>>,
        footer: impl Into<Element<'a, Message, Theme, Renderer>>,
        spacing: f32,
    ) -> Self {
        Self {
            body: body.into(),
            footer: footer.into(),
            spacing,
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PinnedFooter<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.body), Tree::new(&self.footer)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.body, &self.footer]);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let max = limits.max();
        let loose = limits.loose();

        // The footer first, against the full height: it takes what it
        // needs. This is the whole point of the widget.
        let footer = self
            .footer
            .as_widget_mut()
            .layout(&mut tree.children[1], renderer, &loose);
        let footer_height = footer.size().height;

        // The body gets what is left, and never a negative amount.
        let body_room = (max.height - footer_height - self.spacing).max(0.0);
        let body_limits = layout::Limits::new(Size::ZERO, Size::new(max.width, body_room));
        let body = self
            .body
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &body_limits);
        let body_height = body.size().height;

        let width = body.size().width.max(footer.size().width);
        let height = body_height + self.spacing + footer_height;

        layout::Node::with_children(
            Size::new(width, height),
            vec![
                body.move_to((0.0, 0.0)),
                footer.move_to((0.0, body_height + self.spacing)),
            ],
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((child, tree), layout) in [&self.body, &self.footer]
            .into_iter()
            .zip(&tree.children)
            .zip(layout.children())
        {
            child
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            for ((child, state), layout) in [&mut self.body, &mut self.footer]
                .into_iter()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                child
                    .as_widget_mut()
                    .operate(state, layout, renderer, operation);
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for ((child, tree), layout) in [&mut self.body, &mut self.footer]
            .into_iter()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        [&self.body, &self.footer]
            .into_iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let children = [&mut self.body, &mut self.footer]
            .into_iter()
            .zip(&mut tree.children)
            .zip(layout.children())
            .filter_map(|((child, tree), layout)| {
                child
                    .as_widget_mut()
                    .overlay(tree, layout, renderer, viewport, translation)
            })
            .collect::<Vec<_>>();

        (!children.is_empty()).then(|| overlay::Group::with_children(children).overlay())
    }
}

impl<'a, Message, Theme, Renderer> From<PinnedFooter<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(widget: PinnedFooter<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

/// Wraps a region and publishes its laid-out size whenever it changes.
///
/// The size is read from the layout node the engine produced -- never
/// estimated -- on the redraw that follows a layout, and published **only
/// when it differs** from the last value this widget published, so a steady
/// window costs nothing and a resize costs one message.
pub(crate) struct MeasureSize<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_change: Box<dyn Fn(Size) -> Message + 'a>,
}

impl<'a, Message, Theme, Renderer> MeasureSize<'a, Message, Theme, Renderer> {
    pub(crate) fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_change: impl Fn(Size) -> Message + 'a,
    ) -> Self {
        Self {
            content: content.into(),
            on_change: Box::new(on_change),
        }
    }
}

#[derive(Default)]
struct Reported(Option<Size>);

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MeasureSize<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Reported>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Reported::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            let size = layout.bounds().size();
            let reported = tree.state.downcast_mut::<Reported>();
            if reported.0 != Some(size) {
                reported.0 = Some(size);
                shell.publish((self.on_change)(size));
            }
        }

        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<MeasureSize<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(widget: MeasureSize<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

#[cfg(test)]
mod tests;
