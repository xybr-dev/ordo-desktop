//! Presenter surface: the pure layout model (T1.4a), the frame store (T1.4b),
//! the GPUI view over the Engine (T1.4 WP4), the grouped Cue timeline (WP6)
//! and the Ordo rail (WP7).
//!
//! [`PresenterView`] owns the Engine and reads it; the Engine never reads the
//! view. Layout, Interface scale, Grid/List and rail choices are view state and
//! never reach the Engine. Only a button (or, later, a key) sends a
//! `Command`, and only through [`crate::commands::dispatch`].
//!
//! # Tasks, their owner and their shutdown
//!
//! The view owns every long-lived task it starts, as a [`Task`] field, so
//! dropping the view (closing the Presenter) cancels all of them:
//!
//! * `timer`: one sleep until `Engine::next_deadline`, replaced whenever the
//!   deadline changes and cleared when it fires;
//! * `preparing_timer`: one sleep until the slow-frame badge is due,
//!   replaced when the due time changes and cleared when it fires;
//! * `frames`: the compose jobs, held by [`PresenterFrames`] (see its docs).
//!
//! The timeline and rail start no task. Their scroll handles, focus handles
//! and the fold and slide transitions are owned by the view (or by GPUI's
//! keyed window state) and end with it.
//!
//! The only detached work is the one-shot hop that re-syncs frames after a
//! compose failure was reported; it finishes on the next executor turn and
//! holds only a weak handle to the view.
//!
//! Nothing here blocks the GPUI thread: composition and decoding run in the
//! frame store's background jobs, and `render` only reads finished frames.

pub mod dock;
pub mod frames;
pub mod header;
pub mod images;
pub mod layout;
pub mod rail;
pub mod surfaces;
pub mod timeline;

use std::{
    collections::BTreeSet,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use domain::presentation::{CueBehavior, FrameId};
use gpui::{
    Context, FocusHandle, IntoElement, ParentElement, Render, SharedString, Styled, Task,
    WeakFocusHandle, Window, div, prelude::*, px, rgb,
};
use presentation::{
    commands::Command,
    engine::{AutoPlayRefusal, Availability, Engine, Outcome, Refusal, UnavailableSource},
    snapshot_reader::LoadedSnapshot,
    state::{LiveState, Millis, ProgramView},
};
use rendering::PRELOAD_RADIUS;
use ui::{
    NextRegion, PreviousRegion,
    accessibility::{RegionRegistry, label_region},
    environment::{CueView, EnvironmentChange, UiEnvironment, UiPreferences},
    scroll::Scroll,
    tokens,
};

use self::{
    dock::{
        AutoPlayEntry, AutoPlayModel, ControlsModel, OnCommand, PreviewModel, Primary,
        ProgramModel, Shown,
    },
    frames::{FrameView, PresenterFrames},
    header::{DisplayHealth, HeaderModel, OnCueView},
    layout::{
        BAND_GAP, BAND_GUTTER, BAND_TOP_PADDING, DockLayout, LayoutInput, PresenterLayout,
        RAIL_PINNED_WIDTH, RailChoice, RailLayout, Regime, presenter_layout,
    },
    rail::{PinControl, RailItem, RailModel, RailSection},
    surfaces::{Surface, SurfaceStack},
    timeline::{CueKind, CueModel, GroupModel, TimelineModel},
};
use crate::{
    commands::{
        dispatch,
        registry::{self, CommandId, FocusContext},
    },
    shortcut_reference,
};

/// Saves preferences through the owner's save path. The shell passes its
/// background-worker saver; the lab passes one that writes under
/// `target/live-control-lab/`.
pub type SavePreferences = Rc<dyn Fn(UiPreferences)>;

/// What one `render` of the Presenter cost, handed to a [`RenderProbe`].
#[derive(Clone, Copy, Debug)]
pub struct RenderStat {
    /// Wall time of building the element tree (not layout, prepaint or paint).
    pub render: Duration,
    /// Frames the store has composed so far, so a probe sees when a thumbnail
    /// arrived between two renders.
    pub composed: usize,
    /// Frame jobs in flight when the render ended.
    pub jobs_in_flight: usize,
}

/// Measurement hook: called after every render. Only the labs set one.
pub type RenderProbe = Rc<dyn Fn(RenderStat)>;

/// The operator-facing sentence for a refusal. A refusal says what to do
/// next in plain words; it never blames the operator and never shows the
/// Engine's developer-facing `Display` text.
pub fn refusal_text(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::UnavailablePreview { .. } => "Preview is unavailable — choose another Cue",
        Refusal::OutputDisabled => "Output is off — choose Enable Output first",
        Refusal::DisplayNotBack => "The congregation display is not back yet",
        Refusal::OutputDisconnected => "The congregation display is disconnected",
        Refusal::FreezeDuringBlackout => "End Blackout before freezing",
        Refusal::NotLive => "Take Preview live before freezing",
        Refusal::AutoPlayIneligible { reason } => match reason {
            AutoPlayRefusal::NotGeneralPresentation => {
                "Auto-play works only on General Presentations — use Next to move through this item"
            }
            AutoPlayRefusal::NoDurations => {
                "This item has no auto-play time — set a time in the General Presentation, or use Next"
            }
            AutoPlayRefusal::NothingProjectable => {
                "Nothing here can be shown automatically — choose another item, or use Next"
            }
            AutoPlayRefusal::SectionHasIneligibleItem => {
                "This section has items that cannot auto-play — use Next, or loop a General Presentation item on its own"
            }
        },
        Refusal::AutoPlayBlocked => {
            "Auto-play cannot start while frozen, blacked out or disconnected"
        }
    }
}

/// The standing status line when no refusal is being shown.
/// Frames a rail jump may stay pending when the group never reaches the top
/// (the scroll stopped at the end of the timeline): one to apply the scroll,
/// one to see it, and a spare.
const JUMP_RENDERS: u8 = 3;

/// Whether a group's top (`item_top`, window pixels) is at the top of the
/// timeline's `bounds`, within a device pixel. `scroll_to_top_of_item` leaves
/// the viewport top on the shared edge of the group above and this one, so the
/// group under the viewport top (`top_item`) cannot say that it has arrived.
fn jump_landed(item_top: f32, bounds: gpui::Bounds<gpui::Pixels>) -> bool {
    (item_top - f32::from(bounds.origin.y)).abs() <= 1.0
}

fn idle_status(engine: &Engine) -> &'static str {
    match engine.state().live() {
        LiveState::OutputDisabled => "Congregation output disabled",
        LiveState::Disconnected {
            display_back: false,
        } => "Congregation output disconnected",
        LiveState::Disconnected { display_back: true } => "Display back · output not resumed",
        _ => match engine.preview_availability() {
            Availability::Unavailable(_) => "Preview is unavailable — choose another Cue",
            Availability::Presentable => "Output enabled",
        },
    }
}

/// How long a wanted frame may stay pending before the operator sees a
/// "Preparing…" badge. The owner's decision (PR #91 review, 2026-11-04): the
/// congregation output always keeps the previous complete frame, so this only
/// tells the operator that the next slide is slower than expected.
pub const PREPARING_THRESHOLD_MS: u64 = 250;

/// The frame a monitor wants, and when it began to want it (injected clock).
#[derive(Clone, Copy, Default)]
struct Wanted {
    id: Option<FrameId>,
    since: Millis,
    /// Whether the frame was pending when last observed.
    pending: bool,
}

impl Wanted {
    /// Follow the Engine: a different frame restarts the clock, and so does
    /// the same frame going from Ready back to Pending (evicted, then
    /// composed again).
    fn follow(&mut self, id: Option<FrameId>, now: Millis, pending: bool) {
        if self.id != id {
            *self = Self {
                id,
                since: now,
                pending,
            };
        } else {
            if pending && !self.pending {
                self.since = now;
            }
            self.pending = pending;
        }
    }
}

/// The Presenter: header, rail, Cue timeline and Output Dock over one Engine.
pub struct PresenterView {
    /// Reports each render's cost; `None` outside the measurement labs.
    render_probe: Option<RenderProbe>,
    snapshot: Arc<LoadedSnapshot>,
    engine: Engine,
    frames: PresenterFrames,
    /// Injected so tests drive time exactly; production counts from session
    /// start.
    clock: Rc<dyn Fn() -> Millis>,
    environment: UiEnvironment,
    /// Session only (Decision 237); never reaches the Engine or the disk.
    rail_choice: RailChoice,
    /// Whether the rail overlay is open, and how many times it has opened
    /// (the slide restarts on each opening). Session only, like the choice.
    surfaces: SurfaceStack,
    rail_generation: u64,
    /// Which groups are folded. View state: folding never reaches the Engine.
    collapsed: BTreeSet<usize>,
    /// The timeline's scroll surface. The groups are its only children, so a
    /// group's index is its child index for `scroll_to_top_of_item`.
    timeline_scroll: Scroll,
    /// The groups (first, last) the timeline showed when the visible-frame
    /// set was last requested. A different range after a scroll or a resize
    /// asks for a new set, outside `render`.
    visible_range: (usize, usize),
    /// A rail jump the scroll has not applied yet, so the timeline lays that
    /// group's Cues out before it arrives. Dropped once the group's top is at
    /// the timeline's top (`jump_landed`), or after `JUMP_RENDERS` frames when
    /// the scroll could not reach it (the end of the timeline).
    pending_jump: Option<usize>,
    /// Frames drawn since `pending_jump` was set.
    jump_renders: u8,
    /// Test only: lay every group out, the layout before virtualisation.
    #[cfg(test)]
    all_groups_real: bool,
    /// The Preview Cue the timeline last brought into view; a different
    /// Preview on the next frame makes the timeline follow it.
    followed_preview: Option<usize>,
    /// The timeline's follow in flight (see `timeline::Glide`).
    glide: timeline::Glide,
    /// Whether the rail was drawn pinned on the last frame, so a pin toggle
    /// flips what the operator saw.
    rail_is_pinned: bool,
    rail_scroll: Scroll,
    /// The Mass item the keyboard cursor is on while the rail has focus
    /// (arrows move it, `Enter` jumps there). Cleared when focus leaves the
    /// rail, so the ring is only drawn where the keys go.
    rail_cursor: Option<usize>,
    /// Where focus goes when the rail overlay closes (the timeline, which
    /// holds the Cues), and where it goes while the overlay is open.
    timeline_focus: FocusHandle,
    rail_focus: FocusHandle,
    /// Tracked on the Output Dock, so a focused control inside it reads as
    /// the Dock for key policy.
    dock_focus: FocusHandle,
    /// Tracked on the Keyboard shortcuts panel while it is open, so a focused
    /// panel reads as `FocusContext::ShortcutReference` for key policy.
    reference_focus: FocusHandle,
    /// The panel's close button, where focus lands when it opens (`DESIGN.md`).
    reference_close_focus: FocusHandle,
    reference_scroll: Scroll,
    /// Where focus was when the reference opened; `Esc` or `Ctrl+/` puts it
    /// back there (the Cues when that is gone). Session only.
    reference_return: Option<WeakFocusHandle>,
    /// Whether that focus was inside the rail, whose control may be gone by
    /// the time the reference closes.
    reference_from_rail: bool,
    /// Set when an Auto-play menu choice closed the menu; the next frame puts
    /// focus on the (possibly replaced) Auto-play control.
    /// The F6 cycle over the three Presenter regions, which reuse the three
    /// focus handles above (the registry adopts them; it owns no handle here).
    regions: RegionRegistry,
    /// Test only: a real `ui` Field the timeline renders inside the
    /// Presenter's key context once a test asks for it.
    #[cfg(test)]
    text_field: Option<ui::primitives::field::FieldState>,
    /// Test only: a `ui` overlay host mounted inside the Presenter's key
    /// context, so a test can open a real dialog there.
    #[cfg(test)]
    modal_host: Option<ui::overlay::OverlayHost>,
    /// Test only: every key that bubbled up to the Presenter root unhandled.
    #[cfg(test)]
    unhandled_keys: Rc<std::cell::RefCell<Vec<String>>>,
    /// Test only: every Command the view sent to the Engine, in order.
    #[cfg(test)]
    sent: Rc<std::cell::RefCell<Vec<Command>>>,
    cue_view: CueView,
    /// The last refusal in words; cleared by the next accepted command.
    status: Option<SharedString>,
    save_preferences: Option<SavePreferences>,
    /// The deadline the sleeping task waits for, and the task. Dropped with
    /// the view.
    timer: Option<(Millis, Task<()>)>,
    /// The frame Program and Preview want, for the slow-frame badge.
    program_wanted: Wanted,
    preview_wanted: Wanted,
    /// When the next badge becomes due (a time on the injected clock), and
    /// the task sleeping until then. Dropped with the view.
    preparing_timer: Option<(u64, Task<()>)>,
    /// Redraws Program's auto-play countdown while a run is going (see
    /// `tick_auto_play_indicator`). Dropped, and so stopped, when it ends.
    auto_play_ticker: Option<Task<()>>,
    /// Program's progress bar at full length, in device pixels, from the last
    /// draw; it sets how often the bar is redrawn.
    auto_play_bar_px: f32,
    /// The last frame each monitor finished drawing, kept while its
    /// replacement is still composing (the consumer rule in
    /// `rendering::frame_cache`).
    last_program: Shown,
    last_preview: Shown,
}

/// The shortest wait between auto-play indicator redraws, about one 60 Hz
/// frame: a short Cue on a wide monitor moves the bar a pixel more often
/// than that, and a faster redraw would not show.
const AUTO_PLAY_MIN_REDRAW_MS: u64 = 16;

/// How long until Program's auto-play indicator next looks different: the
/// bar grows by one device pixel, or the countdown's whole second changes,
/// whichever comes first. `None` when nothing moves (no duration to count).
fn auto_play_redraw_ms(remaining_ms: Option<u64>, duration_ms: u64, bar_px: f32) -> Option<u64> {
    let remaining = remaining_ms?;
    let to_next_second = match remaining % 1000 {
        0 => 1000,
        ms => ms,
    };
    let per_pixel = if bar_px >= 1.0 && duration_ms > 0 {
        (duration_ms as f64 / f64::from(bar_px)) as u64
    } else {
        u64::MAX
    };
    Some(per_pixel.min(to_next_second).max(AUTO_PLAY_MIN_REDRAW_MS))
}

/// The F6 regions, from a fixed vocabulary, in reading order.
pub(super) const RAIL_REGION: &str = "Ordo rail";
pub(super) const TIMELINE_REGION: &str = "Cue timeline";
pub(super) const OUTPUT_REGION: &str = "Output controls";

impl PresenterView {
    pub fn new(
        snapshot: Arc<LoadedSnapshot>,
        clock: Rc<dyn Fn() -> Millis>,
        environment: UiEnvironment,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let engine = Engine::new(Arc::new(snapshot.snapshot().clone()));
        let mut view = Self {
            render_probe: None,
            frames: PresenterFrames::new(snapshot.clone()),
            snapshot,
            engine,
            clock,
            environment,
            rail_choice: RailChoice::Auto,
            surfaces: SurfaceStack::default(),
            rail_generation: 0,
            collapsed: BTreeSet::new(),
            timeline_scroll: Scroll::new(),
            visible_range: (0, 0),
            pending_jump: None,
            jump_renders: 0,
            #[cfg(test)]
            all_groups_real: false,
            followed_preview: None,
            glide: timeline::Glide::default(),
            rail_is_pinned: false,
            rail_scroll: Scroll::new(),
            rail_cursor: None,
            timeline_focus: cx.focus_handle(),
            rail_focus: cx.focus_handle(),
            dock_focus: cx.focus_handle(),
            reference_focus: cx.focus_handle(),
            reference_close_focus: cx.focus_handle(),
            reference_scroll: Scroll::new(),
            reference_return: None,
            reference_from_rail: false,
            regions: RegionRegistry::default(),
            #[cfg(test)]
            text_field: None,
            #[cfg(test)]
            modal_host: None,
            #[cfg(test)]
            unhandled_keys: Rc::default(),
            #[cfg(test)]
            sent: Rc::default(),
            cue_view: environment.preferences().cue_view,
            status: None,
            save_preferences: None,
            timer: None,
            program_wanted: Wanted::default(),
            preview_wanted: Wanted::default(),
            preparing_timer: None,
            auto_play_ticker: None,
            auto_play_bar_px: 0.0,
            last_program: Shown::default(),
            last_preview: Shown::default(),
        };
        view.engine_changed(cx);
        // The store ends with the view: release every decoded image and its
        // atlas tile. (A new snapshot gets a new view, never a swap.)
        cx.on_release(|view, cx| view.release_images(cx)).detach();
        view
    }

    /// Drop every decoded image and its atlas tile. Runs when the view is
    /// released (see `new`); the window is gone or closing then, so no window
    /// is passed and GPUI drops the tile from every window that remains.
    fn release_images(&mut self, cx: &mut gpui::App) {
        self.frames.release(None, cx);
    }

    /// Where a Grid/List change is saved. Without one the choice lasts for
    /// the session.
    /// Report every render's cost to `probe` (measurement labs only).
    pub fn set_render_probe(&mut self, probe: RenderProbe) {
        self.render_probe = Some(probe);
    }

    pub fn set_save_preferences(&mut self, save: SavePreferences) {
        self.save_preferences = Some(save);
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn frames(&self) -> &PresenterFrames {
        &self.frames
    }

    pub fn cue_view(&self) -> CueView {
        self.cue_view
    }

    pub fn rail_choice(&self) -> RailChoice {
        self.rail_choice
    }

    /// The last refusal, in words, until the next accepted command.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// True while a timer task is waiting for an Engine deadline.
    pub fn timer_pending(&self) -> bool {
        self.timer.is_some()
    }

    /// Send a command through the dispatch seam. A refusal is shown as a
    /// polite status line and changes nothing else.
    pub fn command(
        &mut self,
        command: Command,
        cx: &mut Context<Self>,
    ) -> Result<Outcome, Refusal> {
        let now = (self.clock)();
        #[cfg(test)]
        self.sent.borrow_mut().push(command);
        let result = dispatch(&mut self.engine, command, now);
        match result {
            Ok(_) => {
                self.status = None;
                self.engine_changed(cx);
            }
            Err(refusal) => {
                self.status = Some(refusal_text(refusal).into());
                cx.notify();
            }
        }
        result
    }

    /// Where focus is, for key policy. The reference and the rail are checked
    /// first (they sit above everything else, the reference above the rail),
    /// then the Cues, then the Output Dock.
    fn focus_context(&self, window: &Window, cx: &gpui::App) -> FocusContext {
        let declares = |names: &[&str]| {
            window
                .context_stack()
                .iter()
                .any(|c| names.iter().any(|name| c.contains(name)))
        };
        if declares(registry::MODAL_CONTEXTS) {
            FocusContext::Modal
        } else if declares(registry::TEXT_ENTRY_CONTEXTS) {
            FocusContext::TextEntry
        } else if self.reference_focus.contains_focused(window, cx) {
            FocusContext::ShortcutReference
        } else if self.rail_focus.contains_focused(window, cx) {
            FocusContext::Rail
        } else if self.timeline_focus.contains_focused(window, cx) {
            FocusContext::Timeline
        } else if self.dock_focus.contains_focused(window, cx) {
            FocusContext::Dock
        } else {
            FocusContext::Elsewhere
        }
    }

    /// A registered key was pressed. The policy is checked again here (the
    /// binding predicate already kept text entry and modals out), and a key
    /// the policy refuses reaches nothing. An allowed key takes the same path
    /// as the buttons, so refusal text and redraws behave identically.
    fn press(&mut self, id: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        let entry = registry::entry_for(id);
        let context = self.focus_context(window, cx);
        // In the rail the navigation keys move its cursor, never the Cues.
        // Left and Right step like Up and Down, as they do in the Cue grid.
        if context == FocusContext::Rail && self.move_rail_cursor(id, cx) {
            return;
        }
        if !registry::allows(entry.scope, context) {
            // The Engine gets nothing, but the focused control still hears the
            // key (list arrows, a button's Space).
            cx.propagate();
            return;
        }
        match registry::command_for(id) {
            Some(command) => {
                self.command(command, cx).ok();
            }
            // `O` never changes the pin: a pinned rail only takes focus.
            None if id == CommandId::OpenOrdo => self.focus_ordo(window, cx),
            None => {}
        }
    }

    /// `O`: show the rail and focus it. A pinned rail is already on screen, so
    /// it only takes focus; a collapsed one opens as the overlay. The pin
    /// choice, Preview, Program and scroll are untouched either way.
    fn focus_ordo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rail_is_pinned {
            self.rail_focus.focus(window, cx);
        } else {
            self.open_rail(window, cx);
        }
    }

    /// Close the topmost temporary surface and return focus to the Cue
    /// timeline; false when nothing is open. The scrim press and `Esc` both
    /// come here.
    fn close_topmost(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match self.surfaces.topmost() {
            Some(Surface::RailOverlay) => self.close_rail(window, cx),
            Some(Surface::ShortcutReference) => self.close_reference(window, cx),
            None => return false,
        }
        true
    }

    /// Whether the Keyboard shortcuts reference is open.
    pub fn reference_open(&self) -> bool {
        self.surfaces.contains(Surface::ShortcutReference)
    }

    /// Open the reference (an entry point was pressed, or `Ctrl+/`) and move
    /// focus into it, remembering where focus was. Opening it again only
    /// refocuses it. Nothing the Engine owns changes.
    pub fn open_reference(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.reference_open() {
            self.reference_return = window.focused(cx).map(|focus| focus.downgrade());
            self.reference_from_rail = self.rail_focus.contains_focused(window, cx);
            self.surfaces.open(Surface::ShortcutReference);
        }
        self.reference_close_focus.focus(window, cx);
        cx.notify();
    }

    /// Close the reference and give focus back to where it was, or to the
    /// Cues when that is gone (a rail overlay that closed meanwhile).
    pub fn close_reference(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.surfaces.close(Surface::ShortcutReference) {
            return;
        }
        let rail_shown = self.rail_is_pinned || self.rail_open();
        let from_rail = std::mem::take(&mut self.reference_from_rail);
        let back = self
            .reference_return
            .take()
            .and_then(|focus| focus.upgrade())
            .filter(|focus| *focus != self.reference_focus)
            .filter(|_| rail_shown || !from_rail)
            .unwrap_or_else(|| self.timeline_focus.clone());
        back.focus(window, cx);
        cx.notify();
    }

    /// `Ctrl+/`. Chorded, so it is live in a text field, but a true modal
    /// owns every key and leaves it alone.
    fn toggle_reference(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_context(window, cx) == FocusContext::Modal {
            cx.propagate();
        } else if self.reference_open() {
            self.close_reference(window, cx);
        } else {
            self.open_reference(window, cx);
        }
    }

