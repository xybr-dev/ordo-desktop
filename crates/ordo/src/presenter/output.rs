//! The Presenter's congregation output: the one place that opens and closes
//! the passive congregation window (T1.6 WP3 and WP4).
//!
//! [`OutputOwner`] holds the [`OutputController`] (the rules, in `platform`),
//! the display source that feeds it topology snapshots, and at most one
//! window. `PresenterView` owns it, so the window can only exist while the
//! Presenter does: the Presenter closes it on release.
//!
//! Order on enable: refresh topology, ask the controller, open the window,
//! then the Presenter sends `EnableOutput`. Program is black before `Space`
//! can take anything live. A refusal opens nothing and sends nothing.
//!
//! Display changes (T1.6c): the operator window's bounds observer, which the
//! vendored GPUI patch fires on every display change (Decision 308), calls
//! [`OutputOwner::refresh`]. Losing the active display closes the window
//! without forgetting which display it was on, so a return can be recognized
//! (Decision 272). Output is never moved. If the operating system closes the
//! window by itself, the Presenter treats it as `DisableOutput`: the Engine
//! goes back to Output disabled, so Program is black and nothing claims a
//! surface that is gone.

use gpui::{
    AnyWindowHandle, App, AppContext, Entity, Subscription, Window, WindowHandle, WindowId,
};
use platform::{
    congregation_window::congregation_window_options,
    display::{
        DisplayEvent, DisplayEvidence, DisplayId, OutputAction, OutputController, OutputLifecycle,
        OutputRefusal, Recognition,
    },
};
use ui::environment::UiEnvironment;

use std::{cell::RefCell, rc::Rc};

use super::{
    PresenterView,
    dock::Shown,
    output_window::{OutputWindowView, PointerListener, PressListener},
};
use crate::output::{CursorLocator, DisplaySource, locate_cursor, window_display};

/// Why the output window did not open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenFailure {
    Refused(OutputRefusal),
    /// The platform could not create the window, or placed it on another
    /// display than asked.
    Window,
}

impl OpenFailure {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Refused(refusal) => refusal.message(),
            Self::Window => "The congregation window could not open on its display",
        }
    }
}

/// What a display refresh found, for the Presenter to turn into Engine
/// commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayChange {
    /// Enabled output lost its display (or the display changed under it).
    pub lost: bool,
    /// A display was added.
    pub added: bool,
    /// The congregation display is connected and recognized as the same one.
    pub recognized: bool,
}

pub struct OutputOwner {
    source: Box<dyn DisplaySource>,
    controller: OutputController,
    operator_window: AnyWindowHandle,
    window: Option<WindowHandle<OutputWindowView>>,
    /// Notices the platform closing the window. Dropped with it.
    release: Option<Subscription>,
    pointer_over: bool,
    /// Notices display changes through the operator window's bounds. Dropped
    /// with the owner.
    display_watch: Option<Subscription>,
    /// Test only: the Engine's live state each time the window was closed.
    #[cfg(test)]
    pub(super) closed_at: Vec<presentation::state::LiveState>,
}

impl OutputOwner {
    pub fn new(source: Box<dyn DisplaySource>, operator_window: &Window, cx: &mut App) -> Self {
        let operator = window_display(operator_window, &cx.displays(), cx);
        let controller = OutputController::new(source.snapshot(cx, operator));
        Self {
            source,
            controller,
            operator_window: operator_window.window_handle(),
            window: None,
            release: None,
            pointer_over: false,
            display_watch: None,
            #[cfg(test)]
            closed_at: Vec::new(),
        }
    }

    pub fn watch_displays(&mut self, watch: Subscription) {
        self.display_watch = Some(watch);
    }

    /// Reads a fresh topology and reports what changed. Never opens or closes
    /// a window.
    pub fn refresh(&mut self, operator: Option<gpui::DisplayId>, cx: &App) -> DisplayChange {
        let topology = self.source.snapshot(cx, operator);
        let events = self.controller.refresh(topology);
        DisplayChange {
            lost: events
                .iter()
                .any(|event| matches!(event, DisplayEvent::OutputDisconnected { .. })),
            added: events
                .iter()
                .any(|event| matches!(event, DisplayEvent::DisplayAdded(_))),
            recognized: matches!(
                self.controller.control().recognition(),
                Recognition::Recognized(_)
            ),
        }
    }

