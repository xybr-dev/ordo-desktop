//! Congregation-output composition: the one place where Display Control
//! (`platform`) meets the Presentation Engine (`presentation`).
//!
//! [`CongregationOutput`] is a runtime-owned GPUI entity holding the
//! [`DisplayControl`], the [`PresentationEngine`], and at most one passive
//! congregation window. Startup opens no congregation window. Output appears
//! only through an explicit [`CongregationOutput::enable_output`], which reads
//! a fresh topology, asks Display Control for a placement, publishes black
//! Program, and opens exactly one window through
//! [`congregation_window_options`](platform::congregation_window::congregation_window_options).
//!
//! **Topology refresh is explicit or window-driven, never polled.** The pinned
//! GPUI snapshot has no display-change event, so a refresh happens in three
//! places: [`CongregationOutput::enable_output`] refreshes before it places
//! output; a bounds observer on the open congregation window refreshes when
//! Windows moves that window, as it does when its display is removed; and
//! [`CongregationOutput::refresh_displays`] is available to callers that want
//! one now. No periodic task is started. The bounds observer is owned by this
//! controller in `window_bounds`, which is dropped in `close_window` and
//! `window_closed`, so it never outlives the window it watches.
//!
//! **Clicks do not steal keyboard focus.** The Windows backend activates the
//! passive window on a click (`WM_MOUSEACTIVATE` answers `MA_ACTIVATE`). The
//! congregation surface therefore reports presses, and this controller
//! re-activates the operator window, deferred, after each one.
//!
//! **Pointer hiding is not available in this GPUI snapshot.** `CursorStyle`
//! has no hidden variant, the only hiding facility
//! (`Platform::hide_cursor_until_mouse_moves`) is crate-private to GPUI's
//! keyboard dispatch and is undone by the next mouse move, and Win32
//! `ShowCursor` needs `unsafe`, which the workspace forbids. The congregation
//! view therefore keeps the default arrow and instead reports pointer
//! enter/leave so the operator workspace shows a cursor locator whose return
//! direction comes from the actual [`Topology`]. Physical hiding stays under
//! T0.5's `[MANUAL]` criterion.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    AnyWindowHandle, App, AppContext, Context, Subscription, Window, WindowHandle, WindowId,
};
use platform::{
    congregation_window::congregation_window_options,
    display::{
        AssignError, DisplayAdapter, DisplayControl, DisplayEvent, DisplayId, EnableError,
        OutputLifecycle, RelativeDirection, Topology,
        gpui_adapter::{GpuiDisplayAdapter, display_containing_window},
    },
};
use presentation::{
    CommandRejected, CongregationState, OutputStatus, PresentationCommand, PresentationEngine,
    ProgramFrame,
};

use crate::CongregationView;

/// Where display topology snapshots come from. Production reads GPUI through
/// [`GpuiDisplaySource`]; tests inject an in-memory adapter shared through
/// `Rc<RefCell<_>>` so they can reshape the topology between refreshes.
pub trait DisplaySource {
    /// One discovery snapshot. `operator` is the display currently carrying
    /// the operator window when GPUI knows it; sources with their own notion
    /// of the operator display may ignore it.
    fn snapshot(&self, cx: &App, operator: Option<gpui::DisplayId>) -> Topology;

    /// The display the platform reports `window` on after it was asked to open
    /// on `requested`. GPUI falls back silently to the primary display when it
    /// does not know the requested id, so the caller compares this result with
    /// the request. A source that models a platform placing windows exactly as
    /// asked reports `requested`.
    fn reported_display(
        &self,
        window: &Window,
        requested: DisplayId,
        cx: &App,
    ) -> Option<DisplayId>;
}

/// Production source: the GPUI Adapter over the running application.
#[derive(Clone, Copy, Debug, Default)]
pub struct GpuiDisplaySource;

impl DisplaySource for GpuiDisplaySource {
    fn snapshot(&self, cx: &App, operator: Option<gpui::DisplayId>) -> Topology {
        GpuiDisplayAdapter::new(cx, operator).snapshot()
    }

