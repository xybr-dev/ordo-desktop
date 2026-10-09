//! Passive congregation-output window options.
//!
//! Window style comes from the Windows backend (`gpui-pre-windows` `0.3.8`):
//! `WindowKind::PopUp` creates the window with `WS_EX_TOOLWINDOW |
//! WS_EX_TOPMOST` and no caption or frame style, so it is borderless, stays
//! above ordinary windows, and stays off the taskbar and Alt+Tab.
//!
//! Passivity is partial. `focus: false` with `show: true` reveals the window
//! with `SW_SHOWNOACTIVATE`, so showing it does not take keyboard focus from
//! the operator workspace. The backend answers `WM_MOUSEACTIVATE` with
//! `MA_ACTIVATE`, though, so a click on the congregation window activates it.
//! The window has no controls of its own. The ordo layer must re-activate the
//! operator window after such a click.
//!
//! Bounds are `Windowed` at exactly the placement's exact logical bounds rather
//! than `Fullscreen`: GPUI applies `Fullscreen` by spawning an asynchronous
//! `toggle_fullscreen` after the window is shown, which rewrites the window
//! style, records restore bounds, and marks the window as a "rude" full-screen
//! window to the shell. A captionless pop-up already covers the display
//! synchronously, so native full-screen adds state without adding coverage.
//!
//! `display_id` is only a request. If GPUI no longer has that display, it
//! silently falls back to the primary monitor (`src/window.rs` in
//! `gpui-pre-windows`). Callers must verify the window's placement after
//! opening it rather than trust the request.

use gpui::{WindowBounds, WindowKind, WindowOptions};

use crate::display::OutputPlacement;

pub fn congregation_window_options(placement: &OutputPlacement) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(placement.bounds.into())),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(placement.display.into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{DisplayId, PlacementBounds};
    use gpui::{Bounds, Pixels, point, px, size};

    #[test]
    fn congregation_window_requests_passive_options_on_its_display() {
        // Fractional logical bounds, as GPUI reports them at 150% scaling.
        let placement = OutputPlacement {
            display: DisplayId::new(42),
            bounds: PlacementBounds::new(1706.6666, -0.4, 1365.3334, 767.6),
        };
        let options = congregation_window_options(&placement);

        assert_eq!(options.kind, WindowKind::PopUp);
        assert!(!options.focus);
        assert!(options.show);
        assert!(!options.is_movable);
        assert!(!options.is_resizable);
        assert!(!options.is_minimizable);
        assert!(options.titlebar.is_none());
        assert_eq!(options.display_id, Some(gpui::DisplayId::new(42)));
        assert_eq!(
            options.window_bounds,
            Some(WindowBounds::Windowed(Bounds::<Pixels> {
                origin: point(px(1706.6666), px(-0.4)),
                size: size(px(1365.3334), px(767.6)),
            }))
        );
    }
}
