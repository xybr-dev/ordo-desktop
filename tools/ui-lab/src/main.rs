use gpui::{App, AppContext, Bounds, Focusable, WindowBounds, WindowOptions, px, size};
use gpui_platform::application;
use ui::RootView;

fn main() {
    application().run(|cx: &mut App| {
        ui::init(cx).expect("the UI environment should initialize");
        let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let root =
                        RootView::new(concat!("ui-lab/", env!("CARGO_PKG_VERSION")), window, cx);
                    window.focus(&root.focus_handle(cx), cx);
                    root
                })
            },
        )
        .expect("the UI laboratory window should open");
        cx.activate(true);
    });
}
