use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_platform::application;
use ui::RootView;

const BUILD_IDENTITY: &str = concat!("ordo/", env!("CARGO_PKG_VERSION"));

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| RootView::new(BUILD_IDENTITY, cx)),
        )
        .expect("the operator window should open");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_identity_uses_the_ordo_version() {
        assert_eq!(
            BUILD_IDENTITY.strip_prefix("ordo/"),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
