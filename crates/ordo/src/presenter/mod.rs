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
//! * `wall_clock`: the minute ticker for the header clock, started only by
//!   [`PresenterView::start_wall_clock`];
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
pub mod layout;
pub mod rail;
pub mod timeline;

use std::{
    collections::BTreeSet,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use domain::presentation::{CueBehavior, FrameId};
use gpui::{
    Context, FocusHandle, IntoElement, ParentElement, Render, SharedString, Styled, Task, Window,
    div, prelude::*, px, rgb,
};
use presentation::{
    commands::Command,
    engine::{Availability, Engine, Outcome, Refusal, UnavailableSource},
    snapshot_reader::LoadedSnapshot,
    state::{LiveState, Millis, ProgramView},
};
use rendering::PRELOAD_RADIUS;
use ui::{
    environment::{CueView, EnvironmentChange, UiEnvironment, UiPreferences},
    scroll::Scroll,
    tokens,
};

use self::{
    dock::{ControlsModel, OnCommand, PreviewModel, Primary, ProgramModel, Shown},
    frames::{FrameView, PresenterFrames},
    header::{DisplayHealth, HeaderModel, OnCueView},
    layout::{
        BAND_GAP, BAND_GUTTER, DOCK_GAP, DOCK_PADDING, DockLayout, LayoutInput, PresenterLayout,
        RAIL_PINNED_WIDTH, RailChoice, RailLayout, Regime, presenter_layout,
    },
    rail::{PinControl, RailItem, RailModel, RailSection},
    timeline::{CueKind, CueModel, GroupModel, TimelineModel},
};
use crate::commands::dispatch;

/// Saves preferences through the owner's save path. The shell passes its
/// background-worker saver; the lab passes one that writes under
/// `target/live-control-lab/`.
pub type SavePreferences = Rc<dyn Fn(UiPreferences)>;

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
        Refusal::AutoPlayIneligible { .. } => "Auto-play is not available here",
        Refusal::AutoPlayBlocked => {
            "Auto-play cannot start while frozen, blacked out or disconnected"
        }
    }
}

/// The standing status line when no refusal is being shown.
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