    /// The display went away: closes the window but keeps the controller
    /// disconnected, so the same display can be recognized when it returns.
    pub fn lose(&mut self, cx: &mut App) {
        self.pointer_over = false;
        self.release = None;
        if let Some(window) = self.window.take() {
            window
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
    }

    pub fn is_open(&self) -> bool {
        self.window.is_some()
    }

    pub fn window(&self) -> Option<WindowHandle<OutputWindowView>> {
        self.window
    }

    /// Where the pointer should go to get back to the operator display, while
    /// it is over the output window (Decision 56).
    pub fn cursor_locator(&self) -> Option<CursorLocator> {
        if !self.pointer_over || self.window.is_none() {
            return None;
        }
        match self.controller.control().output() {
            OutputLifecycle::Enabled { display, .. } => {
                locate_cursor(self.controller.control().topology(), *display)
            }
            _ => None,
        }
    }

    pub fn set_pointer_over(&mut self, over: bool) -> bool {
        std::mem::replace(&mut self.pointer_over, over) != over
    }

    /// Reads a fresh topology and opens the one passive window on the
    /// placement the controller chooses. `operator` is the display the
    /// operator window is on, read by the caller from its own window.
    pub fn open(
        &mut self,
        presenter: &Entity<PresenterView>,
        program: Rc<RefCell<Shown>>,
        environment: UiEnvironment,
        operator: Option<gpui::DisplayId>,
        paint_probe: Option<super::PaintProbe>,
        cx: &mut App,
    ) -> Result<(), OpenFailure> {
        if self.window.is_some() {
            return Ok(());
        }
        let topology = self.source.snapshot(cx, operator);
        let placement = match self.controller.enable(topology) {
            OutputAction::Open(placement) => placement,
            OutputAction::Refuse(refusal) => return Err(OpenFailure::Refused(refusal)),
            OutputAction::Close => return Err(OpenFailure::Window),
        };

        let on_pointer: PointerListener = {
            let presenter = presenter.downgrade();
            std::rc::Rc::new(move |over, cx| {
                presenter
                    .update(cx, |presenter, cx| {
                        presenter.set_pointer_over_output(over, cx)
                    })
                    .ok();
            })
        };
        let operator_window = self.operator_window;
        let on_press: PressListener = std::rc::Rc::new(move |cx| {
            // Deferred so the re-activation never runs inside the congregation
            // window's own event dispatch.
            cx.defer(move |cx| {
                operator_window
                    .update(cx, |_, window, _| window.activate_window())
                    .ok();
            });
        });
        let opened = cx.open_window(congregation_window_options(&placement), |_, cx| {
            cx.new(|cx| {
                let mut view = OutputWindowView::new(
                    presenter,
                    program,
                    environment,
                    on_pointer,
                    on_press,
                    cx,
                );
                view.set_paint_probe(paint_probe);
                view
            })
        });
        let Ok(window) = opened else {
            self.controller.disable();
            return Err(OpenFailure::Window);
        };
        // GPUI silently falls back to the primary display for an id it does
        // not know, so the placement is verified rather than trusted.
        let requested = placement.display;
        let reported = window
            .update(cx, |_, placed, cx| {
                self.source.reported_display(placed, requested, cx)
            })
            .ok()
            .flatten();
        if reported != Some(requested) {
            window
                .update(cx, |_, placed, _| placed.remove_window())
                .ok();
            self.controller.disable();
            return Err(OpenFailure::Window);
        }
        if let Ok(view) = window.entity(cx) {
            let id = window.window_id();
            let presenter = presenter.downgrade();
            self.release = Some(cx.observe_release(&view, move |_, cx| {
                // The window went away. Deferred: this runs mid-release.
                let presenter = presenter.clone();
                cx.defer(move |cx| {
                    presenter
                        .update(cx, |presenter, cx| presenter.output_window_closed(id, cx))
                        .ok();
                });
            }));
        }
        self.window = Some(window);
        Ok(())
    }

    /// Closes the window, if open. The caller has already made Program black.
    pub fn close(&mut self, cx: &mut App) {
        self.pointer_over = false;
        // Dropping the subscription first makes the platform close below a
        // planned one, not a loss.
        self.release = None;
        self.controller.disable();
        if let Some(window) = self.window.take() {
            window
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
    }

    /// The platform closed `closed` itself. `true` when it was this window.
    pub fn window_closed(&mut self, closed: WindowId) -> bool {
        if self.window.map(|window| window.window_id()) != Some(closed) {
            return false;
        }
        self.window = None;
        self.release = None;
        self.pointer_over = false;
        self.controller.disable();
        true
    }

    /// The evidence of the display the output was placed on, while it is
    /// enabled or disconnected; `None` once output is disabled. The recovery
    /// journal records it.
    pub fn evidence(&self) -> Option<&DisplayEvidence> {
        match self.controller.control().output() {
            OutputLifecycle::Enabled { evidence, .. }
            | OutputLifecycle::Disconnected { evidence } => Some(evidence),
            OutputLifecycle::Disabled => None,
        }
    }

    /// The display the output is on, while it is enabled.
    pub fn display(&self) -> Option<DisplayId> {
        match self.controller.control().output() {
            OutputLifecycle::Enabled { display, .. } => Some(*display),
            _ => None,
        }
    }
}
