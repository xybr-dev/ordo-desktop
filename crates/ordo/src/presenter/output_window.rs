//! The congregation window's view: Program, and nothing else (T1.6 WP4).
//!
//! It draws the very `Arc<Frame>` and decoded image the Program monitor
//! draws, read from the Presenter that owns them, through the same
//! [`congregation_viewport`] (Decision 310: no raster for live output). When
//! Program is black it paints black. It carries no operator-only element: no
//! Preparing badge, AUTO badge, progress bar, state word or mark, and no
//! focus handle, action or key handler.
//!
//! Its only listeners are the pointer (enter and leave, for the Presenter's
//! cursor locator, Decision 56) and a mouse press, after which the owner
//! re-activates the operator window, because the Windows backend activates
//! the passive window on a click.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    App, Context, Entity, IntoElement, Render, Subscription, Window, div, prelude::*, px, rgb,
};
use ui::environment::UiEnvironment;

use super::{PresenterView, dock::Shown};
use crate::canonical_views::congregation_viewport;

/// Receives `true` when the pointer enters the window and `false` when it
/// leaves.
pub type PointerListener = Rc<dyn Fn(bool, &mut App)>;

/// Receives each mouse-button press on the window.
pub type PressListener = Rc<dyn Fn(&mut App)>;

pub struct OutputWindowView {
    /// What Program drew, shared with the Presenter that writes it. A cell
    /// rather than a read of the Presenter entity, because this window can
    /// first draw while the Presenter is mid-update (it opens from there).
    program: Rc<RefCell<Shown>>,
    environment: UiEnvironment,
    on_pointer: PointerListener,
    on_press: PressListener,
    /// What the last render drew, for tests and diagnostics.
    drawn: Shown,
    pointer_inside: bool,
    /// The window's own hover status render last saw (see
    /// `CongregationView`): element hover misses a fast exit off a
    /// full-screen window, so the window's status is reported as well.
    window_hovered: bool,
    /// Redraws this window when the Presenter changes. Dropped with it.
    _follow: Subscription,
}

impl OutputWindowView {
    pub fn new(
        presenter: &Entity<PresenterView>,
        program: Rc<RefCell<Shown>>,
        environment: UiEnvironment,
        on_pointer: PointerListener,
        on_press: PressListener,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            program,
            environment,
            on_pointer,
            on_press,
            drawn: Shown::default(),
            pointer_inside: false,
            window_hovered: false,
            _follow: cx.observe(presenter, |_, _, cx| cx.notify()),
        }
    }

    /// The frame the last render drew, `None` while the window painted black.
    pub fn drawn_frame(&self) -> Option<&std::sync::Arc<rendering::Frame>> {
        self.drawn.frame.as_ref()
    }

    /// Reports a pointer change once, deferred so render never updates the
    /// owner while this view is mid-update.
    fn report_pointer(&mut self, inside: bool, cx: &mut Context<Self>) {
        if self.pointer_inside == inside {
            return;
        }
        self.pointer_inside = inside;
        let on_pointer = self.on_pointer.clone();
        cx.defer(move |cx| on_pointer(inside, cx));
    }
}

impl Render for OutputWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hovered = window.is_window_hovered();
        if self.window_hovered != hovered {
            self.window_hovered = hovered;
            self.report_pointer(hovered, cx);
        }
        let shown = self.program.borrow().clone();
        if shown.frame.is_some() && shown.opacity < 1.0 {
            // A fade in flight: the Presenter moves the opacity each frame.
            window.request_animation_frame();
        }
        self.drawn = shown.clone();

        // The viewport is 16:9 and as large as fits, centered on black.
        let size = window.viewport_size();
        let (width, height) = (f32::from(size.width), f32::from(size.height));
        let fit = width.min(height * 16.0 / 9.0);
        div()
            .id("congregation-root")
            .debug_selector(|| "output-window".into())
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(ui::tokens::color::PROJECTION_BACKGROUND))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| this.report_pointer(*hovered, cx)))
            .on_any_mouse_down({
                let on_press = self.on_press.clone();
                move |_, _, cx| on_press(cx)
            })
            .child(div().flex_none().w(px(fit)).child(congregation_viewport(
                shown.frame,
                shown.image,
                fit,
                shown.opacity,
                self.environment,
            )))
    }
}
