use iced::advanced::layout::Limits;
use iced::advanced::renderer::Headless;
use iced::advanced::widget::Tree;
use iced::{Element, Font, Pixels, Size};
use std::time::Instant;

fn main() {
    use std::task::{Context, Poll, Waker};
    let fut = <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia"));
    let mut fut = std::pin::pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    let renderer = loop {
        if let Poll::Ready(r) = fut.as_mut().poll(&mut cx) { break r.unwrap(); }
    };
    for (label, font) in [("default", Font::DEFAULT), ("monospace", Font::MONOSPACE)] {
        let mut best = std::time::Duration::MAX;
        for _ in 0..12 {
            let started = Instant::now();
            let rows: Vec<Element<'static, ()>> = (0..28)
                .map(|i| iced::widget::text(format!("  [+] ▣ file-{i:05}.rs [modified]")).size(14).font(font).wrapping(iced::widget::text::Wrapping::None).into())
                .collect();
            let mut el: Element<'static, ()> = iced::widget::column(rows).spacing(2).into();
            let mut tree = Tree::new(el.as_widget());
            let _ = el.as_widget_mut().layout(&mut tree, &renderer, &Limits::new(Size::ZERO, Size::new(268.0, 700.0)));
            best = best.min(started.elapsed());
        }
        println!("{label}: best of 12, 28 rows build+layout = {best:?}");
    }
}
