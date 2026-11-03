//! Presenter surface: the pure layout model (T1.4a), the frame store (T1.4b)
//! and the GPUI view over the Engine (T1.4 WP4).
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

use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use domain::presentation::{CueBehavior, FrameId};
use gpui::{
    Context, IntoElement, ParentElement, Render, SharedString, Styled, Task, Window, div,
    prelude::*, px, rgb,
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
    tokens,
};

use self::{
    dock::{ControlsModel, OnCommand, PreviewModel, Primary, ProgramModel, Shown},
    frames::{FrameView, PresenterFrames},
    header::{DisplayHealth, HeaderModel, OnCueView},
    layout::{
        BAND_GAP, BAND_GUTTER, DOCK_GAP, DOCK_PADDING, DockLayout, LayoutInput, PresenterLayout,
        RailChoice, RailLayout, Regime, TIMELINE_INSET, presenter_layout,
    },
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

    /// Start the header clock: the time now, refreshed on every minute. The
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

    /// The rail's place until T1.4c builds it: the pinned column, or the
    /// narrow ORDO tab. Both carry the ids later work fills.
    fn rail_placeholder(&self, layout: &PresenterLayout) -> gpui::AnyElement {
        let scale = self.environment.preferences().scale;
        let palette = tokens::palette(self.environment.appearance());
        match layout.rail {
            RailLayout::Pinned { width } => div()
                .id("presenter-rail")
                .debug_selector(|| "presenter-rail".into())
                .flex_none()
                .w(px(width))
                .h_full()
                .overflow_hidden()
                .p(px(scale.apply(tokens::spacing::RELATED)))
                .bg(rgb(palette.surface))
                .text_size(px(scale.apply(tokens::typography::META.size)))
                .children(
                    self.snapshot
                        .snapshot()
                        .sections()
                        .iter()
                        .flat_map(|section| section.items())
                        .map(|item| div().truncate().child(item.title().to_owned())),
                )
                .into_any_element(),
            RailLayout::Tab { width } | RailLayout::OverlayOnly { tab_width: width } => div()
                .id("ordo-tab")
                .debug_selector(|| "ordo-tab".into())
                .flex_none()
                .w(px(width))
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(scale.apply(tokens::spacing::GROUP)))
                .bg(rgb(palette.surface))
                .text_color(rgb(palette.text_muted))
                .text_size(px(scale.apply(tokens::typography::CAPTION.size)))
                .children(["O", "R", "D", "O"].map(|letter| div().child(letter)))
                .into_any_element(),
        }
    }

    /// The Cue timeline's place until T1.4c: the Mass-item titles as plain
    /// text in a column sized to the layout's timeline width.
    fn timeline_placeholder(&self, layout: &PresenterLayout) -> gpui::AnyElement {
        let scale = self.environment.preferences().scale;
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_hidden()
            .pl(px(scale.apply(TIMELINE_INSET / 2.0)))
            .child(
                div()
                    .id("cue-timeline")
                    .debug_selector(|| "cue-timeline".into())
                    .w(px(layout.timeline_width))
                    .h_full()
                    .overflow_hidden()
                    .py(px(scale.apply(tokens::spacing::GROUP)))
                    .text_size(px(scale.apply(tokens::typography::BODY.size)))
                    .children(
                        self.snapshot
                            .snapshot()
                            .sections()
                            .iter()
                            .flat_map(|section| section.items())
                            .map(|item| div().truncate().child(item.title().to_owned())),
                    ),
            )
            .into_any_element()
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
        let on_cue_view: OnCueView = Rc::new(move |chosen, _, cx| {
            view.update(cx, |view, cx| view.set_cue_view(chosen, cx))
                .ok();
        });

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
                    .child(self.rail_placeholder(&layout))
                    .child(self.timeline_placeholder(&layout))
                    .child(dock)
                    .into_any_element()
            }
            DockLayout::Band { monitor_width } => {
                // The layout model sizes the band from the whole window, but
                // the tab takes its column first: never let two monitors and
                // their gutters outgrow what is left.
                let column = f32::from(size.width) - layout.rail.width();
                let fits = (column - scale.apply(2.0 * BAND_GUTTER + BAND_GAP)) / 2.0;
                let monitor_width = monitor_width.min(fits.max(0.0));
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
                    .child(self.rail_placeholder(&layout))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(band)
                            .child(controls_row)
                            .child(self.timeline_placeholder(&layout)),
                    )
                    .into_any_element()
            }
        };

        div()
            .id("presenter")
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
        let mut environment = UiEnvironment::new(
            UiPreferences {
                scale,
                ..UiPreferences::default()
            },
            ResolvedAppearance::Dark,
        );
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
        (width, height, scale): (f32, f32, InterfaceScale),
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
                    PresenterView::new(fixture_snapshot(), clock, environment(scale), window, cx);
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
}
