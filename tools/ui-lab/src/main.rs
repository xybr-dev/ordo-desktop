use gpui::{App, AppContext, Bounds, Focusable, WindowBounds, WindowOptions, px, size};
use gpui_platform::application;
use ui::lab::LabView;

fn main() {
    application().run(|cx: &mut App| {
        ui::lab::init(cx).expect("the UI environment should initialize");
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let lab = LabView::new(window, cx);
                    window.focus(&lab.focus_handle(cx), cx);
                    lab
                })
            },
        )
        .expect("the UI laboratory window should open");
        cx.activate(true);
    });
}
