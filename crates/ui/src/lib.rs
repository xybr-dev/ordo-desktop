//! Ordo-owned GPUI primitives and application-shell views.

use gpui::{App, Context, FocusHandle, Focusable, Render, Window, actions, div, prelude::*, rgb};

actions!(ordo_shell, [OpenSongs]);

/// The minimal shell destinations needed to prove action-driven navigation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ShellDestination {
    #[default]
    Masses,
    Songs,
}

/// T0.1 root view used by both the native harness and `ui-lab`.
pub struct RootView {
    active_destination: ShellDestination,
    focus_handle: FocusHandle,
}

impl RootView {
    pub const BUILD_IDENTITY: &str = concat!("ordo/", env!("CARGO_PKG_VERSION"));
    pub const CONNECTION_STATUS: &str = "Offline";

    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            active_destination: ShellDestination::Masses,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn active_destination(&self) -> ShellDestination {
        self.active_destination
    }

    fn open_songs(&mut self, _: &OpenSongs, _: &mut Window, cx: &mut Context<Self>) {
        self.active_destination = ShellDestination::Songs;
        cx.notify();
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RootView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let destination = match self.active_destination {
            ShellDestination::Masses => "Masses",
            ShellDestination::Songs => "Songs",
        };

        div()
            .id("ordo-root")
            .key_context("OrdoShell")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_songs))
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .bg(rgb(0x11100f))
            .text_color(rgb(0xf7f3eb))
            .child(
                div()
                    .debug_selector(|| "build-identity".into())
                    .child(Self::BUILD_IDENTITY),
            )
            .child(
                div()
                    .debug_selector(|| "connection-status".into())
                    .child(Self::CONNECTION_STATUS),
            )
            .child(
                div()
                    .debug_selector(move || format!("destination-{destination}"))
                    .child(destination),
            )
    }
}

pub const DESIGN_TOKENS_JSON: &str = include_str!("../design-tokens.json");

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn root_view_opens_dispatches_navigation_and_stays_offline(cx: &mut TestAppContext) {
        let (root, cx) = cx.add_window_view(|window, cx| {
            let root = RootView::new(cx);
            window.focus(&root.focus_handle, cx);
            root
        });

        root.read_with(cx, |root, _| {
            assert_eq!(RootView::BUILD_IDENTITY, "ordo/0.1.0");
            assert_eq!(RootView::CONNECTION_STATUS, "Offline");
            assert_eq!(root.active_destination(), ShellDestination::Masses);
        });
        assert!(cx.debug_bounds("build-identity").is_some());
        assert!(cx.debug_bounds("connection-status").is_some());
        assert!(cx.debug_bounds("destination-Masses").is_some());

        cx.dispatch_action(OpenSongs);

        root.read_with(cx, |root, _| {
            assert_eq!(root.active_destination(), ShellDestination::Songs);
        });
        assert!(cx.debug_bounds("destination-Songs").is_some());
        cx.run_until_parked();
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }
}