    fn reported_display(&self, window: &Window, _: DisplayId, cx: &App) -> Option<DisplayId> {
        window_display(window, &cx.displays(), cx).map(DisplayId::from)
    }
}

impl<A: DisplayAdapter> DisplaySource for Rc<RefCell<A>> {
    fn snapshot(&self, _: &App, _: Option<gpui::DisplayId>) -> Topology {
        self.borrow().snapshot()
    }

    /// The in-memory double models a platform that places windows exactly
    /// where they are asked to go. Tests for the fallback path use their own
    /// source that reports another display.
    fn reported_display(&self, _: &Window, requested: DisplayId, _: &App) -> Option<DisplayId> {
        Some(requested)
    }
}

/// Operator-only guidance shown while the pointer is over congregation
/// output: which way to move to get back to the operator display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorLocator {
    pub congregation: DisplayId,
    pub operator: DisplayId,
    /// Direction from the congregation display back to the operator display.
    pub direction: RelativeDirection,
}

impl CursorLocator {
    /// Short operator-facing instruction for [`Self::direction`].
    pub fn instruction(&self) -> &'static str {
        match self.direction {
            RelativeDirection::Left => "Move the pointer left to return to the operator display",
            RelativeDirection::Right => "Move the pointer right to return to the operator display",
            RelativeDirection::Above => "Move the pointer up to return to the operator display",
            RelativeDirection::Below => "Move the pointer down to return to the operator display",
            RelativeDirection::AboveLeft => {
                "Move the pointer up and left to return to the operator display"
            }
            RelativeDirection::AboveRight => {
                "Move the pointer up and right to return to the operator display"
            }
            RelativeDirection::BelowLeft => {
                "Move the pointer down and left to return to the operator display"
            }
            RelativeDirection::BelowRight => {
                "Move the pointer down and right to return to the operator display"
            }
            RelativeDirection::Overlapping => {
                "The operator and congregation displays overlap; move the pointer off Program"
            }
        }
    }
}