    /// `F6` or `Shift+F6` from inside an open overlay leaves it for another
    /// region, so the overlay closes (no keys act under a scrim). Focus is
    /// not moved here: the region cycle that follows chooses where it goes.
    ///
    /// From the Keyboard shortcuts panel the panel closes the same way and
    /// gives focus back to where it was first, so the cycle moves on from
    /// there (the panel is not a region of its own).
    fn leave_overlay_by_region_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.reference_focus.contains_focused(window, cx) {
            self.close_reference(window, cx);
        }
        if self.rail_focus.contains_focused(window, cx) && self.surfaces.close(Surface::RailOverlay)
        {
            cx.notify();
        }
    }

    /// `Esc`: close the topmost temporary surface and return focus to the Cue
    /// timeline. With nothing open it still brings focus back from the pinned
    /// rail, the Output controls or any other control, and does nothing from
    /// the Cues. It never reaches the Engine and never collapses a pinned
    /// rail. Returns whether the key was used.
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.close_topmost(window, cx) {
            return true;
        }
        // The Cues are one focus target (tiles are not focusable), so "the
        // active Cue" is the timeline that holds it.
        match self.focus_context(window, cx) {
            FocusContext::Rail | FocusContext::Dock | FocusContext::Elsewhere => {
                self.timeline_focus.focus(window, cx);
                true
            }
            _ => false,
        }
    }

    /// Give focus to the Cues, where the keys are live. The owner calls this
    /// when the Presenter opens.
    pub fn focus_timeline(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.timeline_focus.focus(window, cx);
    }

    /// Let the Engine's clock catch up (a fade ending, an auto-play step).
    /// The timer calls this; a test may call it directly after moving its
    /// clock.
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        let now = (self.clock)();
        self.engine.tick(now);
        self.engine_changed(cx);
    }

    /// Change Interface scale or appearance. View state only: the Engine is
    /// not touched and no frame is recomposed.
    pub fn set_environment(&mut self, environment: UiEnvironment, cx: &mut Context<Self>) {
        self.environment = environment;
        cx.notify();
    }

    /// Choose Grid or List, and save it through the owner's save path.
    pub fn set_cue_view(&mut self, view: CueView, cx: &mut Context<Self>) {
        if self.cue_view == view {
            return;
        }
        self.cue_view = view;
        let preferences = UiPreferences {
            cue_view: view,
            ..self.environment.preferences()
        };
        self.environment
            .apply(EnvironmentChange::SetPreferences(preferences));
        if let Some(save) = &self.save_preferences {
            save(preferences);
        }
        self.request_visible(cx);
        cx.notify();
    }

    /// Bring everything that follows the Engine in line with it: the frame
    /// store, the deadline timer and the screen. Runs after the Engine
    /// changed, never from `render`.
    fn engine_changed(&mut self, cx: &mut Context<Self>) {
        let now = (self.clock)();
        let program = self.engine.program_view(now).shown;
        let preview = self.engine.preview_view(now).shown;
        let preload = self.preload_frames();
        self.frames
            .sync(program, preview, preload, |view| &mut view.frames, cx);
        let pending = |frames: &PresenterFrames, id: Option<FrameId>| {
            id.is_some_and(|id| matches!(frames.peek_full(id), FrameView::Pending))
        };
        let program_pending = pending(&self.frames, program);
        let preview_pending = pending(&self.frames, preview);
        self.program_wanted.follow(program, now, program_pending);
        self.preview_wanted.follow(preview, now, preview_pending);
        self.request_visible(cx);
        self.reschedule(cx);
        self.reschedule_preparing(cx);
        cx.notify();
    }

    /// The groups the timeline shows now, from where it was last laid out:
    /// the first and last group touching its viewport. Before the first
    /// layout this is the first group.
    fn visible_groups(&self) -> (usize, usize) {
        let handle = self.timeline_scroll.handle();
        let last = self.engine.snapshot().item_count().saturating_sub(1);
        (handle.top_item().min(last), handle.bottom_item().min(last))
    }

    /// Ask the frame store for the thumbnails the timeline wants: the Project
    /// frames of the groups on screen, then of their neighbours, nearest
    /// first. The store takes the first `MAX_VISIBLE` distinct ones, never
    /// pins them and keeps the cache within `FRAME_CAPACITY`. Grid tiles and
    /// List rows show the same frames (List draws them small), so the set does
    /// not depend on the mode. Called when something that changes the
    /// set changed (the Engine, Grid/List, a fold, a jump) and from the
    /// one-shot hop `render` schedules after a scroll or resize; never from
    /// `render` itself.
    fn request_visible(&mut self, cx: &mut Context<Self>) {
        self.visible_range = self.visible_groups();
        let ids = self.visible_frames(self.visible_range);
        self.frames
            .request_visible(ids, |view| &mut view.frames, cx);
    }

    /// Frames for `(first, last)` and, in Grid, each step outward, skipping
    /// folded groups and Cues that cannot be shown. List rows are short, so
    /// the groups on screen are already many and their small thumbnails are
    /// all it asks for; the hop after a scroll asks for the next ones.
    fn visible_frames(&self, (first, last): (usize, usize)) -> Vec<FrameId> {
        let snapshot = self.engine.snapshot();
        let items: Vec<_> = snapshot
            .sections()
            .iter()
            .flat_map(|section| section.items())
            .collect();
        let mut order: Vec<usize> = (first..=last).collect();
        let reach = match self.cue_view {
            CueView::Grid => items.len(),
            CueView::List => 1,
        };
        for step in 1..reach {
            order.extend((last + step < items.len()).then_some(last + step));
            order.extend(first.checked_sub(step));
        }
        let mut frames = Vec::new();
        for group in order {
            if self.collapsed.contains(&group) {
                continue;
            }
            let (Some(item), Some(start)) = (items.get(group), snapshot.item_start(group)) else {
                continue;
            };
            for (at, cue) in item.cues().iter().enumerate() {
                if let CueBehavior::Project { frame, .. } = cue.behavior()
                    && matches!(
                        self.engine.cue_availability(start + at),
                        Some(Availability::Presentable)
                    )
                    && !frames.contains(frame)
                {
                    frames.push(*frame);
                }
            }
            if frames.len() >= rendering::MAX_VISIBLE {
                break;
            }
        }
        frames
    }

    /// Project frames of the Cues around Preview, nearest first, Preview's
    /// own excluded.
    ///
    /// The scheduler bounds this set to `2 * PRELOAD_RADIUS` keys. The
    /// timeline's thumbnails are a separate, lower-priority set
    /// (`request_visible`), so together with the two pins they stay within
    /// `FRAME_CAPACITY`.
    fn preload_frames(&self) -> Vec<FrameId> {
        let snapshot = self.engine.snapshot();
        let preview = self.engine.state().preview();
        let mut frames = Vec::new();
        for distance in 1..=PRELOAD_RADIUS {
            for flat in [Some(preview + distance), preview.checked_sub(distance)]
                .into_iter()
                .flatten()
            {
                if let Some(CueBehavior::Project { frame, .. }) =
                    snapshot.cue(flat).map(|cue| cue.behavior())
                    && matches!(
                        self.engine.cue_availability(flat),
                        Some(Availability::Presentable)
                    )
                {
                    frames.push(*frame);
                }
            }
        }
        frames
    }

    /// Keep one sleeping task aimed at the Engine's next deadline.
    fn reschedule(&mut self, cx: &mut Context<Self>) {
        self.tick_auto_play_indicator(cx);
        let Some(deadline) = self.engine.next_deadline() else {
            self.timer = None;
            return;
        };
        if self
            .timer
            .as_ref()
            .is_some_and(|(waiting_for, _)| *waiting_for == deadline)
        {
            return;
        }
        // A deadline at or before now is legitimate: an auto-play step that is
        // already overdue. The wait is then zero, the timer fires, and the tick
        // restarts the run's clock (or settles the fade), so the next deadline
        // lies later and this cannot spin. Nothing is asserted because the
        // Engine may rightly report an overdue step.
        let wait = Duration::from_millis(deadline.0.saturating_sub((self.clock)().0));
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |view, cx| view.timer_fired(cx)).ok();
        });
        self.timer = Some((deadline, task));
    }

    /// While an auto-play run is going, redraw Program each time its AUTO
    /// countdown or progress bar would change (`auto_play_redraw_ms`), so the
    /// bar moves a pixel at a time; no task otherwise, so an idle Presenter
    /// draws nothing.
    fn tick_auto_play_indicator(&mut self, cx: &mut Context<Self>) {
        if self.engine.state().auto_play().is_none() {
            self.auto_play_ticker = None;
            return;
        }
        if self.auto_play_ticker.is_some() {
            return;
        }
        self.auto_play_ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                let Ok(Some(wait)) = this.update(cx, |view, _| view.auto_play_redraw_wait()) else {
                    break;
                };
                cx.background_executor()
                    .timer(Duration::from_millis(wait))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
    }

    /// The wait before the auto-play indicator next changes, or `None` when
    /// the run has stopped or its Cue has no duration to count down.
    fn auto_play_redraw_wait(&self) -> Option<u64> {
        let run = self.engine.state().auto_play()?;
        let duration = run.duration_ms?;
        let elapsed = (self.clock)().0.saturating_sub(run.cue_started.0);
        auto_play_redraw_ms(
            Some(duration.saturating_sub(elapsed)),
            duration,
            self.auto_play_bar_px,
        )
    }

    /// True when `wanted` has been pending for at least the threshold.
    fn is_preparing(&self, wanted: Wanted, now: Millis) -> bool {
        wanted.id.is_some_and(|id| {
            now.0.saturating_sub(wanted.since.0) >= PREPARING_THRESHOLD_MS
                && matches!(self.frames.peek_full(id), FrameView::Pending)
        })
    }

    /// Keep one sleeping task aimed at the earliest moment a still-pending
    /// wanted frame crosses the threshold, so the badge appears without any
    /// other input. A frame that is ready, failed or already past the
    /// threshold needs no timer.
    fn reschedule_preparing(&mut self, cx: &mut Context<Self>) {
        let now = (self.clock)().0;
        let due = [self.program_wanted, self.preview_wanted]
            .into_iter()
            .filter(|wanted| {
                wanted
                    .id
                    .is_some_and(|id| matches!(self.frames.peek_full(id), FrameView::Pending))
            })
            .map(|wanted| wanted.since.0 + PREPARING_THRESHOLD_MS)
            .filter(|due| *due > now)
            .min();
        let Some(due) = due else {
            self.preparing_timer = None;
            return;
        };
        if self
            .preparing_timer
            .as_ref()
            .is_some_and(|(waiting_for, _)| *waiting_for == due)
        {
            return;
        }
        let wait = Duration::from_millis(due - now);
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |view, cx| view.preparing_due(cx)).ok();
        });
        self.preparing_timer = Some((due, task));
    }

    fn preparing_due(&mut self, cx: &mut Context<Self>) {
        // The running task ends as soon as this returns, so it is detached
        // rather than dropped from inside itself.
        if let Some((_, finished)) = self.preparing_timer.take() {
            finished.detach();
        }
        // Another monitor may cross the threshold later.
        self.reschedule_preparing(cx);
        cx.notify();
    }

    fn timer_fired(&mut self, cx: &mut Context<Self>) {
        // This task is the one running; it ends as soon as this returns, so
        // it is detached rather than dropped from inside itself.
        if let Some((_, finished)) = self.timer.take() {
            finished.detach();
        }
        self.tick(cx);
    }

    /// Report composition failures to the Engine, which then marks those
    /// Cues unavailable. The frames follow in a one-shot hop, because the
    /// store must not be synced from `render`.
    fn absorb_failures(&mut self, cx: &mut Context<Self>) {
        let failures = self.frames.take_failures();
        if failures.is_empty() {
            return;
        }
        let now = (self.clock)();
        // Until the deferred `engine_changed` below runs, this frame still
        // draws from the pre-failure view of the Engine: one frame of stale
        // content, after which the store is synced to the new state.
        for (frame, reason) in failures {
            self.engine.mark_frame_unavailable(frame, reason, now);
        }
        cx.spawn(async move |this, cx| {
            this.update(cx, |view, cx| view.engine_changed(cx)).ok();
        })
        .detach();
    }

    /// What one monitor draws now, keeping its last frame while the next one
    /// composes.
    fn resolve(frames: &mut PresenterFrames, last: &mut Shown, view: ProgramView) -> Shown {
        let Some(id) = view.shown else {
            *last = Shown::default();
            return Shown {
                opacity: view.opacity,
                ..Shown::default()
            };
        };
        // The monitors and the output only ever show the full image.
        match frames.get_full(id) {
            FrameView::Ready { frame, image } => {
                *last = Shown {
                    frame: Some(frame),
                    image,
                    opacity: 1.0,
                };
            }
            FrameView::Pending => {}
            FrameView::Unavailable(_) => *last = Shown::default(),
        }
        Shown {
            opacity: view.opacity,
            ..last.clone()
        }
    }

    fn controls(&self, now: Millis) -> ControlsModel {
        let live = self.engine.state().live();
        let allowed = |command| self.engine.check(command, now).is_ok();
        ControlsModel {
            live,
            status: self
                .status
                .clone()
                .unwrap_or_else(|| self.standing_status(now)),
            can_freeze: allowed(Command::ToggleFreeze),
            can_blackout: allowed(Command::ToggleBlackout),
            can_primary: match dock::primary_for(live) {
                Primary::Action { command, .. } => allowed(command),
                Primary::Info(_) => false,
            },
            auto_play: self.auto_play_model(now),
        }
    }

    /// The Auto-play control's words and menu. Each entry asks the Engine
    /// whether it would take the command; a refusal becomes the entry's
    /// Unavailable sentence, so the operator reads why before pressing it.
    fn auto_play_model(&self, now: Millis) -> AutoPlayModel {
        let run = self.engine.state().auto_play();
        let availability = |command| match self.engine.check(command, now) {
            Ok(_) => ui::primitives::Availability::Available,
            Err(refusal) => ui::primitives::Availability::unavailable(refusal_text(refusal)),
        };
        use presentation::state::AutoPlayScope;
        let scope = run.map(|run| run.scope);
        let stop = |label| AutoPlayEntry {
            label,
            command: Command::StopAutoPlay,
            availability: ui::primitives::Availability::Available,
        };
        // One Stop entry, never two. An item run stops with the same command
        // that starts it, so it replaces Start; a section run has no Start
        // (Start would stop it), so its Stop follows the looping entry.
        let mut entries = Vec::new();
        match scope {
            Some(AutoPlayScope::Item(_)) => entries.push(AutoPlayEntry {
                label: "Stop auto-play for this item",
                command: Command::ToggleAutoPlay,
                availability: ui::primitives::Availability::Available,
            }),
            Some(AutoPlayScope::Section(_)) => {}
            None => entries.push(AutoPlayEntry {
                label: "Start auto-play for this item",
                command: Command::ToggleAutoPlay,
                availability: availability(Command::ToggleAutoPlay),
            }),
        }
        let looping = matches!(scope, Some(AutoPlayScope::Section(_)))
            && self
                .engine
                .check(Command::LoopSection, now)
                .is_ok_and(|outcome| outcome == Outcome::Unchanged);
        entries.push(AutoPlayEntry {
            label: "Loop this section",
            command: Command::LoopSection,
            availability: if looping {
                ui::primitives::Availability::unavailable("This section is already looping")
            } else {
                availability(Command::LoopSection)
            },
        });
        match scope {
            Some(AutoPlayScope::Item(_)) => {}
            Some(AutoPlayScope::Section(_)) => entries.push(stop("Stop auto-play")),
            None => entries.push(AutoPlayEntry {
                availability: ui::primitives::Availability::unavailable("Auto-play is not running"),
                ..stop("Stop auto-play")
            }),
        }
        AutoPlayModel {
            label: dock::auto_play_label(scope),
            entries,
        }
    }

    /// The status line when no refusal is showing. A slow frame is told only
    /// while output is working normally: a disabled or disconnected output
    /// is the more important thing to say.
    fn standing_status(&self, now: Millis) -> SharedString {
        let idle = idle_status(&self.engine);
        if matches!(
            self.engine.state().live(),
            LiveState::OutputDisabled | LiveState::Disconnected { .. }
        ) {
            return idle.into();
        }
        if self.is_preparing(self.program_wanted, now) {
            "Program is preparing the next slide".into()
        } else if self.is_preparing(self.preview_wanted, now) {
            "Preview is preparing".into()
        } else {
            idle.into()
        }
    }

    fn preview_model(&self, shown: Shown, preparing: bool) -> PreviewModel {
        let state = self.engine.state();
        let unavailable = match self.engine.preview_availability() {
            Availability::Presentable => None,
            Availability::Unavailable(UnavailableSource::Snapshot { reason }) => {
                Some(SharedString::from(reason.to_owned()))
            }
            Availability::Unavailable(UnavailableSource::Runtime(_)) => {
                Some(SharedString::from("the frame could not be prepared"))
            }
        };
        PreviewModel {
            item_title: self
                .engine
                .snapshot()
                .item_of(state.preview())
                .map(|item| item.title().to_owned())
                .unwrap_or_default()
                .into(),
            private_context: self
                .engine
                .preview_context()
                .map(|context| context.to_owned().into()),
            unavailable,
            shown,
            preparing,
        }
    }

    /// Choose where the rail lives (Decision 237). Session only. Pinning it
    /// closes the overlay, because the rail is then already on screen.
    pub fn set_rail_choice(&mut self, choice: RailChoice, cx: &mut Context<Self>) {
        if self.rail_choice == choice {
            return;
        }
        self.rail_choice = choice;
        if choice == RailChoice::Pinned {
            self.surfaces.close(Surface::RailOverlay);
        }
        cx.notify();
    }

    /// Whether the rail overlay is open.
    pub fn rail_open(&self) -> bool {
        self.surfaces.contains(Surface::RailOverlay)
    }

    /// Open the rail overlay (the ORDO tab was pressed) and move focus into
    /// it. Hover never calls this.
    pub fn open_rail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rail_open() {
            return;
        }
        self.surfaces.open(Surface::RailOverlay);
        self.rail_generation += 1;
        self.rail_focus.focus(window, cx);
        cx.notify();
    }

    /// Close the rail overlay and give focus back to the timeline, which
    /// holds the Cues. The timeline has one focus handle for the whole list
    /// (a Cue is not a focus target of its own until T1.5 maps the keyboard),
    /// so "the active Cue" is the timeline.
    pub fn close_rail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.surfaces.close(Surface::RailOverlay) {
            return;
        }
        self.timeline_focus.focus(window, cx);
        cx.notify();
    }

    /// Bring a group to the top of the timeline. The Engine, the Preview and
    /// every other group are unchanged, so the rail never filters.
    pub fn jump_to_group(&mut self, group: usize, cx: &mut Context<Self>) {
        self.timeline_scroll.handle().scroll_to_top_of_item(group);
        // The scroll is applied at the next layout, so ask for the target
        // group's frames now; `render` corrects the set once it has moved.
        self.visible_range = (group, group);
        self.pending_jump = Some(group);
        self.jump_renders = 0;
        let ids = self.visible_frames((group, group));
        self.frames
            .request_visible(ids, |view| &mut view.frames, cx);
        cx.notify();
    }

    /// Whether a group is folded.
    pub fn group_collapsed(&self, group: usize) -> bool {
        self.collapsed.contains(&group)
    }

    /// Fold or unfold a group. View state only.
    pub fn toggle_group(&mut self, group: usize, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&group) {
            self.collapsed.insert(group);
        }
        self.request_visible(cx);
        cx.notify();
    }

    /// The Mass item that owns the Preview Cue.
    fn preview_item(&self) -> Option<usize> {
        self.engine
            .snapshot()
            .positions()
            .get(self.engine.state().preview())
            .map(|position| position.item_flat)
    }

    /// Move the rail's cursor for a navigation key; false for any other key.
    /// The cursor starts on the item that owns Preview and stops at the ends.
    fn move_rail_cursor(&mut self, id: CommandId, cx: &mut Context<Self>) -> bool {
        let count: usize = self
            .engine
            .snapshot()
            .sections()
            .iter()
            .map(|section| section.items().len())
            .sum();
        let Some(last) = count.checked_sub(1) else {
            return false;
        };
        let from = self
            .rail_cursor
            .or_else(|| self.preview_item())
            .unwrap_or(0)
            .min(last);
        let to = match id {
            CommandId::Previous => from.saturating_sub(1),
            CommandId::Next => (from + 1).min(last),
            CommandId::First => 0,
            CommandId::Last => last,
            _ => return false,
        };
        self.rail_cursor = Some(to);
        self.reveal_rail_item(to);
        cx.notify();
        true
    }

    /// Scroll the rail so `item` is in sight. The rail's rows are each
    /// section's heading followed by its items, so the row of an item is its
    /// index plus the headings up to and including its section.
    fn reveal_rail_item(&self, item: usize) {
        let mut row = item;
        let mut first = 0;
        for section in self.engine.snapshot().sections() {
            if first > item {
                break;
            }
            row += 1;
            first += section.items().len();
        }
        self.rail_scroll.handle().scroll_to_item(row);
    }

    /// `Enter` in the rail goes to the cursor's item from the keyboard: it
    /// closes an overlay rail, moves Preview to the item's first Cue and
    /// gives the keys back to the Cues, so the operator carries on there
    /// without the mouse. A Live output is frozen first, so Program keeps
    /// showing what it showed instead of following Preview across the Mass;
    /// a Frozen, blacked-out or disabled output is left as it is. Enter on
    /// the item that already holds Preview only returns focus. A click on an
    /// item stays a plain jump (`jump_to_group`).
    fn activate_rail_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(item) = self.rail_cursor else {
            return false;
        };
        let first_cue = self
            .engine
            .snapshot()
            .positions()
            .iter()
            .position(|position| position.item_flat == item);
        match first_cue {
            Some(cue) if self.preview_item() != Some(item) => {
                if self.engine.state().live() == LiveState::Live {
                    // Freeze is refused only off Live, ruled out above.
                    self.command(Command::ToggleFreeze, cx).ok();
                }
                // Preview is followed into view like any navigation.
                self.command(Command::GoTo(cue), cx).ok();
            }
            _ => self.jump_to_group(item, cx),
        }
        self.close_rail(window, cx);
        self.timeline_focus.focus(window, cx);
        cx.notify();
        true
    }

    /// The Mass-item sections of the rail, with the item that owns Preview
    /// marked.
    fn rail_model(&self, pin: PinControl) -> RailModel {
        let snapshot = self.engine.snapshot();
        let current = self.preview_item();
        let mut index = 0;
        let sections = snapshot
            .sections()
            .iter()
            .map(|section| RailSection {
                title: section.title().to_owned().into(),
                items: section
                    .items()
                    .iter()
                    .map(|item| {
                        let row = RailItem {
                            index,
                            number: (index + 1).to_string().into(),
                            title: item.title().to_owned().into(),
                            current: current == Some(index),
                            cursor: self.rail_cursor == Some(index),
                        };
                        index += 1;
                        row
                    })
                    .collect(),
            })
            .collect();
        RailModel { sections, pin }
    }

    /// One group per Mass item, with each Cue's frame, marks and caption. It
    /// reads the frame store with `peek`, so a paint never changes what the
    /// cache would evict, and composing follows state changes
    /// (`engine_changed`, `request_visible`), never a paint.
    fn timeline_model(&self, layout: &PresenterLayout, follow_preview: bool) -> TimelineModel {
        let engine = &self.engine;
        let frames = &self.frames;
        let snapshot = engine.snapshot();
        let state = engine.state();
        let preview = state.preview();
        let total = snapshot.cue_count();
        let mut groups = Vec::new();
        let mut flat = 0;
        for section in snapshot.sections() {
            for item in section.items() {
                let index = groups.len();
                let mut cues = Vec::with_capacity(item.cues().len());
                for (at, cue) in item.cues().iter().enumerate() {
                    let (kind, mut frame, mut image) = match cue.behavior() {
                        CueBehavior::Project { frame: id, .. } => match frames.peek(*id) {
                            FrameView::Ready { frame, image } => {
                                (CueKind::Slide, Some(frame), image)
                            }
                            FrameView::Pending | FrameView::Unavailable(_) => {
                                (CueKind::Slide, None, None)
                            }
                        },
                        CueBehavior::OperatorTrigger => (CueKind::Trigger, None, None),
                        CueBehavior::Hold => (CueKind::Hold, None, None),
                        CueBehavior::Wait => (CueKind::Wait, None, None),
                        CueBehavior::Unavailable { .. } => (CueKind::Unavailable, None, None),
                    };
                    let unavailable = match engine.cue_availability(flat) {
                        Some(Availability::Unavailable(UnavailableSource::Snapshot { reason })) => {
                            Some(SharedString::from(reason.to_owned()))
                        }
                        Some(Availability::Unavailable(UnavailableSource::Runtime(_))) => {
                            Some(SharedString::from("the frame could not be prepared"))
                        }
                        _ => None,
                    };
                    if unavailable.is_some() {
                        // An unavailable Cue is black, never a stale frame.
                        (frame, image) = (None, None);
                    }
                    cues.push(CueModel {
                        flat,
                        number: format!("{}.{}", index + 1, at + 1).into(),
                        label: cue.label().to_owned().into(),
                        kind: if unavailable.is_some() {
                            CueKind::Unavailable
                        } else {
                            kind
                        },
                        frame,
                        image,
                        unavailable,
                        air: timeline::air(state, flat),
                        preview: flat == preview,
                        total,
                        item_title: item.title().to_owned().into(),
                        private_context: cue.private_context().map(|text| text.to_owned().into()),
                    });
                    flat += 1;
                }
                groups.push(GroupModel {
                    index,
                    number: index + 1,
                    section: section.title().to_owned().into(),
                    title: item.title().to_owned().into(),
                    collapsed: self.collapsed.contains(&index),
                    cues,
                });
            }
        }
        TimelineModel {
            groups,
            mode: self.cue_view,
            columns: layout.grid_columns as usize,
            cell_width: layout.cell_width,
            follow_preview,
            glide: self.glide.clone(),
            jump_to: self.pending_jump,
            #[cfg(test)]
            virtualise: !self.all_groups_real,
            #[cfg(not(test))]
            virtualise: true,
        }
    }
}

impl Render for PresenterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = Instant::now();
        let element = self.build_view(window, cx);
        if let Some(probe) = &self.render_probe {
            probe(RenderStat {
                render: started.elapsed(),
                composed: self.frames.composed_count(),
                jobs_in_flight: self.frames.jobs_in_flight(),
            });
        }
        element
    }
}