/// The Presenter: header, rail, Cue timeline and Output Dock over one Engine.
pub struct PresenterView {
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
    rail_open: bool,
    rail_generation: u64,
    /// Which groups are folded. View state: folding never reaches the Engine.
    collapsed: BTreeSet<usize>,
    /// The timeline's scroll surface. The groups are its only children, so a
    /// group's index is its child index for `scroll_to_top_of_item`.
    timeline_scroll: Scroll,
    rail_scroll: Scroll,
    /// Where focus goes when the rail overlay closes (the timeline, which
    /// holds the Cues), and where it goes while the overlay is open.
    timeline_focus: FocusHandle,
    rail_focus: FocusHandle,
    cue_view: CueView,
    /// The last refusal in words; cleared by the next accepted command.
    status: Option<SharedString>,
    save_preferences: Option<SavePreferences>,
    /// The deadline the sleeping task waits for, and the task. Dropped with
    /// the view.
    timer: Option<(Millis, Task<()>)>,
    wall_clock: Option<Task<()>>,
    clock_text: Option<SharedString>,
    /// The last frame each monitor finished drawing, kept while its
    /// replacement is still composing (the consumer rule in
    /// `rendering::frame_cache`).
    last_program: Shown,
    last_preview: Shown,
}

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
            frames: PresenterFrames::new(snapshot.clone()),
            snapshot,
            engine,
            clock,
            environment,
            rail_choice: RailChoice::Auto,
            rail_open: false,
            rail_generation: 0,
            collapsed: BTreeSet::new(),
            timeline_scroll: Scroll::new(),
            rail_scroll: Scroll::new(),
            timeline_focus: cx.focus_handle(),
            rail_focus: cx.focus_handle(),
            cue_view: environment.preferences().cue_view,
            status: None,
            save_preferences: None,
            timer: None,
            wall_clock: None,
            clock_text: None,
            last_program: Shown::default(),
            last_preview: Shown::default(),
        };
        view.engine_changed(cx);
        view
    }

    /// Where a Grid/List change is saved. Without one the choice lasts for
    /// the session.
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
        cx.notify();
    }

    /// Start the header clock: the time now, refreshed on every minute.
    ///
    /// The clock is UTC and says so. Local time needs a Windows API behind an
    /// unsafe platform module, which is out of scope here; that seam is
    /// deferred to T1.6. Labelling the zone means it never misleads. The
    /// ticker is owned by the view and stops with it.
    pub fn start_wall_clock(&mut self, cx: &mut Context<Self>) {
        self.wall_clock = Some(cx.spawn(async move |this, cx| {
            loop {
                let seconds = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_secs());
                let text: SharedString = header::format_clock(seconds).into();
                let alive = this
                    .update(cx, |view, cx| {
                        view.clock_text = Some(text);
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    break;
                }
                let wait = Duration::from_secs(60 - seconds % 60);
                cx.background_executor().timer(wait).await;
            }
        }));
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
        self.reschedule(cx);
        cx.notify();
    }

    /// Project frames of the Cues around Preview, nearest first, Preview's
    /// own excluded.
    ///
    /// The timeline's thumbnails read what is already composed and request
    /// nothing of their own: the scheduler bounds the preload set to
    /// `2 * PRELOAD_RADIUS` keys, so a Cue's tile is black until Preview has
    /// been within that radius, and then it stays (the cache holds at most
    /// `FRAME_CAPACITY` entries, Program and Preview pinned). A bigger
    /// preload would need a bigger bound in `rendering`.
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
        match frames.get(id) {
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
                .unwrap_or_else(|| idle_status(&self.engine).into()),
            can_freeze: allowed(Command::ToggleFreeze),
            can_blackout: allowed(Command::ToggleBlackout),
            can_primary: match dock::primary_for(live) {
                Primary::Action { command, .. } => allowed(command),
                Primary::Info(_) => false,
            },
        }
    }

    fn preview_model(&self, shown: Shown) -> PreviewModel {
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
            self.rail_open = false;
        }
        cx.notify();
    }

    /// Whether the rail overlay is open.
    pub fn rail_open(&self) -> bool {
        self.rail_open
    }

    /// Open the rail overlay (the ORDO tab was pressed) and move focus into
    /// it. Hover never calls this.
    pub fn open_rail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rail_open {
            return;
        }
        self.rail_open = true;
        self.rail_generation += 1;
        self.rail_focus.focus(window, cx);
        cx.notify();
    }

    /// Close the rail overlay and give focus back to the timeline, which
    /// holds the Cues. The timeline has one focus handle for the whole list
    /// (a Cue is not a focus target of its own until T1.5 maps the keyboard),
    /// so "the active Cue" is the timeline.
    pub fn close_rail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.rail_open {
            return;
        }
        self.rail_open = false;
        self.timeline_focus.focus(window, cx);
        cx.notify();
    }

    /// Bring a group to the top of the timeline. The Engine, the Preview and
    /// every other group are unchanged, so the rail never filters.
    pub fn jump_to_group(&mut self, group: usize, cx: &mut Context<Self>) {
        self.timeline_scroll.handle().scroll_to_top_of_item(group);
        cx.notify();
    }

    /// Fold or unfold a group. View state only.
    pub fn toggle_group(&mut self, group: usize, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&group) {
            self.collapsed.insert(group);
        }
        cx.notify();
    }

    /// The Mass-item sections of the rail, with the item that owns Preview
    /// marked.
    fn rail_model(&self, pin: PinControl) -> RailModel {
        let snapshot = self.engine.snapshot();
        let current = snapshot
            .positions()
            .get(self.engine.state().preview())
            .map(|position| position.item_flat);
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
    /// asks the frame store only for what is already composed: composing
    /// follows the Engine (`engine_changed`), never a paint.
    fn timeline_model(&mut self, layout: &PresenterLayout) -> TimelineModel {
        let engine = &self.engine;
        let frames = &mut self.frames;
        let snapshot = engine.snapshot();
        let state = engine.state();
        let preview = state.preview();
        let mut groups = Vec::new();
        let mut flat = 0;
        for section in snapshot.sections() {
            for item in section.items() {
                let index = groups.len();
                let mut cues = Vec::with_capacity(item.cues().len());
                for (at, cue) in item.cues().iter().enumerate() {
                    let (kind, mut frame, mut image) = match cue.behavior() {
                        CueBehavior::Project { frame: id, .. } => match frames.get(*id) {
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
        }
    }
}

impl Render for PresenterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.absorb_failures(cx);
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

        let timeline_model = self.timeline_model(&layout);
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
        };
        let preview = self.preview_model(preview_shown);
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
        let on_cue_view: OnCueView = {
            let view = view.clone();
            Rc::new(move |chosen, _, cx| {
                view.update(cx, |view, cx| view.set_cue_view(chosen, cx))
                    .ok();
            })
        };

        // The rail is on screen already when pinned, so a stale open overlay
        // (the window grew, or the rail was pinned) is dropped, not kept for
        // the next unpin to resurrect.
        if matches!(layout.rail, RailLayout::Pinned { .. }) {
            self.rail_open = false;
        }
        let timeline_handlers = timeline::Handlers {
            go_to: {
                let view = view.clone();
                Rc::new(move |flat, _, cx| {
                    view.update(cx, |view, cx| {
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
        let pinned = matches!(layout.rail, RailLayout::Pinned { .. });
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
                    let choice = if pinned {
                        RailChoice::Unpinned
                    } else {
                        RailChoice::Pinned
                    };
                    view.update(cx, |view, cx| view.set_rail_choice(choice, cx))
                        .ok();
                })
            },
            close: {
                let view = view.clone();
                Rc::new(move |window, cx| {
                    view.update(cx, |view, cx| view.close_rail(window, cx)).ok();
                })
            },
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
                        focus: None,
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
                let overlay = self.rail_open.then(|| {
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
                        rail_handlers.close.clone(),
                        environment,
                    )
                });
                (rail::tab(width, on_open_rail, environment), overlay)
            }
        };
        let timeline_element = timeline::timeline(
            timeline_model,
            &self.timeline_scroll,
            &self.timeline_focus,
            &timeline_handlers,
            environment,
            window,
            cx,
        );

        let header = header::header(
            HeaderModel {
                title: self.snapshot.snapshot().title().to_owned().into(),
                clock: self.clock_text.clone(),
                health: DisplayHealth::of(state.live()),
                cue_view: self.cue_view,
                icon_only: layout.regime == Regime::Constrained,
                environment,
            },
            on_cue_view,
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
                    .p(px(scale.apply(DOCK_PADDING)))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(scale.apply(DOCK_GAP)))
                    .bg(rgb(palette.surface))
                    .child(dock::program_monitor(&program, monitor_width, environment))
                    .child(dock::preview_monitor(&preview, monitor_width, environment))
                    .child(dock::output_status(controls.status.clone(), environment))
                    .child(dock::freeze_blackout(&controls, &on_command, environment))
                    .child(dock::primary_action(&controls, &on_command, environment));
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
                    .pt(px(scale.apply(DOCK_GAP)))
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
                    .child(dock::output_status(controls.status.clone(), environment))
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
                                &on_command,
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

        div()
            .id("presenter")
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::frames::test_support::fixture_snapshot;
    use domain::presentation::CueId;
    use gpui::{Bounds, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext, size};
    use presentation::engine::AutoPlayRefusal;
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
    const DOCK_ELEMENTS: [&str; 6] = [
        "monitor-program",
        "monitor-preview",
        "output-status",
        "button-Freeze",
        "button-Blackout",
        "primary-action",
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
        (width, height, _): (f32, f32, InterfaceScale),
        preferences: UiPreferences,
    ) -> (Open, &mut VisualTestContext) {
        cx.update(ui::init).unwrap();
        let now = Rc::new(Cell::new(0_u64));
        let saved = Rc::new(RefCell::new(Vec::new()));
        let (view, cx) = cx.add_window_view({
            let now = now.clone();
            let saved = saved.clone();
            move |window, cx| {
                let clock: Rc<dyn Fn() -> Millis> = Rc::new(move || Millis(now.get()));
                let mut view = PresenterView::new(
                    fixture_snapshot(),
                    clock,
                    environment_with(preferences),
                    window,
                    cx,
                );
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

    fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} should be on screen"))
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

    #[gpui::test]
    fn grid_makes_tiles_and_list_makes_rows_and_the_choice_is_saved_and_restored(
        cx: &mut TestAppContext,
    ) {
        for case in SIZES {
            let s = case.2.apply(1.0);
            let layout = layout_at(case);
            let (open, vcx) = open(cx, case);
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
            assert!(!present(vcx, "cue-1-frame"), "no thumbnails in List");
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
            let (_, vcx) = open_with(cx, case, saved);
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
            // A group of one Cue.
            let single = bounds(cx, cue(0));
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
    }
}
