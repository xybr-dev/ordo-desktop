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
pub mod timeline;

use std::{collections::BTreeSet, rc::Rc, sync::Arc, time::Duration};

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
        BAND_GAP, BAND_GUTTER, BAND_TOP_PADDING, DockLayout, LayoutInput, PresenterLayout,
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
    /// The groups (first, last) the timeline showed when the visible-frame
    /// set was last requested. A different range after a scroll or a resize
    /// asks for a new set, outside `render`.
    visible_range: (usize, usize),
    /// The Preview Cue the timeline last brought into view; a different
    /// Preview on the next frame makes the timeline follow it.
    followed_preview: Option<usize>,
    /// The timeline's follow in flight (see `timeline::Glide`).
    glide: timeline::Glide,
    /// Whether the rail was drawn pinned on the last frame, so a pin toggle
    /// flips what the operator saw.
    rail_is_pinned: bool,
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
    /// The frame Program and Preview want, for the slow-frame badge.
    program_wanted: Wanted,
    preview_wanted: Wanted,
    /// When the next badge becomes due (a time on the injected clock), and
    /// the task sleeping until then. Dropped with the view.
    preparing_timer: Option<(u64, Task<()>)>,
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
            visible_range: (0, 0),
            followed_preview: None,
            glide: timeline::Glide::default(),
            rail_is_pinned: false,
            rail_scroll: Scroll::new(),
            timeline_focus: cx.focus_handle(),
            rail_focus: cx.focus_handle(),
            cue_view: environment.preferences().cue_view,
            status: None,
            save_preferences: None,
            timer: None,
            program_wanted: Wanted::default(),
            preview_wanted: Wanted::default(),
            preparing_timer: None,
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
        // The scroll is applied at the next layout, so ask for the target
        // group's frames now; `render` corrects the set once it has moved.
        self.visible_range = (group, group);
        let ids = self.visible_frames((group, group));
        self.frames
            .request_visible(ids, |view| &mut view.frames, cx);
        cx.notify();
    }

    /// Fold or unfold a group. View state only.
    pub fn toggle_group(&mut self, group: usize, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&group) {
            self.collapsed.insert(group);
        }
        self.request_visible(cx);
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
    /// reads the frame store with `peek`, so a paint never changes what the
    /// cache would evict, and composing follows state changes
    /// (`engine_changed`, `request_visible`), never a paint.
    fn timeline_model(&self, layout: &PresenterLayout, follow_preview: bool) -> TimelineModel {
        let engine = &self.engine;
        let frames = &self.frames;
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
        }
    }
}

impl Render for PresenterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        self.rail_is_pinned = matches!(layout.rail, RailLayout::Pinned { .. });
        if self.rail_is_pinned && self.rail_open {
            self.rail_open = false;
            // The overlay held focus; the Cues get it back, as on any close.
            self.timeline_focus.focus(window, cx);
        }
        // A scroll or a resize moved the groups on screen: ask for their
        // thumbnails on the next turn, because the store is not synced from
        // `render`. The hop holds only a weak handle and ends at once.
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
                    .child(dock::primary_action(&controls, &on_primary, environment));
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
                    .child(dock::output_status(
                        controls.status.clone(),
                        controls.live,
                        environment,
                    ))
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
    #[path = "../e2e.rs"]
    mod e2e;
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
}