impl PresenterView {
    fn build_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        self.absorb_failures(cx);
        // A held frame may have become Ready since the last paint: drop a
        // timer that has nothing left to announce.
        self.reschedule_preparing(cx);
        let now = (self.clock)();
        let environment = self.environment;
        let scale = environment.preferences().scale;
        let palette = tokens::palette(environment.appearance());
        let size = window.viewport_size();
        let layout = presenter_layout(LayoutInput {
            width: f32::from(size.width),
            height: f32::from(size.height),
            scale,
            rail: self.rail_choice,
        });
        self.auto_play_bar_px = match layout.dock {
            DockLayout::Side { monitor_width, .. } | DockLayout::Band { monitor_width } => {
                monitor_width * window.scale_factor()
            }
        };

        // Follow Preview once per move: navigation scrolls the timeline, the
        // operator's own scrolling never is undone.
        let preview_now = self.engine.state().preview();
        let follow_preview = self.followed_preview != Some(preview_now);
        self.followed_preview = Some(preview_now);
        let timeline_model = self.timeline_model(&layout, follow_preview);
        let program_view = self.engine.program_view(now);
        let preview_view = self.engine.preview_view(now);
        if program_view.fading || preview_view.fading {
            window.request_animation_frame();
        }
        let program_shown = Self::resolve(&mut self.frames, &mut self.last_program, program_view);
        let preview_shown = Self::resolve(&mut self.frames, &mut self.last_preview, preview_view);

        let state = self.engine.state();
        let program = ProgramModel {
            live: state.live(),
            unavailable_black: state.unavailable_black(),
            shown: program_shown,
            preparing: self.is_preparing(self.program_wanted, now),
            auto_play: state.auto_play().map(|run| {
                let elapsed = now.0.saturating_sub(run.cue_started.0);
                dock::AutoPlayIndicator {
                    remaining_ms: run.duration_ms.map(|ms| ms.saturating_sub(elapsed)),
                    progress: run
                        .duration_ms
                        .filter(|ms| *ms > 0)
                        .map(|ms| (elapsed as f32 / ms as f32).min(1.0)),
                }
            }),
        };
        let preview =
            self.preview_model(preview_shown, self.is_preparing(self.preview_wanted, now));
        let controls = self.controls(now);

        let view = cx.entity().downgrade();
        let on_command: OnCommand = {
            let view = view.clone();
            Rc::new(move |command, _, cx| {
                view.update(cx, |view, cx| {
                    view.command(command, cx).ok();
                })
                .ok();
            })
        };
        // The primary action replaces itself (Enable Output becomes Take
        // Preview Live, which becomes the LIVE statement), so the button the
        // pointer just focused is gone. Focus returns to the Cues, as it does
        // when the rail overlay closes, so the keys keep reaching the
        // Presenter instead of an element that no longer exists.
        let on_primary: OnCommand = {
            let view = view.clone();
            Rc::new(move |command, window, cx| {
                view.update(cx, |view, cx| {
                    view.command(command, cx).ok();
                    view.timeline_focus.focus(window, cx);
                })
                .ok();
            })
        };
        // Grid|List is a view choice, not a place to work: after it the Cues
        // get focus back, so the arrows and `O` keep reaching them (a header
        // control is `Elsewhere`, where only F, B and Space act).
        let on_cue_view: OnCueView = {
            let view = view.clone();
            Rc::new(move |chosen, window, cx| {
                view.update(cx, |view, cx| {
                    view.set_cue_view(chosen, cx);
                    view.timeline_focus.focus(window, cx);
                })
                .ok();
            })
        };

        // The header and rail footer Keyboard shortcuts entries open the one
        // panel `Ctrl+/` toggles.
        let on_open_reference: rail::OnPress = {
            let view = view.clone();
            Rc::new(move |window, cx| {
                view.update(cx, |view, cx| view.open_reference(window, cx))
                    .ok();
            })
        };
        // The header entry is a place to click, not to work: closing the
        // panel returns focus to the Cues, as Grid/List does, not to the
        // header button where the arrows and `O` do nothing.
        let on_open_reference_from_header: rail::OnPress = {
            let view = view.clone();
            Rc::new(move |window, cx| {
                view.update(cx, |view, cx| {
                    if !view.reference_open() {
                        view.timeline_focus.focus(window, cx);
                    }
                    view.open_reference(window, cx);
                })
                .ok();
            })
        };
        // The rail is on screen already when pinned, so a stale open overlay
        // (the window grew, or the rail was pinned) is dropped, not kept for
        // the next unpin to resurrect.
        self.rail_is_pinned = matches!(layout.rail, RailLayout::Pinned { .. });
        if self.rail_is_pinned && self.surfaces.close(Surface::RailOverlay) {
            // The overlay held focus; the Cues get it back, as on any close.
            self.timeline_focus.focus(window, cx);
        }
        // The rail's cursor ring shows where the keys go, so it goes once
        // focus is anywhere outside the rail (by key, click or close). The
        // handle itself counts, because an overlay opened this frame is not
        // in the previous frame's tree yet.
        let rail_holds_focus = window.focused(cx).is_some_and(|focused| {
            focused == self.rail_focus || self.rail_focus.contains(&focused, window)
        });
        // Focus arriving by any route (`O`, F6, a click) starts the cursor on
        // the item that owns Preview.
        self.rail_cursor = if rail_holds_focus {
            self.rail_cursor.or_else(|| self.preview_item())
        } else {
            None
        };
        // A scroll or a resize moved the groups on screen: ask for their
        // thumbnails on the next turn, because the store is not synced from
        // `render`. The hop holds only a weak handle and ends at once.
        if let Some(group) = self.pending_jump {
            let handle = self.timeline_scroll.handle();
            self.jump_renders = self.jump_renders.saturating_add(1);
            let landed = handle
                .bounds_for_item(group)
                .is_some_and(|item| jump_landed(f32::from(item.origin.y), handle.bounds()));
            if landed || self.jump_renders >= JUMP_RENDERS {
                self.pending_jump = None;
            }
        }
        if self.visible_groups() != self.visible_range {
            self.visible_range = self.visible_groups();
            cx.spawn(async move |this, cx| {
                this.update(cx, |view, cx| view.request_visible(cx)).ok();
            })
            .detach();
        }
        let timeline_handlers = timeline::Handlers {
            go_to: {
                let view = view.clone();
                Rc::new(move |flat, _, cx| {
                    view.update(cx, |view, cx| {
                        // A pressed Cue is where the pointer is: the timeline
                        // stays still under it (only key navigation follows).
                        view.followed_preview = Some(flat);
                        view.command(Command::GoTo(flat), cx).ok();
                    })
                    .ok();
                })
            },
            toggle_group: {
                let view = view.clone();
                Rc::new(move |group, _, cx| {
                    view.update(cx, |view, cx| view.toggle_group(group, cx))
                        .ok();
                })
            },
        };
        let rail_handlers = rail::Handlers {
            jump: {
                let view = view.clone();
                Rc::new(move |group, window, cx| {
                    view.update(cx, |view, cx| {
                        view.jump_to_group(group, cx);
                        // An overlay has done its job once the group is
                        // where the operator asked for it.
                        view.close_rail(window, cx);
                    })
                    .ok();
                })
            },
            toggle_pin: {
                let view = view.clone();
                Rc::new(move |_, cx| {
                    view.update(cx, |view, cx| {
                        // Flip what was on screen: `Auto` can be either.
                        let choice = if view.rail_is_pinned {
                            RailChoice::Unpinned
                        } else {
                            RailChoice::Pinned
                        };
                        view.set_rail_choice(choice, cx);
                    })
                    .ok();
                })
            },
            close: {
                let view = view.clone();
                Rc::new(move |window, cx| {
                    view.update(cx, |view, cx| view.close_rail(window, cx)).ok();
                })
            },
            open_reference: on_open_reference.clone(),
        };
        let on_open_rail: rail::OnPress = {
            let view = view.clone();
            Rc::new(move |window, cx| {
                view.update(cx, |view, cx| view.open_rail(window, cx)).ok();
            })
        };

        // Every frame, so the slide state outlives the overlay being closed.
        let slide = rail::entrance(
            self.rail_generation,
            scale.apply(RAIL_PINNED_WIDTH),
            window,
            cx,
        );
        let (rail_element, rail_overlay) = match layout.rail {
            RailLayout::Pinned { width } => (
                rail::panel(
                    self.rail_model(PinControl::Unpin),
                    rail::Placement {
                        width,
                        overlay: false,
                        focus: Some(&self.rail_focus),
                    },
                    &self.rail_scroll,
                    &rail_handlers,
                    environment,
                    window,
                    cx,
                ),
                None,
            ),
            RailLayout::Tab { width } | RailLayout::OverlayOnly { tab_width: width } => {
                let pin = if matches!(layout.rail, RailLayout::Tab { .. }) {
                    PinControl::Pin
                } else {
                    PinControl::Hidden
                };
                let overlay = self.rail_open().then(|| {
                    let panel_width = scale.apply(RAIL_PINNED_WIDTH);
                    let panel = rail::panel(
                        self.rail_model(pin),
                        rail::Placement {
                            width: panel_width,
                            overlay: true,
                            focus: Some(&self.rail_focus),
                        },
                        &self.rail_scroll,
                        &rail_handlers,
                        environment,
                        window,
                        cx,
                    );
                    rail::overlay(
                        panel,
                        panel_width,
                        scale.apply(layout::HEADER_HEIGHT),
                        slide,
                        {
                            let view = view.clone();
                            Rc::new(move |window, cx| {
                                view.update(cx, |view, cx| view.close_topmost(window, cx))
                                    .ok();
                            })
                        },
                        environment,
                    )
                });
                (
                    rail::tab(width, on_open_rail, environment, window, cx),
                    overlay,
                )
            }
        };
        let timeline_element = timeline::timeline(
            timeline_model,
            &self.timeline_scroll,
            &self.timeline_focus,
            &timeline_handlers,
            environment,
            #[cfg(test)]
            self.text_field.as_ref(),
            window,
            cx,
        );

        let header = header::header(
            HeaderModel {
                title: self.snapshot.snapshot().title().to_owned().into(),
                health: DisplayHealth::of(state.live()),
                cue_view: self.cue_view,
                icon_only: layout.regime == Regime::Constrained,
                environment,
            },
            on_cue_view,
            on_open_reference_from_header,
        );