/// Locator guidance for a pointer on `congregation`, derived only from the
/// topology's actual arrangement. `None` when the topology has no operator
/// display or does not contain `congregation`.
pub fn locate_cursor(topology: &Topology, congregation: DisplayId) -> Option<CursorLocator> {
    let operator = topology.operator()?;
    let direction = topology.direction(congregation, operator)?;
    Some(CursorLocator {
        congregation,
        operator,
        direction,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputError {
    /// Display Control refused to place output.
    Display(EnableError),
    /// The platform could not open the congregation window. Output was
    /// restored to the lifecycle it had before the attempt.
    Window,
    /// The window opened on a display other than the one requested, which
    /// GPUI does when it does not know the requested display. The window was
    /// closed and output restored to its prior lifecycle, so nothing claims
    /// the wrong screen.
    PlacementMismatch {
        requested: DisplayId,
        reported: Option<DisplayId>,
    },
}

/// Runtime-owned congregation-output controller.
///
/// Owner and shutdown: the runtime owns this entity for the process lifetime.
/// It owns the single congregation window handle and that window's bounds
/// observer. Both are released by `close_window`; `window_closed` releases
/// them when the platform closed the window first; `shutdown` closes it.
pub struct CongregationOutput {
    source: Box<dyn DisplaySource>,
    control: DisplayControl,
    engine: PresentationEngine,
    window: Option<WindowHandle<CongregationView>>,
    /// Keeps the bounds observer on `window` alive. Dropping it unsubscribes.
    window_bounds: Option<Subscription>,
    /// Lets production refreshes name the display carrying the operator
    /// window instead of assuming GPUI's primary display.
    operator_window: Option<AnyWindowHandle>,
    /// The last operator display GPUI reported. A window mid-update cannot be
    /// read, so refreshes fall back to this.
    last_operator_display: Option<gpui::DisplayId>,
    pointer_over_output: bool,
}

impl CongregationOutput {
    /// Starts with output disabled, no assignment, black Program, and no
    /// congregation window, reading the initial topology from `source`.
    pub fn new(
        source: Box<dyn DisplaySource>,
        operator_window: Option<AnyWindowHandle>,
        cx: &mut Context<Self>,
    ) -> Self {
        let last_operator_display = read_operator_display(operator_window, cx);
        let control = DisplayControl::new(source.snapshot(cx, last_operator_display));
        Self {
            source,
            control,
            engine: PresentationEngine::disabled(),
            window: None,
            window_bounds: None,
            operator_window,
            last_operator_display,
            pointer_over_output: false,
        }
    }

    pub fn display_control(&self) -> &DisplayControl {
        &self.control
    }

    pub fn engine(&self) -> &PresentationEngine {
        &self.engine
    }

    pub fn congregation(&self) -> &CongregationState {
        self.engine.congregation()
    }

    /// The open congregation window, if output is enabled.
    pub fn window(&self) -> Option<WindowHandle<CongregationView>> {
        self.window
    }

    pub fn pointer_over_output(&self) -> bool {
        self.pointer_over_output
    }

    /// Assigns the congregation display. Never enables output.
    pub fn assign(
        &mut self,
        display: DisplayId,
        cx: &mut Context<Self>,
    ) -> Result<(), AssignError> {
        self.control.assign(display)?;
        cx.notify();
        Ok(())
    }

    /// Explicitly enables congregation output on the assigned display.
    ///
    /// The topology is read fresh first, so a display removed since the last
    /// refresh is refused rather than placed on. Program then goes black, and
    /// exactly one passive window opens. The window's placement is checked
    /// against the request. If opening fails or the placement is wrong, the
    /// controller returns to the lifecycle it had before the call. While a
    /// window is open, a second enable is rejected with
    /// [`EnableError::AlreadyEnabled`] and opens nothing.
    pub fn enable_output(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<WindowHandle<CongregationView>, OutputError> {
        if self.window.is_some() {
            return Err(OutputError::Display(EnableError::AlreadyEnabled));
        }
        // A snapshot from before a cable pull could name a display that is
        // gone, and GPUI would then fall back to the primary display silently.
        self.refresh_displays(cx);
        let prior_control = self.control.clone();
        let prior_status = self.engine.congregation().output_status();
        let placement = self.control.enable_output().map_err(OutputError::Display)?;
        let state = self.engine.apply(PresentationCommand::EnableOutput).clone();
        debug_assert!(state.program().is_black(), "enabled output starts black");

        let output = cx.weak_entity();
        let on_pointer: crate::PointerListener = Rc::new({
            let output = output.clone();
            move |over, cx| {
                output
                    .update(cx, |output, cx| output.set_pointer_over_output(over, cx))
                    .ok();
            }
        });
        let operator = self.operator_window;
        let on_press: crate::PressListener = Rc::new(move |cx| {
            // Deferred so the re-activation never runs inside the congregation
            // window's own event dispatch.
            if let Some(operator) = operator {
                cx.defer(move |cx| {
                    operator
                        .update(cx, |_, window, _| window.activate_window())
                        .ok();
                });
            }
        });

        let opened = cx.open_window(congregation_window_options(&placement), |_, cx| {
            cx.new(|cx| {
                CongregationView::with_listeners(state, Some(on_pointer), Some(on_press), cx)
            })
        });
        let window = match opened {
            Ok(window) => window,
            Err(_) => {
                self.restore_lifecycle(prior_control, prior_status, cx);
                return Err(OutputError::Window);
            }
        };

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
            self.restore_lifecycle(prior_control, prior_status, cx);
            return Err(OutputError::PlacementMismatch {
                requested,
                reported,
            });
        }

        // The observer refreshes topology whenever the window moves or
        // resizes. Windows moves a window off a removed display, so this is
        // how an unplugged projector is noticed without a caller.
        let Ok(bounds) = window.update(cx, |_, placed, cx| {
            cx.observe_window_bounds(placed, move |_, _, cx| {
                // Deferred: the observer runs inside the congregation window's
                // own update, where removing that window would be re-entrant.
                let output = output.clone();
                cx.defer(move |cx| {
                    output
                        .update(cx, |output, cx| {
                            output.refresh_displays(cx);
                        })
                        .ok();
                });
            })
        }) else {
            self.restore_lifecycle(prior_control, prior_status, cx);
            return Err(OutputError::Window);
        };

        self.window = Some(window);
        self.window_bounds = Some(bounds);
        cx.notify();
        Ok(window)
    }

    /// Explicitly disables output: Program black, window closed.
    pub fn disable_output(&mut self, cx: &mut Context<Self>) {
        self.control.disable_output();
        self.engine.apply(PresentationCommand::DisableOutput);
        self.close_window(cx);
    }

    /// Reads one topology snapshot from the injected source and applies it.
    pub fn refresh_displays(&mut self, cx: &mut Context<Self>) -> Vec<DisplayEvent> {
        let operator = self.operator_display(cx);
        let topology = self.source.snapshot(cx, operator);
        self.refresh_with(topology, cx)
    }

    /// Applies a topology snapshot. Losing the enabled display marks
    /// Presentation output `Disconnected` (Program black) and closes the
    /// congregation window; it is never reopened elsewhere.
    pub fn refresh_with(
        &mut self,
        topology: Topology,
        cx: &mut Context<Self>,
    ) -> Vec<DisplayEvent> {
        let events = self.control.refresh(topology);
        // An open window is only valid while its output is still placed. Any
        // other lifecycle here is `Disconnected`, the only state an open window
        // can leave to.
        if self.window.is_some()
            && !matches!(self.control.output(), OutputLifecycle::Enabled { .. })
        {
            self.engine.apply(PresentationCommand::OutputDisconnected);
            self.close_window(cx);
        }
        cx.notify();
        events
    }

    /// Selects the operator's Preview. Never touches Program.
    pub fn select_preview(&mut self, frame: ProgramFrame, cx: &mut Context<Self>) {
        self.engine.apply(PresentationCommand::SelectPreview(frame));
        cx.notify();
    }

    /// Takes `frame` live. Rejected unless output is connected.
    pub fn take_live(
        &mut self,
        frame: ProgramFrame,
        cx: &mut Context<Self>,
    ) -> Result<(), CommandRejected> {
        self.present(PresentationCommand::TakeLive(frame), cx)
    }

    /// The `Space` restore path: takes the current Preview live.
    pub fn take_preview_live(&mut self, cx: &mut Context<Self>) -> Result<(), CommandRejected> {
        self.present(PresentationCommand::TakePreviewLive, cx)
    }

    /// Records whether the pointer is over the congregation window; the
    /// operator shell observes this to show or hide the cursor locator.
    pub fn set_pointer_over_output(&mut self, over: bool, cx: &mut Context<Self>) {
        if self.pointer_over_output != over {
            self.pointer_over_output = over;
            cx.notify();
        }
    }

    /// Locator guidance while the pointer is over enabled congregation
    /// output; `None` otherwise.
    pub fn cursor_locator(&self) -> Option<CursorLocator> {
        if !self.pointer_over_output || self.window.is_none() {
            return None;
        }
        match self.control.output() {
            OutputLifecycle::Enabled { display, .. } => {
                locate_cursor(self.control.topology(), *display)
            }
            _ => None,
        }
    }

    /// The runtime calls this when any window closes. A congregation window
    /// closed by anything other than this controller disables output so
    /// Presentation never claims a surface that no longer exists.
    pub fn window_closed(&mut self, closed: WindowId, cx: &mut Context<Self>) {
        if self.window.map(|window| window.window_id()) != Some(closed) {
            return;
        }
        self.window = None;
        self.window_bounds = None;
        self.pointer_over_output = false;
        self.control.disable_output();
        self.engine.apply(PresentationCommand::DisableOutput);
        cx.notify();
    }

    /// Planned exit: force Program black and close the congregation window.
    pub fn shutdown(&mut self, cx: &mut Context<Self>) {
        self.disable_output(cx);
    }

    fn present(
        &mut self,
        command: PresentationCommand,
        cx: &mut Context<Self>,
    ) -> Result<(), CommandRejected> {
        let state = self.engine.try_apply(command)?.clone();
        if let Some(window) = self.window {
            window
                .update(cx, |view, _, cx| view.set_state(state, cx))
                .ok();
        }
        cx.notify();
        Ok(())
    }

    /// The operator display, read now when GPUI allows it and otherwise the
    /// last value read.
    fn operator_display(&mut self, cx: &mut App) -> Option<gpui::DisplayId> {
        if let Some(display) = read_operator_display(self.operator_window, cx) {
            self.last_operator_display = Some(display);
        }
        self.last_operator_display
    }

    /// Returns Display Control and presentation to the lifecycle captured
    /// before a failed enable. A prior `Disconnected` stays `Disconnected`,
    /// and a prior `Disabled` stays `Disabled`.
    fn restore_lifecycle(
        &mut self,
        control: DisplayControl,
        prior_status: OutputStatus,
        cx: &mut Context<Self>,
    ) {
        self.control = control;
        // Enabling moved presentation to `Connected`, so one command returns
        // to the prior status with Program black.
        let command = match prior_status {
            OutputStatus::Disconnected => PresentationCommand::OutputDisconnected,
            _ => PresentationCommand::DisableOutput,
        };
        self.engine.apply(command);
        cx.notify();
    }

    fn close_window(&mut self, cx: &mut Context<Self>) {
        self.pointer_over_output = false;
        // Dropping the observer first unsubscribes it before its window closes.
        self.window_bounds = None;
        if let Some(window) = self.window.take() {
            // Taking the handle first makes the runtime's deferred
            // `window_closed` a no-op for this planned close.
            window
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
        cx.notify();
    }
}

/// The display carrying the operator window, when it can be read now. A
/// window mid-update cannot be read, and the caller then uses its last value.
///
/// On Windows display ids are `HMONITOR` values reissued on display change,
/// and `Window::display` matches a core id refreshed only on bounds change, so
/// after a hot-plug it can read `None` or name the wrong monitor. The live
/// window bounds are therefore resolved against freshly read displays first,
/// and `Window::display` is only the fallback.
fn read_operator_display(window: Option<AnyWindowHandle>, cx: &mut App) -> Option<gpui::DisplayId> {
    let displays = cx.displays();
    window?
        .update(cx, |_, window, cx| window_display(window, &displays, cx))
        .ok()
        .flatten()
}

/// The display `window` is on: its live bounds resolved against `displays`,
/// with the platform's own report as the tie-break for mixed-DPI overlap and
/// as the fallback when the bounds match nothing.
fn window_display(
    window: &Window,
    displays: &[Rc<dyn gpui::PlatformDisplay>],
    cx: &App,
) -> Option<gpui::DisplayId> {
    let reported = window.display(cx).map(|display| display.id());
    display_containing_window(displays, window.bounds(), reported).or(reported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::display::{DetectedDisplay, DisplayBounds, DisplayEvidence, PlacementBounds};

    fn display(id: u64, x: i32, y: i32) -> DetectedDisplay {
        let bounds = DisplayBounds::new(x, y, 1920, 1080);
        DetectedDisplay {
            id: DisplayId::new(id),
            evidence: DisplayEvidence {
                name: None,
                uuid: None,
                bounds,
            },
            placement: PlacementBounds::from(bounds),
        }
    }

    #[test]
    fn locator_points_back_to_the_operator_display() {
        let topology = Topology::new(
            vec![display(1, 0, 0), display(2, 0, -1080)],
            Some(DisplayId::new(1)),
        );
        let locator = locate_cursor(&topology, DisplayId::new(2)).unwrap();
        assert_eq!(locator.direction, RelativeDirection::Below);
        assert_eq!(locator.operator, DisplayId::new(1));
        assert!(locator.instruction().contains("down"));
    }

    #[test]
    fn locator_needs_an_operator_and_a_known_display() {
        let topology = Topology::new(vec![display(1, 0, 0), display(2, 1920, 0)], None);
        assert_eq!(locate_cursor(&topology, DisplayId::new(2)), None);
        let topology = Topology::new(vec![display(1, 0, 0)], Some(DisplayId::new(1)));
        assert_eq!(locate_cursor(&topology, DisplayId::new(9)), None);
    }
}
