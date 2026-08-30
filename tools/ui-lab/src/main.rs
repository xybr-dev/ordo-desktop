use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_platform::application;
use ui::RootView;

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| RootView::new(concat!("ui-lab/", env!("CARGO_PKG_VERSION")), cx)),
        )
        .expect("the UI laboratory window should open");
        cx.activate(true);
    });
}