        let auto_play_height = match layout.dock {
            DockLayout::Side { .. } => layout::DOCK_AUTO_PLAY_HEIGHT,
            DockLayout::Band { .. } => layout::DOCK_STATUS_HEIGHT,
        };
        let auto_play = dock::auto_play_control(
            &controls.auto_play,
            &on_command,
            auto_play_height,
            environment,
            window,
            cx,
        );
        let body = match layout.dock {
            DockLayout::Side {
                width,
                monitor_width,
            } => {
                let dock = div()
                    .id("output-dock")
                    .debug_selector(|| "output-dock".into())
                    .flex_none()
                    .w(px(width))
                    .h_full()
                    .overflow_hidden()
                    .p(px(scale.apply(layout::dock_padding(layout.regime))))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(scale.apply(layout::dock_gap(layout.regime))))
                    .bg(rgb(palette.surface))
                    .child(dock::program_monitor(&program, monitor_width, environment))
                    .child(dock::preview_monitor(&preview, monitor_width, environment))
                    .child(dock::output_status(
                        controls.status.clone(),
                        controls.live,
                        environment,
                    ))
                    .child(dock::freeze_blackout(&controls, &on_command, environment))
                    .child(dock::primary_action(&controls, &on_primary, environment))
                    .child(auto_play)
                    .track_focus(&self.dock_focus)
                    .map(|dock| label_region(dock, OUTPUT_REGION));
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .flex_row()
                    .child(rail_element)
                    .child(timeline_element)
                    .child(dock)
                    .into_any_element()
            }
            DockLayout::Band { monitor_width } => {
                let band = div()
                    .id("monitor-band")
                    .debug_selector(|| "monitor-band".into())
                    .flex_none()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_start()
                    .justify_center()
                    .gap(px(scale.apply(BAND_GAP)))
                    .px(px(scale.apply(BAND_GUTTER)))
                    .pt(px(scale.apply(BAND_TOP_PADDING)))
                    .child(dock::program_monitor(&program, monitor_width, environment))
                    .child(dock::preview_monitor(&preview, monitor_width, environment));
                let controls_row = div()
                    .id("band-controls")
                    .flex_none()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(scale.apply(tokens::spacing::RELATED)))
                    .px(px(scale.apply(BAND_GUTTER)))
                    .pt(px(scale.apply(tokens::spacing::RELATED)))
                    .track_focus(&self.dock_focus)
                    .map(|controls| label_region(controls, OUTPUT_REGION))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(scale.apply(tokens::spacing::RELATED)))
                            .child(div().flex_1().min_w_0().child(dock::output_status(
                                controls.status.clone(),
                                controls.live,
                                environment,
                            )))
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(scale.apply(layout::BAND_AUTO_PLAY_WIDTH)))
                                    .child(auto_play),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(scale.apply(tokens::spacing::RELATED)))
                            .child(div().flex_1().min_w_0().child(dock::freeze_blackout(
                                &controls,
                                &on_command,
                                environment,
                            )))
                            .child(div().flex_1().min_w_0().child(dock::primary_action(
                                &controls,
                                &on_primary,
                                environment,
                            ))),
                    );
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .flex_row()
                    .child(rail_element)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(band)
                            .child(controls_row)
                            .child(timeline_element),
                    )
                    .into_any_element()
            }
        };

        // F6 visits the regions in reading order. A rail that is neither
        // pinned nor open is not on screen, so it is not a region. The
        // regions reuse the handles that already decide key policy.
        if self.rail_is_pinned || self.rail_open() {
            self.regions
                .adopt(RAIL_REGION, &self.rail_focus, window, cx);
        }
        self.regions
            .adopt(TIMELINE_REGION, &self.timeline_focus, window, cx);
        self.regions
            .adopt(OUTPUT_REGION, &self.dock_focus, window, cx);
        let reference = self.reference_open().then(|| {
            shortcut_reference::panel(
                &shortcut_reference::model(registry::Group::Presenter),
                shortcut_reference::Placement {
                    focus: &self.reference_focus,
                    close_focus: &self.reference_close_focus,
                    scroll: &self.reference_scroll,
                    top: scale.apply(layout::HEADER_HEIGHT),
                },
                {
                    let view = view.clone();
                    Rc::new(move |window, cx| {
                        view.update(cx, |view, cx| view.close_reference(window, cx))
                            .ok();
                    })
                },
                environment,
                window,
                cx,
            )
        });
        let root = self
            .regions
            .root(window, cx)
            .key_context(registry::PRESENTER_CONTEXT)
            .relative()
            .size_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(palette.background))
            .text_color(rgb(palette.text))
            .font_family(tokens::typography::OPERATOR_FAMILY)
            .text_size(px(scale.apply(tokens::typography::BODY.size)))
            .child(header)
            .child(body)
            .children(rail_overlay)
            .children(reference)
            .on_action(
                cx.listener(|this, _: &registry::ToggleReference, window, cx| {
                    this.toggle_reference(window, cx)
                }),
            )
            .capture_action({
                let view = view.clone();
                move |_: &NextRegion, window, cx| {
                    view.update(cx, |view, cx| view.leave_overlay_by_region_key(window, cx))
                        .ok();
                }
            })
            .capture_action({
                let view = view.clone();
                move |_: &PreviousRegion, window, cx| {
                    view.update(cx, |view, cx| view.leave_overlay_by_region_key(window, cx))
                        .ok();
                }
            })
            .on_key_down({
                let view = view.clone();
                move |event, window, cx| {
                    if event.keystroke.key == "enter"
                        && !event.keystroke.modifiers.modified()
                        && view
                            .update(cx, |view, cx| {
                                view.focus_context(window, cx) == FocusContext::Rail
                                    && view.activate_rail_cursor(window, cx)
                            })
                            .unwrap_or(false)
                    {
                        cx.stop_propagation();
                        return;
                    }
                    if event.keystroke.key == "escape"
                        && !event.keystroke.modifiers.modified()
                        && view
                            .update(cx, |view, cx| {
                                let typing = matches!(
                                    view.focus_context(window, cx),
                                    FocusContext::TextEntry | FocusContext::Modal
                                );
                                !typing && view.escape(window, cx)
                            })
                            .unwrap_or(false)
                    {
                        cx.stop_propagation();
                    }
                }
            });
        #[cfg(test)]
        let root = {
            let layer = self
                .modal_host
                .clone()
                .map(|host| host.layer(environment, window, cx));
            root.children(layer)
        };
        #[cfg(test)]
        let root = root.on_key_down({
            let keys = self.unhandled_keys.clone();
            move |event, _, _| keys.borrow_mut().push(event.keystroke.key.clone())
        });
        macro_rules! listeners {
            ($($name:ident),*) => {{
                let root = root;
                $(
                    let root = root.on_action(cx.listener(|this, _: &registry::$name, w, cx| {
                        this.press(CommandId::$name, w, cx)
                    }));
                )*
                root
            }};
        }
        registry::for_each_bound_command!(listeners)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[path = "../e2e.rs"]
    mod e2e;
    use crate::presenter::frames::test_support::fixture_snapshot;
    use domain::presentation::CueId;
    use gpui::{Bounds, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, size};
    use std::cell::{Cell, RefCell};
    use ui::environment::{InterfaceScale, ResolvedAppearance};

    /// The three qualified sizes, then a short window.
    const SIZES: [(f32, f32, InterfaceScale); 3] = [
        (1920.0, 1080.0, InterfaceScale::Percent100),
        (1366.0, 768.0, InterfaceScale::Percent100),
        (1366.0, 768.0, InterfaceScale::Percent200),
    ];
    const SHORT: [(f32, f32, InterfaceScale); 2] = [
        (1366.0, 600.0, InterfaceScale::Percent100),
        (1366.0, 600.0, InterfaceScale::Percent200),
    ];
    const DOCK_ELEMENTS: [&str; 7] = [
        "monitor-program",
        "monitor-preview",
        "output-status",
        "button-Freeze",
        "button-Blackout",
        "primary-action",
        "auto-play-control",
    ];

    fn environment(scale: InterfaceScale) -> UiEnvironment {
        environment_with(UiPreferences {
            scale,
            ..UiPreferences::default()
        })
    }

    fn environment_with(preferences: UiPreferences) -> UiEnvironment {
        let mut environment = UiEnvironment::new(preferences, ResolvedAppearance::Dark);
        environment.apply(EnvironmentChange::EnterPresenter);
        environment
    }

    struct Open {
        view: Entity<PresenterView>,
        now: Rc<Cell<u64>>,
        saved: Rc<RefCell<Vec<UiPreferences>>>,
    }

    fn open(
        cx: &mut TestAppContext,
        case: (f32, f32, InterfaceScale),
    ) -> (Open, &mut VisualTestContext) {
        open_with(
            cx,
            case,
            UiPreferences {
                scale: case.2,
                ..UiPreferences::default()
            },
        )
    }

    fn open_with(
        cx: &mut TestAppContext,
        case: (f32, f32, InterfaceScale),
        preferences: UiPreferences,
    ) -> (Open, &mut VisualTestContext) {
        open_snapshot(cx, case, preferences, fixture_snapshot())
    }

    fn open_snapshot(
        cx: &mut TestAppContext,
        (width, height, _): (f32, f32, InterfaceScale),
        preferences: UiPreferences,
        snapshot: Arc<LoadedSnapshot>,
    ) -> (Open, &mut VisualTestContext) {
        cx.update(ui::init).unwrap();
        let now = Rc::new(Cell::new(0_u64));
        let saved = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let now = now.clone();
            let saved = saved.clone();
            move |window, cx| {
                let clock: Rc<dyn Fn() -> Millis> = Rc::new(move || Millis(now.get()));
                let mut view =
                    PresenterView::new(snapshot, clock, environment_with(preferences), window, cx);
                view.set_save_preferences(Rc::new(move |preferences| {
                    saved.borrow_mut().push(preferences);
                }));
                view
            }
        });
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        (Open { view, now, saved }, cx)
    }

    #[track_caller]
    fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
        match cx.debug_bounds(selector) {
            Some(bounds) => bounds,
            None => panic!("{selector} should be on screen"),
        }
    }

    /// Focus a control by pressing it without letting go, so its own
    /// activation (which may move focus on) never runs.
    fn press_without_release(cx: &mut VisualTestContext, selector: &'static str) {
        let at = bounds(cx, selector).center();
        cx.simulate_mouse_down(at, gpui::MouseButton::Left, Modifiers::default());
        cx.run_until_parked();
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let at = bounds(cx, selector).center();
        cx.simulate_click(at, Modifiers::default());
        cx.run_until_parked();
    }

    fn run(open: &Open, cx: &mut VisualTestContext, command: Command) -> Result<Outcome, Refusal> {
        let result = open.view.update(cx, |view, cx| view.command(command, cx));
        cx.run_until_parked();
        result
    }

    /// Move the injected clock and let the Engine catch up, so a fade the
    /// last command started has ended.
    fn settle(open: &Open, cx: &mut VisualTestContext, at: u64) {
        open.now.set(at);
        open.view.update(cx, |view, cx| view.tick(cx));
        cx.run_until_parked();
    }

    fn live(open: &Open, cx: &VisualTestContext) -> LiveState {
        open.view
            .read_with(cx, |view, _| view.engine.state().live())
    }

    fn engine_state(open: &Open, cx: &VisualTestContext) -> presentation::state::EngineState {
        open.view
            .read_with(cx, |view, _| view.engine.state().clone())
    }

    fn left(b: Bounds<Pixels>) -> f32 {
        f32::from(b.origin.x)
    }
    fn right(b: Bounds<Pixels>) -> f32 {
        f32::from(b.origin.x + b.size.width)
    }
    fn top(b: Bounds<Pixels>) -> f32 {
        f32::from(b.origin.y)
    }
    fn bottom(b: Bounds<Pixels>) -> f32 {
        f32::from(b.origin.y + b.size.height)
    }
    fn inside(b: Bounds<Pixels>, width: f32, height: f32) -> bool {
        left(b) >= -0.5 && top(b) >= -0.5 && right(b) <= width + 0.5 && bottom(b) <= height + 0.5
    }

    #[gpui::test]
    fn wide_places_the_rail_timeline_and_dock_in_order(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, SIZES[0]);
        let rail = bounds(cx, "presenter-rail");
        let timeline = bounds(cx, "cue-timeline");
        let dock = bounds(cx, "output-dock");
        assert!(left(rail).abs() < 0.5, "rail starts at x=0");
        assert!(cx.debug_bounds("ordo-tab").is_none());
        assert!(left(timeline) >= right(rail) - 0.5);
        assert!(right(timeline) <= left(dock) + 0.5);
        assert!((f32::from(dock.size.width) - 520.0).abs() < 0.5);
        assert!((right(dock) - 1920.0).abs() < 0.5);
        for selector in ["monitor-program", "monitor-preview"] {
            let monitor = bounds(cx, selector);
            assert!(
                left(monitor) >= left(dock) - 0.5 && right(monitor) <= right(dock) + 0.5,
                "{selector} {monitor:?} inside {dock:?}"
            );
        }
    }

    #[gpui::test]
    fn compact_uses_the_tab_and_a_360_dock_with_program_above_preview(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, SIZES[1]);
        assert!(cx.debug_bounds("presenter-rail").is_none());
        let tab = bounds(cx, "ordo-tab");
        assert!((f32::from(tab.size.width) - 28.0).abs() < 0.5);
        let dock = bounds(cx, "output-dock");
        assert!((f32::from(dock.size.width) - 360.0).abs() < 0.5);
        let program = bounds(cx, "monitor-program");
        let preview = bounds(cx, "monitor-preview");
        assert!(
            bottom(program) <= top(preview) + 0.5,
            "Program above Preview"
        );
        assert!(right(bounds(cx, "cue-timeline")) <= left(dock) + 0.5);
    }

    #[gpui::test]
    fn constrained_puts_both_monitors_in_a_band_above_the_timeline(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, SIZES[2]);
        assert!(cx.debug_bounds("presenter-rail").is_none());
        assert!(cx.debug_bounds("output-dock").is_none());
        assert!(cx.debug_bounds("ordo-tab").is_some());
        let band = bounds(cx, "monitor-band");
        let timeline = bounds(cx, "cue-timeline");
        let program = bounds(cx, "monitor-program");
        let preview = bounds(cx, "monitor-preview");
        assert!(
            bottom(band) <= top(timeline) + 0.5,
            "band above the timeline"
        );
        assert!((top(program) - top(preview)).abs() < 0.5, "shared top edge");
        assert!(right(program) <= left(preview) + 0.5, "side by side");
        for monitor in [program, preview] {
            assert!(inside(monitor, 1366.0, 768.0));
        }
    }

    #[gpui::test]
    fn the_six_dock_elements_stay_inside_the_window_at_every_size(cx: &mut TestAppContext) {
        for case in SIZES.into_iter().chain(SHORT) {
            let (width, height, scale) = case;
            let (_, cx) = open(cx, case);
            for selector in DOCK_ELEMENTS {
                let element = bounds(cx, selector);
                assert!(
                    inside(element, width, height),
                    "{selector} {element:?} outside {width}x{height} at {scale:?}"
                );
            }
        }
    }

    #[gpui::test]
    fn the_primary_action_walks_the_output_through_its_states(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        assert_eq!(live(&open, cx), LiveState::OutputDisabled);
        // Freeze and Blackout are not usable while output is off.
        let before = engine_state(&open, cx);
        click(cx, "button-Freeze");
        click(cx, "button-Blackout");
        assert_eq!(engine_state(&open, cx), before);

        click(cx, "button-Enable Output");
        assert_eq!(live(&open, cx), LiveState::EnabledBlack);
        assert!(cx.debug_bounds("button-Enable Output").is_none());

        click(cx, "button-Take Preview Live");
        assert_eq!(live(&open, cx), LiveState::Live);
        assert!(cx.debug_bounds("button-Take Preview Live").is_none());
        assert!(cx.debug_bounds("primary-info").is_some(), "LIVE statement");

        settle(&open, cx, 1000);
        click(cx, "button-Freeze");
        assert_eq!(live(&open, cx), LiveState::Frozen);
        assert!(cx.debug_bounds("button-Take Preview Live").is_some());
        click(cx, "button-Take Preview Live");
        assert_eq!(live(&open, cx), LiveState::Live);

        assert!(run(&open, cx, Command::DisplayLost).is_ok());
        assert!(
            cx.debug_bounds("primary-info").is_some(),
            "waiting statement"
        );
        assert!(cx.debug_bounds("button-Resume Output").is_none());
        assert!(run(&open, cx, Command::DisplayReturned { recognized: true }).is_ok());
        assert!(cx.debug_bounds("button-Resume Output").is_some());
        click(cx, "button-Resume Output");
        assert_eq!(live(&open, cx), LiveState::Live);
    }

    #[gpui::test]
    fn both_monitors_paint_the_same_frame_and_resizing_never_recomposes(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        // Cue 2 is the Parish-life image frame.
        assert!(run(&open, cx, Command::GoTo(2)).is_ok());
        assert!(run(&open, cx, Command::Take).is_ok());
        settle(&open, cx, 1000);

        open.view.update(cx, |view, _| {
            assert!(view.last_program.frame.is_some(), "Program paints a frame");
            assert!(view.last_preview.frame.is_some(), "Preview paints a frame");
            assert!(view.last_program.image.is_some(), "the image is decoded");
            let (program, preview) = (
                view.last_program.frame.clone().unwrap(),
                view.last_preview.frame.clone().unwrap(),
            );
            assert!(Arc::ptr_eq(&program, &preview), "one Arc<Frame> in both");
            let id = view.engine.program_view(Millis(1000)).shown.unwrap();
            let FrameView::Ready { frame, .. } = view.frames.get(id) else {
                panic!("the frame is ready");
            };
            assert!(
                Arc::ptr_eq(&frame, &program),
                "the store hands out the same Arc"
            );
        });
        assert!(cx.debug_bounds("live-viewport-image").is_some());
        assert!(
            cx.debug_bounds("live-viewport-fade").is_none(),
            "no fade left"
        );

        let composed = open
            .view
            .read_with(cx, |view, _| view.frames.composed_count());
        assert!(composed > 0);
        for (width, height, scale) in SIZES.into_iter().chain(SIZES) {
            open.view
                .update(cx, |view, cx| view.set_environment(environment(scale), cx));
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();
        }
        for percent in [InterfaceScale::Percent150, InterfaceScale::Percent100] {
            open.view.update(cx, |view, cx| {
                view.set_environment(environment(percent), cx)
            });
            cx.run_until_parked();
        }
        assert_eq!(
            open.view
                .read_with(cx, |view, _| view.frames.composed_count()),
            composed,
            "resize and scale changes never recompose"
        );
    }

    #[gpui::test]
    fn a_fade_draws_a_black_cover_and_the_timer_ends_it(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        assert!(run(&open, cx, Command::Take).is_ok());
        assert!(cx.debug_bounds("live-viewport-fade").is_some(), "fading in");
        assert!(open.view.read_with(cx, |view, _| view.timer_pending()));

        // The timer wakes at the fade's end, ticks the Engine and clears.
        open.now.set(500);
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
        assert!(!open.view.read_with(cx, |view, _| view.timer_pending()));
        assert!(
            cx.debug_bounds("live-viewport-fade").is_none(),
            "fade ended"
        );
    }

    #[gpui::test]
    fn a_refused_command_says_why_and_changes_nothing(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        let state = engine_state(&open, cx);
        assert_eq!(run(&open, cx, Command::Take), Err(Refusal::OutputDisabled));
        open.view.read_with(cx, |view, _| {
            assert_eq!(view.engine.state(), &state);
            assert_eq!(view.engine.state().revision(), state.revision());
            assert_eq!(view.status(), Some(refusal_text(Refusal::OutputDisabled)));
        });
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        assert!(open.view.read_with(cx, |view, _| view.status().is_none()));
    }

    #[test]
    fn every_refusal_has_polite_words() {
        let snapshot = fixture_snapshot();
        let cue: CueId = snapshot.snapshot().cue(0).expect("a Cue").id();
        let mut all = vec![
            Refusal::UnavailablePreview { cue },
            Refusal::OutputDisabled,
            Refusal::DisplayNotBack,
            Refusal::OutputDisconnected,
            Refusal::FreezeDuringBlackout,
            Refusal::NotLive,
            Refusal::AutoPlayBlocked,
        ];
        for reason in [
            AutoPlayRefusal::NotGeneralPresentation,
            AutoPlayRefusal::NoDurations,
            AutoPlayRefusal::NothingProjectable,
            AutoPlayRefusal::SectionHasIneligibleItem,
        ] {
            all.push(Refusal::AutoPlayIneligible { reason });
        }
        for refusal in all {
            let text = refusal_text(refusal);
            assert!(!text.trim().is_empty(), "{refusal:?}");
            assert!(!text.contains("Engine"), "{refusal:?} is operator wording");
            assert!(
                !text.contains("Mass text"),
                "{refusal:?} is not a domain term"
            );
            if matches!(refusal, Refusal::AutoPlayIneligible { .. }) {
                assert!(
                    text.contains("Next") || text.contains("choose"),
                    "{refusal:?} names a next step"
                );
            }
        }
    }

    #[gpui::test]
    fn grid_list_saves_the_preference_and_never_touches_the_engine(cx: &mut TestAppContext) {
        for (index, case) in SIZES.into_iter().enumerate() {
            let (open, cx) = open(cx, case);
            let before = engine_state(&open, cx);
            assert_eq!(
                open.view.read_with(cx, |view, _| view.cue_view()),
                CueView::Grid
            );
            let list = bounds(cx, "button-List");
            let side = f32::from(list.size.width);
            let square = 36.0 * case.2.apply(1.0);
            if index == 2 {
                assert!(
                    (side - square).abs() < 1.5,
                    "icon-only in Constrained: {side}"
                );
            } else {
                assert!(side > square + 8.0, "labelled when there is room: {side}");
            }
            click(cx, "button-List");
            assert_eq!(
                open.view.read_with(cx, |view, _| view.cue_view()),
                CueView::List
            );
            assert_eq!(
                open.saved.borrow().last().map(|saved| saved.cue_view),
                Some(CueView::List)
            );
            assert_eq!(engine_state(&open, cx), before);
            click(cx, "button-Grid");
            assert_eq!(
                open.saved.borrow().last().map(|saved| saved.cue_view),
                Some(CueView::Grid)
            );
        }
    }

    #[gpui::test]
    fn freeze_is_presenter_height_scaled_by_interface_scale(cx: &mut TestAppContext) {
        for (case, expected) in [(SIZES[0], 40.0), (SIZES[2], 80.0)] {
            let (_, cx) = open(cx, case);
            let height = f32::from(bounds(cx, "button-Freeze").size.height);
            assert!((height - expected).abs() < 0.5, "{height} != {expected}");
        }
    }

    // ---- Timeline and rail (T1.4 WP6, WP7) -------------------------------

    /// A selector built at run time. Leaking a few short strings in a test is
    /// the price of `debug_bounds` taking `&'static str`.
    fn sel(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    fn cue(flat: usize) -> &'static str {
        sel(format!("cue-{flat}"))
    }

    fn layout_at((width, height, scale): (f32, f32, InterfaceScale)) -> PresenterLayout {
        presenter_layout(LayoutInput {
            width,
            height,
            scale,
            rail: RailChoice::Auto,
        })
    }

    fn present(cx: &mut VisualTestContext, selector: &'static str) -> bool {
        cx.debug_bounds(selector).is_some()
    }

    /// Click `selector` a few pixels inside its top-left corner, for a target
    /// taller than the window (a tile at 200%).
    fn click_corner(cx: &mut VisualTestContext, selector: &'static str) {
        let origin = bounds(cx, selector).origin;
        cx.simulate_click(origin + gpui::point(px(6.0), px(6.0)), Modifiers::default());
        cx.run_until_parked();
    }

    /// Let `millis` of animation pass and draw again, as the frames a real
    /// window delivers would.
    /// Lay out every group, as before virtualisation, for a test of geometry
    /// that does not scroll.
    fn lay_every_group_out(open: &Open, cx: &mut VisualTestContext) {
        open.view.update(cx, |view, cx| {
            view.all_groups_real = true;
            cx.notify();
        });
        cx.run_until_parked();
    }

    fn animate(open: &Open, cx: &mut VisualTestContext, millis: u64) {
        cx.executor().advance_clock(Duration::from_millis(millis));
        open.view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }

    /// Press the ORDO tab and let the overlay finish sliding in, so its
    /// controls are on screen to be pressed.
    fn open_overlay(open: &Open, cx: &mut VisualTestContext) {
        click(cx, "ordo-tab");
        animate(open, cx, 400);
        assert!(open.view.read_with(cx, |view, _| view.rail_open()));
    }

    fn timeline_offset(open: &Open, cx: &VisualTestContext) -> f32 {
        open.view.read_with(cx, |view, _| {
            f32::from(view.timeline_scroll.handle().offset().y)
        })
    }

    #[gpui::test]
    fn wide_pins_the_rail_and_compact_and_constrained_open_it_from_the_tab(
        cx: &mut TestAppContext,
    ) {
        for case in SIZES {
            let (open, cx) = open(cx, case);
            if case.0 == 1920.0 {
                assert!(present(cx, "presenter-rail"), "pinned at Wide");
                assert!(!present(cx, "ordo-tab"));
                assert!(!present(cx, "rail-overlay"));
                continue;
            }
            assert!(present(cx, "ordo-tab"), "{case:?}");
            assert!(
                !present(cx, "presenter-rail"),
                "no rail until the tab is pressed"
            );
            // Hover never opens it.
            let at = bounds(cx, "ordo-tab").center();
            cx.simulate_mouse_move(at, None, Modifiers::default());
            cx.run_until_parked();
            assert!(
                !present(cx, "presenter-rail"),
                "hover must not open the rail"
            );
            click(cx, "ordo-tab");
            assert!(open.view.read_with(cx, |view, _| view.rail_open()));
            assert!(present(cx, "presenter-rail"), "{case:?}");
            assert!(present(cx, "rail-scrim"));
            assert!(
                present(cx, "ordo-tab"),
                "the tab column stays under the overlay"
            );
        }
    }

    fn tab_width(cx: &mut VisualTestContext) -> f32 {
        f32::from(bounds(cx, "ordo-tab").size.width)
    }

    fn hover_tab(cx: &mut VisualTestContext) {
        let at = bounds(cx, "ordo-tab").center();
        cx.simulate_mouse_move(at, None, Modifiers::default());
        cx.run_until_parked();
    }

    fn leave_tab(cx: &mut VisualTestContext) {
        cx.simulate_mouse_move(
            gpui::point(px(700.0), px(500.0)),
            None,
            Modifiers::default(),
        );
        cx.run_until_parked();
    }

    #[gpui::test]
    fn hovering_the_tab_grows_it_over_the_grid_without_moving_anything(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        let timeline = bounds(cx, "cue-timeline");
        let slot = bounds(cx, "ordo-tab-slot");
        let rest = tab_width(cx);
        hover_tab(cx);
        animate(&open, cx, 30);
        let midway = tab_width(cx);
        assert!(midway > rest + 0.5 && midway < rest + 6.0 - 0.5, "{midway}");
        animate(&open, cx, 200);
        assert!((tab_width(cx) - (rest + 6.0)).abs() < 0.5);
        assert_eq!(
            bounds(cx, "cue-timeline"),
            timeline,
            "the grid does not move"
        );
        assert_eq!(bounds(cx, "ordo-tab-slot"), slot, "the layout width stays");
        let tab = bounds(cx, "ordo-tab");
        let centre = left(tab) + f32::from(tab.size.width) / 2.0;
        assert!((centre - (left(tab) + (rest + 6.0) / 2.0)).abs() < 0.5);
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));

        leave_tab(cx);
        animate(&open, cx, 200);
        assert!((tab_width(cx) - rest).abs() < 0.5, "unhover gives it back");
        assert_eq!(bounds(cx, "cue-timeline"), timeline);
    }

    #[gpui::test]
    fn an_unhover_mid_grow_reverses_from_the_current_width(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        let rest = tab_width(cx);
        hover_tab(cx);
        animate(&open, cx, 80);
        let grown = tab_width(cx);
        leave_tab(cx);
        animate(&open, cx, 20);
        let shrinking = tab_width(cx);
        assert!(shrinking < grown && shrinking > rest + 0.1, "{shrinking}");
        animate(&open, cx, 200);
        assert!((tab_width(cx) - rest).abs() < 0.5);
    }

    #[gpui::test]
    fn reduced_motion_never_changes_the_tab_width(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        cx.update(|_, cx| cx.set_reduce_motion(true));
        let rest = tab_width(cx);
        hover_tab(cx);
        for millis in [0, 30, 200] {
            animate(&open, cx, millis);
            assert!((tab_width(cx) - rest).abs() < 0.01);
        }
        leave_tab(cx);
        animate(&open, cx, 200);
        assert!((tab_width(cx) - rest).abs() < 0.01);
    }

    #[gpui::test]
    fn a_click_on_the_grown_tab_still_opens_the_rail(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        hover_tab(cx);
        animate(&open, cx, 200);
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        click(cx, "ordo-tab");
        assert!(open.view.read_with(cx, |view, _| view.rail_open()));
    }

    #[gpui::test]
    fn operator_only_and_unavailable_cues_carry_a_shape_as_well_as_a_word(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        let kinds = open.view.read_with(cx, |view, _| {
            let snapshot = view.engine.snapshot();
            (0..snapshot.cue_count())
                .map(|flat| snapshot.cue(flat).unwrap().behavior().clone())
                .collect::<Vec<_>>()
        });
        let trigger = kinds
            .iter()
            .position(|b| matches!(b, CueBehavior::OperatorTrigger))
            .expect("the fixture has a trigger");
        let group = open.view.read_with(cx, |view, _| {
            let snapshot = view.engine.snapshot();
            (0..snapshot.item_count())
                .rev()
                .find(|g| {
                    snapshot
                        .item_start(*g)
                        .is_some_and(|start| start <= trigger)
                })
                .expect("the trigger's group")
        });
        open.view
            .update(cx, |view, cx| view.jump_to_group(group, cx));
        cx.run_until_parked();
        animate(&open, cx, 0);
        let trigger_marker: &'static str = format!("cue-{trigger}-marker").leak();
        assert!(
            present(cx, trigger_marker),
            "a Grid row leads with its marker"
        );
        open.view.update(cx, |view, cx| view.jump_to_group(0, cx));
        cx.run_until_parked();
        animate(&open, cx, 0);

        // Cue 1's frame fails: its tile keeps its place with the operator-only
        // placeholder, and its group says so even when folded.
        let CueBehavior::Project { frame, .. } = kinds[1] else {
            panic!("Cue 1 projects")
        };
        let before = engine_state(&open, cx);
        let _ = run(
            &open,
            cx,
            Command::FrameUnavailable {
                frame,
                reason: rendering::UnavailableReason::DecodeFailed,
            },
        );
        assert!(present(cx, "cue-1-unavailable"));
        assert!(present(cx, "group-1-unavailable"));
        assert_eq!(engine_state(&open, cx).live(), before.live());
    }

    #[gpui::test]
    fn grid_makes_tiles_and_list_makes_rows_and_the_choice_is_saved_and_restored(
        cx: &mut TestAppContext,
    ) {
        for case in SIZES {
            let s = case.2.apply(1.0);
            let layout = layout_at(case);
            let (open, vcx) = open(cx, case);
            // Tile geometry is what is measured here, wherever the Cue is: the
            // timeline is a few hundred pixels tall at 200%, so a Cue below
            // the first group would be a spacer.
            lay_every_group_out(&open, vcx);
            let before = engine_state(&open, vcx);
            // Cue 1 is a Project Cue.
            let tile = bounds(vcx, cue(1));
            let frame = bounds(vcx, "cue-1-frame");
            assert!(
                (f32::from(frame.size.width) - layout.cell_width).abs() < 0.5,
                "{case:?}: frame {frame:?} at cell {}",
                layout.cell_width
            );
            assert!(
                (f32::from(frame.size.height) - layout.cell_width * 9.0 / 16.0).abs() < 0.5,
                "{case:?}: 16:9 frame"
            );
            assert!(f32::from(tile.size.height) > f32::from(frame.size.height));
            assert!(
                (f32::from(tile.size.width) - (layout.cell_width + 4.0 * s)).abs() < 0.5,
                "{case:?}: a tile is the cell plus its padding"
            );

            click(vcx, "button-List");
            let row = bounds(vcx, cue(1));
            assert!(
                (f32::from(row.size.height) - 32.0 * s).abs() < 0.5,
                "{case:?}: row height {row:?}"
            );
            assert!(!present(vcx, "cue-1-frame"), "no tile frame in List");
            // A row leads with a small canonical thumbnail inside its height.
            let mini = bounds(vcx, "cue-1-mini");
            assert!(
                (f32::from(mini.size.width) - timeline::MINI_WIDTH * s).abs() < 0.5
                    && f32::from(mini.size.height) <= f32::from(row.size.height),
                "{case:?}: mini {mini:?} in row {row:?}"
            );
            assert!(
                (right(row) - left(row) - layout.timeline_width).abs() < 1.0,
                "{case:?}: a row spans the group"
            );
            assert_eq!(engine_state(&open, vcx), before, "Grid/List is view state");
            assert_eq!(
                engine_state(&open, vcx).revision(),
                before.revision(),
                "the revision does not move"
            );
            let saved = *open.saved.borrow().last().expect("List is saved");
            assert_eq!(saved.cue_view, CueView::List);

            // A new view over the saved preferences opens in List.
            let (restored, vcx) = open_with(cx, case, saved);
            lay_every_group_out(&restored, vcx);
            let row = bounds(vcx, cue(1));
            assert!(
                (f32::from(row.size.height) - 32.0 * s).abs() < 0.5,
                "{case:?}"
            );
            assert!(!present(vcx, "cue-1-frame"));
        }
    }

    #[gpui::test]
    fn only_slides_are_tiles_and_trigger_wait_and_hold_stay_rows_in_grid(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, SIZES[0]);
        let layout = layout_at(SIZES[0]);
        // Cue 10 is the greeting trigger, cue 12 the Hold; cue 4 a slide.
        for flat in [10, 12] {
            let row = bounds(cx, cue(flat));
            assert!(
                (f32::from(row.size.height) - 32.0).abs() < 0.5,
                "cue {flat} is a compact row in Grid"
            );
            assert!(!present(cx, sel(format!("cue-{flat}-frame"))));
            assert!((right(row) - left(row) - layout.timeline_width).abs() < 1.0);
        }
        assert!(present(cx, "cue-4-frame"));
    }

    #[gpui::test]
    fn the_270_floor_and_sparse_rows_hold_while_the_window_narrows(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, SIZES[0]);
        for (width, height) in [(1920.0, 1080.0), (1500.0, 1000.0), (1200.0, 900.0)] {
            let case = (width, height, InterfaceScale::Percent100);
            cx.simulate_resize(size(px(width), px(height)));
            cx.run_until_parked();
            let layout = layout_at(case);
            // Entrance song: five slides in flat order 4..=8.
            let first = bounds(cx, cue(4));
            let on_first_row = (4..=8)
                .filter(|flat| (top(bounds(cx, cue(*flat))) - top(first)).abs() < 0.5)
                .count();
            assert_eq!(
                on_first_row,
                (layout.grid_columns as usize).min(5),
                "{width}: columns follow the layout model"
            );
            let full = f32::from(first.size.width);
            if layout.grid_columns > 1 {
                assert!(full >= 270.0, "{width}: tile {full} is under the floor");
            }
            // The last slide sits alone or in a short row; same width.
            let last = bounds(cx, cue(8));
            assert!(
                (f32::from(last.size.width) - full).abs() < 0.5,
                "{width}: a sparse row keeps the full-row width"
            );
            // Cue 1 opens a row of three: its width is the reference too.
            let reference = f32::from(bounds(cx, cue(1)).size.width);
            assert!(
                (f32::from(last.size.width) - reference).abs() < 0.5,
                "{width}: the last tile of a sparse row matches cue 1"
            );
            // A group of one Cue.
            let single = bounds(cx, cue(0));
            assert!(
                (f32::from(single.size.width) - reference).abs() < 0.5,
                "{width}: a one-Cue group matches cue 1"
            );
            assert!(
                (f32::from(single.size.width) - full).abs() < 0.5,
                "{width}: a one-Cue group keeps the full-row width"
            );
            assert!(right(single) <= width + 0.5);
        }
    }

    #[gpui::test]
    fn the_rail_lists_every_item_under_its_section_and_a_click_brings_a_group_to_the_top(
        cx: &mut TestAppContext,
    ) {
        for case in SIZES {
            let (open, cx) = open(cx, case);
            let snapshot = fixture_snapshot();
            let titles: Vec<String> = snapshot
                .snapshot()
                .sections()
                .iter()
                .flat_map(|section| section.items())
                .map(|item| item.title().to_owned())
                .collect();
            let model = open
                .view
                .read_with(cx, |view, _| view.rail_model(PinControl::Pin));
            assert_eq!(model.sections.len(), 6, "six sections (Decision 171)");
            let shown: Vec<String> = model
                .sections
                .iter()
                .flat_map(|section| &section.items)
                .map(|item| item.title.to_string())
                .collect();
            assert_eq!(shown, titles, "titles only, in order");

            if case.0 != 1920.0 {
                open_overlay(&open, cx);
            }
            let mut last = f32::MIN;
            for k in 0..titles.len() {
                let row = bounds(cx, sel(format!("rail-item-{k}")));
                assert!(top(row) > last, "{case:?}: rail row {k} follows the last");
                last = top(row);
            }

            let before = engine_state(&open, cx);
            // A late group, still inside the rail's first screenful at 200%.
            let target = 4;
            click(cx, sel(format!("rail-item-{target}")));
            animate(&open, cx, 0);
            let timeline = bounds(cx, "cue-timeline");
            let group = bounds(cx, sel(format!("group-{target}")));
            assert!(
                (top(group) - top(timeline)).abs() <= 1.0,
                "{case:?}: group {target} at {} under timeline top {}",
                top(group),
                top(timeline)
            );
            for k in 0..titles.len() {
                assert!(
                    present(cx, sel(format!("group-{k}"))),
                    "group {k} is still present"
                );
            }
            assert_eq!(
                engine_state(&open, cx),
                before,
                "the rail never reaches the Engine"
            );
            assert!(
                !open.view.read_with(cx, |view, _| view.rail_open()),
                "an overlay closes once the group is in place"
            );
        }
    }

    #[gpui::test]
    fn the_last_group_comes_to_the_top_after_the_first_four_are_folded(cx: &mut TestAppContext) {
        for case in SIZES {
            let (open, cx) = open(cx, case);
            for group in 0..4 {
                open.view
                    .update(cx, |view, cx| view.toggle_group(group, cx));
            }
            animate(&open, cx, 400);
            animate(&open, cx, 0);
            let last = open
                .view
                .read_with(cx, |view, _| view.engine.snapshot().item_count() - 1);
            let before = engine_state(&open, cx);
            open.view
                .update(cx, |view, cx| view.jump_to_group(last, cx));
            cx.run_until_parked();
            animate(&open, cx, 0);
            let timeline = bounds(cx, "cue-timeline");
            let group = bounds(cx, sel(format!("group-{last}")));
            assert!(
                (top(group) - top(timeline)).abs() <= 1.0,
                "{case:?}: last group at {} under timeline top {}",
                top(group),
                top(timeline)
            );
            assert!(timeline_offset(&open, cx) < -1.0, "{case:?}: it scrolled");
            assert_eq!(engine_state(&open, cx), before);
        }
    }

    #[gpui::test]
    fn pinning_and_unpinning_swap_the_rail_and_the_tab(cx: &mut TestAppContext) {
        // Wide: pinned by default; Unpin gives the tab, the tab opens the
        // overlay, and Pin there pins it again.
        let (open, cx) = open(cx, SIZES[0]);
        click(cx, "button-Unpin Ordo");
        assert!(!present(cx, "presenter-rail"));
        assert!(present(cx, "ordo-tab"));
        assert_eq!(
            open.view.read_with(cx, |view, _| view.rail_choice()),
            RailChoice::Unpinned
        );
        open_overlay(&open, cx);
        assert!(present(cx, "presenter-rail"));
        assert!(present(cx, "button-Pin Ordo"));
        assert!(!present(cx, "button-Unpin Ordo"));
        click(cx, "button-Pin Ordo");
        assert!(present(cx, "presenter-rail"));
        assert!(!present(cx, "ordo-tab"));
        assert!(!present(cx, "rail-overlay"), "pinning closes the overlay");
        assert!(present(cx, "button-Unpin Ordo"));
    }

    #[gpui::test]
    fn compact_can_pin_the_rail_for_the_session(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        open_overlay(&open, cx);
        click(cx, "button-Pin Ordo");
        assert!(present(cx, "presenter-rail"));
        assert!(!present(cx, "ordo-tab"));
        let rail = bounds(cx, "presenter-rail");
        assert!((f32::from(rail.size.width) - 200.0).abs() < 0.5);
        assert!(left(bounds(cx, "cue-timeline")) >= right(rail) - 0.5);
        assert_eq!(
            open.view.read_with(cx, |view, _| view.rail_choice()),
            RailChoice::Pinned
        );
        click(cx, "button-Unpin Ordo");
        assert!(present(cx, "ordo-tab"));
        assert!(!present(cx, "presenter-rail"));
    }

    #[gpui::test]
    fn constrained_overlay_has_no_pin_and_a_scrim_press_closes_it_without_going_through(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[2]);
        // Bring a Cue tile under where the scrim will be, so a press that
        // went through would choose that Cue.
        open.view.update(cx, |view, cx| view.jump_to_group(1, cx));
        cx.run_until_parked();
        open_overlay(&open, cx);
        assert!(present(cx, "presenter-rail"));
        assert!(!present(cx, "button-Pin Ordo"));
        assert!(!present(cx, "button-Unpin Ordo"));
        assert!(present(cx, "button-Close Ordo"));
        let before = engine_state(&open, cx);
        let panel = bounds(cx, "presenter-rail");
        let tile = bounds(cx, "cue-1");
        let at = gpui::point(px(right(panel) + 20.0), tile.origin.y + px(6.0));
        assert!(
            tile.contains(&at),
            "the press lands on a tile: {at:?} in {tile:?}"
        );
        cx.simulate_click(at, Modifiers::default());
        cx.run_until_parked();
        assert!(
            !present(cx, "presenter-rail"),
            "the scrim closes the overlay"
        );
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        assert_eq!(engine_state(&open, cx), before, "the press was consumed");
        cx.update(|window, cx| {
            open.view.read_with(cx, |view, _| {
                assert!(
                    view.timeline_focus.is_focused(window),
                    "focus returns to the timeline"
                );
            });
        });
    }

    fn timeline_has_focus(open: &Open, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| {
            open.view
                .read_with(cx, |view, _| view.timeline_focus.is_focused(window))
        })
    }

    fn bind_registry_keys(cx: &mut VisualTestContext) {
        cx.update(|_, cx| cx.bind_keys(registry::key_bindings()));
    }

    fn revision(open: &Open, cx: &VisualTestContext) -> u64 {
        open.view
            .read_with(cx, |view, _| view.engine.state().revision())
    }

    fn preview_index(open: &Open, cx: &VisualTestContext) -> usize {
        open.view
            .read_with(cx, |view, _| view.engine.state().preview())
    }

    fn press_keys(cx: &mut VisualTestContext, keys: &str) {
        cx.simulate_keystrokes(keys);
        cx.run_until_parked();
    }

    /// Choosing Grid or List hands focus back to the Cues, so the arrows and
    /// `O` work straight after the click without an `Esc` first.
    #[gpui::test]
    fn choosing_grid_or_list_keeps_the_arrows_and_o_live(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        for label in ["button-List", "button-Grid"] {
            click(cx, label);
            assert!(timeline_has_focus(&open, cx), "after {label}");
            let before = preview_index(&open, cx);
            press_keys(cx, "right");
            assert_eq!(preview_index(&open, cx), before + 1, "right after {label}");
        }
        press_keys(cx, "o");
        let rail_focused = cx.update(|window, cx| {
            open.view
                .read_with(cx, |view, cx| view.rail_focus.contains_focused(window, cx))
        });
        assert!(rail_focused, "O after Grid|List");
    }

    /// In the rail the arrows move a visible cursor (Left/Right like Up/Down),
    /// never the Cues; `Enter` jumps to its item; the ring goes when focus
    /// leaves the rail.
    #[gpui::test]
    fn arrows_in_the_rail_move_a_visible_cursor(cx: &mut TestAppContext) {
        for size in [SIZES[0], SIZES[1]] {
            let (open, cx) = open(cx, size);
            bind_registry_keys(cx);
            open.view
                .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
            cx.run_until_parked();
            let preview = preview_index(&open, cx);
            press_keys(cx, "o");
            assert!(
                cx.debug_bounds("rail-item-0-cursor").is_some(),
                "{size:?}: starts on Preview's item"
            );
            press_keys(cx, "down");
            assert!(
                cx.debug_bounds("rail-item-1-cursor").is_some(),
                "{size:?}: down"
            );
            assert!(
                cx.debug_bounds("rail-item-0-cursor").is_none(),
                "{size:?}: one ring"
            );
            press_keys(cx, "right");
            assert!(
                cx.debug_bounds("rail-item-2-cursor").is_some(),
                "{size:?}: right"
            );
            press_keys(cx, "left");
            press_keys(cx, "up");
            assert!(
                cx.debug_bounds("rail-item-0-cursor").is_some(),
                "{size:?}: left, up"
            );
            press_keys(cx, "up");
            assert!(
                cx.debug_bounds("rail-item-0-cursor").is_some(),
                "{size:?}: stops at the top"
            );
            assert_eq!(
                preview_index(&open, cx),
                preview,
                "{size:?}: the Cues never moved"
            );
            press_keys(cx, "end");
            let last = open.view.read_with(cx, |view, _| {
                view.engine
                    .snapshot()
                    .sections()
                    .iter()
                    .map(|s| s.items().len())
                    .sum::<usize>()
                    - 1
            });
            let selector: &'static str = format!("rail-item-{last}-cursor").leak();
            assert!(cx.debug_bounds(selector).is_some(), "{size:?}: end");
            press_keys(cx, "home");
            press_keys(cx, "down");
            press_keys(cx, "enter");
            // Enter returns the keys to the Cues; an overlay closes.
            let (pinned, overlay_open) = open
                .view
                .read_with(cx, |view, _| (view.rail_is_pinned, view.rail_open()));
            assert!(pinned || !overlay_open, "{size:?}: an overlay closes");
            assert!(
                cx.debug_bounds("rail-item-1-cursor").is_none(),
                "{size:?}: ring left with focus"
            );
            if pinned {
                // Back into the rail, the ring starts on Preview's new item.
                open.view
                    .update_in(cx, |view, window, cx| view.rail_focus.focus(window, cx));
                cx.run_until_parked();
                assert!(
                    cx.debug_bounds("rail-item-1-cursor").is_some(),
                    "{size:?}: focus in"
                );
            }
        }
    }

    /// The first Cue of a Mass item, by its flat rail index.
    fn first_cue_of(open: &Open, cx: &VisualTestContext, item: usize) -> usize {
        open.view.read_with(cx, |view, _| {
            view.engine
                .snapshot()
                .positions()
                .iter()
                .position(|position| position.item_flat == item)
                .expect("the item has Cues")
        })
    }

    /// `Enter` on a rail item, keyboard only: Preview moves to the item's
    /// first Cue, the keys go back to the Cues, and a Live output is frozen
    /// so Program stays put. Frozen and Blackout are left as they are.
    #[gpui::test]
    fn enter_in_the_rail_selects_the_item_and_returns_to_the_cues(cx: &mut TestAppContext) {
        for (size, setup, after) in [
            (SIZES[0], None, LiveState::Frozen),
            (SIZES[1], None, LiveState::Frozen),
            (SIZES[0], Some(Command::ToggleFreeze), LiveState::Frozen),
            (SIZES[0], Some(Command::ToggleBlackout), LiveState::Blackout),
        ] {
            let (open, cx) = open(cx, size);
            bind_registry_keys(cx);
            run(&open, cx, Command::EnableOutput).unwrap();
            run(&open, cx, Command::Take).unwrap();
            if let Some(command) = setup {
                run(&open, cx, command).unwrap();
            }
            let program = engine_state(&open, cx).program();
            open.view
                .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
            cx.run_until_parked();
            press_keys(cx, "o");
            press_keys(cx, "down");
            press_keys(cx, "down");
            press_keys(cx, "down");
            press_keys(cx, "enter");
            let target = first_cue_of(&open, cx, 3);
            let case = format!("{size:?} {setup:?}");
            assert_eq!(preview_index(&open, cx), target, "{case}: Preview");
            let state = engine_state(&open, cx);
            assert_eq!(state.live(), after, "{case}: output state");
            assert_eq!(state.program(), program, "{case}: Program held");
            assert_eq!(
                focus_context_now(&open, cx),
                FocusContext::Timeline,
                "{case}: keys back on the Cues"
            );
            press_keys(cx, "right");
            assert_eq!(preview_index(&open, cx), target + 1, "{case}: arrows work");
        }
    }

    /// A click on a rail item stays a plain jump: Preview and the output are
    /// unchanged.
    #[gpui::test]
    fn a_click_in_the_rail_only_jumps(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        run(&open, cx, Command::Take).unwrap();
        let before = engine_state(&open, cx);
        click(cx, "rail-item-3");
        assert_eq!(engine_state(&open, cx), before);
    }

    #[gpui::test]
    fn timeline_focus_navigates_with_the_registry_keys(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        open.view
            .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
        let start = preview_index(&open, cx);
        press_keys(cx, "right");
        assert_eq!(preview_index(&open, cx), start + 1);
        press_keys(cx, "down");
        assert_eq!(preview_index(&open, cx), start + 2);
        press_keys(cx, "left up");
        assert_eq!(preview_index(&open, cx), start);
        press_keys(cx, "end");
        assert!(preview_index(&open, cx) > start + 2);
        press_keys(cx, "home");
        assert_eq!(preview_index(&open, cx), 0);
    }

    #[gpui::test]
    fn timeline_keys_report_a_refusal_in_words(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        open.view
            .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
        let before = revision(&open, cx);
        press_keys(cx, "space");
        assert_eq!(revision(&open, cx), before);
        assert_eq!(
            open.view
                .read_with(cx, |view, _| view.status().map(str::to_owned)),
            Some(refusal_text(Refusal::OutputDisabled).to_owned())
        );
    }

    /// Show a real `ui` Field inside the Presenter, focus it and return its
    /// state (the first call creates it; later calls refocus it).
    fn focus_text_field(
        open: &Open,
        cx: &mut VisualTestContext,
    ) -> ui::primitives::field::FieldState {
        let state = open.view.update_in(cx, |view, window, cx| {
            let state = view
                .text_field
                .get_or_insert_with(|| ui::primitives::field::FieldState::new(window, cx))
                .clone();
            cx.notify();
            state
        });
        cx.run_until_parked();
        cx.update(|window, cx| state.focus(window, cx));
        cx.run_until_parked();
        state
    }

    /// Open a real `ui` dialog inside the Presenter (the first call mounts the
    /// test host) and let it take focus.
    fn open_modal(open: &Open, cx: &mut VisualTestContext) {
        let host = open.view.update_in(cx, |view, window, cx| {
            let host = view
                .modal_host
                .get_or_insert_with(|| ui::overlay::OverlayHost::new(cx))
                .clone();
            cx.notify();
            let _ = window;
            host
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            host.open(
                ui::overlay::Overlay::dialog("Test dialog").description("A true modal"),
                window,
                cx,
            )
        });
        cx.run_until_parked();
    }

    /// Close every test dialog still open.
    fn close_modals(open: &Open, cx: &mut VisualTestContext) {
        let host = open.view.read_with(cx, |view, _| view.modal_host.clone());
        if let Some(host) = host {
            while cx.update(|_, cx| host.depth(cx)) > 0 {
                cx.update(|window, cx| host.close_top(window, cx));
                cx.run_until_parked();
            }
        }
    }

    fn focus_context_now(open: &Open, cx: &mut VisualTestContext) -> FocusContext {
        cx.update(|window, cx| {
            open.view
                .read_with(cx, |view, cx| view.focus_context(window, cx))
        })
    }

    fn unhandled_keys(open: &Open, cx: &VisualTestContext) -> Vec<String> {
        open.view
            .read_with(cx, |view, _| view.unhandled_keys.borrow().clone())
    }

    #[gpui::test]
    fn rail_focus_leaves_only_f_b_and_space_active(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        open_overlay(&open, cx);
        let before = engine_state(&open, cx);

        // Navigation, auto-play and `O` do nothing at all, and `O` neither
        // reopens nor closes the overlay nor moves focus out of it.
        for keys in ["right", "down", "pageup", "end", "a", "o"] {
            press_keys(cx, keys);
            assert_eq!(engine_state(&open, cx), before, "{keys}");
            assert_eq!(
                open.view
                    .read_with(cx, |view, _| view.status().map(str::to_owned)),
                None,
                "{keys}"
            );
            assert!(
                open.view.read_with(cx, |view, _| view.rail_open()),
                "{keys} left the rail open"
            );
            assert_eq!(focus_context_now(&open, cx), FocusContext::Rail, "{keys}");
        }

        // Space takes Preview live, then F and B reach the Engine.
        press_keys(cx, "space");
        assert_eq!(live(&open, cx), LiveState::Live);
        let taken = revision(&open, cx);
        assert!(taken > before.revision());
        press_keys(cx, "f");
        assert!(revision(&open, cx) > taken, "F reached the Engine");
        assert_eq!(live(&open, cx), LiveState::Frozen);
        press_keys(cx, "space");
        assert_eq!(live(&open, cx), LiveState::Live);
        let live_revision = revision(&open, cx);
        press_keys(cx, "b");
        assert!(revision(&open, cx) > live_revision, "B reached the Engine");
        assert_eq!(live(&open, cx), LiveState::Blackout);
    }

    /// A key the policy refuses reaches the Engine not at all but is still
    /// heard by whatever is focused: it bubbles on instead of being swallowed.
    #[gpui::test]
    fn a_refused_key_propagates_to_the_focused_control(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        open_overlay(&open, cx);
        let before = engine_state(&open, cx);
        press_keys(cx, "a");
        assert_eq!(engine_state(&open, cx), before);
        assert_eq!(unhandled_keys(&open, cx), ["a"]);

        // An allowed key is consumed by the command and never bubbles.
        run(&open, cx, Command::EnableOutput).unwrap();
        press_keys(cx, "space");
        assert_eq!(live(&open, cx), LiveState::Live);
        assert_eq!(unhandled_keys(&open, cx), ["a"]);
    }

    #[gpui::test]
    fn a_pinned_rail_counts_as_rail_focus_and_never_closes_on_escape(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        bind_registry_keys(cx);
        assert!(present(cx, "presenter-rail"));
        run(&open, cx, Command::EnableOutput).unwrap();
        // The pinned panel tracks the same handle as the overlay does. (The
        // rows are not focus targets and the only button, Unpin, would
        // remove the rail, so focus is moved the way Tab or F6 will.)
        open.view
            .update_in(cx, |view, window, cx| view.rail_focus.focus(window, cx));
        assert_eq!(focus_context_now(&open, cx), FocusContext::Rail);
        let before = engine_state(&open, cx);

        press_keys(cx, "right");
        assert_eq!(engine_state(&open, cx), before, "navigation is refused");
        press_keys(cx, "escape");
        assert!(present(cx, "presenter-rail"), "Esc keeps the pinned rail");
        assert_eq!(focus_context_now(&open, cx), FocusContext::Timeline);
        open.view
            .update_in(cx, |view, window, cx| view.rail_focus.focus(window, cx));
        assert_eq!(focus_context_now(&open, cx), FocusContext::Rail);

        press_keys(cx, "space");
        assert_eq!(live(&open, cx), LiveState::Live);
        press_keys(cx, "f");
        assert_eq!(live(&open, cx), LiveState::Frozen);
        press_keys(cx, "b");
        assert_eq!(live(&open, cx), LiveState::Blackout);
    }

    #[gpui::test]
    fn dock_focus_allows_operating_keys(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        run(&open, cx, Command::Take).unwrap();
        settle(&open, cx, 1000);
        click(cx, "button-Freeze");
        assert_eq!(live(&open, cx), LiveState::Frozen);
        assert_eq!(focus_context_now(&open, cx), FocusContext::Dock);
        let start = preview_index(&open, cx);
        press_keys(cx, "right");
        assert_eq!(preview_index(&open, cx), start + 1);
    }

    /// With a header control focused, only the Output keys act.
    #[gpui::test]
    fn elsewhere_allows_output_keys_only(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        press_without_release(cx, "button-List");
        assert_eq!(focus_context_now(&open, cx), FocusContext::Elsewhere);
        let before = engine_state(&open, cx);
        press_keys(cx, "right");
        assert_eq!(engine_state(&open, cx), before);
        press_keys(cx, "space");
        assert_eq!(live(&open, cx), LiveState::Live);
    }

    fn reference_open(open: &Open, cx: &VisualTestContext) -> bool {
        open.view.read_with(cx, |view, _| view.reference_open())
    }

    fn close_button_focused(open: &Open, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| {
            open.view
                .read_with(cx, |view, _| view.reference_close_focus.is_focused(window))
        })
    }

    fn focus_dock(open: &Open, cx: &mut VisualTestContext) {
        open.view
            .update_in(cx, |view, window, cx| view.dock_focus.focus(window, cx));
    }

    #[gpui::test]
    fn ctrl_slash_toggles_the_reference_and_restores_prior_focus(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        for (from, name) in [
            (FocusContext::Timeline, "timeline"),
            (FocusContext::Dock, "dock"),
        ] {
            match from {
                FocusContext::Timeline => open
                    .view
                    .update_in(cx, |view, window, cx| view.focus_timeline(window, cx)),
                _ => focus_dock(&open, cx),
            }
            assert_eq!(focus_context_now(&open, cx), from, "{name}");
            let before = engine_state(&open, cx);

            press_keys(cx, "ctrl-/");
            assert!(reference_open(&open, cx), "{name}: opened");
            assert!(present(cx, "shortcut-reference"), "{name}: drawn");
            assert_eq!(
                focus_context_now(&open, cx),
                FocusContext::ShortcutReference,
                "{name}"
            );
            assert!(
                close_button_focused(&open, cx),
                "{name}: focus starts on the close button (DESIGN.md)"
            );
            assert_eq!(engine_state(&open, cx), before, "{name}: Program untouched");

            press_keys(cx, "ctrl-/");
            assert!(!reference_open(&open, cx), "{name}: closed");
            assert!(!present(cx, "shortcut-reference"), "{name}: gone");
            assert_eq!(focus_context_now(&open, cx), from, "{name}: focus restored");

            press_keys(cx, "ctrl-/");
            press_keys(cx, "escape");
            assert!(!reference_open(&open, cx), "{name}: Esc closes");
            assert_eq!(focus_context_now(&open, cx), from, "{name}: Esc restores");
            assert_eq!(engine_state(&open, cx), before, "{name}");
        }
    }

    #[gpui::test]
    fn inside_the_reference_only_f_b_and_space_reach_the_engine(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        open.view
            .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
        press_keys(cx, "ctrl-/");
        let before = engine_state(&open, cx);
        for keys in ["right", "down", "pagedown", "home", "a", "o"] {
            press_keys(cx, keys);
            assert_eq!(engine_state(&open, cx), before, "{keys}");
            assert!(reference_open(&open, cx), "{keys} left it open");
            assert_eq!(
                focus_context_now(&open, cx),
                FocusContext::ShortcutReference,
                "{keys}"
            );
        }
        press_keys(cx, "space");
        assert_eq!(live(&open, cx), LiveState::Live);
        press_keys(cx, "f");
        assert_eq!(live(&open, cx), LiveState::Frozen);
        press_keys(cx, "space");
        press_keys(cx, "b");
        assert_eq!(live(&open, cx), LiveState::Blackout);
        assert!(reference_open(&open, cx), "the panel is passive");
    }

    #[gpui::test]
    fn escape_over_an_open_rail_overlay_closes_only_the_reference(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        open_overlay(&open, cx);
        press_keys(cx, "ctrl-/");
        assert!(reference_open(&open, cx));
        assert!(open.view.read_with(cx, |view, _| view.rail_open()));
        assert_eq!(
            focus_context_now(&open, cx),
            FocusContext::ShortcutReference
        );

        press_keys(cx, "escape");
        assert!(!reference_open(&open, cx));
        assert!(
            open.view.read_with(cx, |view, _| view.rail_open()),
            "the overlay stays"
        );
        assert_eq!(focus_context_now(&open, cx), FocusContext::Rail);

        press_keys(cx, "escape");
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        assert_eq!(focus_context_now(&open, cx), FocusContext::Timeline);
    }

    #[gpui::test]
    fn the_header_and_the_rail_footer_open_the_same_panel(cx: &mut TestAppContext) {
        // Wide: the rail is pinned, so both entry points are on screen.
        let (open, cx) = open(cx, SIZES[0]);
        bind_registry_keys(cx);
        open.view
            .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
        // Closing from the header returns to the Cues (the header button is
        // no place for the keys); from the rail footer, to the rail.
        for (selector, back) in [
            ("header-shortcuts", FocusContext::Timeline),
            ("rail-shortcuts", FocusContext::Rail),
        ] {
            click(cx, selector);
            assert!(reference_open(&open, cx), "{selector}");
            assert_eq!(
                focus_context_now(&open, cx),
                FocusContext::ShortcutReference,
                "{selector}"
            );
            click(cx, "shortcut-reference-close");
            assert!(!reference_open(&open, cx), "{selector}: close button");
            assert_eq!(focus_context_now(&open, cx), back, "{selector}");
        }
        // The footer also shows the `O` binding, from the registry.
        assert!(present(cx, "rail-focus-hint"));
    }

    /// Opened from the header and closed with the mouse or `Esc`, the panel
    /// leaves the arrows working on the Cues.
    #[gpui::test]
    fn the_header_entry_returns_the_keys_to_the_cues(cx: &mut TestAppContext) {
        for close in ["shortcut-reference-close", "escape"] {
            let (open, cx) = open(cx, SIZES[1]);
            bind_registry_keys(cx);
            click(cx, "header-shortcuts");
            assert!(reference_open(&open, cx), "{close}");
            if close == "escape" {
                press_keys(cx, "escape");
            } else {
                click(cx, close);
            }
            assert!(!reference_open(&open, cx), "{close}");
            let before = preview_index(&open, cx);
            press_keys(cx, "right");
            assert_eq!(preview_index(&open, cx), before + 1, "{close}");
        }
    }

    #[gpui::test]
    fn the_panel_is_bounded_to_the_viewport_at_every_size(cx: &mut TestAppContext) {
        for case in SIZES.into_iter().chain(SHORT) {
            let (open, cx) = open(cx, case);
            open.view
                .update_in(cx, |view, window, cx| view.open_reference(window, cx));
            cx.run_until_parked();
            let panel = bounds(cx, "shortcut-reference");
            assert!(inside(panel, case.0, case.1), "{case:?} {panel:?}");
            // The workspace exterior gutter stays at the right and the bottom.
            let gutter = case.2.apply(tokens::density::WORKSPACE_GUTTER_WIDE);
            assert!(right(panel) <= case.0 - gutter + 0.5, "{case:?} {panel:?}");
            assert!(bottom(panel) <= case.1 - gutter + 0.5, "{case:?} {panel:?}");
            let header = bounds(cx, "presenter-header");
            assert!(
                top(panel) >= bottom(header) - 0.5,
                "{case:?} below the header"
            );
        }
    }

    #[gpui::test]
    fn the_panel_closes_to_the_timeline_when_the_rail_it_came_from_is_gone(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        open_overlay(&open, cx);
        // The rail footer's entry takes focus (a control inside the rail), then
        // opens the panel.
        click(cx, "rail-shortcuts");
        assert!(reference_open(&open, cx));
        // The overlay goes away under the panel without moving focus.
        open.view.update(cx, |view, cx| {
            view.surfaces.close(Surface::RailOverlay);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        press_keys(cx, "escape");
        assert!(!reference_open(&open, cx));
        assert_eq!(
            focus_context_now(&open, cx),
            FocusContext::Timeline,
            "the control it came from is gone"
        );
        assert!(timeline_has_focus(&open, cx));
    }

    #[gpui::test]
    fn a_scrim_press_over_the_open_panel_closes_only_the_panel(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        open_overlay(&open, cx);
        press_keys(cx, "ctrl-/");
        assert!(reference_open(&open, cx));
        let rail = bounds(cx, "presenter-rail");
        let panel = bounds(cx, "shortcut-reference");
        // Below the panel, in the bottom gutter, clear of the rail.
        let at = gpui::point(
            panel.origin.x + panel.size.width / 2.0,
            panel.origin.y + panel.size.height + px(6.0),
        );
        assert!(!panel.contains(&at) && !rail.contains(&at), "{at:?}");
        let before = engine_state(&open, cx);
        cx.simulate_click(at, Modifiers::default());
        cx.run_until_parked();
        assert!(!reference_open(&open, cx), "the topmost surface closes");
        assert!(
            open.view.read_with(cx, |view, _| view.rail_open()),
            "the rail overlay stays"
        );
        assert_eq!(engine_state(&open, cx), before);
    }

    #[gpui::test]
    fn the_header_shortcuts_button_fits_the_header_at_every_size(cx: &mut TestAppContext) {
        for case in SIZES.into_iter().chain(SHORT) {
            let (_open, cx) = open(cx, case);
            let header = bounds(cx, "presenter-header");
            let entry = bounds(cx, "header-shortcuts");
            let grid = bounds(cx, "button-Grid");
            let list = bounds(cx, "button-List");
            assert!(inside(header, case.0, case.1), "{case:?} {header:?}");
            for (name, b) in [("entry", entry), ("Grid", grid), ("List", list)] {
                assert!(
                    left(b) >= left(header) - 0.5
                        && right(b) <= right(header) + 0.5
                        && top(b) >= top(header) - 0.5
                        && bottom(b) <= bottom(header) + 0.5,
                    "{case:?}: {name} {b:?} outside {header:?}"
                );
            }
            // Left to right: the entry, Grid, then List, none overlapping.
            assert!(right(entry) <= left(grid) + 0.5, "{case:?}");
            assert!(right(grid) <= left(list) + 0.5, "{case:?}");
        }
    }

    #[gpui::test]
    fn the_timeline_reports_the_preview_cue_as_its_active_descendant(cx: &mut TestAppContext) {
        // `Window::debug_a11y_tree_json` is `None` under the pinned test
        // platform (no `a11y_init`; see `GPUI-BASE-T0.4a.md`), so the tree
        // cannot be read here. This reads what the renderer applies to every
        // tile and row (`timeline::cue_semantics`) and whether the node is
        // drawn at all.
        for mode in [CueView::Grid, CueView::List] {
            let case = SIZES[1];
            let (open, cx) = open_with(
                cx,
                case,
                UiPreferences {
                    scale: case.2,
                    cue_view: mode,
                    ..UiPreferences::default()
                },
            );
            bind_registry_keys(cx);
            open.view
                .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
            let active = |open: &Open, cx: &mut VisualTestContext| {
                open.view.read_with(cx, |view, _| {
                    let model = view.timeline_model(&layout_at(case), false);
                    let at = timeline::active_descendant(&model);
                    let preview = view.engine.state().preview();
                    let semantics: Vec<_> = model
                        .groups
                        .iter()
                        .flat_map(|g| &g.cues)
                        .filter(|cue| cue.preview)
                        .map(timeline::cue_semantics)
                        .collect();
                    (at, preview, semantics)
                })
            };
            let (at, preview, semantics) = active(&open, cx);
            assert_eq!(at, Some(preview), "{mode:?}");
            assert_eq!(semantics.len(), 1, "only the Preview Cue is selected");
            assert!(semantics[0].selected && semantics[0].active_descendant);
            assert!(
                semantics[0]
                    .name
                    .starts_with(&format!("Cue {} of ", preview + 1))
            );
            assert!(
                semantics[0].name.ends_with(", Preview"),
                "{}",
                semantics[0].name
            );
            assert!(present(cx, cue(preview)), "its node is drawn");

            for keys in ["right", "down"] {
                press_keys(cx, keys);
                animate(&open, cx, 400);
                let (at, now, semantics) = active(&open, cx);
                assert_ne!(now, preview, "{keys} moved Preview");
                assert_eq!(at, Some(now), "{mode:?} {keys}");
                assert!(
                    semantics[0]
                        .name
                        .starts_with(&format!("Cue {} of ", now + 1))
                );
                assert!(present(cx, cue(now)), "{keys}: its node is drawn");
            }

            // Fold the group Preview is in: no Cue node is drawn for it, so the
            // report falls back to the timeline container.
            let (_, now, _) = active(&open, cx);
            let group = open.view.read_with(cx, |view, _| {
                let model = view.timeline_model(&layout_at(case), false);
                model
                    .groups
                    .iter()
                    .position(|g| g.cues.iter().any(|c| c.flat == now))
                    .unwrap()
            });
            open.view
                .update(cx, |view, cx| view.toggle_group(group, cx));
            animate(&open, cx, 400);
            let (at, _, _) = active(&open, cx);
            assert_eq!(at, None, "{mode:?}: folded group");
            assert!(!present(cx, cue(now)), "no dangling node");
        }
    }

    #[gpui::test]
    fn ctrl_slash_works_in_a_text_field_but_the_panel_still_holds_no_text_entry(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        focus_text_field(&open, cx);
        assert_eq!(focus_context_now(&open, cx), FocusContext::TextEntry);
        press_keys(cx, "ctrl-/");
        assert!(reference_open(&open, cx));
        press_keys(cx, "ctrl-/");
        assert!(!reference_open(&open, cx));
        assert_eq!(
            focus_context_now(&open, cx),
            FocusContext::TextEntry,
            "back in the field"
        );
    }

    #[gpui::test]
    fn text_entry_swallows_every_single_key_command(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        focus_text_field(&open, cx);
        let before = engine_state(&open, cx);
        for keys in ["f", "b", "o", "space", "right", "a", "home", "pagedown"] {
            press_keys(cx, keys);
            assert_eq!(engine_state(&open, cx), before, "{keys}");
            assert_eq!(
                open.view
                    .read_with(cx, |view, _| view.status().map(str::to_owned)),
                None,
                "{keys}"
            );
        }
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        // Leaving the field gives the keys back.
        open.view
            .update_in(cx, |view, window, cx| view.focus_timeline(window, cx));
        press_keys(cx, "right");
        assert_ne!(engine_state(&open, cx), before);
    }

    #[gpui::test]
    fn a_real_ui_field_takes_the_letters_and_no_engine_command_runs(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        run(&open, cx, Command::EnableOutput).unwrap();
        let field = focus_text_field(&open, cx);
        assert_eq!(focus_context_now(&open, cx), FocusContext::TextEntry);
        let before = engine_state(&open, cx);
        press_keys(cx, "f b o a space right");
        assert_eq!(engine_state(&open, cx), before);
        assert_eq!(cx.update(|_, cx| field.value(cx)).as_ref(), "fboa ");
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
    }

    /// The primary action replaces itself when pressed, so it hands focus to
    /// the Cues; otherwise the keys would go to a button that no longer exists.
    #[gpui::test]
    fn pressing_the_primary_action_returns_focus_to_the_cues(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        for label in ["button-Enable Output", "button-Take Preview Live"] {
            click(cx, label);
            assert!(timeline_has_focus(&open, cx), "after {label}");
        }
        assert_eq!(
            open.view
                .read_with(cx, |view, _| view.engine.state().live()),
            LiveState::Live
        );
    }

    /// Navigation keeps Preview's Cue on screen, scrolling down and back up.
    /// Let a follow land: the frame that sees Preview move, the glide's full
    /// run, and the frame drawn at the final offset.
    fn land(open: &Open, cx: &mut VisualTestContext) {
        animate(open, cx, 0);
        animate(
            open,
            cx,
            u64::from(tokens::motion::STANDARD_DURATION_MS) + 40,
        );
        animate(open, cx, 0);
    }

    fn centre_gap(cx: &mut VisualTestContext, flat: usize) -> f32 {
        let selector: &'static str = format!("cue-{flat}").leak();
        let cue = bounds(cx, selector);
        let timeline = bounds(cx, "cue-timeline");
        let middle = |b: gpui::Bounds<gpui::Pixels>| f32::from(b.top() + b.bottom()) / 2.0;
        (middle(cue) - middle(timeline)).abs()
    }

    /// Navigation glides Preview's Cue to the middle of the timeline, so what
    /// comes next is in sight below it; under reduced motion it is simply
    /// there.
    #[gpui::test]
    fn navigation_glides_preview_to_the_middle_of_the_timeline(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        // The first Cue of a later group of two or more, so its neighbour is
        // on the same row.
        let middle = open.view.read_with(cx, |view, _| {
            let snapshot = view.engine.snapshot();
            (6..snapshot.item_count() - 1)
                .find_map(|g| {
                    let (start, next) = (snapshot.item_start(g)?, snapshot.item_start(g + 1)?);
                    (next - start >= 2).then_some(start)
                })
                .expect("a later group with two Cues")
        });
        let start = timeline_offset(&open, cx);
        assert!(run(&open, cx, Command::GoTo(middle)).is_ok());
        animate(&open, cx, 0);
        animate(&open, cx, 40);
        let midway = timeline_offset(&open, cx);
        land(&open, cx);
        let end = timeline_offset(&open, cx);
        assert!(
            midway < start && midway > end,
            "the timeline glides: {start} -> {midway} -> {end}"
        );
        assert!(centre_gap(cx, middle) < 1.0, "Preview sits on the middle");

        // Along the same row nothing moves; the next row moves by one row.
        assert!(run(&open, cx, Command::GoTo(middle + 1)).is_ok());
        land(&open, cx);
        assert!((timeline_offset(&open, cx) - end).abs() < 0.5);

        cx.update(|_, cx| cx.set_reduce_motion(true));
        let later = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().item_start(10).unwrap());
        assert!(run(&open, cx, Command::GoTo(later)).is_ok());
        animate(&open, cx, 0);
        animate(&open, cx, 0);
        assert!(centre_gap(cx, later) < 1.0, "reduced motion: there at once");
    }

    /// Stepping within a row never moves the timeline, whether output is off,
    /// Live (where Program can stay behind on a frame while Preview is on a
    /// trigger, Wait or Hold Cue) or frozen: it moves only when Preview's
    /// row does.
    #[gpui::test]
    fn the_timeline_moves_only_when_preview_changes_row(cx: &mut TestAppContext) {
        for setup in [
            &[][..],
            &[Command::EnableOutput, Command::Take][..],
            &[Command::EnableOutput, Command::Take, Command::ToggleFreeze][..],
        ] {
            let (open, cx) = open(cx, SIZES[1]);
            for command in setup {
                assert!(run(&open, cx, *command).is_ok(), "{command:?}");
            }
            land(&open, cx);
            let last = open
                .view
                .read_with(cx, |view, _| view.engine.snapshot().cue_count() - 1);
            let row_top = |cx: &mut VisualTestContext, flat: usize| {
                let selector: &'static str = format!("cue-{flat}").leak();
                f32::from(bounds(cx, selector).top())
            };
            for flat in 1..=last {
                // Where Preview's row was before the move, in content terms.
                let before = timeline_offset(&open, cx);
                let same_row = (row_top(cx, flat) - row_top(cx, flat - 1)).abs() < 0.5;
                assert!(run(&open, cx, Command::GoTo(flat)).is_ok());
                land(&open, cx);
                if same_row {
                    assert!(
                        (timeline_offset(&open, cx) - before).abs() < 0.5,
                        "{setup:?}: Cue {flat} is on the row of {}, yet the timeline moved",
                        flat - 1
                    );
                }
            }
        }
    }

    /// A step within the row during a glide leaves the glide alone: it does
    /// not restart its curve towards the same place.
    #[gpui::test]
    fn a_same_row_step_during_a_glide_does_not_restart_it(cx: &mut TestAppContext) {
        let mut offsets = Vec::new();
        for interrupt in [false, true] {
            let (open, cx) = open(cx, SIZES[1]);
            let middle = open.view.read_with(cx, |view, _| {
                let snapshot = view.engine.snapshot();
                (6..snapshot.item_count() - 1)
                    .find_map(|g| {
                        let (start, next) = (snapshot.item_start(g)?, snapshot.item_start(g + 1)?);
                        (next - start >= 2).then_some(start)
                    })
                    .expect("a later group with two Cues")
            });
            assert!(run(&open, cx, Command::GoTo(middle)).is_ok());
            animate(&open, cx, 0);
            animate(&open, cx, 40);
            if interrupt {
                assert!(run(&open, cx, Command::GoTo(middle + 1)).is_ok());
            }
            animate(&open, cx, 0);
            animate(&open, cx, 40);
            offsets.push(timeline_offset(&open, cx));
        }
        assert!(
            (offsets[0] - offsets[1]).abs() < 0.5,
            "the glide runs on as if uninterrupted: {offsets:?}"
        );
    }

    /// The operator's own scroll during a glide wins.
    #[gpui::test]
    fn scrolling_during_a_glide_ends_it(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        let far = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().item_start(10).unwrap());
        assert!(run(&open, cx, Command::GoTo(far)).is_ok());
        animate(&open, cx, 0);
        animate(&open, cx, 40);
        open.view.update(cx, |view, _| {
            view.timeline_scroll
                .handle()
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(-10.0)))
        });
        land(&open, cx);
        assert!(
            (timeline_offset(&open, cx) + 10.0).abs() < 0.5,
            "the timeline stays where the operator put it"
        );
    }

    #[gpui::test]
    fn the_timeline_follows_preview_down_and_back_up(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        let last = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().cue_count() - 1);
        let inside = |cx: &mut VisualTestContext, flat: usize| {
            let selector: &'static str = format!("cue-{flat}").leak();
            let cue = bounds(cx, selector);
            let timeline = bounds(cx, "cue-timeline");
            cue.top() >= timeline.top() && cue.bottom() <= timeline.bottom()
        };
        for flat in 1..=last {
            assert!(run(&open, cx, Command::GoTo(flat)).is_ok());
            land(&open, cx);
            assert!(inside(cx, flat), "Cue {flat} is in view after Next");
        }
        assert!(run(&open, cx, Command::GoTo(0)).is_ok());
        land(&open, cx);
        assert!(
            inside(cx, 0),
            "back at the first Cue, the timeline is at the top"
        );

        // The operator's own scrolling is not undone while Preview stays put.
        open.view.update(cx, |view, cx| view.jump_to_group(14, cx));
        land(&open, cx);
        assert!(
            !inside(cx, 0),
            "a jump away from Preview stays where it went"
        );
    }

    /// While frozen, the held Program Cue is brought back into view with
    /// Preview when both fit. Stepping from Cue to Cue keeps both on screen by
    /// itself; this covers the operator having scrolled them away first.
    #[gpui::test]
    fn while_frozen_the_timeline_keeps_program_in_view_with_preview_if_both_fit(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[0]);
        let in_view = |cx: &mut VisualTestContext, flat: usize| {
            let selector: &'static str = format!("cue-{flat}").leak();
            let cue = bounds(cx, selector);
            let timeline = bounds(cx, "cue-timeline");
            cue.top() >= timeline.top() && cue.bottom() <= timeline.bottom()
        };
        // Program held on the first Cue of the Entrance song (group 2).
        let (held, last) = open.view.read_with(cx, |view, _| {
            let snapshot = view.engine.snapshot();
            (snapshot.item_start(2).unwrap(), snapshot.cue_count() - 1)
        });
        for command in [
            Command::EnableOutput,
            Command::GoTo(held),
            Command::Take,
            Command::ToggleFreeze,
        ] {
            assert!(run(&open, cx, command).is_ok(), "{command:?}");
        }
        land(&open, cx);

        // The operator scrolls far away, then steps Preview on.
        open.view.update(cx, |view, cx| view.jump_to_group(12, cx));
        land(&open, cx);
        assert!(!in_view(cx, held));
        // The next group's first Cue: below Program's rows, so following
        // Preview alone would leave Program above the viewport.
        let next = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().item_start(3).unwrap());
        assert!(run(&open, cx, Command::GoTo(next)).is_ok());
        land(&open, cx);
        assert!(in_view(cx, next), "Preview is in view");
        assert!(in_view(cx, held), "the held Program Cue came back with it");

        // Too far apart to fit together: Preview wins.
        assert!(run(&open, cx, Command::GoTo(last)).is_ok());
        land(&open, cx);
        assert!(in_view(cx, last), "Preview is in view");
        assert!(!in_view(cx, held), "Program gives way when both cannot fit");
        assert_eq!(
            open.view
                .read_with(cx, |view, _| view.engine.state().live()),
            LiveState::Frozen
        );
    }

    #[gpui::test]
    fn the_close_button_and_escape_both_close_the_overlay(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        open_overlay(&open, cx);
        click(cx, "button-Close Ordo");
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        open_overlay(&open, cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
    }

    fn pinned(open: &Open, cx: &VisualTestContext) -> bool {
        open.view.read_with(cx, |view, _| view.rail_is_pinned)
    }

    fn focus_timeline_now(open: &Open, cx: &mut VisualTestContext) {
        open.view
            .update_in(cx, |view, window, cx| view.timeline_focus.focus(window, cx));
    }

    #[gpui::test]
    fn o_from_the_timeline_opens_and_focuses_the_overlay_without_touching_the_pin(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        focus_timeline_now(&open, cx);
        let before = engine_state(&open, cx);
        let choice = open.view.read_with(cx, |view, _| view.rail_choice());
        assert!(!pinned(&open, cx));

        press_keys(cx, "o");
        assert!(open.view.read_with(cx, |view, _| view.rail_open()));
        assert_eq!(focus_context_now(&open, cx), FocusContext::Rail);
        assert!(!pinned(&open, cx));
        assert_eq!(
            open.view.read_with(cx, |view, _| view.rail_choice()),
            choice
        );
        assert_eq!(engine_state(&open, cx), before);
    }

    #[gpui::test]
    fn o_with_a_pinned_rail_focuses_it_and_keeps_it_pinned(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        bind_registry_keys(cx);
        assert!(pinned(&open, cx));
        focus_timeline_now(&open, cx);
        let before = engine_state(&open, cx);
        let offset = timeline_offset(&open, cx);

        press_keys(cx, "o");
        assert_eq!(focus_context_now(&open, cx), FocusContext::Rail);
        assert!(pinned(&open, cx), "O never changes the pin");
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        assert_eq!(engine_state(&open, cx), before);
        assert_eq!(timeline_offset(&open, cx), offset);
    }

    #[gpui::test]
    fn escape_closes_the_overlay_and_returns_to_the_timeline(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        focus_timeline_now(&open, cx);
        press_keys(cx, "o");
        let before = engine_state(&open, cx);
        let offset = timeline_offset(&open, cx);

        press_keys(cx, "escape");
        assert_eq!(timeline_offset(&open, cx), offset, "Esc never scrolls");
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        assert!(!present(cx, "presenter-rail"));
        assert!(timeline_has_focus(&open, cx));
        assert_eq!(
            engine_state(&open, cx),
            before,
            "Esc never touches the Engine"
        );

        // Nothing open: Esc from the Cues does nothing at all.
        press_keys(cx, "escape");
        assert!(timeline_has_focus(&open, cx));
        assert_eq!(engine_state(&open, cx), before);
    }

    #[gpui::test]
    fn escape_in_a_text_field_leaves_the_overlay_open(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        focus_timeline_now(&open, cx);
        press_keys(cx, "o");
        focus_text_field(&open, cx);
        press_keys(cx, "escape");
        assert!(open.view.read_with(cx, |view, _| view.rail_open()));
    }

    #[gpui::test]
    fn escape_with_the_overlay_open_and_focus_in_the_dock_closes_it(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        focus_timeline_now(&open, cx);
        press_keys(cx, "o");
        open.view
            .update_in(cx, |view, window, cx| view.dock_focus.focus(window, cx));
        assert!(open.view.read_with(cx, |view, _| view.rail_open()));
        press_keys(cx, "escape");
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        assert!(timeline_has_focus(&open, cx));
    }

    #[gpui::test]
    fn escape_from_a_header_control_returns_to_the_timeline(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        bind_registry_keys(cx);
        press_without_release(cx, "button-List");
        assert_eq!(focus_context_now(&open, cx), FocusContext::Elsewhere);
        let before = engine_state(&open, cx);
        press_keys(cx, "escape");
        assert!(timeline_has_focus(&open, cx));
        assert_eq!(engine_state(&open, cx), before);
    }

    #[gpui::test]
    fn escape_on_a_pinned_rail_or_the_dock_moves_focus_without_collapsing(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        bind_registry_keys(cx);
        focus_timeline_now(&open, cx);
        let before = engine_state(&open, cx);
        press_keys(cx, "o");
        assert_eq!(focus_context_now(&open, cx), FocusContext::Rail);
        press_keys(cx, "escape");
        assert!(pinned(&open, cx));
        assert!(present(cx, "presenter-rail"));
        assert!(timeline_has_focus(&open, cx));

        open.view
            .update_in(cx, |view, window, cx| view.dock_focus.focus(window, cx));
        press_keys(cx, "escape");
        assert!(timeline_has_focus(&open, cx));
        assert_eq!(engine_state(&open, cx), before);
    }

    /// F6 from the Keyboard shortcuts panel closes it and moves on from
    /// where focus was before it opened.
    #[gpui::test]
    fn f6_from_the_shortcut_reference_closes_it_and_moves_on(cx: &mut TestAppContext) {
        for (keys, expected) in [("f6", FocusContext::Dock), ("shift-f6", FocusContext::Rail)] {
            let (open, cx) = open(cx, SIZES[0]);
            bind_registry_keys(cx);
            focus_timeline_now(&open, cx);
            cx.run_until_parked();
            press_keys(cx, "ctrl-/");
            assert!(open.view.read_with(cx, |view, _| view.reference_open()));
            press_keys(cx, keys);
            assert!(
                !open.view.read_with(cx, |view, _| view.reference_open()),
                "{keys}"
            );
            assert_eq!(focus_context_now(&open, cx), expected, "{keys}");
        }
    }

    #[gpui::test]
    fn f6_cycles_the_visible_regions_and_shift_f6_reverses(cx: &mut TestAppContext) {
        // Pinned rail: rail, timeline, output controls.
        let (open, cx) = open(cx, SIZES[0]);
        bind_registry_keys(cx);
        focus_timeline_now(&open, cx);
        let before = engine_state(&open, cx);
        let expected = [
            FocusContext::Dock,
            FocusContext::Rail,
            FocusContext::Timeline,
            FocusContext::Dock,
        ];
        for want in expected {
            press_keys(cx, "f6");
            assert_eq!(focus_context_now(&open, cx), want);
        }
        for want in [
            FocusContext::Timeline,
            FocusContext::Rail,
            FocusContext::Dock,
        ] {
            press_keys(cx, "shift-f6");
            assert_eq!(focus_context_now(&open, cx), want);
        }
        assert_eq!(
            engine_state(&open, cx),
            before,
            "F6 never reaches the Engine"
        );
    }

    #[gpui::test]
    fn f6_skips_a_closed_unpinned_rail_and_leaving_an_open_overlay_closes_it(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[2]);
        bind_registry_keys(cx);
        focus_timeline_now(&open, cx);
        let before = engine_state(&open, cx);
        for want in [
            FocusContext::Dock,
            FocusContext::Timeline,
            FocusContext::Dock,
        ] {
            press_keys(cx, "f6");
            assert_eq!(focus_context_now(&open, cx), want, "closed rail skipped");
        }
        for want in [FocusContext::Timeline, FocusContext::Dock] {
            press_keys(cx, "shift-f6");
            assert_eq!(focus_context_now(&open, cx), want);
        }

        // With the overlay open, F6 or Shift+F6 from inside it leaves it, and
        // the overlay closes: no keys act under a scrim.
        for (key, want) in [
            ("f6", FocusContext::Timeline),
            ("shift-f6", FocusContext::Dock),
        ] {
            focus_timeline_now(&open, cx);
            open.view
                .update_in(cx, |view, window, cx| view.open_rail(window, cx));
            cx.run_until_parked();
            assert_eq!(focus_context_now(&open, cx), FocusContext::Rail);
            press_keys(cx, key);
            assert_eq!(focus_context_now(&open, cx), want, "{key}");
            assert!(
                !open.view.read_with(cx, |view, _| view.rail_open()),
                "{key}"
            );
        }
        assert_eq!(engine_state(&open, cx), before);
    }

    #[gpui::test]
    fn the_overlay_slides_in_and_settles_at_the_left_edge(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[1]);
        click(cx, "ordo-tab");
        let start = bounds(cx, "presenter-rail");
        assert!(left(start) < -1.0, "starts off the edge, found {start:?}");
        animate(&open, cx, 400);
        let panel = bounds(cx, "presenter-rail");
        assert!(left(panel).abs() < 0.5, "settled at x=0, found {panel:?}");
        assert!((f32::from(panel.size.width) - 200.0).abs() < 0.5);
    }

    #[gpui::test]
    fn scrolling_the_timeline_never_moves_the_output_dock(cx: &mut TestAppContext) {
        for case in SIZES {
            let (width, height, _) = case;
            let (open, cx) = open(cx, case);
            let before: Vec<_> = DOCK_ELEMENTS.iter().map(|id| bounds(cx, id)).collect();
            open.view.update(cx, |view, cx| {
                view.timeline_scroll.handle().scroll_to_bottom();
                cx.notify();
            });
            cx.run_until_parked();
            assert!(
                timeline_offset(&open, cx) < -1.0,
                "{case:?}: the timeline did scroll"
            );
            for (id, was) in DOCK_ELEMENTS.iter().zip(before) {
                let now = bounds(cx, id);
                assert_eq!(now, was, "{case:?}: {id} moved");
                assert!(inside(now, width, height));
            }
        }
    }

    #[gpui::test]
    fn program_and_preview_marks_follow_the_engine_in_grid_and_in_list(cx: &mut TestAppContext) {
        for mode in [CueView::Grid, CueView::List] {
            let (open, cx) = open(cx, SIZES[0]);
            open.view.update(cx, |view, cx| view.set_cue_view(mode, cx));
            cx.run_until_parked();
            let program_mark = |flat: usize| sel(format!("cue-{flat}-program-mark"));
            let preview_mark = |flat: usize| sel(format!("cue-{flat}-preview-mark"));

            assert!(!present(cx, program_mark(2)), "nothing is on air yet");
            assert!(
                present(cx, preview_mark(0)),
                "Preview starts on the first Cue"
            );

            assert!(run(&open, cx, Command::EnableOutput).is_ok());
            assert!(run(&open, cx, Command::GoTo(2)).is_ok());
            assert!(run(&open, cx, Command::Take).is_ok());
            settle(&open, cx, 1000);
            assert!(
                present(cx, program_mark(2)),
                "{mode:?}: Program mark on cue 2"
            );
            assert!(
                present(cx, preview_mark(2)),
                "linked Live: Preview on the same Cue"
            );
            assert!(!present(cx, preview_mark(0)), "the bar left the first Cue");

            // Frozen: Program stays on 2 while Preview moves on.
            assert!(run(&open, cx, Command::ToggleFreeze).is_ok());
            assert!(run(&open, cx, Command::GoTo(5)).is_ok());
            assert!(present(cx, program_mark(2)), "{mode:?}: held on cue 2");
            assert!(!present(cx, preview_mark(2)));
            assert!(present(cx, preview_mark(5)), "{mode:?}: Preview on cue 5");
            assert!(!present(cx, program_mark(5)));
        }
    }

    #[gpui::test]
    fn clicking_a_cue_goes_to_it_through_the_command_seam_without_scrolling(
        cx: &mut TestAppContext,
    ) {
        for case in SIZES {
            let (open, cx) = open(cx, case);
            let revision = engine_state(&open, cx).revision();
            // Cue 1 opens group 1. Bring that group to the top so its tile
            // is on screen at every size, then press near the tile's corner
            // (at 200% only its upper part fits).
            open.view.update(cx, |view, cx| view.jump_to_group(1, cx));
            cx.run_until_parked();
            let offset = timeline_offset(&open, cx);
            click_corner(cx, "cue-1");
            assert_eq!(
                open.view
                    .read_with(cx, |view, _| view.engine.state().preview()),
                1,
                "{case:?}: the click chose the Cue"
            );
            assert!(engine_state(&open, cx).revision() > revision);
            assert!(
                (timeline_offset(&open, cx) - offset).abs() < 0.5,
                "{case:?}: navigation never scrolls the timeline"
            );
        }
    }

    #[gpui::test]
    fn folding_a_group_hides_its_cues_and_leaves_the_engine_alone(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        let before = engine_state(&open, cx);
        assert!(present(cx, "cue-1"));
        click(cx, "button-Announcements");
        animate(&open, cx, 400);
        assert!(!present(cx, "cue-1"), "the folded group shows no Cue");
        assert!(present(cx, "group-1-head"), "its header stays");
        assert!(present(cx, "cue-4"), "other groups are untouched");
        assert_eq!(engine_state(&open, cx), before);
        click(cx, "button-Announcements");
        animate(&open, cx, 400);
        assert!(present(cx, "cue-1"));
    }

    #[gpui::test]
    fn walking_the_whole_mass_never_exceeds_the_frame_cache_bound(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        let count = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().cue_count());
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        for flat in 0..count {
            assert!(run(&open, cx, Command::GoTo(flat)).is_ok());
            let held = open.view.read_with(cx, |view, _| view.frames.cache_len());
            assert!(
                held <= rendering::FRAME_CAPACITY,
                "cue {flat}: {held} cached frames"
            );
        }
        // Thumbnails of Cues Preview has visited are composed and kept.
        assert!(
            open.view
                .read_with(cx, |view, _| view.frames.composed_count())
                > 5
        );
        // Visible requests join the walk: scrolling to every group, in Grid,
        // asks for its thumbnails too, and the bound still holds.
        let groups = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().item_count());
        for group in (0..groups).chain((0..groups).rev()) {
            open.view
                .update(cx, |view, cx| view.jump_to_group(group, cx));
            cx.run_until_parked();
            animate(&open, cx, 0);
            let held = open.view.read_with(cx, |view, _| view.frames.cache_len());
            assert!(
                held <= rendering::FRAME_CAPACITY,
                "group {group}: {held} cached frames"
            );
        }
    }

    #[gpui::test]
    fn walking_the_mass_in_grid_keeps_decoded_images_within_the_cached_frames(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[0]);
        let count = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().cue_count());
        let groups = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().item_count());
        let mut decoded_some = false;
        let mut check = |open: &Open, cx: &mut VisualTestContext, step: &str| {
            let (full, thumb, referenced) = open.view.read_with(cx, |view, _| {
                let (full, thumb) = view.frames.image_counts();
                (full, thumb, view.frames.referenced_image_count())
            });
            assert!(
                full + thumb <= referenced,
                "{step}: {full} full + {thumb} thumbnail images for {referenced} assets"
            );
            assert!(full <= rendering::FRAME_CAPACITY, "{step}: {full} full");
            decoded_some |= full + thumb > 0;
        };
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        for group in (0..groups).chain((0..groups).rev()) {
            open.view
                .update(cx, |view, cx| view.jump_to_group(group, cx));
            cx.run_until_parked();
            animate(&open, cx, 0);
            check(&open, cx, &format!("group {group}"));
        }
        for flat in 0..count {
            assert!(run(&open, cx, Command::GoTo(flat)).is_ok());
            cx.run_until_parked();
            check(&open, cx, &format!("cue {flat}"));
        }
        assert!(decoded_some, "the walk must decode the fixture's image");
    }

    #[gpui::test]
    fn releasing_the_view_images_drops_every_decoded_image(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        let snapshot = fixture_snapshot();
        let image_frame = frames::test_support::frame_ids(&snapshot, true)[0];
        let count = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().cue_count());
        let flat = (0..count)
            .find(|flat| {
                open.view.read_with(cx, |view, _| {
                    matches!(
                        view.engine.snapshot().cue(*flat).map(|cue| cue.behavior()),
                        Some(CueBehavior::Project { frame, .. }) if *frame == image_frame
                    )
                })
            })
            .expect("the fixture has an image cue");
        assert!(run(&open, cx, Command::GoTo(flat)).is_ok());
        cx.run_until_parked();
        let held = open
            .view
            .read_with(cx, |view, _| view.frames.image_counts());
        assert_eq!(held.0 + held.1, 1, "the image cue decoded its image");

        // The on_release hook is `release_images`. The test window's root
        // keeps the entity alive for the whole test, so the hook itself
        // cannot be observed firing; its body is what is checked here.
        open.view.update(cx, |view, cx| view.release_images(cx));
        let after = open
            .view
            .read_with(cx, |view, _| view.frames.image_counts());
        assert_eq!(after, (0, 0), "every image is released");
    }

    /// Every Project Cue's frame, by Cue, with whether it is composed.
    fn tile_frames(open: &Open, cx: &VisualTestContext, cues: &[usize]) -> Vec<(usize, bool)> {
        open.view.read_with(cx, |view, _| {
            cues.iter()
                .map(|flat| {
                    let CueBehavior::Project { frame, .. } =
                        view.engine.snapshot().cue(*flat).unwrap().behavior()
                    else {
                        panic!("cue {flat} is not a slide");
                    };
                    (
                        *flat,
                        matches!(view.frames.peek(*frame), FrameView::Ready { .. }),
                    )
                })
                .collect()
        })
    }

    #[gpui::test]
    fn grid_thumbnails_are_composed_for_groups_far_from_preview(cx: &mut TestAppContext) {
        for case in SIZES {
            let (open, cx) = open(cx, case);
            // Preview sits on cue 0: the last group's slides (cues 31 and 32)
            // are nowhere near Program, Preview or the preload radius.
            let far = [31, 32];
            open.view.update(cx, |view, cx| view.jump_to_group(14, cx));
            cx.run_until_parked();
            animate(&open, cx, 0);
            cx.run_until_parked();
            for (flat, ready) in tile_frames(&open, cx, &far) {
                assert!(ready, "{case:?}: cue {flat} is still black");
            }
            let held = open.view.read_with(cx, |view, _| view.frames.cache_len());
            assert!(
                held <= rendering::FRAME_CAPACITY + 2,
                "{held} cached frames"
            );
            // The tile paints the thumbnail, not a black box: the viewport
            // draws glyphs for a ready frame.
            assert!(present(cx, "cue-31-frame"));
        }
    }

    #[gpui::test]
    fn the_visible_set_follows_the_groups_on_screen_nearest_first_and_list_keeps_to_the_screen(
        cx: &mut TestAppContext,
    ) {
        let (open, cx) = open(cx, SIZES[0]);
        open.view.update(cx, |view, cx| view.jump_to_group(14, cx));
        cx.run_until_parked();
        animate(&open, cx, 0);
        let (first, wanted, list) = open.view.update(cx, |view, cx| {
            view.set_cue_view(CueView::List, cx);
            let list = view.visible_frames((14, 14));
            view.set_cue_view(CueView::Grid, cx);
            (view.visible_groups(), view.visible_frames((14, 14)), list)
        });
        assert!(first.1 == 14, "the last group is on screen: {first:?}");
        let last_group_frames = open.view.read_with(cx, |view, _| {
            let snapshot = view.engine.snapshot();
            [31, 32]
                .map(|flat| match snapshot.cue(flat).unwrap().behavior() {
                    CueBehavior::Project { frame, .. } => *frame,
                    _ => panic!("slide"),
                })
                .to_vec()
        });
        assert_eq!(
            wanted[..2],
            last_group_frames[..],
            "the group on screen comes first"
        );
        assert!(wanted.len() > 2, "Grid reaches out to the neighbours");
        assert_eq!(
            list, last_group_frames,
            "List wants the small thumbnails of the groups on screen only"
        );
    }

    /// Hold every compose job started from now on for `ms` of executor time.
    fn hold_jobs(open: &Open, cx: &mut VisualTestContext, ms: u64) {
        open.view.update(cx, |view, _| {
            view.frames.set_hold(Duration::from_millis(ms));
        });
    }

    /// Move both the injected clock and the executor clock to `at`, which
    /// must be later than the last call, and let everything that woke run.
    fn advance_to(open: &Open, cx: &mut VisualTestContext, at: u64) {
        let step = at - open.now.get();
        open.now.set(at);
        cx.executor().advance_clock(Duration::from_millis(step));
        cx.run_until_parked();
    }

    /// A slow-frame fixture: List view (only the groups on screen compose
    /// thumbnails, nothing ahead) and a Cue far from the start (off screen and
    /// outside the preload radius), whose frame is therefore still to be
    /// composed when Preview goes there.
    fn open_for_slow_frames(cx: &mut TestAppContext) -> (Open, &mut VisualTestContext, usize) {
        let (open, cx) = open_with(
            cx,
            SIZES[0],
            UiPreferences {
                cue_view: CueView::List,
                ..UiPreferences::default()
            },
        );
        let far = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().cue_count() - 1);
        (open, cx, far)
    }

    fn status_line(open: &Open, cx: &VisualTestContext) -> String {
        open.view.read_with(cx, |view, _| {
            view.controls(Millis(open.now.get())).status.to_string()
        })
    }

    #[gpui::test]
    fn a_slow_preview_frame_shows_the_badge_at_250_ms_and_keeps_the_old_frame(
        cx: &mut TestAppContext,
    ) {
        let (open, cx, far) = open_for_slow_frames(cx);
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        let before = open
            .view
            .read_with(cx, |view, _| view.last_preview.frame.clone())
            .expect("Preview drew a frame");

        hold_jobs(&open, cx, 1000);
        assert!(run(&open, cx, Command::GoTo(far)).is_ok());
        assert_eq!(open.now.get(), 0);
        assert!(cx.debug_bounds("monitor-preview-preparing").is_none());

        advance_to(&open, cx, 249);
        assert!(
            cx.debug_bounds("monitor-preview-preparing").is_none(),
            "no badge at 249 ms"
        );

        advance_to(&open, cx, 250);
        assert!(
            cx.debug_bounds("monitor-preview-preparing").is_some(),
            "the timer woke the view and the badge is up at 250 ms"
        );
        assert!(cx.debug_bounds("monitor-program-preparing").is_none());
        assert_eq!(status_line(&open, cx), "Preview is preparing");
        let during = open
            .view
            .read_with(cx, |view, _| view.last_preview.frame.clone())
            .expect("Preview still draws a frame");
        assert!(Arc::ptr_eq(&before, &during), "the previous frame stays");

        advance_to(&open, cx, 1000);
        assert!(
            cx.debug_bounds("monitor-preview-preparing").is_none(),
            "the badge goes when the frame is ready"
        );
        assert_eq!(status_line(&open, cx), "Output enabled");
        let after = open
            .view
            .read_with(cx, |view, _| view.last_preview.frame.clone())
            .expect("Preview draws the new frame");
        assert!(!Arc::ptr_eq(&before, &after), "the new frame shows");
    }

    #[gpui::test]
    fn a_slow_program_frame_shows_its_own_badge_and_status(cx: &mut TestAppContext) {
        let (open, cx, far) = open_for_slow_frames(cx);
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        assert!(run(&open, cx, Command::Take).is_ok());
        settle(&open, cx, 1000);
        let before = open
            .view
            .read_with(cx, |view, _| view.last_program.frame.clone())
            .expect("Program drew a frame after Take");

        hold_jobs(&open, cx, 1000);
        assert!(run(&open, cx, Command::GoTo(far)).is_ok());
        advance_to(&open, cx, 1249);
        assert!(cx.debug_bounds("monitor-program-preparing").is_none());
        advance_to(&open, cx, 1250);
        assert!(cx.debug_bounds("monitor-program-preparing").is_some());
        assert_eq!(
            status_line(&open, cx),
            "Program is preparing the next slide"
        );
        let during = open
            .view
            .read_with(cx, |view, _| view.last_program.frame.clone())
            .expect("Program still draws a frame");
        assert!(Arc::ptr_eq(&before, &during), "the previous frame stays");

        advance_to(&open, cx, 2250);
        assert!(cx.debug_bounds("monitor-program-preparing").is_none());
        let after = open
            .view
            .read_with(cx, |view, _| view.last_program.frame.clone())
            .expect("Program draws the new frame");
        assert!(!Arc::ptr_eq(&before, &after), "the new frame shows");
    }

    #[gpui::test]
    fn fast_navigation_never_shows_the_badge(cx: &mut TestAppContext) {
        let (open, cx, far) = open_for_slow_frames(cx);
        assert!(run(&open, cx, Command::EnableOutput).is_ok());
        assert!(run(&open, cx, Command::Take).is_ok());
        // Jobs take 200 ms: slow enough to be pending, under the threshold.
        hold_jobs(&open, cx, 200);
        let middle = far / 2;
        let mut at = 0;
        for flat in [far, middle] {
            assert!(run(&open, cx, Command::GoTo(flat)).is_ok());
            let started = at;
            for step in [100, 199, 200, 300] {
                at = started + step;
                advance_to(&open, cx, at);
                assert!(cx.debug_bounds("monitor-preview-preparing").is_none());
                assert!(cx.debug_bounds("monitor-program-preparing").is_none());
            }
        }
    }

    /// A view that draws the live viewport alone, as the congregation output
    /// does.
    struct BareViewport(UiEnvironment);

    impl Render for BareViewport {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            crate::canonical_views::live_viewport_fading(None, None, 320.0, None, 1.0, self.0)
        }
    }

    #[gpui::test]
    fn the_badge_belongs_to_the_operator_monitor_and_not_the_viewport(cx: &mut TestAppContext) {
        let (open, cx, far) = open_for_slow_frames(cx);
        hold_jobs(&open, cx, 1000);
        assert!(run(&open, cx, Command::GoTo(far)).is_ok());
        advance_to(&open, cx, 250);
        let badge = bounds(cx, "monitor-preview-preparing");
        let monitor = bounds(cx, "monitor-preview");
        assert!(
            left(badge) >= left(monitor) - 0.5
                && right(badge) <= right(monitor) + 0.5
                && top(badge) >= top(monitor) - 0.5
                && bottom(badge) <= bottom(monitor) + 0.5,
            "{badge:?} inside {monitor:?}"
        );
        assert!(cx.debug_bounds("monitor-program-preparing").is_none());
    }

    #[gpui::test]
    fn the_live_viewport_alone_draws_no_badge(cx: &mut TestAppContext) {
        cx.update(ui::init).unwrap();
        let (_, cx) =
            cx.add_window_view(|_, _| BareViewport(environment(InterfaceScale::Percent100)));
        cx.run_until_parked();
        for selector in ["monitor-program-preparing", "monitor-preview-preparing"] {
            assert!(cx.debug_bounds(selector).is_none(), "{selector}");
        }
    }

    // --- T1.5c: auto-play controls ----------------------------------------

    fn auto_play(
        open: &Open,
        cx: &VisualTestContext,
    ) -> Option<presentation::state::AutoPlayScope> {
        open.view.read_with(cx, |view, _| {
            view.engine.state().auto_play().map(|run| run.scope)
        })
    }

    fn status(open: &Open, cx: &VisualTestContext) -> Option<String> {
        open.view
            .read_with(cx, |view, _| view.status().map(str::to_owned))
    }

    fn timeline_at(open: &Open, cx: &mut VisualTestContext, flat: usize) {
        bind_registry_keys(cx);
        run(open, cx, Command::GoTo(flat)).unwrap();
        focus_timeline_now(open, cx);
    }

    fn menu_open(cx: &mut VisualTestContext) -> bool {
        cx.debug_bounds("choice-0").is_some()
    }

    /// The Auto-play trigger spans its slot, as the primary button spans the
    /// Side column, in both Dock layouts and at both scales.
    #[gpui::test]
    fn the_auto_play_trigger_fills_its_slot(cx: &mut TestAppContext) {
        for size in SIZES.into_iter().chain(SHORT) {
            let (_open, cx) = open(cx, size);
            let slot = bounds(cx, "auto-play-control");
            let trigger = bounds(cx, "button-Auto-play off");
            let gap = f32::from(slot.size.width - trigger.size.width);
            assert!(
                gap.abs() < 1.0,
                "{size:?}: slot {slot:?}, trigger {trigger:?}"
            );
        }
    }

    #[gpui::test]
    fn a_toggles_auto_play_on_an_eligible_item_from_the_timeline(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 1);
        assert_eq!(auto_play(&open, cx), None);
        assert!(cx.debug_bounds("button-Auto-play off").is_some());

        press_keys(cx, "a");
        assert!(matches!(
            auto_play(&open, cx),
            Some(presentation::state::AutoPlayScope::Item(_))
        ));
        assert_eq!(status(&open, cx), None);
        assert!(
            cx.debug_bounds("button-Auto-play: this item").is_some(),
            "the state is said in words"
        );
        press_keys(cx, "a");
        assert_eq!(auto_play(&open, cx), None);
        assert!(cx.debug_bounds("button-Auto-play off").is_some());
    }

    /// While a run is going Program shows AUTO with a countdown beside its
    /// state word and a progress bar that moves on its own; both go when the
    /// run stops, and so does the redraw task.
    #[gpui::test]
    fn program_shows_the_auto_play_countdown_and_progress(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 1);
        assert!(cx.debug_bounds("program-auto-play").is_none());
        press_keys(cx, "a");
        assert!(cx.debug_bounds("program-auto-play").is_some(), "badge");
        let duration = open.view.read_with(cx, |view, _| {
            view.engine
                .state()
                .auto_play()
                .and_then(|run| run.duration_ms)
        });
        let duration = duration.expect("the fixture's Cue 1 has a duration");
        let start = bounds(cx, "program-auto-play-progress").size.width;
        // No Engine change and no explicit redraw: only the ticker.
        advance_to(&open, cx, open.now.get() + duration / 2);
        let later = bounds(cx, "program-auto-play-progress").size.width;
        assert!(later > start, "the bar moved: {start:?} -> {later:?}");
        press_keys(cx, "a");
        assert!(cx.debug_bounds("program-auto-play").is_none(), "badge gone");
        assert!(
            cx.debug_bounds("program-auto-play-progress").is_none(),
            "bar gone"
        );
        assert!(
            open.view
                .read_with(cx, |view, _| view.auto_play_ticker.is_none())
        );
    }

    /// The bar is redrawn when it would grow by a pixel or the countdown's
    /// second changes, never faster than about one 60 Hz frame, and not at
    /// all on a Cue with nothing to count.
    #[test]
    fn the_auto_play_indicator_redraws_per_pixel_or_second() {
        // 10 s across 400 px: a pixel every 25 ms.
        assert_eq!(auto_play_redraw_ms(Some(9_500), 10_000, 400.0), Some(25));
        // 60 s across 400 px: 150 ms a pixel, but the second turns sooner.
        assert_eq!(auto_play_redraw_ms(Some(30_100), 60_000, 400.0), Some(100));
        assert_eq!(auto_play_redraw_ms(Some(30_000), 60_000, 400.0), Some(150));
        // 2 s across 1000 device px: 2 ms a pixel, held to one frame.
        assert_eq!(
            auto_play_redraw_ms(Some(1_500), 2_000, 1_000.0),
            Some(AUTO_PLAY_MIN_REDRAW_MS)
        );
        // Before the first draw the bar's length is unknown: the second rules.
        assert_eq!(auto_play_redraw_ms(Some(4_250), 5_000, 0.0), Some(250));
        assert_eq!(auto_play_redraw_ms(None, 5_000, 400.0), None);
    }

    #[gpui::test]
    fn a_on_liturgical_content_explains_the_refusal_and_changes_nothing(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 0);
        let before = engine_state(&open, cx);
        press_keys(cx, "a");
        assert_eq!(engine_state(&open, cx), before);
        let refusal = Refusal::AutoPlayIneligible {
            reason: AutoPlayRefusal::NotGeneralPresentation,
        };
        assert_eq!(status(&open, cx), Some(refusal_text(refusal).to_owned()));
        let text = refusal_text(refusal);
        assert!(text.contains("General Presentations") && text.contains("Next"));
        assert!(!text.contains("Mass text"));
    }

    fn menu_labels(open: &Open, cx: &VisualTestContext) -> Vec<&'static str> {
        open.view.read_with(cx, |view, _| {
            view.auto_play_model(Millis(0))
                .entries
                .iter()
                .map(|entry| entry.label)
                .collect()
        })
    }

    fn width(b: Bounds<Pixels>) -> f32 {
        right(b) - left(b)
    }

    #[gpui::test]
    fn the_menu_opens_from_the_dock_loops_the_section_and_stops(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 1);
        run(&open, cx, Command::Last).unwrap();
        let program = engine_state(&open, cx).program();
        assert!(!menu_open(cx));

        click(cx, "button-Auto-play off");
        assert!(menu_open(cx), "the menu is open");
        assert_eq!(auto_play(&open, cx), None, "opening changes nothing");

        // Start for this item.
        click(cx, "choice-0");
        assert!(matches!(
            auto_play(&open, cx),
            Some(presentation::state::AutoPlayScope::Item(_))
        ));
        assert!(!menu_open(cx), "choosing closes the menu");
        assert_eq!(focus_context_now(&open, cx), FocusContext::Dock);

        // The same entry now stops this item's run, and there is one Stop.
        click(cx, "button-Auto-play: this item");
        assert_eq!(
            menu_labels(&open, cx),
            ["Stop auto-play for this item", "Loop this section"]
        );
        click(cx, "choice-0");
        assert_eq!(auto_play(&open, cx), None);
        click(cx, "button-Auto-play off");
        click(cx, "choice-0");

        // Loop this section widens the run.
        click(cx, "button-Auto-play: this item");
        click(cx, "choice-1");
        assert!(matches!(
            auto_play(&open, cx),
            Some(presentation::state::AutoPlayScope::Section(_))
        ));
        assert!(cx.debug_bounds("button-Auto-play: this section").is_some());
        assert_eq!(focus_context_now(&open, cx), FocusContext::Dock);

        // The section already loops, so Loop is Unavailable and Stop follows it.
        click(cx, "button-Auto-play: this section");
        click(cx, "choice-0");
        assert!(matches!(
            auto_play(&open, cx),
            Some(presentation::state::AutoPlayScope::Section(_))
        ));
        assert!(menu_open(cx), "an Unavailable entry does nothing");
        click(cx, "choice-1");
        assert_eq!(auto_play(&open, cx), None);
        assert_eq!(engine_state(&open, cx).program(), program);
    }

    #[gpui::test]
    fn refused_menu_entries_are_unavailable_and_say_why(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 0);
        let before = engine_state(&open, cx);
        let (start, looping, stop) = open.view.read_with(cx, |view, _| {
            let model = view.auto_play_model(Millis(0));
            let reason = |i: usize| {
                model.entries[i]
                    .availability
                    .reason()
                    .map(|reason| reason.to_string())
            };
            (reason(0), reason(1), reason(2))
        });
        let ineligible = |reason| refusal_text(Refusal::AutoPlayIneligible { reason }).to_owned();
        assert_eq!(
            start,
            Some(ineligible(AutoPlayRefusal::NotGeneralPresentation))
        );
        assert_eq!(
            looping,
            Some(ineligible(AutoPlayRefusal::SectionHasIneligibleItem))
        );
        assert_eq!(stop.as_deref(), Some("Auto-play is not running"));

        click(cx, "button-Auto-play off");
        assert!(menu_open(cx));
        assert!(cx.debug_bounds("choice-unavailable").is_some());
        for entry in ["choice-0", "choice-1", "choice-2"] {
            click(cx, entry);
            assert_eq!(engine_state(&open, cx), before, "{entry} does nothing");
        }
    }

    #[gpui::test]
    fn escape_with_the_menu_open_closes_only_the_menu(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 1);
        click(cx, "button-Auto-play off");
        assert!(menu_open(cx));
        let before = engine_state(&open, cx);
        let choice = open.view.read_with(cx, |view, _| view.rail_choice());

        press_keys(cx, "escape");
        assert!(!menu_open(cx), "the menu closed");
        assert_eq!(
            focus_context_now(&open, cx),
            FocusContext::Dock,
            "focus returns to the Dock control, not the Cues"
        );
        assert_eq!(engine_state(&open, cx), before);
        assert_eq!(
            open.view.read_with(cx, |view, _| view.rail_choice()),
            choice
        );
        assert!(!open.view.read_with(cx, |view, _| view.rail_open()));
        assert!(unhandled_keys(&open, cx).iter().all(|key| key != "escape"));

        // With nothing open, Esc then brings focus back to the Cues.
        press_keys(cx, "escape");
        assert_eq!(focus_context_now(&open, cx), FocusContext::Timeline);
    }

    #[gpui::test]
    fn a_section_loop_says_it_is_already_looping_and_keeps_one_stop(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 1);
        run(&open, cx, Command::Last).unwrap();
        run(&open, cx, Command::LoopSection).unwrap();
        let (labels, looping, stop) = open.view.read_with(cx, |view, _| {
            let model = view.auto_play_model(Millis(0));
            (
                model.entries.iter().map(|e| e.label).collect::<Vec<_>>(),
                model.entries[0]
                    .availability
                    .reason()
                    .map(|r| r.to_string()),
                model.entries[1].availability.is_disabled(),
            )
        });
        assert_eq!(labels, ["Loop this section", "Stop auto-play"]);
        assert_eq!(looping.as_deref(), Some("This section is already looping"));
        assert!(!stop);
    }

    #[gpui::test]
    fn a_change_of_entries_under_an_open_menu_keeps_focus_in_the_dock(cx: &mut TestAppContext) {
        let (open, cx) = open(cx, SIZES[0]);
        timeline_at(&open, cx, 1);
        run(&open, cx, Command::EnableOutput).unwrap();
        run(&open, cx, Command::Take).unwrap();
        click(cx, "button-Auto-play off");
        assert!(menu_open(cx));

        // Freeze turns Start and Loop into refusals, so the entries change
        // while the menu is open.
        press_keys(cx, "f");
        assert_eq!(live(&open, cx), LiveState::Frozen);
        assert_eq!(focus_context_now(&open, cx), FocusContext::Dock);
        let model = open
            .view
            .read_with(cx, |view, _| view.auto_play_model(Millis(0)));
        assert!(
            model.entries[0].availability.reason().is_some(),
            "{model:?}"
        );

        // The keys still reach the Engine from where focus is.
        press_keys(cx, "f");
        assert_eq!(live(&open, cx), LiveState::Live);
        press_keys(cx, "b");
        assert_eq!(live(&open, cx), LiveState::Blackout);
        press_keys(cx, "b");
        assert_eq!(live(&open, cx), LiveState::Live);
        assert_eq!(focus_context_now(&open, cx), FocusContext::Dock);
    }

    #[gpui::test]
    fn the_control_sits_in_the_dock_in_every_layout(cx: &mut TestAppContext) {
        for case in SIZES.into_iter().chain(SHORT) {
            let (_, cx) = open(cx, case);
            let control = bounds(cx, "auto-play-control");
            let primary = bounds(cx, "primary-action");
            assert!(
                top(control) >= bottom(primary) - 0.5
                    || left(control) >= right(primary) - 0.5
                    || bottom(control) <= top(primary) + 0.5,
                "{case:?}: never over the primary"
            );
            assert!(bottom(control) <= f32::from(cx.update(|w, _| w.viewport_size().height)) + 0.5);
        }
    }

    /// In the Band the control shares the status row, so Freeze/Blackout and
    /// the primary keep the two equal T1.4 columns and the Live sentence has
    /// its whole column.
    #[gpui::test]
    fn the_band_keeps_its_two_columns_and_the_live_sentence(cx: &mut TestAppContext) {
        for case in [SIZES[1], SIZES[2], SHORT[0], SHORT[1]] {
            if !matches!(layout_at(case).dock, DockLayout::Band { .. }) {
                continue;
            }
            let (open, cx) = open(cx, case);
            run(&open, cx, Command::EnableOutput).unwrap();
            run(&open, cx, Command::Take).unwrap();
            cx.run_until_parked();
            let freeze = bounds(cx, "button-Freeze");
            let blackout = bounds(cx, "button-Blackout");
            let primary = bounds(cx, "primary-action");
            let info = bounds(cx, "primary-info");
            let control = bounds(cx, "auto-play-control");
            // The two T1.4 columns: Freeze/Blackout and the primary each take
            // half of the row, whatever the status row now holds.
            let row = right(primary) - left(freeze);
            let toggles = right(blackout) - left(freeze);
            assert!(
                (width(primary) - row / 2.0).abs() < case.2.apply(8.0)
                    && (toggles - row / 2.0).abs() < case.2.apply(8.0),
                "{case:?}: two equal columns, toggles {toggles} primary {primary:?}"
            );
            assert!(
                (width(info) - width(primary)).abs() < 0.5 && width(info) >= case.2.apply(300.0),
                "{case:?}: the Live sentence has its whole column: {info:?}"
            );
            assert!(
                bottom(control) <= top(freeze) + 0.5,
                "{case:?}: the control sits in the status row"
            );
        }
    }

    /// The synthetic 500-Cue snapshot (60 groups), written to a scratch
    /// directory and read back.
    fn synthetic_snapshot() -> Arc<LoadedSnapshot> {
        use presentation::snapshot_reader::{DirectorySource, read_snapshot};
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ordo-virtualise-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        presentation::test_support::write_synthetic_snapshot(&dir).expect("synthetic snapshot");
        let snapshot = read_snapshot(&DirectorySource::new(dir.clone())).expect("reads back");
        std::fs::remove_dir_all(&dir).ok();
        Arc::new(snapshot)
    }

    /// Scroll to `y` pixels from the top and draw, so the virtualised range follows the offset.
    fn scroll_to(open: &Open, cx: &mut VisualTestContext, y: f32) {
        open.view.read_with(cx, |view, _| {
            view.timeline_scroll
                .handle()
                .set_offset(gpui::point(px(0.0), px(-y)));
        });
        animate(open, cx, 0);
    }

    /// The scroll positions that walk the whole timeline, a little under one
    /// viewport apart.
    fn sweep_offsets(open: &Open, cx: &VisualTestContext) -> Vec<f32> {
        let (viewport, max) = open.view.read_with(cx, |view, _| {
            let handle = view.timeline_scroll.handle();
            (
                f32::from(handle.bounds().size.height),
                f32::from(handle.max_offset().y),
            )
        });
        let step = viewport * 0.9;
        let mut at = vec![0.0];
        while *at.last().unwrap() < max {
            at.push((at.last().unwrap() + step).min(max));
        }
        at
    }

    fn open_many(
        cx: &mut TestAppContext,
        case: (f32, f32, InterfaceScale),
        mode: CueView,
    ) -> (Open, &mut VisualTestContext) {
        open_snapshot(
            cx,
            case,
            UiPreferences {
                scale: case.2,
                cue_view: mode,
                ..UiPreferences::default()
            },
            synthetic_snapshot(),
        )
    }

    #[gpui::test]
    fn a_spacer_is_exactly_as_tall_as_the_group_it_stands_for(cx: &mut TestAppContext) {
        // Every group is seen real and as a spacer along the sweep; its height
        // must not change, or the scroll would shift when it swaps.
        let cases = [
            (1366.0, 768.0, InterfaceScale::Percent100),
            (1366.0, 768.0, InterfaceScale::Percent200),
            (1920.0, 1080.0, InterfaceScale::Percent100),
            (1920.0, 1080.0, InterfaceScale::Percent200),
        ];
        for case in cases {
            for mode in [CueView::Grid, CueView::List] {
                for fold in [false, true] {
                    let (mut real, mut spacer) = (0, 0);
                    let (open, cx) = open_snapshot(
                        cx,
                        case,
                        UiPreferences {
                            scale: case.2,
                            cue_view: mode,
                            ..UiPreferences::default()
                        },
                        synthetic_snapshot(),
                    );
                    let groups = open
                        .view
                        .read_with(cx, |view, _| view.engine.snapshot().item_count());
                    if fold {
                        for group in (0..groups).step_by(3) {
                            open.view
                                .update(cx, |view, cx| view.toggle_group(group, cx));
                        }
                        animate(&open, cx, 400);
                    }
                    let firsts: Vec<usize> = open.view.read_with(cx, |view, _| {
                        (0..groups)
                            .map(|group| view.engine.snapshot().item_start(group).unwrap())
                            .collect()
                    });
                    let mut seen: Vec<Vec<f32>> = vec![Vec::new(); groups];
                    for y in sweep_offsets(&open, cx) {
                        scroll_to(&open, cx, y);
                        for group in 0..groups {
                            let h =
                                f32::from(bounds(cx, sel(format!("group-{group}"))).size.height);
                            seen[group].push(h);
                            if !fold || group % 3 != 0 {
                                if present(cx, cue(firsts[group])) {
                                    real += 1;
                                } else {
                                    spacer += 1;
                                }
                            }
                        }
                    }
                    for (group, heights) in seen.iter().enumerate() {
                        let first = heights[0];
                        assert!(
                            heights.iter().all(|h| (h - first).abs() < 0.01),
                            "{case:?} {mode:?} fold {fold}: group {group} changed height {heights:?}"
                        );
                    }
                    // Sixty groups are many viewports: each case has groups
                    // laid out and groups standing in as spacers.
                    assert!(
                        real > 0 && spacer > 0,
                        "{case:?} {mode:?} fold {fold}: {real} real, {spacer} spacers"
                    );
                }
            }
        }
    }

    #[gpui::test]
    fn scrolling_through_every_cue_lays_each_tile_out_where_it_was(cx: &mut TestAppContext) {
        use std::collections::{BTreeMap, BTreeSet};
        for (case, mode) in [
            (SIZES[1], CueView::Grid),
            (SIZES[1], CueView::List),
            (SIZES[2], CueView::Grid),
            (SIZES[2], CueView::List),
        ] {
            {
                let collect =
                    |cx: &mut TestAppContext, all_real: bool, offsets: Option<Vec<f32>>| {
                        let (open, cx) = open_many(cx, case, mode);
                        open.view.update(cx, |view, cx| {
                            view.all_groups_real = all_real;
                            cx.notify();
                        });
                        // Every fourth group is folded, so the sweep crosses
                        // groups of both heights.
                        let groups = open
                            .view
                            .read_with(cx, |view, _| view.engine.snapshot().item_count());
                        for group in (0..groups).step_by(4) {
                            open.view
                                .update(cx, |view, cx| view.toggle_group(group, cx));
                        }
                        animate(&open, cx, 400);
                        // The Cues of the groups that are open.
                        let total = open.view.read_with(cx, |view, _| {
                            let snapshot = view.engine.snapshot();
                            (0..groups)
                                .filter(|group| !view.group_collapsed(*group))
                                .map(|group| {
                                    let start = snapshot.item_start(group).unwrap();
                                    let end = snapshot
                                        .item_start(group + 1)
                                        .unwrap_or_else(|| snapshot.cue_count());
                                    end - start
                                })
                                .sum::<usize>()
                        });
                        let all = open
                            .view
                            .read_with(cx, |view, _| view.engine.snapshot().cue_count());
                        let mut tiles = BTreeMap::new();
                        let mut most = 0;
                        let offsets = offsets.unwrap_or_else(|| sweep_offsets(&open, cx));
                        for (step, y) in offsets.iter().copied().enumerate() {
                            scroll_to(&open, cx, y);
                            let region = bounds(cx, "cue-timeline");
                            let mut present_now = 0;
                            for flat in 0..all {
                                if let Some(b) = cx.debug_bounds(cue(flat)) {
                                    present_now += 1;
                                    if bottom(b) > top(region) && top(b) < bottom(region) {
                                        tiles.insert(
                                            (step, flat),
                                            (left(b), top(b), width(b), f32::from(b.size.height)),
                                        );
                                    }
                                }
                            }
                            most = most.max(present_now);
                        }
                        let seen: BTreeSet<usize> = tiles.keys().map(|(_, flat)| *flat).collect();
                        assert_eq!(
                            seen.len(),
                            total,
                            "{case:?} {mode:?}: every Cue is scrolled past"
                        );
                        (tiles, most, total, offsets)
                    };
                let (reference, all_present, total, offsets) = collect(cx, true, None);
                let (tiles, most_present, ..) = collect(cx, false, Some(offsets));
                assert_eq!(all_present, total, "the reference lays out every Cue");
                assert!(
                    most_present < total / 2,
                    "{case:?} {mode:?}: {most_present} of {total} Cues laid out at once"
                );
                assert_eq!(tiles.len(), reference.len(), "{case:?} {mode:?}");
                for (key, got) in &tiles {
                    let want = reference[key];
                    assert!(
                        [
                            got.0 - want.0,
                            got.1 - want.1,
                            got.2 - want.2,
                            got.3 - want.3
                        ]
                        .iter()
                        .all(|d| d.abs() < 0.01),
                        "{case:?} {mode:?} {key:?}: {got:?} against {want:?}"
                    );
                }
            }
        }
    }

    #[gpui::test]
    fn the_preview_group_stays_laid_out_far_from_the_viewport(cx: &mut TestAppContext) {
        let (open, cx) = open_many(cx, SIZES[1], CueView::Grid);
        let (preview, last) = open.view.read_with(cx, |view, _| {
            (
                view.engine.state().preview(),
                view.engine.snapshot().cue_count() - 1,
            )
        });
        let max = sweep_offsets(&open, cx).last().copied().unwrap();
        scroll_to(&open, cx, max);
        assert!(
            present(cx, cue(preview)),
            "Preview's Cue is laid out far away"
        );
        assert!(present(cx, cue(last)), "the last group is on screen");
        assert!(!present(cx, cue(preview + 150)), "a far group is a spacer");
    }
    /// Every group whose box overlaps the timeline, except `skip`, has its
    /// Cues laid out (folded groups have none to show).
    fn assert_visible_groups_are_real(
        open: &Open,
        cx: &mut VisualTestContext,
        skip: usize,
        context: &str,
    ) {
        let region = bounds(cx, "cue-timeline");
        let (groups, firsts, collapsed) = open.view.read_with(cx, |view, _| {
            let snapshot = view.engine.snapshot();
            let groups = snapshot.item_count();
            (
                groups,
                (0..groups)
                    .map(|group| snapshot.item_start(group).unwrap())
                    .collect::<Vec<_>>(),
                (0..groups)
                    .map(|group| view.group_collapsed(group))
                    .collect::<Vec<_>>(),
            )
        });
        let mut checked = 0;
        for group in (0..groups).filter(|group| *group != skip && !collapsed[*group]) {
            let b = bounds(cx, sel(format!("group-{group}")));
            if bottom(b) > top(region) && top(b) < bottom(region) {
                checked += 1;
                assert!(
                    present(cx, cue(firsts[group])),
                    "{context}: group {group} is in view ({:?}) but a spacer",
                    (top(b), bottom(b), top(region), bottom(region))
                );
            }
        }
        assert!(checked > 0, "{context}: a group below is in view");
    }

    #[gpui::test]
    fn the_groups_below_a_group_that_is_unfolding_or_folding_stay_laid_out(
        cx: &mut TestAppContext,
    ) {
        for case in [SHORT[1], SIZES[2]] {
            let (open, cx) = open_many(cx, case, CueView::Grid);
            let group = 2;
            let viewport = open.view.read_with(cx, |view, _| {
                f32::from(view.timeline_scroll.handle().bounds().size.height)
            });
            let tall = f32::from(bounds(cx, sel(format!("group-{group}"))).size.height);
            assert!(
                tall > 1.5 * viewport,
                "{case:?}: the group ({tall}) is taller than the timeline ({viewport})"
            );
            // Fold it away, then unfold it and look at it mid-flight.
            open.view
                .update(cx, |view, cx| view.toggle_group(group, cx));
            animate(&open, cx, 400);
            assert!(f32::from(bounds(cx, sel(format!("group-{group}"))).size.height) < 200.0);
            open.view
                .update(cx, |view, cx| view.toggle_group(group, cx));
            // The fold takes 160 ms: these are frames 40, 80 and 120 ms in.
            for step in 0..3 {
                animate(&open, cx, 40);
                let now = f32::from(bounds(cx, sel(format!("group-{group}"))).size.height);
                assert!(now < tall, "{case:?}: still unfolding at step {step}");
                assert_visible_groups_are_real(
                    &open,
                    cx,
                    group,
                    &format!("{case:?} unfolding, step {step}"),
                );
            }
            animate(&open, cx, 400);
            // And the other way: folding the settled group.
            open.view
                .update(cx, |view, cx| view.toggle_group(group, cx));
            for step in 0..4 {
                animate(&open, cx, 40);
                assert_visible_groups_are_real(
                    &open,
                    cx,
                    group,
                    &format!("{case:?} folding, step {step}"),
                );
            }
        }
    }

    #[gpui::test]
    fn a_big_wheel_scroll_or_a_jump_shows_no_spacer_in_the_next_frame(cx: &mut TestAppContext) {
        for (case, mode) in [
            (SIZES[0], CueView::Grid),
            (SIZES[0], CueView::List),
            (SIZES[1], CueView::Grid),
            (SIZES[2], CueView::List),
        ] {
            let (open, cx) = open_many(cx, case, mode);
            let region = bounds(cx, "cue-timeline");
            let viewport = f32::from(region.size.height);
            let centre = region.center();
            // Three timeline heights in one wheel event, down and back up;
            // the very next frame is already drawn at the new offset.
            for sign in [-1.0, 1.0] {
                cx.simulate_mouse_move(centre, None, gpui::Modifiers::default());
                cx.simulate_event(gpui::ScrollWheelEvent {
                    position: centre,
                    delta: gpui::ScrollDelta::Pixels(gpui::point(
                        px(0.0),
                        px(sign * 3.0 * viewport),
                    )),
                    modifiers: gpui::Modifiers::default(),
                    touch_phase: gpui::TouchPhase::Moved,
                });
                cx.run_until_parked();
                assert_visible_groups_are_real(
                    &open,
                    cx,
                    usize::MAX,
                    &format!("{case:?} {mode:?} wheel {sign}"),
                );
            }
            // A jump to the far end and back: the glide's frames never show a
            // group that is only a spacer.
            bind_registry_keys(cx);
            focus_timeline_now(&open, cx);
            for (keys, name) in [("end", "End"), ("home", "Home")] {
                press_keys(cx, keys);
                for step in 0..14 {
                    animate(&open, cx, 12);
                    assert_visible_groups_are_real(
                        &open,
                        cx,
                        usize::MAX,
                        &format!("{case:?} {mode:?} {name}, frame {step}"),
                    );
                }
            }
        }
    }

    #[gpui::test]
    fn a_rail_jump_stops_holding_its_group_open_once_the_scroll_lands(cx: &mut TestAppContext) {
        let (open, cx) = open_many(cx, SIZES[1], CueView::Grid);
        let pending = |open: &Open, cx: &VisualTestContext| {
            open.view.read_with(cx, |view, _| view.pending_jump)
        };
        open.view.update(cx, |view, cx| view.jump_to_group(14, cx));
        assert_eq!(pending(&open, cx), Some(14));
        // One frame applies the scroll and the next sees the group at the top.
        animate(&open, cx, 0);
        animate(&open, cx, 0);
        assert_eq!(pending(&open, cx), None, "the group is at the top");
        let group_top = top(bounds(cx, "group-14"));
        let timeline_top = top(bounds(cx, "cue-timeline"));
        assert!((group_top - timeline_top).abs() <= 1.0);
        // A jump the scroll cannot complete (the end of the timeline leaves the
        // group short of the top) is dropped after a few frames anyway.
        let last = open
            .view
            .read_with(cx, |view, _| view.engine.snapshot().item_count() - 1);
        open.view
            .update(cx, |view, cx| view.jump_to_group(last, cx));
        for _ in 0..JUMP_RENDERS {
            animate(&open, cx, 0);
        }
        assert_eq!(pending(&open, cx), None);
    }

    #[test]
    fn a_jump_has_landed_when_the_group_top_is_the_timeline_top() {
        let timeline = Bounds {
            origin: gpui::point(px(0.0), px(100.0)),
            size: size(px(500.0), px(600.0)),
        };
        assert!(jump_landed(100.0, timeline));
        assert!(jump_landed(100.9, timeline));
        assert!(!jump_landed(101.5, timeline));
        assert!(!jump_landed(40.0, timeline));
    }
}
