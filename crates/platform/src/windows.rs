use gpui::{Bounds, WindowBounds, WindowDecorations, WindowOptions};

pub fn preparation_window_options(bounds: Bounds<gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_decorations: Some(WindowDecorations::Server),
        ..Default::default()
    }
}

pub fn presenter_window_options(bounds: Bounds<gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Fullscreen(bounds)),
        titlebar: None,
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Point, Size, px};

    fn bounds() -> Bounds<gpui::Pixels> {
        Bounds {
            origin: Point::default(),
            size: Size {
                width: px(1920.0),
                height: px(1080.0),
            },
        }
    }

    #[test]
    fn preparation_uses_server_decorations() {
        let options = preparation_window_options(bounds());
        assert_eq!(options.window_decorations, Some(WindowDecorations::Server));
        assert!(matches!(
            options.window_bounds,
            Some(WindowBounds::Windowed(_))
        ));
        assert!(options.titlebar.is_some());
    }

    #[test]
    fn presenter_is_borderless_fullscreen() {
        let options = presenter_window_options(bounds());
        assert_eq!(options.window_decorations, Some(WindowDecorations::Client));
        assert!(matches!(
            options.window_bounds,
            Some(WindowBounds::Fullscreen(_))
        ));
        assert!(options.titlebar.is_none());
    }
}
